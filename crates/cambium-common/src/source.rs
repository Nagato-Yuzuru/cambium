//! Registry of source files: names + full text. [`FileId`] is an index into
//! it. Implements codespan-reporting's [`Files`] so diagnostics (and, later,
//! DWARF generation) can map spans back to lines.

use codespan_reporting::files::{Error, Files, SimpleFile};

use crate::span::FileId;

/// Owns every source text of a session (files, REPL lines).
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SimpleFile<String, String>>,
}

impl SourceMap {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    /// Registers a source text under `name` and returns its handle.
    pub fn add(&mut self, name: impl Into<String>, source: impl Into<String>) -> FileId {
        let _ = (name, source);
        todo!()
    }

    /// The name a file was registered under.
    ///
    /// # Panics
    ///
    /// Panics on an unknown [`FileId`] (one minted by a different map).
    #[must_use]
    pub fn name(&self, id: FileId) -> &str {
        let _ = id;
        todo!()
    }

    /// The full source text of a file.
    ///
    /// # Panics
    ///
    /// Panics on an unknown [`FileId`] (one minted by a different map).
    #[must_use]
    pub fn source(&self, id: FileId) -> &str {
        let _ = id;
        todo!()
    }

    fn file(&self, id: FileId) -> Result<&SimpleFile<String, String>, Error> {
        let _ = id;
        todo!()
    }
}

impl<'a> Files<'a> for SourceMap {
    type FileId = FileId;
    type Name = &'a str;
    type Source = &'a str;

    fn name(&'a self, id: FileId) -> Result<&'a str, Error> {
        Ok(self.file(id)?.name().as_str())
    }

    fn source(&'a self, id: FileId) -> Result<&'a str, Error> {
        Ok(self.file(id)?.source().as_str())
    }

    fn line_index(&'a self, id: FileId, byte_index: usize) -> Result<usize, Error> {
        self.file(id)?.line_index((), byte_index)
    }

    fn line_range(
        &'a self,
        id: FileId,
        line_index: usize,
    ) -> Result<std::ops::Range<usize>, Error> {
        self.file(id)?.line_range((), line_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_registers_files_with_distinct_ids_and_exact_content() {
        let mut map = SourceMap::new();
        let a = map.add("a.scm", "(+ 1 2)");
        let b = map.add("empty.scm", "");
        assert_ne!(a, b);
        assert_eq!(map.name(a), "a.scm");
        assert_eq!(map.source(a), "(+ 1 2)");
        assert_eq!(map.name(b), "empty.scm");
        assert_eq!(map.source(b), "");
    }

    #[test]
    fn line_index_locates_byte_offsets() {
        let mut map = SourceMap::new();
        let id = map.add("lines.scm", "ab\ncd\n");
        assert_eq!(Files::line_index(&map, id, 0).unwrap(), 0);
        assert_eq!(Files::line_index(&map, id, 2).unwrap(), 0);
        assert_eq!(Files::line_index(&map, id, 3).unwrap(), 1);
        assert_eq!(Files::line_range(&map, id, 1).unwrap(), 3..6);
    }

    #[test]
    #[should_panic(expected = "unknown FileId")]
    fn name_panics_on_unknown_file_id() {
        let map = SourceMap::new();
        let _ = map.name(FileId(99));
    }
}
