use serde::{Deserialize, Serialize};

/// Ordered exact-text patch for Project Spec Markdown.
///
/// Shared by Agent `update_spec` and automatic workspace-spec update jobs.
/// Callers own persistence; this module only validates and applies edits in memory.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecTextEdit {
    pub old_text: String,
    pub new_text: String,
}

/// Pure validation/application failures for ordered exact-text Spec patches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecPatchError {
    EmptyEdits,
    EmptyOldText { index: usize },
    NotFound { index: usize },
    AmbiguousMatch { index: usize },
    NoChange,
}

impl SpecPatchError {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyEdits => "edits must contain at least one edit".to_string(),
            Self::EmptyOldText { index } => {
                format!("edits[{index}].oldText must not be empty")
            }
            Self::NotFound { index } => {
                format!("edits[{index}].oldText was not found in the current Project Spec")
            }
            Self::AmbiguousMatch { index } => {
                format!("edits[{index}].oldText matched more than once in the current Project Spec")
            }
            Self::NoChange => "edits must change the Project Spec content".to_string(),
        }
    }
}

/// Apply ordered exact-text edits to Spec Markdown.
///
/// Rules:
/// - `edits` must be non-empty
/// - each `oldText` must be non-empty
/// - each `oldText` must match the current content exactly once
/// - edits apply in declaration order
/// - the final content must differ from the input
///
/// On any error, no partial result is returned (callers must not write).
pub fn apply_spec_text_edits(
    current_content: &str,
    edits: &[SpecTextEdit],
) -> Result<String, SpecPatchError> {
    if edits.is_empty() {
        return Err(SpecPatchError::EmptyEdits);
    }

    let mut content = current_content.to_string();
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(SpecPatchError::EmptyOldText { index });
        }

        let mut matches = content.char_indices().filter_map(|(match_start, _)| {
            content[match_start..]
                .starts_with(&edit.old_text)
                .then_some(match_start)
        });
        let Some(match_start) = matches.next() else {
            return Err(SpecPatchError::NotFound { index });
        };
        if matches.next().is_some() {
            return Err(SpecPatchError::AmbiguousMatch { index });
        }

        let match_end = match_start + edit.old_text.len();
        content.replace_range(match_start..match_end, &edit.new_text);
    }

    if content == current_content {
        return Err(SpecPatchError::NoChange);
    }

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_spec_text_edits_applies_ordered_unique_matches() {
        let result = apply_spec_text_edits(
            "# Spec\n\nAlpha\nBeta",
            &[
                SpecTextEdit {
                    old_text: "Alpha".to_string(),
                    new_text: "Gamma\nDelta".to_string(),
                },
                SpecTextEdit {
                    old_text: "Delta\nBeta".to_string(),
                    new_text: "Delta\nEpsilon".to_string(),
                },
            ],
        )
        .expect("ordered edits");

        assert_eq!(result, "# Spec\n\nGamma\nDelta\nEpsilon");
    }

    #[test]
    fn apply_spec_text_edits_rejects_empty_ambiguous_and_noop() {
        assert_eq!(
            apply_spec_text_edits("alpha", &[]),
            Err(SpecPatchError::EmptyEdits)
        );
        assert_eq!(
            apply_spec_text_edits(
                "alpha",
                &[SpecTextEdit {
                    old_text: String::new(),
                    new_text: "x".to_string(),
                }]
            ),
            Err(SpecPatchError::EmptyOldText { index: 0 })
        );
        assert_eq!(
            apply_spec_text_edits(
                "alpha alpha",
                &[SpecTextEdit {
                    old_text: "alpha".to_string(),
                    new_text: "beta".to_string(),
                }]
            ),
            Err(SpecPatchError::AmbiguousMatch { index: 0 })
        );
        assert_eq!(
            apply_spec_text_edits(
                "alpha",
                &[SpecTextEdit {
                    old_text: "missing".to_string(),
                    new_text: "beta".to_string(),
                }]
            ),
            Err(SpecPatchError::NotFound { index: 0 })
        );
        assert_eq!(
            apply_spec_text_edits(
                "alpha",
                &[SpecTextEdit {
                    old_text: "alpha".to_string(),
                    new_text: "alpha".to_string(),
                }]
            ),
            Err(SpecPatchError::NoChange)
        );
    }

    #[test]
    fn apply_spec_text_edits_deletes_matched_text() {
        let result = apply_spec_text_edits(
            "# Spec\n\nKeep\nRemove me",
            &[SpecTextEdit {
                old_text: "\nRemove me".to_string(),
                new_text: String::new(),
            }],
        )
        .expect("deletion");
        assert_eq!(result, "# Spec\n\nKeep");
    }

    #[test]
    fn spec_text_edit_rejects_unknown_fields() {
        let error = serde_json::from_value::<SpecTextEdit>(serde_json::json!({
            "oldText": "a",
            "newText": "b",
            "extra": true
        }))
        .expect_err("unknown fields");
        assert!(error.to_string().contains("unknown field"));
    }
}
