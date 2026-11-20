//! Modern AutoCAD `.dwg` (`R2004` `AC1018` & `R2007`–`R2024+` `AC1021`..`AC1032`)
//! Compressed Section Page Codec and Extended CAD Entity Builders (`HATCH`, `DIMENSION`, `VIEWPORT`).
//!
//! Implements:
//! - OpenDWG R2004 file-header LCG PRNG XOR stream cipher (`0x343FD` / `0x269EC3`) at offset `0x80`
//! - Lossless OpenDWG R2004/R2007+ opcode-driven LZ77 sliding-window compressor & decompressor
//! - `R2007+` (`AC1021`–`AC1032`) GF(2^8) Reed-Solomon `(255, 239)` & `(255, 251)` block interleaving
//!   and `TU` UTF-16LE string stream reader/writer
//! - Modern `.dwg` section-page container encoder & decoder (`AC1018`..`AC1032`)
//! - Multi-loop `HATCH`, linear `DIMENSION` (extension lines, arrowheads, measured text), and
//!   paper-space `VIEWPORT` ShapeSheet builders plus ASCII DXF extended entity extraction.

use crate::CadError;
use crate::dwg_reader::{
    DwgBitReader, DwgBitWriter, DwgVersion, encode_dwg_r2000_from_page, import_dwg_to_page,
};
use crate::dxf_codec::{
    DxfValue, aci_to_rgba, apply_cad_shape_metadata, init_2d_cad_sheet,
    insunits_to_unit_and_factor, parse_dxf_groups, resolve_or_create_layer, set_sheet_color,
    strip_mtext_formatting, upsert_cad_layer,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use vexio_shapesheet::{
    GeometryRow, GeometrySection, ReactiveEngine, RgbaColor, ShapeKind, Unit, VisioDocument,
    VisioPage, VisioShape, format_clean_f64,
};

const R2004_FILE_HEADER_MAGIC: &[u8; 11] = b"AcFssFcAJMB";
const R2004_HEADER_BLOCK_LEN: usize = 0x6C;
const R2004_DATA_PAGE_MAGIC: u32 = 0x4163_043B;
const R2004_SYS_PAGE_MAGIC: u32 = 0x4163_003B;
const R2004_SEC_XOR_MAGIC: u32 = 0x4164_536B;

// ============================================================================
// 1. OpenDWG R2004 File Header LCG PRNG XOR Cipher (`offset 0x80`)
// ============================================================================

fn apply_r2004_header_xor_cipher(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut seed: u32 = 1;
    for &b in input {
        seed = seed.wrapping_mul(0x0003_43FD).wrapping_add(0x0026_9EC3);
        let mask = ((seed >> 16) & 0xFF) as u8;
        out.push(b ^ mask);
    }
    out
}

/// Decrypts the `0x6C`-byte (or arbitrary-length) R2004 file header block at offset `0x80`
/// using the standard OpenDWG LCG PRNG (`seed = seed * 0x343FD + 0x269EC3; (seed >> 16) as u8`).
pub fn decrypt_r2004_header_block(encrypted_0x80: &[u8]) -> Vec<u8> {
    apply_r2004_header_xor_cipher(encrypted_0x80)
}

/// Encrypts a plaintext R2004 file header block (`AcFssFcAJMB...`) for placement at offset `0x80`.
pub fn encrypt_r2004_header_block(plain: &[u8]) -> Vec<u8> {
    apply_r2004_header_xor_cipher(plain)
}

// ============================================================================
// 2. OpenDWG R2004 / R2007+ Opcode-Driven LZ77 Sliding-Window Codec
// ============================================================================

fn read_lz77_ext_len(src: &[u8], pos: &mut usize) -> Result<usize, CadError> {
    let mut total = 0usize;
    loop {
        if *pos >= src.len() {
            return Err(CadError::CorruptDwgStream(
                "Truncated extended length in R2004 LZ77 stream".into(),
            ));
        }
        let b = src[*pos];
        *pos += 1;
        if b == 0 {
            total = total.checked_add(255).ok_or_else(|| {
                CadError::CorruptDwgStream("Overflow in R2004 LZ77 extended length".into())
            })?;
        } else {
            total = total.checked_add(b as usize).ok_or_else(|| {
                CadError::CorruptDwgStream("Overflow in R2004 LZ77 extended length".into())
            })?;
            return Ok(total);
        }
    }
}

fn write_lz77_ext_len(out: &mut Vec<u8>, mut extra: usize) {
    // Note: in OpenDWG R2004 LZ77, `0x00` adds 255 and a final non-zero byte `1..=255`
    // terminates the run. If `extra` is an exact multiple of 255 (including > 0), subtract
    // 255 for each `0x00` until `1..=255` remains; `extra` passed here is always `>= 1`.
    while extra > 255 {
        out.push(0x00);
        extra -= 255;
    }
    out.push(extra as u8);
}

fn copy_lz77_backref(out: &mut Vec<u8>, offset: usize, match_len: usize) -> Result<(), CadError> {
    if offset == 0 || offset > out.len() {
        return Err(CadError::CorruptDwgStream(format!(
            "Invalid R2004 LZ77 back-reference offset {offset} at output position {}",
            out.len()
        )));
    }
    out.reserve(match_len);
    let mut read_idx = out.len() - offset;
    for _ in 0..match_len {
        let byte = out[read_idx];
        out.push(byte);
        read_idx += 1;
    }
    Ok(())
}

fn copy_lz77_literals(
    src: &[u8],
    pos: &mut usize,
    lit_len: usize,
    out: &mut Vec<u8>,
) -> Result<(), CadError> {
    let end = pos.checked_add(lit_len).ok_or_else(|| {
        CadError::CorruptDwgStream("Literal run length overflow in R2004 LZ77".into())
    })?;
    if end > src.len() {
        return Err(CadError::CorruptDwgStream(format!(
            "Truncated R2004 LZ77 literal run (needed {lit_len} bytes at {pos}, len={})",
            src.len()
        )));
    }
    out.extend_from_slice(&src[*pos..end]);
    *pos = end;
    Ok(())
}

/// Decompresses an OpenDWG R2004 / R2007+ (`Compression Type 2`) opcode-driven LZ77 byte stream.
pub fn decompress_dwg_r2004_lz77(
    src: &[u8],
    expected_uncompressed_len: usize,
) -> Result<Vec<u8>, CadError> {
    if src.is_empty() {
        if expected_uncompressed_len == 0 {
            return Ok(Vec::new());
        }
        return Err(CadError::CorruptDwgStream(
            "Empty R2004 LZ77 input for non-empty expected output".into(),
        ));
    }

    let mut out = Vec::with_capacity(expected_uncompressed_len.max(src.len()));
    let mut pos = 0usize;

    // Handle initial 1..=3 short literal prefix (`0x12..=0x14`, i.e. `opcode - 0x11` literals
    // at start of stream when sliding window is empty).
    let first = src[0];
    if first == 0x11 {
        if expected_uncompressed_len > 0 {
            return Err(CadError::CorruptDwgStream(format!(
                "Premature 0x11 EOF at start of R2004 LZ77 stream (expected {expected_uncompressed_len} bytes)"
            )));
        }
        return Ok(out);
    }
    if (0x12..=0x14).contains(&first) {
        pos = 1;
        let init_lit = (first - 0x11) as usize;
        copy_lz77_literals(src, &mut pos, init_lit, &mut out)?;
    }

    while pos < src.len() {
        let op = src[pos];
        pos += 1;

        if op == 0x11 {
            // Explicit R2004 LZ77 stream terminator (`0x11`, optionally followed by `0x00, 0x00`)
            break;
        }

        match op {
            0x00..=0x0F => {
                let lit_len = if op == 0x00 {
                    read_lz77_ext_len(src, &mut pos)? + 18
                } else {
                    (op as usize) + 3
                };
                copy_lz77_literals(src, &mut pos, lit_len, &mut out)?;
            }
            0x10..=0x1F => {
                // Long back-reference (`0x4001..=0xBFFF`)
                let base_len = (op & 0x07) as usize;
                let match_len = if base_len == 0 {
                    read_lz77_ext_len(src, &mut pos)? + 9
                } else {
                    base_len + 2
                };
                if pos + 2 > src.len() {
                    return Err(CadError::CorruptDwgStream(
                        "Truncated 16-bit offset in R2004 LZ77 long match".into(),
                    ));
                }
                let w = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
                pos += 2;
                let rel = (((op as usize) & 0x08) << 11) | (w >> 2);
                if rel == 0 {
                    break;
                }
                let offset = 0x4000 + rel;
                let trailing_lit = w & 0x03;
                copy_lz77_backref(&mut out, offset, match_len)?;
                if trailing_lit > 0 {
                    copy_lz77_literals(src, &mut pos, trailing_lit, &mut out)?;
                }
            }
            0x20..=0x3F => {
                // Medium back-reference (`1..=0x4000`, length `3..=33` or extended)
                let base_len = (op & 0x1F) as usize;
                let match_len = if base_len == 0 {
                    read_lz77_ext_len(src, &mut pos)? + 33
                } else {
                    base_len + 2
                };
                if pos + 2 > src.len() {
                    return Err(CadError::CorruptDwgStream(
                        "Truncated 16-bit offset in R2004 LZ77 medium match".into(),
                    ));
                }
                let w = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
                pos += 2;
                let offset = (w >> 2) + 1;
                let trailing_lit = w & 0x03;
                copy_lz77_backref(&mut out, offset, match_len)?;
                if trailing_lit > 0 {
                    copy_lz77_literals(src, &mut pos, trailing_lit, &mut out)?;
                }
            }
            0x40..=0xFF => {
                // Short back-reference (`1..=1024`, length `3..=14`)
                if pos >= src.len() {
                    return Err(CadError::CorruptDwgStream(
                        "Truncated offset byte in R2004 LZ77 short match".into(),
                    ));
                }
                let next_b = src[pos] as usize;
                pos += 1;
                let match_len = ((op >> 4) as usize) - 1;
                let offset = ((next_b << 2) | (((op >> 2) & 0x03) as usize)) + 1;
                let trailing_lit = (op & 0x03) as usize;
                copy_lz77_backref(&mut out, offset, match_len)?;
                if trailing_lit > 0 {
                    copy_lz77_literals(src, &mut pos, trailing_lit, &mut out)?;
                }
            }
        }
    }

    if expected_uncompressed_len > 0 {
        if out.len() < expected_uncompressed_len {
            return Err(CadError::CorruptDwgStream(format!(
                "R2004 LZ77 decompressed {} bytes, expected {expected_uncompressed_len}",
                out.len()
            )));
        }
        if out.len() > expected_uncompressed_len {
            out.truncate(expected_uncompressed_len);
        }
    }
    Ok(out)
}

fn lz77_hash3(data: &[u8], idx: usize) -> usize {
    let h = ((data[idx] as u32).wrapping_mul(2654435761))
        ^ ((data[idx + 1] as u32).wrapping_mul(40503))
        ^ ((data[idx + 2] as u32) << 7);
    ((h >> 12) ^ h) as usize & 0x1FFF
}

fn flush_lz77_literals(
    plain: &[u8],
    lit_start: usize,
    lit_end: usize,
    last_match_lit_slot: &mut Option<usize>,
    out: &mut Vec<u8>,
) {
    let lit_len = lit_end.saturating_sub(lit_start);
    if lit_len == 0 {
        return;
    }
    if lit_len <= 3 {
        if let Some(slot_idx) = last_match_lit_slot.take() {
            out[slot_idx] |= lit_len as u8;
            out.extend_from_slice(&plain[lit_start..lit_end]);
            return;
        }
        // Stream start (`lit_start == 0`) with 1..=3 initial literals
        out.push(0x11 + lit_len as u8);
        out.extend_from_slice(&plain[lit_start..lit_end]);
        return;
    }

    *last_match_lit_slot = None;
    if lit_len <= 18 {
        out.push((lit_len - 3) as u8);
    } else {
        out.push(0x00);
        write_lz77_ext_len(out, lit_len - 18);
    }
    out.extend_from_slice(&plain[lit_start..lit_end]);
}

/// Compresses `plain` into a standards-compliant OpenDWG R2004 / R2007+ LZ77 byte stream
/// terminated with `[0x11, 0x00, 0x00]`.
pub fn compress_dwg_r2004_lz77(plain: &[u8]) -> Vec<u8> {
    if plain.is_empty() {
        return vec![0x11, 0x00, 0x00];
    }
    if plain.len() < 4 {
        let mut out = Vec::with_capacity(plain.len() + 4);
        out.push(0x11 + plain.len() as u8);
        out.extend_from_slice(plain);
        out.extend_from_slice(&[0x11, 0x00, 0x00]);
        return out;
    }

    let mut out = Vec::with_capacity(plain.len() / 2 + 16);
    let mut dict = [usize::MAX; 8192];
    let mut lit_start = 0usize;
    let mut cur = 0usize;
    let mut last_match_lit_slot: Option<usize> = None;

    let max_scan = plain.len() - 2;
    while cur < max_scan {
        let h = lz77_hash3(plain, cur);
        let cand = dict[h];
        dict[h] = cur;

        let mut best_len = 0usize;
        let mut best_off = 0usize;

        if cand != usize::MAX && cur > cand {
            let offset = cur - cand;
            if offset <= 0xBFFF
                && plain[cand] == plain[cur]
                && plain[cand + 1] == plain[cur + 1]
                && plain[cand + 2] == plain[cur + 2]
            {
                let max_len = (plain.len() - cur).min(4096);
                let mut mlen = 3usize;
                while mlen < max_len && plain[cand + mlen] == plain[cur + mlen] {
                    mlen += 1;
                }
                // Long offsets (`> 0x4000`) require `mlen >= 4` so opcode never collides with `0x11` EOF.
                if offset <= 0x4000 || mlen >= 4 {
                    best_len = mlen;
                    best_off = offset;
                }
            }
        }

        if best_len >= 3 {
            flush_lz77_literals(plain, lit_start, cur, &mut last_match_lit_slot, &mut out);

            if best_off <= 1024 && best_len <= 14 {
                let off_m1 = best_off - 1;
                let op = (((best_len + 1) as u8) << 4) | (((off_m1 & 0x03) as u8) << 2);
                let op_idx = out.len();
                out.push(op);
                out.push((off_m1 >> 2) as u8);
                last_match_lit_slot = Some(op_idx);
            } else if best_off <= 0x4000 {
                let off_m1 = best_off - 1;
                if best_len <= 33 {
                    out.push(0x20 | ((best_len - 2) as u8));
                } else {
                    out.push(0x20);
                    write_lz77_ext_len(&mut out, best_len - 33);
                }
                let w = (off_m1 << 2) as u16;
                let w_lo_idx = out.len();
                out.extend_from_slice(&w.to_le_bytes());
                last_match_lit_slot = Some(w_lo_idx);
            } else {
                let rel = best_off - 0x4000;
                let hi_bit = ((rel >> 11) & 0x08) as u8;
                if best_len <= 9 {
                    out.push(0x10 | hi_bit | ((best_len - 2) as u8));
                } else {
                    out.push(0x10 | hi_bit);
                    write_lz77_ext_len(&mut out, best_len - 9);
                }
                let w = ((rel & 0x3FFF) << 2) as u16;
                let w_lo_idx = out.len();
                out.extend_from_slice(&w.to_le_bytes());
                last_match_lit_slot = Some(w_lo_idx);
            }

            let step_end = (cur + best_len).min(max_scan);
            for p in (cur + 1)..step_end {
                dict[lz77_hash3(plain, p)] = p;
            }
            cur += best_len;
            lit_start = cur;
        } else {
            cur += 1;
        }
    }

    flush_lz77_literals(
        plain,
        lit_start,
        plain.len(),
        &mut last_match_lit_slot,
        &mut out,
    );
    out.extend_from_slice(&[0x11, 0x00, 0x00]);
    out
}

// ============================================================================
// 3. R2007+ (`AC1021`–`AC1032`) Reed-Solomon Block Interleaving & `TU` Strings
// ============================================================================

fn gf256_mul(mut a: u8, mut b: u8) -> u8 {
    // GF(2^8) multiplication modulo primitive polynomial x^8 + x^4 + x^3 + x^2 + 1 (`0x11D`).
    let mut p = 0u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            p ^= a;
        }
        let hi = (a & 0x80) != 0;
        a <<= 1;
        if hi {
            a ^= 0x1D;
        }
        b >>= 1;
    }
    p
}

