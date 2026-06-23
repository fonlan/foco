use serde::Serialize;

use crate::memory::{
    MemoryDreamChangeStatus, MemoryDreamJobStatus, MemoryDreamRunMode, MemoryDreamScope,
    MemoryDreamTriggerType, MemoryExtractionJobStatus, MemoryKind, MemoryRelationKind, MemoryScope,
    MemorySourceType, MemoryStatus,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoryDatabaseKind {
    Global,
    Workspace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemorySource<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub source_type: MemorySourceType,
    pub source_id: Option<&'a str>,
    pub title: &'a str,
    pub content: &'a str,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateMemorySource<'a> {
    pub id: &'a str,
    pub title: Option<&'a str>,
    pub content: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryFact<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub status: MemoryStatus,
    pub kind: MemoryKind,
    pub fact: &'a str,
    pub confidence: Option<f64>,
    pub pinned: bool,
    pub source_ids: &'a [&'a str],
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateMemoryFact<'a> {
    pub id: &'a str,
    pub scope: Option<MemoryScope>,
    pub chat_id: Option<&'a str>,
    pub status: Option<MemoryStatus>,
    pub kind: Option<MemoryKind>,
    pub fact: Option<&'a str>,
    pub confidence: Option<f64>,
    pub pinned: Option<bool>,
    pub is_latest: Option<bool>,
    pub expires_at: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryEdge<'a> {
    pub id: &'a str,
    pub source_fact_id: &'a str,
    pub target_fact_id: &'a str,
    pub relation: MemoryRelationKind,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryEdgeRecord {
    pub id: String,
    pub source_fact_id: String,
    pub target_fact_id: String,
    pub relation: String,
    pub metadata_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryProfile<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub profile_text: &'a str,
    pub metadata_json: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryExtractionJob<'a> {
    pub id: &'a str,
    pub scope: MemoryScope,
    pub chat_id: Option<&'a str>,
    pub status: MemoryExtractionJobStatus,
    pub model_id: Option<&'a str>,
    pub input_json: &'a str,
    pub output_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryDreamJob<'a> {
    pub id: &'a str,
    pub scope: MemoryDreamScope,
    pub workspace_id: Option<&'a str>,
    pub trigger_type: MemoryDreamTriggerType,
    pub mode: MemoryDreamRunMode,
    pub status: MemoryDreamJobStatus,
    pub model_id: Option<&'a str>,
    pub input_summary_json: &'a str,
    pub output_summary_json: Option<&'a str>,
    pub transcript_chat_id: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateMemoryDreamJob<'a> {
    pub id: &'a str,
    pub status: MemoryDreamJobStatus,
    pub output_summary_json: Option<&'a str>,
    pub transcript_chat_id: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMemoryDreamChange<'a> {
    pub id: &'a str,
    pub job_id: &'a str,
    pub operation: &'a str,
    pub target_fact_ids_json: &'a str,
    pub new_fact_id: Option<&'a str>,
    pub before_json: Option<&'a str>,
    pub after_json: Option<&'a str>,
    pub reason: &'a str,
    pub confidence: Option<f64>,
    pub risk_level: &'a str,
    pub status: MemoryDreamChangeStatus,
    pub evidence_json: &'a str,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateMemoryDreamChange<'a> {
    pub id: &'a str,
    pub status: MemoryDreamChangeStatus,
    pub after_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemorySourceRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub title: String,
    pub content: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryFactRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub status: String,
    pub kind: String,
    pub fact: String,
    pub confidence: Option<f64>,
    pub pinned: bool,
    pub is_latest: bool,
    pub expires_at: Option<String>,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryProfileRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub profile_text: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryExtractionJobRecord {
    pub id: String,
    pub scope: String,
    pub chat_id: Option<String>,
    pub status: String,
    pub model_id: Option<String>,
    pub input_json: String,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryDreamJobRecord {
    pub id: String,
    pub scope: String,
    pub workspace_id: Option<String>,
    pub trigger_type: String,
    pub mode: String,
    pub status: String,
    pub model_id: Option<String>,
    pub input_summary_json: String,
    pub output_summary_json: Option<String>,
    pub transcript_chat_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryDreamChangeRecord {
    pub id: String,
    pub job_id: String,
    pub operation: String,
    pub target_fact_ids_json: String,
    pub new_fact_id: Option<String>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub reason: String,
    pub confidence: Option<f64>,
    pub risk_level: String,
    pub status: String,
    pub evidence_json: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub applied_at: Option<String>,
}
