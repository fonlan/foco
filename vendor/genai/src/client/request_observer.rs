use std::sync::Arc;

use reqwest::Request;

/// Read-only callback invoked after the provider adapter and reqwest have built
/// the final streaming HTTP request, immediately before it is handed to the
/// streaming decoder. The request is prepared and executed only once.
pub type PreparedRequestObserver = Arc<dyn Fn(&Request) + Send + Sync>;
