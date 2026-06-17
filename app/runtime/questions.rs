use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::ApiError;

#[derive(Clone, Default)]
pub(crate) struct QuestionRegistry {
    pending: Arc<Mutex<HashMap<String, PendingQuestion>>>,
}

struct PendingQuestion {
    request: QuestionRequest,
    answer_tx: oneshot::Sender<QuestionAnswer>,
}

pub(crate) struct QuestionRegistration {
    pub(crate) answer_rx: oneshot::Receiver<QuestionAnswer>,
    _cleanup: QuestionCleanup,
}

struct QuestionCleanup {
    registry: QuestionRegistry,
    pub(crate) question_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskQuestionInput {
    pub(crate) questions: Vec<AskQuestionItemInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskQuestionItemInput {
    pub(crate) question: String,
    pub(crate) options: Option<Vec<QuestionOption>>,
    pub(crate) allow_free_text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionOption {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) description: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionRequest {
    pub(crate) id: String,
    pub(crate) tool_call_id: String,
    pub(crate) workspace_id: String,
    pub(crate) chat_id: String,
    pub(crate) questions: Vec<QuestionItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionItem {
    pub(crate) id: String,
    pub(crate) question: String,
    pub(crate) options: Vec<QuestionOption>,
    pub(crate) allow_free_text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionAnswer {
    pub(crate) answers: Vec<QuestionItemAnswer>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionItemAnswer {
    pub(crate) id: String,
    pub(crate) answer: String,
    #[serde(default)]
    pub(crate) selected_option_value: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionAnswerResponse {
    pub(crate) ok: bool,
    pub(crate) question_id: String,
}

impl QuestionRegistry {
    pub(crate) fn register(
        &self,
        request: QuestionRequest,
    ) -> Result<QuestionRegistration, ApiError> {
        let question_id = request.id.clone();
        let (answer_tx, answer_rx) = oneshot::channel();
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApiError::internal("question registry lock is poisoned"))?;

        if pending
            .insert(question_id.clone(), PendingQuestion { request, answer_tx })
            .is_some()
        {
            return Err(ApiError::internal(format!(
                "duplicate pending question id: {question_id}"
            )));
        }

        Ok(QuestionRegistration {
            answer_rx,
            _cleanup: QuestionCleanup {
                registry: self.clone(),
                question_id,
            },
        })
    }

    pub(crate) fn answer(&self, question_id: &str, answer: QuestionAnswer) -> Result<(), ApiError> {
        let question_id = question_id.trim();

        if question_id.is_empty() {
            return Err(ApiError::bad_request("question id must not be empty"));
        }

        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApiError::internal("question registry lock is poisoned"))?;
        let pending_question = pending.get(question_id).ok_or_else(|| {
            ApiError::bad_request(format!(
                "question is not waiting for an answer: {question_id}"
            ))
        })?;
        validate_question_answer(&pending_question.request, &answer)?;
        let pending_question = pending
            .remove(question_id)
            .expect("pending question should still exist after validation");

        pending_question.answer_tx.send(answer).map_err(|_| {
            ApiError::bad_request(format!(
                "question is no longer waiting for an answer: {question_id}"
            ))
        })
    }

    #[cfg(test)]
    pub(crate) fn is_pending(&self, question_id: &str) -> Result<bool, ApiError> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| ApiError::internal("question registry lock is poisoned"))?;
        Ok(pending.contains_key(question_id))
    }

    fn remove(&self, question_id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(question_id);
        }
    }
}

impl Drop for QuestionCleanup {
    fn drop(&mut self) {
        self.registry.remove(&self.question_id);
    }
}

fn validate_question_answer(
    request: &QuestionRequest,
    answer: &QuestionAnswer,
) -> Result<(), ApiError> {
    if answer.answers.len() != request.questions.len() {
        return Err(ApiError::bad_request(format!(
            "question '{}' requires answers for all {} questions",
            request.id,
            request.questions.len()
        )));
    }

    let mut answered_question_ids = HashSet::new();

    for answer in &answer.answers {
        let question_id = answer.id.trim();

        if question_id.is_empty() {
            return Err(ApiError::bad_request(
                "answer question id must not be empty",
            ));
        }

        if !answered_question_ids.insert(question_id) {
            return Err(ApiError::bad_request(format!(
                "duplicate answer for question item: {question_id}"
            )));
        }

        let question = request
            .questions
            .iter()
            .find(|question| question.id == question_id)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "answer references unknown question item: {question_id}"
                ))
            })?;

        validate_question_item_answer(question, answer)?;
    }

    for question in &request.questions {
        if !answered_question_ids.contains(question.id.as_str()) {
            return Err(ApiError::bad_request(format!(
                "missing answer for question item: {}",
                question.id
            )));
        }
    }

    Ok(())
}

fn validate_question_item_answer(
    question: &QuestionItem,
    answer: &QuestionItemAnswer,
) -> Result<(), ApiError> {
    let answer_text = answer.answer.trim();
    let selected_option_value = answer
        .selected_option_value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(selected_option_value) = selected_option_value {
        let selected_option = question
            .options
            .iter()
            .find(|option| option.value == selected_option_value)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "selected option was not found for question item '{}': {selected_option_value}",
                    question.id
                ))
            })?;

        if answer_text != selected_option.value {
            return Err(ApiError::bad_request(
                "answer must match selectedOptionValue when an option is selected",
            ));
        }

        return Ok(());
    }

    if !question.allow_free_text {
        return Err(ApiError::bad_request(format!(
            "question item '{}' requires selecting one of the provided options",
            question.id
        )));
    }

    if answer_text.is_empty() {
        return Err(ApiError::bad_request("answer must not be empty"));
    }

    Ok(())
}
