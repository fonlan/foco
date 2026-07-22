use crate::CodeGraphError;

use super::{ExtractionContext, GraphExtractor, facts::ExtractedGraphFile, legacy};

pub(crate) struct RustExtractor;

impl GraphExtractor for RustExtractor {
    fn extract(
        &self,
        context: ExtractionContext<'_>,
    ) -> Result<ExtractedGraphFile, CodeGraphError> {
        legacy::extract(context)
    }
}
