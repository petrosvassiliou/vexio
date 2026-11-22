//! Pratt parser and AST for Microsoft Visio ShapeSheet formulas.
//!
//! Supports Visio's full expression syntax:
//! - Unit-annotated numbers (`25.4 mm`, `12 pt`, `45 deg`, `50%`, `2 ft`)
//! - Doubled-quote strings (`"Line 1 ""Quoted"""`)
//! - Cross-page and cross-shape cell references (`Pages[Page-1]!Sheet.4!Width`,
//!   `Sheet.2!Controls.Row_1.Y`, `ThePage!PageWidth`, `TheDoc!DocLangID`, `Prop.Cost`)
//! - Arithmetic, string `&`, comparison, and infix/function logical operators (`AND`, `OR`, `NOT`)
//! - Arbitrary ShapeSheet function calls (`GUARD`, `DEPENDSON`, `SETF`, `PAR`, `PNT`, `INTERSECTX`, etc.)

use crate::units::{Unit, parse_scalar_with_unit};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FormulaParseError {
    #[error("Unexpected token '{found}' at byte {pos}, expected {expected}")]
    UnexpectedToken {
        pos: usize,
        found: String,
        expected: String,
    },
    #[error("Unterminated string literal starting at byte {pos}")]
    UnterminatedString { pos: usize },
    #[error("Invalid number literal '{lexeme}' at byte {pos}")]
    InvalidNumber { pos: usize, lexeme: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SheetTarget {
    SelfSheet,
    ThePage,
    TheDoc,
    ShapeId(u64),
    NamedShape(String),
    PageTarget {
        page_name: String,
        inner: Box<SheetTarget>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellRefPath {
    pub target: SheetTarget,
    /// Canonical dot-joined cell identifier (e.g., `Width`, `Geometry1.X1`, `Controls.Row_1.Y`, `Prop.Cost`).
    pub cell: String,
}

impl CellRefPath {
    pub fn local(cell: impl Into<String>) -> Self {
        Self {
            target: SheetTarget::SelfSheet,
            cell: cell.into(),
        }
    }

    pub fn canonical_cell_key(&self) -> String {
        canonical_cell_name(&self.cell)
    }
}

pub fn canonical_cell_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Number { value: f64, unit: Unit },
    Bool(bool),
    Str(String),
    CellRef(CellRefPath),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call { name: String, args: Vec<Expr> },
}

impl Expr {
    /// Collect all cell references referenced statically by this formula AST.
    pub fn collect_refs(&self, out: &mut Vec<CellRefPath>) {
        match self {
            Self::Number { .. } | Self::Bool(_) | Self::Str(_) => {}
            Self::CellRef(r) => out.push(r.clone()),
            Self::Unary { expr, .. } => expr.collect_refs(out),
            Self::Binary { lhs, rhs, .. } => {
                lhs.collect_refs(out);
                rhs.collect_refs(out);
            }
            Self::Call { args, .. } => {
                for a in args {
                    a.collect_refs(out);
                }
            }
        }
    }

    /// True if the top-level expression wraps its value in `GUARD(...)`.
    pub fn is_top_level_guard(&self) -> bool {
        matches!(self, Self::Call { name, .. } if name.eq_ignore_ascii_case("GUARD"))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Number(f64, Option<Unit>),
    String(String),
    Ident(String),
    BracketIdent(String),
    Bang,
    Dot,
    Comma,
    Semi,
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Amp,
    Percent,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    pos: usize,
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>, FormulaParseError> {
        let mut tokens = Vec::new();
        self.skip_ws();
        // Leading '=' in ShapeSheet formulas is optional syntactic sugar.
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'=' {
            self.pos += 1;
        }
        loop {
            self.skip_ws();
            if self.pos >= self.bytes.len() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    pos: self.pos,
                });
                break;
            }
            let start = self.pos;
            let b = self.bytes[start];
            match b {
                b'"' => {
                    self.pos += 1;
                    let mut out = String::new();
                    let mut closed = false;
                    while self.pos < self.bytes.len() {
                        let Some(ch) = self.src.get(self.pos..).and_then(|s| s.chars().next()) else {
                            break;
                        };
                        self.pos += ch.len_utf8();
                        if ch == '"' {
                            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'"' {
                                out.push('"');
                                self.pos += 1;
                            } else {
                                closed = true;
                                break;
                            }
                        } else {
                            out.push(ch);
                        }
                    }
                    if !closed {
                        return Err(FormulaParseError::UnterminatedString { pos: start });
                    }
                    tokens.push(Token {
                        kind: TokenKind::String(out),
                        pos: start,
                    });
                }
                b'[' => {
                    self.pos += 1;
                    let content_start = self.pos;
                    while self.pos < self.bytes.len() && self.bytes[self.pos] != b']' {
                        self.pos += 1;
                    }
                    if self.pos >= self.bytes.len() {
                        return Err(FormulaParseError::UnexpectedToken {
                            pos: start,
                            found: "EOF".into(),
                            expected: "']'".into(),
                        });
                    }
                    let inner = self.src[content_start..self.pos].trim().to_string();
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::BracketIdent(inner),
                        pos: start,
                    });
                }
                b'0'..=b'9' | b'.'
                    if b != b'.'
                        || (!matches!(
                            tokens.last().map(|t| &t.kind),
                            Some(TokenKind::Ident(_) | TokenKind::BracketIdent(_))
                        ) && start + 1 < self.bytes.len()
                            && self.bytes[start + 1].is_ascii_digit()) =>
                {
                    while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                        self.pos += 1;
                    }
                    if self.pos < self.bytes.len() && self.bytes[self.pos] == b'.' {
                        self.pos += 1;
                        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                            self.pos += 1;
                        }
                    }
                    if self.pos < self.bytes.len()
                        && (self.bytes[self.pos] == b'e' || self.bytes[self.pos] == b'E')
                    {
                        let save = self.pos;
                        self.pos += 1;
                        if self.pos < self.bytes.len()
                            && (self.bytes[self.pos] == b'+' || self.bytes[self.pos] == b'-')
                        {
                            self.pos += 1;
                        }
                        let exp_start = self.pos;
                        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                            self.pos += 1;
                        }
                        if self.pos == exp_start {
                            self.pos = save;
                        }
                    }
                    let num_lexeme = &self.src[start..self.pos];
                    let val: f64 =
                        num_lexeme
                            .parse()
                            .map_err(|_| FormulaParseError::InvalidNumber {
                                pos: start,
                                lexeme: num_lexeme.into(),
                            })?;

                    // Check for optional immediate or whitespace-separated unit token (e.g., `25.4 mm`, `45deg`, `50%`).
                    let after_num = self.pos;
                    self.skip_ws();
                    let unit_start = self.pos;
                    let mut parsed_unit = None;
                    if self.pos < self.bytes.len() && self.bytes[self.pos] == b'%' {
                        self.pos += 1;
                        parsed_unit = Some(Unit::Percent);
                    } else if self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_alphabetic() {
                        while self.pos < self.bytes.len()
                            && self.bytes[self.pos].is_ascii_alphabetic()
                        {
                            self.pos += 1;
                        }
                        let candidate = &self.src[unit_start..self.pos];
                        // Ensure candidate isn't `AND` or `OR` infix operator!
                        let upper = candidate.to_ascii_uppercase();
                        let next_non_ws = {
                            let mut p = self.pos;
                            while p < self.bytes.len() && self.bytes[p].is_ascii_whitespace() {
                                p += 1;
                            }
                            self.bytes.get(p).copied()
                        };
                        if upper != "AND"
                            && upper != "OR"
                            && next_non_ws != Some(b'(')
                            && next_non_ws != Some(b'.')
                            && next_non_ws != Some(b'!')
                        {
                            if let Some(u) = Unit::from_token(candidate) {
                                parsed_unit = Some(u);
                            } else {
                                self.pos = after_num;
                            }
                        } else {
                            self.pos = after_num;
                        }
                    } else {
                        self.pos = after_num;
                    }
                    tokens.push(Token {
                        kind: TokenKind::Number(val, parsed_unit),
                        pos: start,
                    });
                }
                b'!' => {
                    if start + 1 < self.bytes.len() && self.bytes[start + 1] == b'=' {
                        self.pos += 2;
                        tokens.push(Token {
                            kind: TokenKind::NotEq,
                            pos: start,
                        });
                    } else {
                        self.pos += 1;
                        tokens.push(Token {
                            kind: TokenKind::Bang,
                            pos: start,
                        });
                    }
                }
                b'.' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Dot,
                        pos: start,
                    });
                }
                b',' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Comma,
                        pos: start,
                    });
                }
                b';' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Semi,
                        pos: start,
                    });
                }
                b'(' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::LParen,
                        pos: start,
                    });
                }
                b')' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::RParen,
                        pos: start,
                    });
                }
                b'+' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Plus,
                        pos: start,
                    });
                }
                b'-' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Minus,
                        pos: start,
                    });
                }
                b'*' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Star,
                        pos: start,
                    });
                }
                b'/' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Slash,
                        pos: start,
                    });
                }
                b'^' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Caret,
                        pos: start,
                    });
                }
                b'&' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Amp,
                        pos: start,
                    });
                }
                b'%' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::Percent,
                        pos: start,
                    });
                }
                b'=' => {
                    if start + 1 < self.bytes.len() && self.bytes[start + 1] == b'=' {
                        self.pos += 2;
                    } else {
                        self.pos += 1;
                    }
                    tokens.push(Token {
                        kind: TokenKind::Eq,
                        pos: start,
                    });
                }
                b'<' => {
                    if start + 1 < self.bytes.len() && self.bytes[start + 1] == b'>' {
                        self.pos += 2;
                        tokens.push(Token {
                            kind: TokenKind::NotEq,
                            pos: start,
                        });
                    } else if start + 1 < self.bytes.len() && self.bytes[start + 1] == b'=' {
                        self.pos += 2;
                        tokens.push(Token {
                            kind: TokenKind::Le,
                            pos: start,
                        });
                    } else {
                        self.pos += 1;
                        tokens.push(Token {
                            kind: TokenKind::Lt,
                            pos: start,
                        });
                    }
                }
                b'>' => {
                    if start + 1 < self.bytes.len() && self.bytes[start + 1] == b'=' {
                        self.pos += 2;
                        tokens.push(Token {
                            kind: TokenKind::Ge,
                            pos: start,
                        });
                    } else {
                        self.pos += 1;
                        tokens.push(Token {
                            kind: TokenKind::Gt,
                            pos: start,
                        });
                    }
                }
                _ => {
                    let Some(first_ch) = self.src.get(start..).and_then(|s| s.chars().next()) else {
                        break;
                    };
                    if first_ch.is_alphabetic() || first_ch == '_' || first_ch == '$' {
                        self.pos += first_ch.len_utf8();
                        while let Some(ch) = self.src.get(self.pos..).and_then(|s| s.chars().next()) {
                            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                                self.pos += ch.len_utf8();
                            } else {
                                break;
                            }
                        }
                        let ident = self.src[start..self.pos].to_string();
                        tokens.push(Token {
                            kind: TokenKind::Ident(ident),
                            pos: start,
                        });
                    } else {
                        return Err(FormulaParseError::UnexpectedToken {
                            pos: start,
                            found: first_ch.to_string(),
                            expected: "expression token".into(),
                        });
                    }
                }
            }
        }
        Ok(tokens)
    }
}

