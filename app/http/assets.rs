use std::path::PathBuf;

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../web/dist"]
struct WebAssets;

pub(crate) async fn static_asset(uri: axum::http::Uri) -> Response {
    let request_path = uri.path().trim_start_matches('/');
    if request_path.starts_with("api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("API route not found"))
            .expect("static asset response is valid");
    }

    let asset_path = if request_path.is_empty() {
        "index.html"
    } else {
        request_path
    };
    let asset = WebAssets::get(asset_path).or_else(|| {
        if asset_path.rsplit_once('.').is_none() {
            WebAssets::get("index.html")
        } else {
            None
        }
    });

    match asset {
        Some(asset) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, asset.metadata.mimetype())
            .body(Body::from(asset.data.into_owned()))
            .expect("static asset response is valid"),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("frontend asset not found"))
            .expect("static asset response is valid"),
    }
}

pub(crate) fn verify_frontend_assets() -> Result<(), String> {
    if WebAssets::get("index.html").is_some() {
        return Ok(());
    }

    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_dir = app_dir
        .parent()
        .ok_or_else(|| "app crate must live inside the Foco repository".to_string())?;
    let index_file = repo_dir.join("web").join("dist").join("index.html");

    Err(format!(
        "frontend build missing at {}. Run `npm run build -w web` before starting the backend or release build.",
        index_file.display()
    ))
}
