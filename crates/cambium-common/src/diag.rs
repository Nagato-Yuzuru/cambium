//! Diagnostic data model. Pure data — rendering (colors, terminal output)
//! belongs to the CLI crate.

use crate::span::FileId;

/// Workspace-wide diagnostic type: codespan-reporting's data model keyed by
/// [`FileId`].
pub type Diagnostic = codespan_reporting::diagnostic::Diagnostic<FileId>;

/// Conversion from a crate-local error into a renderable [`Diagnostic`].
///
/// Every public error type in the workspace implements this; the CLI is the
/// single place that turns the result into terminal output.
pub trait ToDiagnostic {
    /// Builds the diagnostic (message plus labeled spans) for this error.
    fn to_diagnostic(&self) -> Diagnostic;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceMap;
    use crate::span::Span;
    use codespan_reporting::diagnostic::Label;

    struct DummyError {
        span: Span,
    }

    impl ToDiagnostic for DummyError {
        fn to_diagnostic(&self) -> Diagnostic {
            Diagnostic::error()
                .with_message("dummy failed")
                .with_labels(vec![Label::primary(self.span.file, self.span.range())])
        }
    }

    #[test]
    fn to_diagnostic_carries_message_and_span() {
        let mut map = SourceMap::new();
        let file = map.add("x.scm", "(oops)");
        let span = Span::new(file, 1, 5);

        let diagnostic = DummyError { span }.to_diagnostic();

        assert_eq!(diagnostic.message, "dummy failed");
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.labels[0].file_id, file);
        assert_eq!(diagnostic.labels[0].range, 1..5);
    }
}
