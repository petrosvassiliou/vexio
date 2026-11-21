//! Deterministic Command Undo/Redo Stack and CRC32-Verified Binary Write-Ahead Log (WAL).
//!
//! Designed for both native mmap/file persistence and ChromeOS PWA Origin Private
//! File System (`FileSystemSyncAccessHandle`) zero-latency crash-proof autosave.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vexio_shapesheet::{GlueEndpoint, MarkupInkStroke, ReactiveEngine, VisioDocument, VisioPage, VisioShape};

const WAL_MAGIC: &[u8; 8] = b"VEXIOWAL";

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Corrupt WAL header or CRC32 mismatch at entry {index}")]
    CorruptWal { index: usize },
    #[error("Target page {0} not found")]
    PageNotFound(u32),
    #[error("Target shape {0} not found")]
    ShapeNotFound(u64),
    #[error("Cell '{0}' is protected by GUARD")]
    CellGuarded(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocCommand {
    SetCellFormula {
        page_id: u32,
        shape_id: u64,
        cell_name: String,
        old_formula: String,
        new_formula: String,
        force_setf: bool,
    },
    SetShapeText {
        page_id: u32,
        shape_id: u64,
        old_text: String,
        new_text: String,
    },
    AddShape {
        page_id: u32,
        shape: VisioShape,
    },
    DeleteShape {
        page_id: u32,
        shape: VisioShape,
        removed_connects: Vec<GlueEndpoint>,
    },
    AddPage {
        page: VisioPage,
    },
    DeletePage {
        index: usize,
        page: VisioPage,
    },
    AddConnect {
        page_id: u32,
        connect: GlueEndpoint,
    },
    AddInkStroke {
        page_id: u32,
        stroke: MarkupInkStroke,
    },
    Batch {
        label: String,
        commands: Vec<DocCommand>,
    },
    ReplaceDocument {
        label: String,
        before: Box<VisioDocument>,
        after: Box<VisioDocument>,
    },
}

impl DocCommand {
    pub fn apply(&self, doc: &mut VisioDocument) -> Result<(), JournalError> {
        match self {
            Self::SetCellFormula {
                page_id,
                shape_id,
                cell_name,
                new_formula,
                force_setf,
                ..
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                let shape = page
                    .find_shape_mut(*shape_id)
                    .ok_or(JournalError::ShapeNotFound(*shape_id))?;
                shape
                    .sheet
                    .try_set_formula_respecting_guard(cell_name, new_formula.clone(), *force_setf)
                    .map_err(|_| JournalError::CellGuarded(cell_name.clone()))?;
            }
            Self::SetShapeText {
                page_id,
                shape_id,
                new_text,
                ..
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                let shape = page
                    .find_shape_mut(*shape_id)
                    .ok_or(JournalError::ShapeNotFound(*shape_id))?;
                shape.text = new_text.clone();
            }
            Self::AddShape { page_id, shape } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.shapes.push(shape.clone());
            }
            Self::DeleteShape {
                page_id, shape, ..
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.shapes.retain(|s| s.id != shape.id);
                page.connects.retain(|c| {
                    c.connector_shape_id != shape.id && c.target_shape_id != shape.id
                });
            }
            Self::AddPage { page } => {
                doc.pages.push(page.clone());
            }
            Self::DeletePage { page, .. } => {
                doc.pages.retain(|p| p.id != page.id);
            }
            Self::AddConnect { page_id, connect } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.connects.push(connect.clone());
            }
            Self::AddInkStroke { page_id, stroke } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.ink_strokes.push(stroke.clone());
            }
            Self::Batch { commands, .. } => {
                for cmd in commands {
                    cmd.apply(doc)?;
                }
            }
            Self::ReplaceDocument { after, .. } => {
                *doc = (**after).clone();
            }
        }
        Ok(())
    }

    pub fn revert(&self, doc: &mut VisioDocument) -> Result<(), JournalError> {
        match self {
            Self::SetCellFormula {
                page_id,
                shape_id,
                cell_name,
                old_formula,
                ..
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                let shape = page
                    .find_shape_mut(*shape_id)
                    .ok_or(JournalError::ShapeNotFound(*shape_id))?;
                shape
                    .sheet
                    .try_set_formula_respecting_guard(cell_name, old_formula.clone(), true)
                    .map_err(|_| JournalError::CellGuarded(cell_name.clone()))?;
            }
            Self::SetShapeText {
                page_id,
                shape_id,
                old_text,
                ..
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                let shape = page
                    .find_shape_mut(*shape_id)
                    .ok_or(JournalError::ShapeNotFound(*shape_id))?;
                shape.text = old_text.clone();
            }
            Self::AddShape { page_id, shape } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.shapes.retain(|s| s.id != shape.id);
            }
            Self::DeleteShape {
                page_id,
                shape,
                removed_connects,
            } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.shapes.push(shape.clone());
                page.connects.extend(removed_connects.iter().cloned());
            }
            Self::AddPage { page } => {
                doc.pages.retain(|p| p.id != page.id);
            }
            Self::DeletePage { index, page } => {
                let pos = (*index).min(doc.pages.len());
                doc.pages.insert(pos, page.clone());
            }
            Self::AddConnect { page_id, connect } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.connects.retain(|c| c != connect);
            }
            Self::AddInkStroke { page_id, stroke } => {
                let page = doc
                    .page_mut(*page_id)
                    .ok_or(JournalError::PageNotFound(*page_id))?;
                page.ink_strokes.retain(|s| s.id != stroke.id);
            }
            Self::Batch { commands, .. } => {
                for cmd in commands.iter().rev() {
                    cmd.revert(doc)?;
                }
            }
            Self::ReplaceDocument { before, .. } => {
                *doc = (**before).clone();
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandJournal {
    undo_stack: Vec<DocCommand>,
    redo_stack: Vec<DocCommand>,
}

impl CommandJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, doc: &mut VisioDocument, cmd: DocCommand) -> Result<(), JournalError> {
        cmd.apply(doc)?;
        ReactiveEngine::recalculate_document(doc);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        Ok(())
    }

    /// Execute an arbitrary document mutation closure and record an atomic `ReplaceDocument` undo entry
    /// whenever the document state changed.
    pub fn record_mutation<R, F>(&mut self, doc: &mut VisioDocument, label: impl Into<String>, f: F) -> R
    where
        F: FnOnce(&mut VisioDocument) -> R,
    {
        let before = doc.clone();
        let result = f(doc);
        ReactiveEngine::recalculate_document(doc);
        if *doc != before {
            self.undo_stack.push(DocCommand::ReplaceDocument {
                label: label.into(),
                before: Box::new(before),
                after: Box::new(doc.clone()),
            });
            self.redo_stack.clear();
        }
        result
    }

    pub fn undo(&mut self, doc: &mut VisioDocument) -> Result<bool, JournalError> {
        let Some(cmd) = self.undo_stack.pop() else {
            return Ok(false);
        };
        cmd.revert(doc)?;
        ReactiveEngine::recalculate_document(doc);
        self.redo_stack.push(cmd);
        Ok(true)
    }

    pub fn redo(&mut self, doc: &mut VisioDocument) -> Result<bool, JournalError> {
        let Some(cmd) = self.redo_stack.pop() else {
            return Ok(false);
        };
        cmd.apply(doc)?;
        ReactiveEngine::recalculate_document(doc);
        self.undo_stack.push(cmd);
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Encode the entire command journal into a CRC32-framed binary WAL stream suitable for OPFS `SyncAccessHandle`.
    pub fn encode_wal(&self) -> Result<Vec<u8>, JournalError> {
        let mut out = Vec::new();
        out.extend_from_slice(WAL_MAGIC);
        out.extend_from_slice(&(self.undo_stack.len() as u32).to_le_bytes());
        for cmd in &self.undo_stack {
            let payload = serde_json::to_vec(cmd)?;
            let crc = crc32fast::hash(&payload);
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&payload);
        }
        Ok(out)
    }

    /// Replay a CRC32-framed binary WAL onto `doc`, restoring exact undo history after crash recovery.
    pub fn replay_wal(bytes: &[u8], doc: &mut VisioDocument) -> Result<Self, JournalError> {
        if bytes.len() < 12 || &bytes[0..8] != WAL_MAGIC {
            return Err(JournalError::CorruptWal { index: 0 });
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut off = 12usize;
        let mut journal = Self::new();
        for idx in 0..count {
            if off + 8 > bytes.len() {
                return Err(JournalError::CorruptWal { index: idx });
            }
            let len = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
            let expected_crc = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
            off += 8;
            if off + len > bytes.len() {
                return Err(JournalError::CorruptWal { index: idx });
            }
            let payload = &bytes[off..off + len];
            off += len;
            if crc32fast::hash(payload) != expected_crc {
                return Err(JournalError::CorruptWal { index: idx });
            }
            let cmd: DocCommand = serde_json::from_slice(payload)?;
            cmd.apply(doc)?;
            journal.undo_stack.push(cmd);
        }
        ReactiveEngine::recalculate_document(doc);
        Ok(journal)
    }
}
