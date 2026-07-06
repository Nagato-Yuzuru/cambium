//! Cambium's bottom layer: source positions, symbol interning, the source-file
//! registry, and the diagnostic data model shared by every other crate.
//!
//! This crate knows nothing about syntax, IRs, runtime values, or IO
//! (boundary rules: design.md §2). Rendering of diagnostics lives in the CLI.

mod diag;
mod intern;
mod source;
mod span;

pub use diag::{Diagnostic, ToDiagnostic};
pub use intern::{Interner, Sym};
pub use source::SourceMap;
pub use span::{FileId, Span, Spanned};
