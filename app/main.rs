use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use axum::{Json, Router, routing::get};
use foco_store::{config::load_or_create_global_config, workspace::initialize_workspace_databases};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

mod logging;

const DEFAULT_PORT: u16 = 3210;
const PORT_ENV: &str = "FOCO_PORT";

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Foco startup failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> AppResult<()> {
    let loaded_config = load_or_create_global_config()?;
    logging::init(&loaded_config.paths.logs_dir)?;

    tracing::info!(
        config = %loaded_config.config.to_redacted_log_json()?,
        "loaded global config"
    );

    let workspace_databases = initialize_workspace_databases(&loaded_config.config.workspaces)?;
    tracing::info!(
        count = workspace_databases.len(),
        "initialized workspace databases"
    );

    let addr = local_addr()?;
    let frontend_dir = frontend_dist_dir()?;
    let app = Router::new()
        .route("/api/health", get(health))
        .fallback_service(ServeDir::new(frontend_dir));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting local HTTP server");
    println!("Foco is running at http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "foco",
        status: "ok",
    })
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

fn local_addr() -> Result<SocketAddr, String> {
    let port = match env::var(PORT_ENV) {
        Ok(value) => parse_port(&value)?,
        Err(env::VarError::NotPresent) => DEFAULT_PORT,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{PORT_ENV} must be valid Unicode"));
        }
    };

    Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

fn parse_port(value: &str) -> Result<u16, String> {
    let port = value
        .parse::<u16>()
        .map_err(|_| format!("{PORT_ENV} must be a number from 1 to 65535"))?;

    if port == 0 {
        return Err(format!("{PORT_ENV} must be a number from 1 to 65535"));
    }

    Ok(port)
}

fn frontend_dist_dir() -> Result<PathBuf, String> {
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_dir = app_dir
        .parent()
        .ok_or_else(|| "app crate must live inside the Foco repository".to_string())?;
    let dist_dir = repo_dir.join("web").join("dist");
    let index_file = dist_dir.join("index.html");

    if !index_file.is_file() {
        return Err(format!(
            "frontend build missing at {}. Run `npm run build -w web` before starting the backend.",
            index_file.display()
        ));
    }

    Ok(dist_dir)
}