fn compute_rs_parity(payload: &[u8], parity_bytes: usize) -> Vec<u8> {
    if parity_bytes == 0 {
        return Vec::new();
    }
    // Build generator polynomial g(x) = prod_{i=0..parity_bytes-1} (x - alpha^i) over GF(2^8)
    let mut gen = vec![0u8; parity_bytes + 1];
    gen[0] = 1;
    let mut root = 1u8;
    for i in 0..parity_bytes {
        for j in (1..=i + 1).rev() {
            gen[j] = gen[j - 1] ^ gf256_mul(gen[j], root);
        }
        gen[0] = gf256_mul(gen[0], root);
        root = gf256_mul(root, 2);
    }
    // Reverse so gen_desc[0] is leading x^parity_bytes = 1
    gen.reverse();

    let mut rem = vec![0u8; parity_bytes];
    for &byte in payload {
        let feedback = byte ^ rem[0];
        rem.copy_within(1..parity_bytes, 0);
        rem[parity_bytes - 1] = 0;
        if feedback != 0 {
            for k in 0..parity_bytes {
                rem[k] ^= gf256_mul(gen[k + 1], feedback);
            }
        }
    }
    rem
}

/// Deinterleaves `R2007+` (`AC1021`–`AC1032`) Reed-Solomon `(255, block_size)` superblocks
/// (`block_size = 239` for system pages, `block_size = 251` for data pages) across `factor` codewords.
pub fn deinterleave_reed_solomon_blocks(data: &[u8], block_size: usize, factor: usize) -> Vec<u8> {
    if data.is_empty() || block_size == 0 || factor == 0 {
        return Vec::new();
    }
    let cw_len = block_size.max(255);
    let superblock_len = cw_len * factor;
    let mut out = Vec::with_capacity((data.len() / cw_len + 1) * block_size);

    let full_chunks = data.len() / superblock_len;
    for s in 0..full_chunks {
        let base = s * superblock_len;
        let sb = &data[base..base + superblock_len];
        for cw in 0..factor {
            for i in 0..block_size {
                out.push(sb[i * factor + cw]);
            }
        }
    }

    let rem_start = full_chunks * superblock_len;
    if rem_start < data.len() {
        let rem = &data[rem_start..];
        for cw in 0..factor {
            for i in 0..block_size {
                let idx = i * factor + cw;
                if idx < rem.len() {
                    out.push(rem[idx]);
                }
            }
        }
    }
    out
}

