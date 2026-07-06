//! Symbol interning: cheap `Copy` handles for strings, plus `gensym` for
//! hygienic renaming and IR temporaries.

use string_interner::{DefaultBackend, DefaultSymbol, StringInterner};

/// Interned symbol handle. `Copy + Eq + Hash`; resolve back to text with
/// [`Interner::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sym(DefaultSymbol);

/// The pipeline-wide symbol interner (reader, expander, and backends share one).
#[derive(Debug)]
pub struct Interner {
    inner: StringInterner<DefaultBackend>,
    gensym_next: u64,
}

impl Interner {
    /// Creates an empty interner.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    /// Interns `s`, returning the same [`Sym`] for equal strings.
    pub fn intern(&mut self, s: &str) -> Sym {
        let _ = s;
        todo!()
    }

    /// The text behind `sym`.
    ///
    /// # Panics
    ///
    /// Panics if `sym` came from a different interner (programmer error).
    #[must_use]
    pub fn resolve(&self, sym: Sym) -> &str {
        let _ = sym;
        todo!()
    }

    /// A fresh symbol distinct from every [`Sym`] this interner has returned
    /// so far (from `intern` or `gensym` alike).
    ///
    /// Spelled `{prefix}%{n}` with a monotonically increasing `n`, skipping
    /// spellings that are already interned. Note that *later* interning of the
    /// same spelling resolves to the same `Sym`; hygiene relies on the
    /// expander minting temporaries only after user symbols are interned.
    pub fn gensym(&mut self, prefix: &str) -> Sym {
        let _ = prefix;
        todo!()
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn intern_same_string_returns_same_symbol() {
        let mut interner = Interner::new();
        assert_eq!(interner.intern("foo"), interner.intern("foo"));
    }

    #[test]
    fn intern_different_strings_returns_distinct_symbols() {
        let mut interner = Interner::new();
        assert_ne!(interner.intern("foo"), interner.intern("bar"));
    }

    #[test]
    fn resolve_round_trips_the_original_string() {
        let mut interner = Interner::new();
        for text in ["lambda", "call/cc", "λ", ""] {
            let sym = interner.intern(text);
            assert_eq!(interner.resolve(sym), text);
        }
    }

    #[test]
    #[should_panic(expected = "different interner")]
    fn resolve_panics_on_symbol_from_another_interner() {
        let mut a = Interner::new();
        let mut b = Interner::new();
        b.intern("only-in-b");
        let foreign = b.intern("x");
        let _ = a.resolve(foreign);
    }

    #[test]
    fn gensym_returns_fresh_symbols_each_call() {
        let mut interner = Interner::new();
        let user = interner.intern("t");
        let mut seen: HashSet<Sym> = HashSet::new();
        for _ in 0..100 {
            let sym = interner.gensym("t");
            assert_ne!(sym, user);
            assert!(seen.insert(sym), "gensym repeated a symbol");
        }
    }

    #[test]
    fn gensym_skips_spellings_already_interned() {
        let mut interner = Interner::new();
        let user = interner.intern("tmp%0");
        let generated = interner.gensym("tmp");
        assert_ne!(generated, user);
        assert_eq!(interner.resolve(generated), "tmp%1");
    }
}
