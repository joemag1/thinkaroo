use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::state::{AppState, ContentType};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReadingContents {
    pub title: String,
    pub story: String,
    pub questions: Vec<String>,
}

pub async fn reading_contents(
    State(state): State<AppState>,
) -> Result<Json<ReadingContents>, (axum::http::StatusCode, String)> {
    // Try to get an existing cached story
    let contents = if let Some(contents) = state
        .get_timed_object(ContentType::Reading)
        .await
        .map_err(|e| e.into_status())?
    {
        contents
    } else {
        // No cached story available, generate a new one
        let contents = state.generate_story().await.map_err(|e| e.into_status())?;

        // Store it for future use
        state
            .store_timed_object(&contents, ContentType::Reading)
            .await
            .map_err(|e| e.into_status())?;

        contents
    };

    Ok(Json(contents))
}