/// Encodes `data` into `R2007+` (`AC1021`–`AC1032`) GF(2^8) Reed-Solomon `(255, 255 - parity_bytes)`
/// codewords (`parity_bytes = 16` for `(255, 239)`, `parity_bytes = 4` for `(255, 251)`) and
/// interleaves groups of `factor` codewords into `255 * factor`-byte section page superblocks.
pub fn interleave_reed_solomon_blocks(data: &[u8], parity_bytes: usize, factor: usize) -> Vec<u8> {
    if data.is_empty() || factor == 0 || parity_bytes >= 255 {
        return Vec::new();
    }
    let block_size = 255 - parity_bytes;
    let input_group_len = block_size * factor;
    let superblock_len = 255 * factor;
    let num_groups = data.len().div_ceil(input_group_len);
    let mut out = Vec::with_capacity(num_groups * superblock_len);

    for g in 0..num_groups {
        let g_start = g * input_group_len;
        let mut codewords = vec![vec![0u8; 255]; factor];
        for (cw, cw_buf) in codewords.iter_mut().enumerate() {
            let src_start = g_start + cw * block_size;
            if src_start < data.len() {
                let src_end = (src_start + block_size).min(data.len());
                let slice = &data[src_start..src_end];
                cw_buf[..slice.len()].copy_from_slice(slice);
            }
            let parity = compute_rs_parity(&cw_buf[..block_size], parity_bytes);
            cw_buf[block_size..255].copy_from_slice(&parity);
        }

        let base = out.len();
        out.resize(base + superblock_len, 0);
        for i in 0..255 {
            for cw in 0..factor {
                out[base + i * factor + cw] = codewords[cw][i];
            }
        }
    }
    out
}

/// Reads an `R2007+` (`AC1021`–`AC1032`) Unicode Text (`TU`) string: `BS` UTF-16 code unit count
/// followed by `count` little-endian `u16` UTF-16LE code units.
pub fn read_tu_utf16_string(reader: &mut DwgBitReader<'_>) -> Result<String, CadError> {
    let char_count = reader.read_bs()?;
    if char_count <= 0 {
        return Ok(String::new());
    }
    let n = char_count as usize;
    let needed_bits = n.saturating_mul(16);
    if reader.bit_offset().saturating_add(needed_bits) > reader.total_bits() {
        return Err(CadError::CorruptDwgStream(format!(
            "Truncated R2007+ TU UTF-16LE string of {n} code units"
        )));
    }
    let mut units = Vec::with_capacity(n);
    for _ in 0..n {
        let lo = reader.read_rc()?;
        let hi = reader.read_rc()?;
        units.push(u16::from_le_bytes([lo, hi]));
    }
    if units.last() == Some(&0) {
        units.pop();
    }
    Ok(String::from_utf16_lossy(&units))
}

/// Writes an `R2007+` Unicode Text (`TU`) UTF-16LE string to `DwgBitWriter`.
pub fn write_tu_utf16_string(writer: &mut DwgBitWriter, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let len = units.len().min(32_767);
    writer.write_bs(len as i16);
    for &u in &units[..len] {
        let [lo, hi] = u.to_le_bytes();
        writer.write_rc(lo);
        writer.write_rc(hi);
    }
}

// ============================================================================
// 4. Modern `.dwg` (`AC1018`–`AC1032`) Section Page Container Encoder/Decoder
// ============================================================================

/// Encodes `page` into a compressed modern AutoCAD `.dwg` (`AC1018`, `AC1021`, `AC1024`,
/// `AC1027`, or `AC1032`) binary container with:
/// - `0x00..0x80` version header
/// - `0x80..0xEC` LCG-encrypted `AcFssFcAJMB` R2004+ file header block
/// - `0x100..` `0x4163043B` data section page compressed with R2004 LZ77 and (for `R2007+`)
///   interleaved with `(255, 251)` GF(2^8) Reed-Solomon parity blocks.
pub fn encode_dwg_modern_container(
    version: DwgVersion,
    page: &VisioPage,
) -> Result<Vec<u8>, CadError> {
    if matches!(version, DwgVersion::R13 | DwgVersion::R14 | DwgVersion::R2000) {
        return Err(CadError::UnsupportedDwgVersion(format!(
            "encode_dwg_modern_container requires R2004+ (AC1018..AC1032), got {}",
            version.magic_str()
        )));
    }

    let mut temp_doc = VisioDocument::new_blank(&page.name);
    temp_doc.pages = vec![page.clone()];
    let raw_r2000 = encode_dwg_r2000_from_page(&temp_doc, page.id)?;
    let lz_stream = compress_dwg_r2004_lz77(&raw_r2000);

    let uses_rs = !matches!(version, DwgVersion::R2004);
    let rs_factor: u32 = if uses_rs { 2 } else { 0 };
    let page_body = if uses_rs {
        interleave_reed_solomon_blocks(&lz_stream, 4, rs_factor as usize)
    } else {
        lz_stream.clone()
    };

    let raw_crc = crc32fast::hash(&raw_r2000);
    let section_offset: u32 = 0x100;

    // 32-byte R2004+ Data Section Page Header
    let mut sec_page_hdr = [0u8; 32];
    sec_page_hdr[0..4].copy_from_slice(&R2004_DATA_PAGE_MAGIC.to_le_bytes());
    sec_page_hdr[4..8].copy_from_slice(&1u32.to_le_bytes()); // section_id = 1
    sec_page_hdr[8..12].copy_from_slice(&(lz_stream.len() as u32).to_le_bytes());
    sec_page_hdr[12..16].copy_from_slice(&(raw_r2000.len() as u32).to_le_bytes());
    sec_page_hdr[16..20].copy_from_slice(&0u32.to_le_bytes()); // start_offset = 0
    sec_page_hdr[20..24].copy_from_slice(&2u32.to_le_bytes()); // compression_type = 2 (LZ77)
    sec_page_hdr[24..28].copy_from_slice(&raw_crc.to_le_bytes());
    sec_page_hdr[28..32].copy_from_slice(&(page_body.len() as u32).to_le_bytes());

    // 0x6C-byte plaintext R2004+ header block (`AcFssFcAJMB\0`)
    let mut hdr_plain = vec![0u8; R2004_HEADER_BLOCK_LEN];
    hdr_plain[0..11].copy_from_slice(R2004_FILE_HEADER_MAGIC);
    hdr_plain[11] = 0;
    hdr_plain[0x18..0x1C].copy_from_slice(&section_offset.to_le_bytes());
    hdr_plain[0x1C..0x20].copy_from_slice(&(page_body.len() as u32).to_le_bytes());
    hdr_plain[0x20..0x24].copy_from_slice(&(raw_r2000.len() as u32).to_le_bytes());
    hdr_plain[0x24..0x28].copy_from_slice(&(lz_stream.len() as u32).to_le_bytes());
    hdr_plain[0x28..0x2C].copy_from_slice(&rs_factor.to_le_bytes());
    hdr_plain[0x2C..0x30].copy_from_slice(&raw_crc.to_le_bytes());
    let name_bytes = page.name.as_bytes();
    let name_len = name_bytes.len().min(32);
    hdr_plain[0x30] = name_len as u8;
    hdr_plain[0x31..0x31 + name_len].copy_from_slice(&name_bytes[..name_len]);
    let hdr_crc = crc32fast::hash(&hdr_plain[..0x68]);
    hdr_plain[0x68..0x6C].copy_from_slice(&hdr_crc.to_le_bytes());

    let hdr_encrypted = encrypt_r2004_header_block(&hdr_plain);

    let mut out = vec![0u8; 0x100];
    out[0..6].copy_from_slice(version.magic_str().as_bytes());
    out[0x0B] = match version {
        DwgVersion::R2004 => 0x04,
        DwgVersion::R2007 => 0x09,
        DwgVersion::R2010 => 0x12,
        DwgVersion::R2013 => 0x19,
        DwgVersion::R2018 => 0x1D,
        _ => 0x0F,
    };
    out[0x0C] = 0x03;
    out[0x13..0x15].copy_from_slice(&30u16.to_le_bytes()); // ANSI_1252
    out[0x80..0x80 + R2004_HEADER_BLOCK_LEN].copy_from_slice(&hdr_encrypted);
    out.extend_from_slice(&sec_page_hdr);
    out.extend_from_slice(&page_body);
    Ok(out)
}

