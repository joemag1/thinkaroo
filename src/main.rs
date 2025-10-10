use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{error, info};

async fn health() -> &'static str {
    "OK"
}

async fn stream_file(file_path: &str) -> Result<Response, (StatusCode, String)> {
    let file = File::open(file_path).await.map_err(|e| {
        error!("Failed to open file {}: {}", file_path, e);
        (StatusCode::NOT_FOUND, "File not found".to_string())
    })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(body)
        .map_err(|e| {
            error!("Failed to build response for {}: {}", file_path, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?;

    Ok(response)
}

async fn home() -> Result<Response, (StatusCode, String)> {
    stream_file("static/home.html").await
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/home", get(home));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    info!("Server listening on http://0.0.0.0:8080");

    axum::serve(listener, app).await.unwrap();
}
