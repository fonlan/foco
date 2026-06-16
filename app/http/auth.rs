use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
    ApiError, AppState, auth_cookie, config_snapshot, expired_auth_cookie,
    request_has_valid_auth_cookie, verify_password, web_auth_enabled,
};

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "foco",
        status: "ok",
    })
}

pub(crate) async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if auth_route_is_public(request.uri().path()) {
        return next.run(request).await;
    }

    let config = match config_snapshot(&state) {
        Ok(config) => config,
        Err(error) => return error.into_response(),
    };

    if !web_auth_enabled(&config) || request_has_valid_auth_cookie(request.headers(), &config) {
        return next.run(request).await;
    }

    ApiError::unauthorized("authentication required").into_response()
}

fn auth_route_is_public(path: &str) -> bool {
    path == "/api/health"
        || path == "/api/auth/status"
        || path == "/api/auth/login"
        || path == "/api/auth/logout"
        || path == "/api/native/browser-probe.svg"
        || !path.starts_with("/api/")
}

pub(crate) async fn auth_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthStatusResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let enabled = web_auth_enabled(&config);
    let authenticated = !enabled || request_has_valid_auth_cookie(&headers, &config);

    Ok(Json(AuthStatusResponse {
        enabled,
        authenticated,
    }))
}

pub(crate) async fn auth_login(
    State(state): State<AppState>,
    Json(request): Json<AuthLoginRequest>,
) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;
    let Some(password_hash) = config.app.web_server.password_hash.as_deref() else {
        return Err(ApiError::bad_request("web authentication is not enabled"));
    };

    if !verify_password(&request.password, password_hash) {
        return Err(ApiError::unauthorized("invalid password"));
    }

    let cookie = auth_cookie(password_hash);
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(AuthStatusResponse {
            enabled: true,
            authenticated: true,
        }),
    )
        .into_response())
}

pub(crate) async fn auth_logout(State(state): State<AppState>) -> Result<Response, ApiError> {
    let config = config_snapshot(&state)?;

    Ok((
        [(header::SET_COOKIE, expired_auth_cookie())],
        Json(AuthStatusResponse {
            enabled: web_auth_enabled(&config),
            authenticated: false,
        }),
    )
        .into_response())
}

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

#[derive(Serialize)]
pub(crate) struct AuthStatusResponse {
    enabled: bool,
    authenticated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthLoginRequest {
    password: String,
}