/// Decodes a modern (`AC1018`–`AC1032`) or classic (`AC1012`–`AC1015`) `.dwg` container into
/// a recalculated `VisioDocument`.
pub fn decode_dwg_modern_container(bytes: &[u8]) -> Result<VisioDocument, CadError> {
    let version = DwgVersion::from_magic_bytes(bytes)?;
    if matches!(version, DwgVersion::R13 | DwgVersion::R14 | DwgVersion::R2000) {
        let mut doc = VisioDocument::new_blank("DWG Drawing");
        {
            let page = doc.page_mut(1).ok_or(CadError::PageNotFound(1))?;
            import_dwg_to_page(page, bytes)?;
        }
        ReactiveEngine::recalculate_document(&mut doc);
        return Ok(doc);
    }

    if bytes.len() < 0x100 {
        return Err(CadError::CorruptDwgStream(
            "Modern DWG (AC1018+) shorter than 256-byte file header".into(),
        ));
    }

    let raw_hdr_slice = &bytes[0x80..0x80 + R2004_HEADER_BLOCK_LEN];
    let hdr_plain = if &raw_hdr_slice[0..11] == R2004_FILE_HEADER_MAGIC {
        raw_hdr_slice.to_vec()
    } else {
        decrypt_r2004_header_block(raw_hdr_slice)
    };

    if &hdr_plain[0..11] != R2004_FILE_HEADER_MAGIC {
        return Err(CadError::CorruptDwgStream(
            "R2004+ file header at 0x80 did not decrypt to 'AcFssFcAJMB'".into(),
        ));
    }

    let sec_offset = u32::from_le_bytes([
        hdr_plain[0x18],
        hdr_plain[0x19],
        hdr_plain[0x1A],
        hdr_plain[0x1B],
    ]) as usize;
    let encoded_len = u32::from_le_bytes([
        hdr_plain[0x1C],
        hdr_plain[0x1D],
        hdr_plain[0x1E],
        hdr_plain[0x1F],
    ]) as usize;
    let uncompressed_len = u32::from_le_bytes([
        hdr_plain[0x20],
        hdr_plain[0x21],
        hdr_plain[0x22],
        hdr_plain[0x23],
    ]) as usize;
    let lz_len = u32::from_le_bytes([
        hdr_plain[0x24],
        hdr_plain[0x25],
        hdr_plain[0x26],
        hdr_plain[0x27],
    ]) as usize;
    let rs_factor = u32::from_le_bytes([
        hdr_plain[0x28],
        hdr_plain[0x29],
        hdr_plain[0x2A],
        hdr_plain[0x2B],
    ]) as usize;
    let expected_crc = u32::from_le_bytes([
        hdr_plain[0x2C],
        hdr_plain[0x2D],
        hdr_plain[0x2E],
        hdr_plain[0x2F],
    ]);

    let page_name_len = (hdr_plain[0x30] as usize).min(32);
    let restored_page_name = if page_name_len > 0 {
        String::from_utf8_lossy(&hdr_plain[0x31..0x31 + page_name_len]).into_owned()
    } else {
        "Page-1".to_string()
    };

    // Path A: Section page descriptor at `sec_offset` with 32-byte `0x4163043B` page header
    if sec_offset >= 0x100
        && sec_offset.saturating_add(32).saturating_add(encoded_len) <= bytes.len()
        && uncompressed_len > 0
    {
        let page_hdr = &bytes[sec_offset..sec_offset + 32];
        let raw_magic = u32::from_le_bytes([page_hdr[0], page_hdr[1], page_hdr[2], page_hdr[3]]);
        let unmasked_magic = raw_magic ^ R2004_SEC_XOR_MAGIC.wrapping_add(sec_offset as u32);
        if raw_magic == R2004_DATA_PAGE_MAGIC || unmasked_magic == R2004_DATA_PAGE_MAGIC {
            let body = &bytes[sec_offset + 32..sec_offset + 32 + encoded_len];
            let lz_bytes: Vec<u8> = if rs_factor > 0 {
                let mut deint = deinterleave_reed_solomon_blocks(body, 251, rs_factor);
                if lz_len > 0 && deint.len() > lz_len {
                    deint.truncate(lz_len);
                }
                deint
            } else {
                body.to_vec()
            };
            let decompressed = decompress_dwg_r2004_lz77(&lz_bytes, uncompressed_len)?;
            if expected_crc != 0 && crc32fast::hash(&decompressed) != expected_crc {
                return Err(CadError::CorruptDwgStream(
                    "R2004+ section page payload CRC32 mismatch".into(),
                ));
            }
            if decompressed.starts_with(b"AC1015") {
                let mut doc = VisioDocument::new_blank(&restored_page_name);
                {
                    let page = doc.page_mut(1).ok_or(CadError::PageNotFound(1))?;
                    page.name = restored_page_name.clone();
                    page.name_u = restored_page_name;
                    import_dwg_to_page(page, &decompressed)?;
                    page.page_sheet.set_user_cell(
                        "CadDwgVersion",
                        format!("\"{}\"", version.magic_str()),
                        "DWG Version Magic",
                    );
                    page.page_sheet.set_user_cell(
                        "CadDwgR2004Header",
                        "TRUE",
                        "R2004+ File Header Signature",
                    );
                }
                ReactiveEngine::recalculate_document(&mut doc);
                return Ok(doc);
            }
        }
    }

    // Path B: OpenDWG section page stream scan (`0x4163043B` / `0x4163003B` unmasked or XOR-masked)
    let mut assembled_stream = Vec::new();
    let mut scan = 0x100usize;
    while scan + 32 <= bytes.len() {
        let m_raw = u32::from_le_bytes([
            bytes[scan],
            bytes[scan + 1],
            bytes[scan + 2],
            bytes[scan + 3],
        ]);
        let sec_mask = R2004_SEC_XOR_MAGIC ^ (scan as u32);
        let m_xor = m_raw ^ sec_mask;
        let is_unmasked_data = m_raw == R2004_DATA_PAGE_MAGIC || m_raw == R2004_SYS_PAGE_MAGIC;
        let is_masked_data = m_xor == R2004_DATA_PAGE_MAGIC || m_xor == R2004_SYS_PAGE_MAGIC;

        if is_unmasked_data || is_masked_data {
            let mut hdr32 = [0u8; 32];
            hdr32.copy_from_slice(&bytes[scan..scan + 32]);
            if is_masked_data {
                for chunk in hdr32.chunks_exact_mut(4) {
                    let v = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) ^ sec_mask;
                    chunk.copy_from_slice(&v.to_le_bytes());
                }
            }
            let comp_size =
                u32::from_le_bytes([hdr32[8], hdr32[9], hdr32[10], hdr32[11]]) as usize;
            let decomp_size =
                u32::from_le_bytes([hdr32[12], hdr32[13], hdr32[14], hdr32[15]]) as usize;
            let comp_type = u32::from_le_bytes([hdr32[20], hdr32[21], hdr32[22], hdr32[23]]);
            let data_start = scan + 32;
            if comp_size > 0 && data_start.saturating_add(comp_size) <= bytes.len() {
                let slice = &bytes[data_start..data_start + comp_size];
                if comp_type == 2 {
                    if let Ok(dec) = decompress_dwg_r2004_lz77(slice, decomp_size) {
                        assembled_stream.extend_from_slice(&dec);
                    }
                } else {
                    assembled_stream.extend_from_slice(slice);
                }
                scan = data_start + comp_size;
                continue;
            }
        }
        scan += 0x20;
    }

    let mut doc = VisioDocument::new_blank("DWG Modern Drawing");
    {
        let page = doc.page_mut(1).ok_or(CadError::PageNotFound(1))?;
        if assembled_stream.starts_with(b"AC1015") {
            import_dwg_to_page(page, &assembled_stream)?;
        } else {
            import_dwg_to_page(page, bytes)?;
        }
        page.page_sheet.set_user_cell(
            "CadDwgVersion",
            format!("\"{}\"", version.magic_str()),
            "DWG Version Magic",
        );
    }
    ReactiveEngine::recalculate_document(&mut doc);
    Ok(doc)
}

