use std::fmt::Write;

/// An owned, Tree-sitter-independent position. Lines and columns retain the
/// parser's 0-based convention until the persistence boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExtractedPosition {
    pub(crate) line: u32,
    pub(crate) column: u32,
}

/// An owned source range used by every extracted graph fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExtractedRange {
    pub(crate) start: ExtractedPosition,
    pub(crate) end: ExtractedPosition,
}

/// Facts extracted from one file before SQLite row identifiers exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtractedGraphFile {
    pub(crate) local_key: String,
    pub(crate) parse_status: &'static str,
    pub(crate) parse_error_message: Option<String>,
    pub(crate) nodes: Vec<ExtractedNode>,
    pub(crate) imports: Vec<ExtractedImport>,
    pub(crate) references: Vec<ExtractedReference>,
    pub(crate) edges: Vec<ExtractedEdge>,
}

/// A declared file-local graph node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtractedNode {
    pub(crate) local_key: String,
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) range: ExtractedRange,
    pub(crate) name_range: ExtractedRange,
    pub(crate) signature: Option<String>,
    pub(crate) documentation: Option<String>,
}

/// An import declaration observed in a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtractedImport {
    pub(crate) module: String,
    pub(crate) imported_symbol: Option<String>,
    pub(crate) alias: Option<String>,
    pub(crate) range: ExtractedRange,
}

/// An identifier occurrence, optionally linked to another extracted local key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtractedReference {
    pub(crate) name: String,
    pub(crate) target_local_key: Option<String>,
    pub(crate) range: ExtractedRange,
}

/// A graph edge whose target may be resolved locally or deferred to a later
/// cross-file resolver.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExtractedEdge {
    pub(crate) source_local_key: String,
    pub(crate) target: ExtractedEdgeTarget,
    pub(crate) edge_kind: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ExtractedEdgeTarget {
    Local(String),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the phase-two IR reserves unresolved targets for the later cross-file resolver"
        )
    )]
    Unresolved {
        description: String,
    },
}

pub(crate) fn file_local_key(relative_path: &str) -> String {
    format!("file:{relative_path}")
}

pub(crate) fn node_local_key(
    file_key: &str,
    kind: &str,
    qualified_name: &str,
    start: ExtractedPosition,
) -> String {
    let mut key = String::with_capacity(file_key.len() + kind.len() + qualified_name.len() + 32);
    let _ = write!(
        key,
        "{file_key}:node:{kind}:{qualified_name}:{}:{}",
        start.line, start.column
    );
    key
}
