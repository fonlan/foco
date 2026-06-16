use serde::Serialize;

use crate::memory::{
    MemoryExtractionJobStatus, MemoryKind, MemoryRelationKind, MemoryScope, MemorySourceType,
    MemoryStatus,
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
