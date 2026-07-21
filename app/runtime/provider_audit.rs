use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use foco_providers::{
    NeutralChatStream, ProviderAuditRequestDump, ProviderFinalResponseDump,
    ProviderRequestDumpObserver, ProviderRequestFailure,
};
use serde::Serialize;

use crate::{ApiError, open_workspace_database};

#[derive(Clone)]
pub(crate) struct ProviderAuditCapture {
    workspace_path: PathBuf,
    request_id: String,
    save_details: bool,
}

impl ProviderAuditCapture {
    pub(crate) fn new(
        workspace_path: &Path,
        request_id: impl Into<String>,
        save_details: bool,
    ) -> Self {
        Self {
            workspace_path: workspace_path.to_path_buf(),
            request_id: request_id.into(),
            save_details,
        }
    }

    pub(crate) fn observer(&self) -> Option<ProviderRequestDumpObserver> {
        self.save_details.then(|| {
            let capture = self.clone();
            Arc::new(move |dump: &ProviderAuditRequestDump| {
                if let Err(error) = capture.persist_request_dump(dump) {
                    tracing::warn!(
                        request_id = %capture.request_id,
                        error = %error.message,
                        "failed to persist captured provider request dump"
                    );
                }
            }) as ProviderRequestDumpObserver
        })
    }

    pub(crate) fn persist_request_failure(
        &self,
        failure: &ProviderRequestFailure,
    ) -> Result<(), ApiError> {
        if let Some(dump) = failure.request_dump.as_ref() {
            self.persist_request_dump(dump)?;
        }
        Ok(())
    }

    pub(crate) fn request_json(
        &self,
        dump: Option<&ProviderAuditRequestDump>,
    ) -> Result<Option<String>, ApiError> {
        self.serialize_detail(dump)
    }

    pub(crate) fn captured_request_json(&self) -> Result<Option<String>, ApiError> {
        if !self.save_details {
            return Ok(None);
        }
        let database = open_workspace_database(&self.workspace_path)?;
        let request = database
            .llm_request(&self.request_id)
            .map_err(ApiError::from_workspace_error)?;
        Ok(request.and_then(|request| request.request_body_json))
    }

    pub(crate) fn response_json(
        &self,
        dump: Option<&ProviderFinalResponseDump>,
    ) -> Result<Option<String>, ApiError> {
        if !self.save_details {
            return Ok(None);
        }
        dump.map(|dump| {
            dump.audit_json().map_err(|source| {
                ApiError::internal(format!(
                    "failed to serialize provider audit response detail: {source}"
                ))
            })
        })
        .transpose()
    }

    pub(crate) fn failed_response_json(
        &self,
        message: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
    ) -> Result<Option<String>, ApiError> {
        let dump = ProviderFinalResponseDump::failed(message, status_code, partial);
        self.response_json(Some(&dump))
    }

    pub(crate) fn failed_stream_response_json(
        &self,
        stream: &NeutralChatStream,
        message: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
    ) -> Result<Option<String>, ApiError> {
        let dump = stream.failed_final_response_dump(message, status_code, partial);
        self.response_json(dump.as_ref())
    }

    pub(crate) fn interrupted_stream_response_json(
        &self,
        stream: &NeutralChatStream,
        message: impl Into<String>,
    ) -> Result<Option<String>, ApiError> {
        let dump = stream.interrupted_final_response_dump(message);
        self.response_json(dump.as_ref())
    }

    fn persist_request_dump(&self, dump: &ProviderAuditRequestDump) -> Result<(), ApiError> {
        let Some(request_json) = self.request_json(Some(dump))? else {
            return Ok(());
        };
        let mut database = open_workspace_database(&self.workspace_path)?;
        database
            .update_llm_request_body(&self.request_id, Some(&request_json))
            .map_err(ApiError::from_workspace_error)
    }

    fn serialize_detail<T: Serialize>(
        &self,
        detail: Option<&T>,
    ) -> Result<Option<String>, ApiError> {
        if !self.save_details {
            return Ok(None);
        }
        detail
            .map(|detail| {
                serde_json::to_string(detail).map_err(|source| {
                    ApiError::internal(format!(
                        "failed to serialize provider audit detail: {source}"
                    ))
                })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_capture_does_not_create_observer_or_details() {
        let capture = ProviderAuditCapture::new(Path::new("."), "llm-test", false);
        assert!(capture.observer().is_none());
        assert!(
            capture
                .failed_response_json("cancelled", None, true)
                .expect("failed response")
                .is_none()
        );
    }

    #[test]
    fn failed_response_uses_versioned_envelope() {
        let capture = ProviderAuditCapture::new(Path::new("."), "llm-test", true);
        let value = capture
            .failed_response_json("timed out", Some(504), true)
            .expect("failed response")
            .expect("detail");
        let value: serde_json::Value = serde_json::from_str(&value).expect("json");
        assert_eq!(value["format"], "provider_final_response_v1");
        assert_eq!(value["state"], "failed");
        assert_eq!(value["statusCode"], 504);
        assert_eq!(value["partial"], true);
    }
}
