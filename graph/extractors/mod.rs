mod ast;
pub(crate) mod facts;
mod language;
mod legacy;
mod rust;
mod typescript;

use std::path::Path;

use crate::CodeGraphError;

pub(crate) use facts::ExtractedGraphFile;
pub(crate) use language::{LanguageKind, detect_language};
use rust::RustExtractor;
use typescript::TypeScriptFamilyExtractor;

pub(crate) struct ExtractionContext<'a> {
    pub(crate) language: LanguageKind,
    pub(crate) relative_path: &'a str,
    pub(crate) file_path: &'a Path,
    pub(crate) text: &'a str,
}

trait GraphExtractor {
    fn extract(&self, context: ExtractionContext<'_>)
    -> Result<ExtractedGraphFile, CodeGraphError>;
}

struct LegacyFallbackExtractor;

impl GraphExtractor for LegacyFallbackExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        legacy::extract(context)
    }
}

/// Routes each parsed file to a language-specific extractor while preserving a
/// generic fallback for languages whose syntax has not been specialized yet.
struct LanguageRegistry {
    rust: RustExtractor,
    typescript: TypeScriptFamilyExtractor,
    fallback: LegacyFallbackExtractor,
}

impl LanguageRegistry {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        match context.language {
            LanguageKind::Rust => self.rust.extract(context),
            LanguageKind::TypeScript | LanguageKind::Tsx | LanguageKind::JavaScript => {
                self.typescript.extract(context)
            }
            _ => self.fallback.extract(context),
        }
    }
}

pub(crate) fn extract_file(
    language: LanguageKind,
    relative_path: &str,
    file_path: &Path,
    text: &str,
) -> Result<ExtractedGraphFile, CodeGraphError> {
    LanguageRegistry {
        rust: RustExtractor,
        typescript: TypeScriptFamilyExtractor,
        fallback: LegacyFallbackExtractor,
    }
    .extract(ExtractionContext {
        language,
        relative_path,
        file_path,
        text,
    })
}
