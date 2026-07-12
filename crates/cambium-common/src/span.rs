//! Source positions: file handles, byte spans, and spanned values.

use std::ops::Range;

/// Handle to a file registered in a [`crate::SourceMap`].
///
/// Only a `SourceMap` mints `FileId`s; using an id with a different map is a
/// programmer error and panics on lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub(crate) u32);

/// Half-open byte range `[start, end)` within a single source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// File this range points into.
    pub file: FileId,
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
}

impl Span {
    /// Creates a span. `start <= end` is the caller's contract (debug-asserted).
    #[must_use]
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        debug_assert!(
            start <= end,
            "span requires start <= end, got {start}..{end}"
        );
        Self { file, start, end }
    }

    /// The smallest span covering both `self` and `other`.
    ///
    /// # Panics
    ///
    /// Panics if the two spans point into different files.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        assert_eq!(
            self.file, other.file,
            "cannot merge spans from different files"
        );
        Self {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Byte range form, as consumed by diagnostic labels and slicing.
    #[must_use]
    pub fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// A value paired with the source span it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The value itself.
    pub node: T,
    /// Where `node` was read from.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Pairs a value with its span.
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }

    /// Transforms the value, keeping the span.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            node: f(self.node),
            span: self.span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> FileId {
        FileId(0)
    }

    #[test]
    fn merge_joins_two_disjoint_spans() {
        let merged = Span::new(file(), 3, 7).merge(Span::new(file(), 10, 12));
        assert_eq!(merged, Span::new(file(), 3, 12));
    }

    #[test]
    fn merge_is_order_independent() {
        let a = Span::new(file(), 3, 7);
        let b = Span::new(file(), 10, 12);
        assert_eq!(a.merge(b), b.merge(a));
    }

    #[test]
    fn merge_of_identical_spans_is_identity() {
        let a = Span::new(file(), 5, 9);
        assert_eq!(a.merge(a), a);
    }

    #[test]
    fn merge_of_nested_spans_keeps_the_outer_one() {
        let outer = Span::new(file(), 2, 20);
        let inner = Span::new(file(), 5, 9);
        assert_eq!(outer.merge(inner), outer);
    }

    #[test]
    #[should_panic(expected = "different files")]
    fn merge_panics_across_files() {
        let _ = Span::new(FileId(0), 0, 1).merge(Span::new(FileId(1), 0, 1));
    }

    #[test]
    #[should_panic(expected = "start <= end")]
    fn new_rejects_inverted_bounds() {
        let _ = Span::new(file(), 5, 2);
    }

    #[test]
    fn range_is_half_open_usize() {
        assert_eq!(Span::new(file(), 1, 4).range(), 1..4);
    }

    #[test]
    fn range_of_empty_span_is_empty() {
        assert!(Span::new(file(), 3, 3).range().is_empty());
    }

    #[test]
    fn spanned_map_transforms_node_and_keeps_span() {
        let span = Span::new(file(), 0, 2);
        let spanned = Spanned::new(21_i64, span).map(|n| n * 2);
        assert_eq!(spanned.node, 42);
        assert_eq!(spanned.span, span);
    }
}
