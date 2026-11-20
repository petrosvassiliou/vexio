//! OpenXML Packaging Conventions (OPC) constants and archive helpers for Visio formats.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Error)]
pub enum VsdxError {
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("Missing required OpenXML part: {0}")]
    MissingPart(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VsdxPackageKind {
    Drawing,              // .vsdx
    MacroDrawing,         // .vsdm
    Stencil,              // .vssx
    MacroStencil,         // .vssm
    Template,             // .vstx
    MacroTemplate,        // .vstm
}

impl VsdxPackageKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "vsdm" => Self::MacroDrawing,
            "vssx" => Self::Stencil,
            "vssm" => Self::MacroStencil,
            "vstx" => Self::Template,
            "vstm" => Self::MacroTemplate,
            _ => Self::Drawing,
        }
    }

    pub fn from_main_content_type(ct: &str) -> Self {
        match ct {
            "application/vnd.ms-visio.drawing.macroEnabled.main+xml" => Self::MacroDrawing,
            "application/vnd.ms-visio.stencil.main+xml" => Self::Stencil,
            "application/vnd.ms-visio.stencil.macroEnabled.main+xml" => Self::MacroStencil,
            "application/vnd.ms-visio.template.main+xml" => Self::Template,
            "application/vnd.ms-visio.template.macroEnabled.main+xml" => Self::MacroTemplate,
            _ => Self::Drawing,
        }
    }

    pub fn main_content_type(self) -> &'static str {
        match self {
            Self::Drawing => "application/vnd.ms-visio.drawing.main+xml",
            Self::MacroDrawing => "application/vnd.ms-visio.drawing.macroEnabled.main+xml",
            Self::Stencil => "application/vnd.ms-visio.stencil.main+xml",
            Self::MacroStencil => "application/vnd.ms-visio.stencil.macroEnabled.main+xml",
            Self::Template => "application/vnd.ms-visio.template.main+xml",
            Self::MacroTemplate => "application/vnd.ms-visio.template.macroEnabled.main+xml",
        }
    }

    pub fn is_macro_enabled(self) -> bool {
        matches!(
            self,
            Self::MacroDrawing | Self::MacroStencil | Self::MacroTemplate
        )
    }
}

pub const NS_VISIO_MAIN: &str = "http://schemas.microsoft.com/office/visio/2012/main";
pub const NS_RELS_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_RELS_PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

#[derive(Debug, Clone, Default)]
pub struct RawOpcArchive {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl RawOpcArchive {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, VsdxError> {
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        let mut files = BTreeMap::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let normalized = entry.name().trim_start_matches('/').replace('\\', "/");
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            files.insert(normalized, buf);
        }
        Ok(Self { files })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, VsdxError> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o644);
            for (path, data) in &self.files {
                writer.start_file(path, opts)?;
                writer.write_all(data)?;
            }
            writer.finish()?;
        }
        Ok(cursor.into_inner())
    }

    pub fn get_str(&self, path: &str) -> Result<Option<String>, VsdxError> {
        let key = path.trim_start_matches('/');
        match self.files.get(key) {
            Some(bytes) => Ok(Some(String::from_utf8(bytes.clone())?)),
            None => Ok(None),
        }
    }
}

/// Known standard part paths generated directly by `vexio-vsdx`; any other part in an opened
/// archive is preserved byte-for-byte in `VisioDocument::opaque_bag`.
pub fn is_managed_opc_part(path: &str) -> bool {
    let p = path.trim_start_matches('/');
    p == "[Content_Types].xml"
        || p == "_rels/.rels"
        || p == "docProps/core.xml"
        || p == "docProps/app.xml"
        || p == "docProps/custom.xml"
        || p == "visio/document.xml"
        || p == "visio/_rels/document.xml.rels"
        || p == "visio/windows.xml"
        || p == "visio/comments.xml"
        || p == "visio/masters/masters.xml"
        || p == "visio/masters/_rels/masters.xml.rels"
        || (p.starts_with("visio/masters/master") && p.ends_with(".xml"))
        || p == "visio/pages/pages.xml"
        || p == "visio/pages/_rels/pages.xml.rels"
        || (p.starts_with("visio/pages/page") && p.ends_with(".xml"))
}

pub fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
