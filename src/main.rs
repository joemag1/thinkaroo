use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{error, info};

#[derive(Serialize)]
struct ReadingContents {
    story: String,
    questions: Vec<String>,
}

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

async fn reading() -> Result<Response, (StatusCode, String)> {
    stream_file("static/reading.html").await
}

async fn reading_contents() -> Json<ReadingContents> {
    // Placeholder implementation - will be replaced with AI generation later
    let contents = ReadingContents {
        story: "Once upon a time, in a small village nestled between rolling hills, there lived a curious young girl named Maya. Every day after school, she would explore the forests near her home, discovering new plants and animals. One afternoon, Maya stumbled upon a hidden grove where butterflies of every color danced among wildflowers. She sat quietly, watching them for hours, learning their patterns and behaviors. From that day forward, Maya knew she wanted to become a scientist who studied nature.".to_string(),
        questions: vec![
            "What is the main character's name?".to_string(),
            "Where does Maya like to spend her time after school?".to_string(),
            "What did Maya discover in the forest?".to_string(),
            "What did Maya decide she wanted to become?".to_string(),
            "How would you describe Maya's personality based on the story?".to_string(),
        ],
    };

    Json(contents)
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
        .route("/home", get(home))
        .route("/", get(home))
        .route("/reading", get(reading))
        .route("/reading_contents", get(reading_contents));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    info!("Server listening on http://0.0.0.0:8080");

    axum::serve(listener, app).await.unwrap();
}