// ============================================================================
// 5. CAD `HATCH`, `DIMENSION` & `VIEWPORT` Entity Builders + DXF Extractor
// ============================================================================

/// A single boundary loop (outer contour or inner island hole) of a CAD `HATCH` entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadHatchLoop {
    pub vertices: Vec<[f64; 2]>,
    pub closed: bool,
}

fn hatch_pattern_to_visio_fill_code(pattern_name: &str) -> &'static str {
    let upper = pattern_name.trim().to_ascii_uppercase();
    match upper.as_str() {
        "" | "SOLID" => "1",
        "ANSI31" | "LINE" | "SLASH" => "5",
        "ANSI32" | "ANSI33" | "ANSI34" => "7",
        "ANSI37" | "CROSS" => "9",
        "NET" | "NET3" | "GRID" | "SQUARE" => "8",
        "DOTS" => "10",
        _ => "5",
    }
}

/// Builds a multi-loop Visio shape representing an AutoCAD `HATCH` entity with pattern-mapped
/// `FillPattern` (`SOLID` -> `1`, `ANSI31` -> `5`, `ANSI37` -> `9`, `NET` -> `8`) and one
/// `GeometrySection` per boundary loop.
pub fn build_cad_hatch_shape(
    id: u64,
    pattern_name: &str,
    loops: &[CadHatchLoop],
    color: RgbaColor,
    scale_to_in: f64,
) -> VisioShape {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for lp in loops {
        for &[vx, vy] in &lp.vertices {
            let x = vx * scale_to_in;
            let y = vy * scale_to_in;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if !min_x.is_finite() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 1.0;
        max_y = 1.0;
    }

    let w = (max_x - min_x).max(1e-4);
    let h = (max_y - min_y).max(1e-4);
    let mut sheet = init_2d_cad_sheet(min_x, min_y, w, h);
    sheet.geometries.clear();

    let mut geom_idx = 1u32;
    for lp in loops {
        if lp.vertices.is_empty() {
            continue;
        }
        let mut geom = GeometrySection::new(geom_idx);
        geom.no_fill = if lp.closed { "FALSE".into() } else { "TRUE".into() };
        geom.no_line = "FALSE".into();
        let first = lp.vertices[0];
        let fx = first[0] * scale_to_in - min_x;
        let fy = first[1] * scale_to_in - min_y;
        geom.rows.push(GeometryRow::move_to(
            1,
            format!("{} in", format_clean_f64(fx)),
            format!("{} in", format_clean_f64(fy)),
        ));
        let mut row_ix = 2u32;
        for &[vx, vy] in &lp.vertices[1..] {
            let lx = vx * scale_to_in - min_x;
            let ly = vy * scale_to_in - min_y;
            geom.rows.push(GeometryRow::line_to(
                row_ix,
                format!("{} in", format_clean_f64(lx)),
                format!("{} in", format_clean_f64(ly)),
            ));
            row_ix += 1;
        }
        if lp.closed && lp.vertices.len() >= 2 {
            let last = lp.vertices[lp.vertices.len() - 1];
            if ((last[0] - first[0]) * scale_to_in).hypot((last[1] - first[1]) * scale_to_in) > 1e-6
            {
                geom.rows.push(GeometryRow::line_to(
                    row_ix,
                    format!("{} in", format_clean_f64(fx)),
                    format!("{} in", format_clean_f64(fy)),
                ));
            }
        }
        sheet.add_geometry(geom);
        geom_idx += 1;
    }

    let fill_pat = hatch_pattern_to_visio_fill_code(pattern_name);
    let name = format!("CAD.Hatch.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: String::new(),
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, "HATCH", 0, false, color, Some(color));
    shape
        .sheet
        .set_formula("FillPattern", fill_pat, Some(Unit::Number));
    set_sheet_color(&mut shape.sheet, "FillBkgnd", RgbaColor::WHITE);
    shape.sheet.set_user_cell(
        "CadHatchPattern",
        format!("\"{}\"", pattern_name.trim()),
        "CAD Hatch Pattern Name",
    );
    shape
}

/// Builds a multi-geometry Visio shape representing a CAD linear `DIMENSION` entity with two
/// extension lines, a dimension line terminated by triangular arrowheads, and the exact
/// measured (or overridden) dimension label.
pub fn build_cad_linear_dimension_shape(
    id: u64,
    p1: [f64; 2],
    p2: [f64; 2],
    dim_line_pt: [f64; 2],
    override_text: Option<&str>,
    color: RgbaColor,
    scale_to_in: f64,
) -> VisioShape {
    let raw_dist = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
    let measured_str = format_clean_f64((raw_dist * 10_000.0).round() / 10_000.0);
    let label = match override_text.map(str::trim) {
        None | Some("") | Some("<>") => measured_str,
        Some(ovr) if ovr.contains("<>") => {
            strip_mtext_formatting(&ovr.replace("<>", &measured_str))
        }
        Some(ovr) => strip_mtext_formatting(ovr),
    };

    let p1_in = [p1[0] * scale_to_in, p1[1] * scale_to_in];
    let p2_in = [p2[0] * scale_to_in, p2[1] * scale_to_in];
    let dp_in = [dim_line_pt[0] * scale_to_in, dim_line_pt[1] * scale_to_in];

    let dx = p2_in[0] - p1_in[0];
    let dy = p2_in[1] - p1_in[1];
    let span = dx.hypot(dy);
    let (ux, uy) = if span > 1e-9 {
        (dx / span, dy / span)
    } else {
        (1.0, 0.0)
    };
    // Unit normal perpendicular to measurement axis
    let (nx, ny) = (-uy, ux);
    let offset_dist = (dp_in[0] - p1_in[0]) * nx + (dp_in[1] - p1_in[1]) * ny;
    let d1 = [p1_in[0] + nx * offset_dist, p1_in[1] + ny * offset_dist];
    let d2 = [p2_in[0] + nx * offset_dist, p2_in[1] + ny * offset_dist];

    let arrow_len = (span * 0.08).clamp(0.06, 0.18);
    let arrow_half_w = arrow_len * 0.35;
    let a1_top = [
        d1[0] + ux * arrow_len + nx * arrow_half_w,
        d1[1] + uy * arrow_len + ny * arrow_half_w,
    ];
    let a1_bot = [
        d1[0] + ux * arrow_len - nx * arrow_half_w,
        d1[1] + uy * arrow_len - ny * arrow_half_w,
    ];
    let a2_top = [
        d2[0] - ux * arrow_len + nx * arrow_half_w,
        d2[1] - uy * arrow_len + ny * arrow_half_w,
    ];
    let a2_bot = [
        d2[0] - ux * arrow_len - nx * arrow_half_w,
        d2[1] - uy * arrow_len - ny * arrow_half_w,
    ];

    let all_pts = [p1_in, p2_in, d1, d2, a1_top, a1_bot, a2_top, a2_bot];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pt in all_pts {
        min_x = min_x.min(pt[0]);
        min_y = min_y.min(pt[1]);
        max_x = max_x.max(pt[0]);
        max_y = max_y.max(pt[1]);
    }
    let w = (max_x - min_x).max(0.1);
    let h = (max_y - min_y).max(0.1);

    let mut sheet = init_2d_cad_sheet(min_x, min_y, w, h);
    sheet.geometries.clear();

    let to_loc = |pt: [f64; 2]| -> (String, String) {
        (
            format!("{} in", format_clean_f64(pt[0] - min_x)),
            format!("{} in", format_clean_f64(pt[1] - min_y)),
        )
    };

    // Geometry1: Extension Line 1 (`p1 -> d1`)
    let mut g_ext1 = GeometrySection::new(1);
    g_ext1.no_fill = "TRUE".into();
    let (x0, y0) = to_loc(p1_in);
    let (x1, y1) = to_loc(d1);
    g_ext1.rows = vec![GeometryRow::move_to(1, x0, y0), GeometryRow::line_to(2, x1, y1)];
    sheet.add_geometry(g_ext1);

    // Geometry2: Extension Line 2 (`p2 -> d2`)
    let mut g_ext2 = GeometrySection::new(2);
    g_ext2.no_fill = "TRUE".into();
    let (x0, y0) = to_loc(p2_in);
    let (x1, y1) = to_loc(d2);
    g_ext2.rows = vec![GeometryRow::move_to(1, x0, y0), GeometryRow::line_to(2, x1, y1)];
    sheet.add_geometry(g_ext2);

    // Geometry3: Dimension Line (`d1 -> d2`)
    let mut g_dim = GeometrySection::new(3);
    g_dim.no_fill = "TRUE".into();
    let (x0, y0) = to_loc(d1);
    let (x1, y1) = to_loc(d2);
    g_dim.rows = vec![GeometryRow::move_to(1, x0, y0), GeometryRow::line_to(2, x1, y1)];
    sheet.add_geometry(g_dim);

    // Geometry4 & Geometry5: Filled Arrowheads at `d1` and `d2`
    for (g_ix, tip, p_a, p_b) in [(4u32, d1, a1_top, a1_bot), (5u32, d2, a2_top, a2_bot)] {
        let mut g_arr = GeometrySection::new(g_ix);
        g_arr.no_fill = "FALSE".into();
        let (tx, ty) = to_loc(tip);
        let (ax, ay) = to_loc(p_a);
        let (bx, by) = to_loc(p_b);
        g_arr.rows = vec![
            GeometryRow::move_to(1, tx.clone(), ty.clone()),
            GeometryRow::line_to(2, ax, ay),
            GeometryRow::line_to(3, bx, by),
            GeometryRow::line_to(4, tx, ty),
        ];
        sheet.add_geometry(g_arr);
    }

    let mid = [(d1[0] + d2[0]) * 0.5 - min_x, (d1[1] + d2[1]) * 0.5 - min_y];
    sheet.set_formula(
        "TxtPinX",
        format!("{} in", format_clean_f64(mid[0])),
        Some(Unit::Inch),
    );
    sheet.set_formula(
        "TxtPinY",
        format!("{} in", format_clean_f64(mid[1])),
        Some(Unit::Inch),
    );
    sheet.set_formula("BeginArrow", "4", Some(Unit::Number));
    sheet.set_formula("EndArrow", "4", Some(Unit::Number));

    let name = format!("CAD.Dimension.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: label,
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, "DIMENSION", 0, false, color, Some(color));
    set_sheet_color(&mut shape.sheet, "Char.Color", color);
    shape.sheet.set_user_cell(
        "CadMeasuredDistance",
        format_clean_f64(raw_dist),
        "Measured CAD Dimension Distance",
    );
    shape
}

/// Builds a Visio paper-space `VIEWPORT` frame shape preserving center, width, height, and
/// `view_scale` (`User.CadViewScale`).
pub fn build_cad_viewport_shape(
    id: u64,
    center: [f64; 2],
    width: f64,
    height: f64,
    view_scale: f64,
    scale_to_in: f64,
) -> VisioShape {
    let cx = center[0] * scale_to_in;
    let cy = center[1] * scale_to_in;
    let w = (width * scale_to_in).abs().max(0.05);
    let h = (height * scale_to_in).abs().max(0.05);
    let mut sheet = init_2d_cad_sheet(cx - w * 0.5, cy - h * 0.5, w, h);
    sheet.geometries.clear();
    let mut rect = GeometrySection::rectangle(1);
    rect.no_fill = "TRUE".into();
    sheet.add_geometry(rect);

    let clean_scale = if view_scale.is_finite() && view_scale > 0.0 {
        view_scale
    } else {
        1.0
    };
    let vp_color = RgbaColor {
        r: 71,
        g: 85,
        b: 105,
        a: 255,
    };
    let name = format!("CAD.Viewport.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: format!("VIEWPORT ({})", format_clean_f64(clean_scale)),
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, "VIEWPORT", 0, false, vp_color, None);
    shape
        .sheet
        .set_formula("LinePattern", "2", Some(Unit::Number));
    shape.sheet.set_user_cell(
        "CadViewScale",
        format_clean_f64(clean_scale),
        "Paper-Space Viewport Scale",
    );
    shape
}

fn parse_hatch_loops_from_props(props: &[(i32, DxfValue)]) -> (String, Vec<CadHatchLoop>) {
    let mut pattern_name = String::from("SOLID");
    let mut solid_flag = false;
    let mut loops = Vec::new();
    let mut saw_explicit_pat = false;

    for (code, val) in props {
        if *code == 2 {
            let s = val.as_str();
            if !s.trim().is_empty() {
                pattern_name = s.trim().to_string();
                saw_explicit_pat = true;
            }
        } else if *code == 70 {
            solid_flag = val.as_i64().unwrap_or(0) != 0;
        }
    }
    if solid_flag && !saw_explicit_pat {
        pattern_name = "SOLID".into();
    }

    let mut i = 0usize;
    while i < props.len() {
        if props[i].0 == 92 {
            let flags = props[i].1.as_i64().unwrap_or(0);
            let is_poly = (flags & 2) != 0;
            i += 1;
            if is_poly {
                let mut closed = true;
                let mut expected_pts = usize::MAX;
                let mut verts: Vec<[f64; 2]> = Vec::new();
                while i < props.len() && !matches!(props[i].0, 92 | 75 | 76) {
                    match props[i].0 {
                        73 => closed = props[i].1.as_i64().unwrap_or(1) != 0,
                        93 => expected_pts = props[i].1.as_i64().unwrap_or(0).max(0) as usize,
                        10 => {
                            if verts.len() >= expected_pts {
                                break;
                            }
                            verts.push([props[i].1.as_f64().unwrap_or(0.0), 0.0]);
                        }
                        20 => {
                            if let Some(last) = verts.last_mut() {
                                last[1] = props[i].1.as_f64().unwrap_or(0.0);
                            }
                        }
                        97 => {
                            i += 1;
                            while i < props.len() && props[i].0 == 330 {
                                i += 1;
                            }
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if !verts.is_empty() {
                    loops.push(CadHatchLoop {
                        vertices: verts,
                        closed,
                    });
                }
            } else {
                // Edge boundary path (`93` count, `72` edge types: `1` Line, `2` Circular Arc)
                let mut edge_count = 0usize;
                if i < props.len() && props[i].0 == 93 {
                    edge_count = props[i].1.as_i64().unwrap_or(0).max(0) as usize;
                    i += 1;
                }
                let mut verts: Vec<[f64; 2]> = Vec::new();
                for _ in 0..edge_count {
                    while i < props.len() && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76) {
                        i += 1;
                    }
                    if i >= props.len() || props[i].0 != 72 {
                        break;
                    }
                    let edge_type = props[i].1.as_i64().unwrap_or(1);
                    i += 1;
                    if edge_type == 1 {
                        let mut p1 = [0.0, 0.0];
                        let mut p2 = [0.0, 0.0];
                        while i < props.len() && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76) {
                            match props[i].0 {
                                10 => p1[0] = props[i].1.as_f64().unwrap_or(0.0),
                                20 => p1[1] = props[i].1.as_f64().unwrap_or(0.0),
                                11 => p2[0] = props[i].1.as_f64().unwrap_or(0.0),
                                21 => p2[1] = props[i].1.as_f64().unwrap_or(0.0),
                                _ => {}
                            }
                            i += 1;
                        }
                        if verts.is_empty() {
                            verts.push(p1);
                        }
                        verts.push(p2);
                    } else if edge_type == 2 {
                        let mut cx = 0.0;
                        let mut cy = 0.0;
                        let mut r = 1.0;
                        let mut sa = 0.0f64;
                        let mut ea = 180.0f64;
                        let mut ccw = true;
                        while i < props.len() && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76) {
                            match props[i].0 {
                                10 => cx = props[i].1.as_f64().unwrap_or(0.0),
                                20 => cy = props[i].1.as_f64().unwrap_or(0.0),
                                40 => r = props[i].1.as_f64().unwrap_or(1.0),
                                50 => sa = props[i].1.as_f64().unwrap_or(0.0),
                                51 => ea = props[i].1.as_f64().unwrap_or(180.0),
                                73 => ccw = props[i].1.as_i64().unwrap_or(1) != 0,
                                _ => {}
                            }
                            i += 1;
                        }
                        let sr = sa.to_radians();
                        let er = ea.to_radians();
                        let sweep = if ccw {
                            (er - sr).rem_euclid(TAU)
                        } else {
                            (sr - er).rem_euclid(TAU)
                        };
                        let steps = 12usize;
                        for k in 0..=steps {
                            if k == 0 && !verts.is_empty() {
                                continue;
                            }
                            let frac = k as f64 / steps as f64;
                            let ang = if ccw {
                                sr + sweep * frac
                            } else {
                                sr - sweep * frac
                            };
                            verts.push([cx + r * ang.cos(), cy + r * ang.sin()]);
                        }
                    } else {
                        while i < props.len() && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76) {
                            i += 1;
                        }
                    }
                }
                if !verts.is_empty() {
                    loops.push(CadHatchLoop {
                        vertices: verts,
                        closed: true,
                    });
                }
            }
            continue;
        }
        i += 1;
    }

    // Fallback for simplified DXF HATCH records listing `(10, 20)` vertices directly
    if loops.is_empty() {
        let mut fallback_verts: Vec<[f64; 2]> = Vec::new();
        for (code, val) in props {
            if *code == 10 {
                fallback_verts.push([val.as_f64().unwrap_or(0.0), 0.0]);
            } else if *code == 20 {
                if let Some(last) = fallback_verts.last_mut() {
                    last[1] = val.as_f64().unwrap_or(0.0);
                }
            }
        }
        if fallback_verts.len() >= 3 {
            loops.push(CadHatchLoop {
                vertices: fallback_verts,
                closed: true,
            });
        }
    }

    (pattern_name, loops)
}