pub fn parse_formula(src: &str) -> Result<Expr, FormulaParseError> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Ok(Expr::Number {
            value: 0.0,
            unit: Unit::Number,
        });
    }
    // Fast path for plain unit literals without formula prefix (`=`) that aren't expressions.
    if !trimmed.starts_with('=') {
        if let Some((val, unit)) = parse_scalar_with_unit(trimmed) {
            return Ok(Expr::Number { value: val, unit });
        }
    }
    let mut lexer = Lexer::new(trimmed);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser { tokens, idx: 0 };
    let expr = parser.parse_expr(0)?;
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let tok = parser.peek();
        return Err(FormulaParseError::UnexpectedToken {
            pos: tok.pos,
            found: format!("{:?}", tok.kind),
            expected: "end of formula".into(),
        });
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.idx]
    }

    fn peek_n(&self, n: usize) -> &Token {
        let i = (self.idx + n).min(self.tokens.len() - 1);
        &self.tokens[i]
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens[self.idx].clone();
        if self.idx + 1 < self.tokens.len() {
            self.idx += 1;
        }
        tok
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, FormulaParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix `%`
            if matches!(self.peek().kind, TokenKind::Percent) {
                if 15 < min_bp {
                    break;
                }
                self.bump();
                lhs = Expr::Binary {
                    op: BinaryOp::Mul,
                    lhs: Box::new(lhs),
                    rhs: Box::new(Expr::Number {
                        value: 0.01,
                        unit: Unit::Number,
                    }),
                };
                continue;
            }

            let Some((op, l_bp, r_bp)) = self.peek_infix_op() else {
                break;
            };
            if l_bp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn peek_infix_op(&self) -> Option<(BinaryOp, u8, u8)> {
        match &self.peek().kind {
            TokenKind::Ident(id) if id.eq_ignore_ascii_case("OR") && !matches!(self.peek_n(1).kind, TokenKind::LParen) => {
                Some((BinaryOp::Or, 1, 2))
            }
            TokenKind::Ident(id) if id.eq_ignore_ascii_case("AND") && !matches!(self.peek_n(1).kind, TokenKind::LParen) => {
                Some((BinaryOp::And, 3, 4))
            }
            TokenKind::Eq => Some((BinaryOp::Eq, 5, 6)),
            TokenKind::NotEq => Some((BinaryOp::NotEq, 5, 6)),
            TokenKind::Lt => Some((BinaryOp::Lt, 5, 6)),
            TokenKind::Le => Some((BinaryOp::Le, 5, 6)),
            TokenKind::Gt => Some((BinaryOp::Gt, 5, 6)),
            TokenKind::Ge => Some((BinaryOp::Ge, 5, 6)),
            TokenKind::Amp => Some((BinaryOp::Concat, 7, 8)),
            TokenKind::Plus => Some((BinaryOp::Add, 9, 10)),
            TokenKind::Minus => Some((BinaryOp::Sub, 9, 10)),
            TokenKind::Star => Some((BinaryOp::Mul, 11, 12)),
            TokenKind::Slash => Some((BinaryOp::Div, 11, 12)),
            TokenKind::Caret => Some((BinaryOp::Pow, 14, 13)), // right-associative
            _ => None,
        }
    }

    fn parse_prefix(&mut self) -> Result<Expr, FormulaParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Number(val, opt_unit) => {
                self.bump();
                Ok(Expr::Number {
                    value: val,
                    unit: opt_unit.unwrap_or(Unit::Number),
                })
            }
            TokenKind::String(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            TokenKind::Minus => {
                self.bump();
                let rhs = self.parse_expr(13)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(rhs),
                })
            }
            TokenKind::Plus => {
                self.bump();
                let rhs = self.parse_expr(13)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Pos,
                    expr: Box::new(rhs),
                })
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.parse_expr(0)?;
                match self.peek().kind {
                    TokenKind::RParen => {
                        self.bump();
                        Ok(inner)
                    }
                    _ => Err(FormulaParseError::UnexpectedToken {
                        pos: self.peek().pos,
                        found: format!("{:?}", self.peek().kind),
                        expected: "')'".into(),
                    }),
                }
            }
            TokenKind::Ident(ref id) => {
                let upper = id.to_ascii_uppercase();
                if upper == "TRUE" && !matches!(self.peek_n(1).kind, TokenKind::LParen) {
                    self.bump();
                    return Ok(Expr::Bool(true));
                }
                if upper == "FALSE" && !matches!(self.peek_n(1).kind, TokenKind::LParen) {
                    self.bump();
                    return Ok(Expr::Bool(false));
                }
                if upper == "NOT" && !matches!(self.peek_n(1).kind, TokenKind::LParen) {
                    self.bump();
                    let rhs = self.parse_expr(13)?;
                    return Ok(Expr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(rhs),
                    });
                }
                // Function call: `IDENT(...)`
                if matches!(self.peek_n(1).kind, TokenKind::LParen) {
                    let name = id.clone();
                    self.bump(); // ident
                    self.bump(); // (
                    let mut args = Vec::new();
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        loop {
                            // Visio allows omitted arguments like `BOUND(Width, 0, FALSE, 1 in, 5 in)` or `RGB(255,,0)`.
                            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semi) {
                                args.push(Expr::Number {
                                    value: 0.0,
                                    unit: Unit::Number,
                                });
                            } else {
                                args.push(self.parse_expr(0)?);
                            }
                            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semi) {
                                self.bump();
                                if matches!(self.peek().kind, TokenKind::RParen) {
                                    args.push(Expr::Number {
                                        value: 0.0,
                                        unit: Unit::Number,
                                    });
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        return Err(FormulaParseError::UnexpectedToken {
                            pos: self.peek().pos,
                            found: format!("{:?}", self.peek().kind),
                            expected: "')'".into(),
                        });
                    }
                    self.bump();
                    return Ok(Expr::Call { name, args });
                }

                // Otherwise parse a Visio cell reference path (`Pages[Page-1]!Sheet.5!Controls.Row_1.Y`, `Prop.Cost`, `Width`).
                self.parse_cell_ref()
            }
            TokenKind::BracketIdent(_) => self.parse_cell_ref(),
            _ => Err(FormulaParseError::UnexpectedToken {
                pos: tok.pos,
                found: format!("{:?}", tok.kind),
                expected: "number, string, function, or cell reference".into(),
            }),
        }
    }

    fn parse_cell_ref(&mut self) -> Result<Expr, FormulaParseError> {
        let mut target = SheetTarget::SelfSheet;

        // Optional `Pages[PageName]!` prefix
        if let TokenKind::Ident(id) = &self.peek().kind {
            if id.eq_ignore_ascii_case("Pages") && matches!(self.peek_n(1).kind, TokenKind::BracketIdent(_)) {
                self.bump(); // Pages
                let bumped = self.bump();
                let TokenKind::BracketIdent(page_name) = bumped.kind else {
                    return Err(FormulaParseError::UnexpectedToken {
                        pos: bumped.pos,
                        found: format!("{:?}", bumped.kind),
                        expected: "'[PageName]'".into(),
                    });
                };
                if !matches!(self.peek().kind, TokenKind::Bang) {
                    return Err(FormulaParseError::UnexpectedToken {
                        pos: self.peek().pos,
                        found: format!("{:?}", self.peek().kind),
                        expected: "'!' after Pages[...]".into(),
                    });
                }
                self.bump(); // !
                let inner_expr = self.parse_cell_ref()?;
                if let Expr::CellRef(inner_ref) = inner_expr {
                    return Ok(Expr::CellRef(CellRefPath {
                        target: SheetTarget::PageTarget {
                            page_name,
                            inner: Box::new(inner_ref.target),
                        },
                        cell: inner_ref.cell,
                    }));
                }
            }
        }

        // Read segment up to optional `!`
        let seg1 = self.read_dotted_segment()?;
        if matches!(self.peek().kind, TokenKind::Bang) {
            self.bump(); // !
            let lower = seg1.to_ascii_lowercase();
            target = if lower == "thepage" {
                SheetTarget::ThePage
            } else if lower == "thedoc" {
                SheetTarget::TheDoc
            } else if let Some(num_str) = lower.strip_prefix("sheet.") {
                if let Ok(id) = num_str.parse::<u64>() {
                    SheetTarget::ShapeId(id)
                } else {
                    SheetTarget::NamedShape(seg1)
                }
            } else {
                SheetTarget::NamedShape(seg1)
            };
            let cell = self.read_dotted_segment()?;
            Ok(Expr::CellRef(CellRefPath { target, cell }))
        } else {
            Ok(Expr::CellRef(CellRefPath { target, cell: seg1 }))
        }
    }

    fn read_dotted_segment(&mut self) -> Result<String, FormulaParseError> {
        let mut out = String::new();
        match &self.peek().kind {
            TokenKind::Ident(id) => {
                out.push_str(id);
                self.bump();
            }
            TokenKind::BracketIdent(id) => {
                out.push_str(id);
                self.bump();
            }
            _ => {
                return Err(FormulaParseError::UnexpectedToken {
                    pos: self.peek().pos,
                    found: format!("{:?}", self.peek().kind),
                    expected: "identifier".into(),
                });
            }
        }

        loop {
            if matches!(self.peek().kind, TokenKind::Dot) {
                self.bump();
                out.push('.');
                match &self.peek().kind {
                    TokenKind::Ident(id) => {
                        out.push_str(id);
                        self.bump();
                    }
                    TokenKind::Number(n, None) => {
                        out.push_str(&(*n as i64).to_string());
                        self.bump();
                    }
                    TokenKind::BracketIdent(id) => {
                        out.push_str(id);
                        self.bump();
                    }
                    _ => {
                        return Err(FormulaParseError::UnexpectedToken {
                            pos: self.peek().pos,
                            found: format!("{:?}", self.peek().kind),
                            expected: "cell member name after '.'".into(),
                        });
                    }
                }
            } else if let TokenKind::BracketIdent(idx_str) = &self.peek().kind {
                out.push('[');
                out.push_str(idx_str);
                out.push(']');
                self.bump();
            } else {
                break;
            }
        }
        Ok(out)
    }
}
