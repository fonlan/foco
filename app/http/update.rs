use axum::{Json, extract::State};
use serde::Deserialize;

use crate::{ApiError, AppState, config_snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSettingsRequest {
    auto_check_enabled: bool,
}

pub(crate) async fn update_status(
    State(state): State<AppState>,
) -> Result<Json<crate::update_runtime::UpdateStatusSummary>, ApiError> {
    let config = config_snapshot(&state)?;
    crate::update_runtime::update_status_summary(&state, &config).map(Json)
}

pub(crate) async fn check_update(
    State(state): State<AppState>,
) -> Result<Json<crate::update_runtime::UpdateStatusSummary>, ApiError> {
    crate::update_runtime::check_for_updates(&state, true)
        .await
        .map(Json)
}

pub(crate) async fn save_update_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateSettingsRequest>,
) -> Result<Json<crate::update_runtime::UpdateStatusSummary>, ApiError> {
    crate::update_runtime::save_update_settings(&state, request.auto_check_enabled)
        .await
        .map(Json)
}

pub(crate) async fn install_update(
    State(state): State<AppState>,
) -> Result<Json<crate::update_runtime::UpdateStatusSummary>, ApiError> {
    crate::update_runtime::install_update(&state)
        .await
        .map(Json)
}