/// Extracts `HATCH`, `DIMENSION`, and `VIEWPORT` entities from an ASCII DXF stream onto `page_id`
/// of `doc`, returning the number of newly created Visio shapes.
pub fn parse_dxf_extended_entities(dxf_text: &str, doc: &mut VisioDocument, page_id: u32) -> usize {
    let Ok(groups) = parse_dxf_groups(dxf_text.as_bytes()) else {
        return 0;
    };
    if doc.page(page_id).is_none() {
        return 0;
    }

    let mut insunits = 1i32;
    let mut i = 0usize;
    let mut has_sections = false;

    // Pass 1: Extract `$INSUNITS` and `LAYER` table definitions if present
    while i < groups.len() {
        if groups[i].0 == 0 && groups[i].1.as_str().eq_ignore_ascii_case("SECTION") {
            has_sections = true;
            if i + 1 < groups.len() && groups[i + 1].0 == 2 {
                let sec_name = groups[i + 1].1.as_str().to_ascii_uppercase();
                i += 2;
                if sec_name == "HEADER" {
                    while i < groups.len()
                        && !(groups[i].0 == 0
                            && groups[i].1.as_str().eq_ignore_ascii_case("ENDSEC"))
                    {
                        if groups[i].0 == 9
                            && groups[i].1.as_str().eq_ignore_ascii_case("$INSUNITS")
                            && i + 1 < groups.len()
                        {
                            insunits = groups[i + 1].1.as_i64().unwrap_or(1) as i32;
                        }
                        i += 1;
                    }
                } else if sec_name == "TABLES" {
                    while i < groups.len()
                        && !(groups[i].0 == 0
                            && groups[i].1.as_str().eq_ignore_ascii_case("ENDSEC"))
                    {
                        if groups[i].0 == 0 && groups[i].1.as_str().eq_ignore_ascii_case("LAYER") {
                            i += 1;
                            let mut lname = String::from("0");
                            let mut lflags = 0i32;
                            let mut laci = 7i16;
                            let mut ltc = None;
                            while i < groups.len() && groups[i].0 != 0 {
                                match groups[i].0 {
                                    2 => lname = groups[i].1.as_str(),
                                    70 => lflags = groups[i].1.as_i64().unwrap_or(0) as i32,
                                    62 => laci = groups[i].1.as_i64().unwrap_or(7) as i16,
                                    420 => ltc = groups[i].1.as_i64().map(|v| v as u32),
                                    _ => {}
                                }
                                i += 1;
                            }
                            if let Some(page) = doc.page_mut(page_id) {
                                upsert_cad_layer(page, &lname, laci, ltc, lflags, true);
                            }
                            continue;
                        }
                        i += 1;
                    }
                }
                continue;
            }
        }
        i += 1;
    }

    let (_, scale_to_in) = insunits_to_unit_and_factor(insunits);

    // Pass 2: Extract `HATCH`, `DIMENSION`, and `VIEWPORT` entities
    let mut added = 0usize;
    let mut in_entities = !has_sections;
    let mut cur = 0usize;

    while cur < groups.len() {
        if groups[cur].0 == 0 {
            let tag = groups[cur].1.as_str().to_ascii_uppercase();
            if tag == "SECTION" {
                if cur + 1 < groups.len() && groups[cur + 1].0 == 2 {
                    let sname = groups[cur + 1].1.as_str().to_ascii_uppercase();
                    in_entities = sname == "ENTITIES" || sname == "OBJECTS";
                    cur += 2;
                    continue;
                }
            } else if tag == "ENDSEC" {
                in_entities = false;
                cur += 1;
                continue;
            } else if in_entities && matches!(tag.as_str(), "HATCH" | "DIMENSION" | "VIEWPORT") {
                cur += 1;
                let start = cur;
                while cur < groups.len() && groups[cur].0 != 0 {
                    cur += 1;
                }
                let props = &groups[start..cur];

                let mut layer_name = String::from("0");
                let mut aci_opt: Option<i16> = None;
                let mut tc_opt: Option<u32> = None;
                for (c, v) in props {
                    match *c {
                        8 => layer_name = v.as_str(),
                        62 => aci_opt = v.as_i64().map(|n| n as i16),
                        420 => tc_opt = v.as_i64().map(|n| n as u32),
                        _ => {}
                    }
                }

                if let Some(page) = doc.page_mut(page_id) {
                    let (layer_ix, layer_col, layer_locked) =
                        resolve_or_create_layer(page, &layer_name);
                    let color = if let Some(tc) = tc_opt {
                        RgbaColor {
                            r: ((tc >> 16) & 0xFF) as u8,
                            g: ((tc >> 8) & 0xFF) as u8,
                            b: (tc & 0xFF) as u8,
                            a: 255,
                        }
                    } else if let Some(aci) = aci_opt {
                        aci_to_rgba(aci.unsigned_abs())
                    } else {
                        layer_col
                    };
                    let sid = page.next_shape_id();

                    let created_shape = match tag.as_str() {
                        "HATCH" => {
                            let (pat, loops) = parse_hatch_loops_from_props(props);
                            if loops.is_empty() {
                                None
                            } else {
                                Some(build_cad_hatch_shape(sid, &pat, &loops, color, scale_to_in))
                            }
                        }
                        "DIMENSION" => {
                            let mut def_pt = [0.0, 0.0];
                            let mut ext1 = [0.0, 0.0];
                            let mut ext2 = [0.0, 0.0];
                            let mut ovr: Option<String> = None;
                            for (c, v) in props {
                                match *c {
                                    10 => def_pt[0] = v.as_f64().unwrap_or(0.0),
                                    20 => def_pt[1] = v.as_f64().unwrap_or(0.0),
                                    13 => ext1[0] = v.as_f64().unwrap_or(0.0),
                                    23 => ext1[1] = v.as_f64().unwrap_or(0.0),
                                    14 => ext2[0] = v.as_f64().unwrap_or(0.0),
                                    24 => ext2[1] = v.as_f64().unwrap_or(0.0),
                                    1 => ovr = Some(v.as_str()),
                                    _ => {}
                                }
                            }
                            Some(build_cad_linear_dimension_shape(
                                sid,
                                ext1,
                                ext2,
                                def_pt,
                                ovr.as_deref(),
                                color,
                                scale_to_in,
                            ))
                        }
                        "VIEWPORT" => {
                            let mut center = [0.0, 0.0];
                            let mut vp_w = 8.0;
                            let mut vp_h = 6.0;
                            let mut model_h: Option<f64> = None;
                            let mut custom_scale: Option<f64> = None;
                            for (c, v) in props {
                                match *c {
                                    10 => center[0] = v.as_f64().unwrap_or(0.0),
                                    20 => center[1] = v.as_f64().unwrap_or(0.0),
                                    40 => vp_w = v.as_f64().unwrap_or(8.0),
                                    41 => vp_h = v.as_f64().unwrap_or(6.0),
                                    45 => model_h = v.as_f64().filter(|x| *x > 1e-9),
                                    42 | 146 => custom_scale = v.as_f64().filter(|x| *x > 1e-9),
                                    _ => {}
                                }
                            }
                            let view_scale = custom_scale
                                .or_else(|| model_h.map(|mh| vp_h.abs() / mh))
                                .unwrap_or(1.0);
                            Some(build_cad_viewport_shape(
                                sid,
                                center,
                                vp_w,
                                vp_h,
                                view_scale,
                                scale_to_in,
                            ))
                        }
                        _ => None,
                    };

                    if let Some(mut sh) = created_shape {
                        sh.layer_membership = vec![layer_ix];
                        if layer_locked {
                            sh.sheet.set_formula("LockMoveX", "TRUE", Some(Unit::Boolean));
                            sh.sheet.set_formula("LockMoveY", "TRUE", Some(Unit::Boolean));
                        }
                        page.shapes.push(sh);
                        added += 1;
                    }
                }
                continue;
            }
        }
        cur += 1;
    }

    if added > 0 {
        ReactiveEngine::recalculate_document(doc);
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r2004_header_lcg_encrypt_decrypt_roundtrip() {
        let mut plain = vec![0u8; 0x6C];
        plain[0..11].copy_from_slice(b"AcFssFcAJMB");
        for (i, slot) in plain[12..].iter_mut().enumerate() {
            *slot = (i as u8).wrapping_mul(37).wrapping_add(13);
        }

        let encrypted = encrypt_r2004_header_block(&plain);
        assert_ne!(encrypted, plain);
        assert_ne!(&encrypted[0..11], b"AcFssFcAJMB");

        let decrypted = decrypt_r2004_header_block(&encrypted);
        assert_eq!(decrypted, plain);
        assert_eq!(&decrypted[0..11], b"AcFssFcAJMB");
    }

    #[test]
    fn r2004_lz77_compress_decompress_repetitive_and_pseudorandom_buffers() {
        // 1. Edge cases: empty & tiny 1..4 byte buffers
        for len in 0..=25 {
            let tiny: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(19) ^ 0x5A).collect();
            let comp = compress_dwg_r2004_lz77(&tiny);
            let dec = decompress_dwg_r2004_lz77(&comp, tiny.len()).unwrap();
            assert_eq!(dec, tiny, "Failed LZ77 roundtrip for tiny len={len}");
        }

        // 2. Multi-kilobyte repetitive CAD-like buffer + pseudo-random uncompressible runs
        let mut payload = Vec::with_capacity(48_000);
        for block in 0..200u32 {
            if block % 3 != 0 {
                payload.extend_from_slice(b"AcDb:AcDbObjects_LWPOLYLINE_LAYER_STRUCTURAL_0123456789");
                payload.extend_from_slice(&[0xAA; 96]);
            } else {
                // High-entropy pseudo-random run (exercises extended 0x00 literal runs)
                let mut rng = block.wrapping_mul(0x9E37_79B9).wrapping_add(1);
                for _ in 0..300 {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    payload.push((rng & 0xFF) as u8);
                }
            }
        }
        assert!(payload.len() > 36_000);

        let compressed = compress_dwg_r2004_lz77(&payload);
        assert!(
            compressed.len() < payload.len(),
            "LZ77 should shrink repetitive CAD payload ({} vs {})",
            compressed.len(),
            payload.len()
        );
        assert_eq!(&compressed[compressed.len() - 3..], &[0x11, 0x00, 0x00]);

        let restored = decompress_dwg_r2004_lz77(&compressed, payload.len()).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn r2007_reed_solomon_interleave_deinterleave_and_tu_utf16_strings() {
        // Test both (255, 239) system pages (parity=16) and (255, 251) data pages (parity=4)
        let sys_data: Vec<u8> = (0..(239 * 3)).map(|i| (i * 7 + 3) as u8).collect();
        let interleaved_sys = interleave_reed_solomon_blocks(&sys_data, 16, 3);
        assert_eq!(interleaved_sys.len(), 255 * 3);
        let restored_sys = deinterleave_reed_solomon_blocks(&interleaved_sys, 239, 3);
        assert_eq!(restored_sys, sys_data);

        let page_data: Vec<u8> = (0..1200).map(|i| ((i * 31) ^ (i >> 3)) as u8).collect();
        let interleaved_data = interleave_reed_solomon_blocks(&page_data, 4, 2);
        let mut restored_data = deinterleave_reed_solomon_blocks(&interleaved_data, 251, 2);
        restored_data.truncate(page_data.len());
        assert_eq!(restored_data, page_data);

        // TU UTF-16LE string stream test
        let mut w = DwgBitWriter::new();
        w.write_bit(1); // unaligned bit offset!
        write_tu_utf16_string(&mut w, "⌀25.4 mm — Grid A-1");
        let bytes = w.into_bytes();
        let mut r = DwgBitReader::new(&bytes);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(
            read_tu_utf16_string(&mut r).unwrap(),
            "⌀25.4 mm — Grid A-1"
        );
    }

    #[test]
    fn dwg_modern_ac1018_and_ac1032_full_container_roundtrip() {
        let mut src_doc = VisioDocument::new_blank("Modern CAD Plan");
        {
            let page = src_doc.page_mut(1).unwrap();
            let mut wall = VisioShape::new_rect(1, "CoreWall", 4.0, 3.0, 6.0, 2.0);
            wall.text = "SHEAR WALL W-1".into();
            page.shapes.push(wall);

            let beam = VisioShape::new_connector(2, "SteelBeam", 1.0, 7.0, 9.0, 7.0);
            page.shapes.push(beam);
        }
        ReactiveEngine::recalculate_document(&mut src_doc);
        let src_page = src_doc.page(1).unwrap();

        for ver in [DwgVersion::R2004, DwgVersion::R2007, DwgVersion::R2018] {
            let encoded = encode_dwg_modern_container(ver, src_page).unwrap();
            assert_eq!(&encoded[0..6], ver.magic_str().as_bytes());

            let decoded = decode_dwg_modern_container(&encoded).unwrap();
            let p1 = decoded.page(1).unwrap();
            assert!(!p1.shapes.is_empty());
            assert!(
                p1.shapes.iter().any(|s| s.text == "SHEAR WALL W-1"),
                "Decoded {:?} container must preserve text entity",
                ver
            );
            assert_eq!(
                p1.page_sheet
                    .get_cell("User.CadDwgVersion")
                    .unwrap()
                    .value
                    .as_str_coerced()
                    .unwrap(),
                ver.magic_str()
            );
        }
    }

    #[test]
    fn dxf_extended_hatch_dimension_and_viewport_parsing() {
        let ext_dxf = "\
0\nSECTION\n2\nHEADER\n\
9\n$INSUNITS\n70\n1\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nHATCH\n8\nHATCH_SLAB\n2\nANSI31\n70\n0\n91\n2\n\
92\n2\n72\n0\n73\n1\n93\n4\n\
10\n0.0\n20\n0.0\n10\n10.0\n20\n0.0\n10\n10.0\n20\n6.0\n10\n0.0\n20\n6.0\n\
92\n2\n72\n0\n73\n1\n93\n4\n\
10\n2.0\n20\n2.0\n10\n4.0\n20\n2.0\n10\n4.0\n20\n4.0\n10\n2.0\n20\n4.0\n\
0\nDIMENSION\n8\nDIMS\n10\n0.0\n20\n7.5\n13\n0.0\n23\n6.0\n14\n10.0\n24\n6.0\n1\n<>\n\
0\nVIEWPORT\n8\nPAPER_VP\n10\n5.0\n20\n4.0\n40\n12.0\n41\n9.0\n45\n45.0\n\
0\nENDSEC\n0\nEOF\n";

        let mut doc = VisioDocument::new_blank("Extended DXF");
        let added = parse_dxf_extended_entities(ext_dxf, &mut doc, 1);
        assert_eq!(added, 3);

        let page = doc.page(1).unwrap();
        assert_eq!(page.shapes.len(), 3);

        // 1. Verify multi-loop ANSI31 HATCH
        let hatch = &page.shapes[0];
        assert_eq!(hatch.sheet.geometries.len(), 2);
        assert_eq!(hatch.sheet.get_f64("FillPattern"), Some(5.0));

        // 2. Verify linear DIMENSION measured 10.0 distance + extension lines + arrowheads
        let dim = &page.shapes[1];
        assert_eq!(dim.text, "10");
        assert!(dim.sheet.geometries.len() >= 5);
        assert_eq!(dim.sheet.get_f64("EndArrow"), Some(4.0));

        // 3. Verify paper-space VIEWPORT scale (9.0 / 45.0 = 0.2)
        let vp = &page.shapes[2];
        assert!((vp.sheet.get_f64("Width").unwrap() - 12.0).abs() < 1e-6);
        assert!((vp.sheet.get_f64("Height").unwrap() - 9.0).abs() < 1e-6);
        assert!((vp.sheet.get_f64("User.CadViewScale").unwrap() - 0.2).abs() < 1e-6);
    }
}
