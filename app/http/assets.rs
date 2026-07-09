use std::path::PathBuf;

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../web/dist"]
pub(crate) struct WebAssets;

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
    let served_asset_path = WebAssets::get(asset_path)
        .map(|asset| (asset_path, asset))
        .or_else(|| {
            if asset_path.rsplit_once('.').is_none() {
                WebAssets::get("index.html").map(|asset| ("index.html", asset))
            } else {
                None
            }
        });

    match served_asset_path {
        Some((served_path, asset)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, asset.metadata.mimetype())
            .header(header::CACHE_CONTROL, cache_control_for_asset(served_path))
            .body(Body::from(asset.data.into_owned()))
            .expect("static asset response is valid"),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("frontend asset not found"))
            .expect("static asset response is valid"),
    }
}

fn cache_control_for_asset(asset_path: &str) -> &'static str {
    if asset_path == "index.html" {
        "no-cache, no-store, must-revalidate"
    } else {
        "public, max-age=31536000, immutable"
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

#[cfg(test)]
mod tests {
    use super::cache_control_for_asset;

    #[test]
    fn index_html_is_not_cached_across_updates() {
        assert_eq!(
            cache_control_for_asset("index.html"),
            "no-cache, no-store, must-revalidate"
        );
        assert_eq!(
            cache_control_for_asset("assets/index-CtVZAx5V.js"),
            "public, max-age=31536000, immutable"
        );
    }
}
