use crate::CodeGraphError;

use super::{ExtractionContext, GraphExtractor, facts::ExtractedGraphFile, legacy};

pub(crate) struct TypeScriptFamilyExtractor;

impl GraphExtractor for TypeScriptFamilyExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        legacy::extract(context)
    }
}
