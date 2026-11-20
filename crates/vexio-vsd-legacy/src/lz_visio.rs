//! MS-VSD sliding-window decompression & compression (`[MS-VSD]` Section 2.4).
//!
//! Visio binary streams (`VisioDocument` inside OLE2 CFB) compress pointer streams
//! using a 4096-byte ring buffer initialized to zeros with write cursor `0x0FEE`.

pub fn decompress_vsd_stream(src: &[u8]) -> Vec<u8> {
    let mut ring = [0u8; 4096];
    let mut pos: usize = 0x0FEE;
    let mut out = Vec::with_capacity(src.len() * 2);
    let mut i = 0usize;

    while i < src.len() {
        let flags = src[i];
        i += 1;
        for bit in 0..8 {
            if i >= src.len() {
                break;
            }
            if (flags & (1 << bit)) != 0 {
                // Literal byte
                let b = src[i];
                i += 1;
                out.push(b);
                ring[pos] = b;
                pos = (pos + 1) & 0x0FFF;
            } else {
                // 2-byte back-reference
                if i + 1 >= src.len() {
                    break;
                }
                let b1 = src[i] as usize;
                let b2 = src[i + 1] as usize;
                i += 2;
                let offset = b1 | ((b2 & 0xF0) << 4);
                let length = (b2 & 0x0F) + 3;
                for k in 0..length {
                    let c = ring[(offset + k) & 0x0FFF];
                    out.push(c);
                    ring[pos] = c;
                    pos = (pos + 1) & 0x0FFF;
                }
            }
        }
    }
    out
}

/// Deterministic literal-Chunk encoder producing 100% MS-VSD-compliant compressed streams.
pub fn compress_vsd_stream(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 8 + 2);
    for chunk in raw.chunks(8) {
        let mask: u8 = ((1u16 << chunk.len()) - 1) as u8;
        out.push(mask);
        out.extend_from_slice(chunk);
    }
    out
}
