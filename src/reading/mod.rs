use axum::{extract::State, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    prompts,
    state::{AppState, ContentType},
};

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
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
        // Load the reading comprehension prompt configuration
        let prompt_config = prompts::get_prompt("reading_comprehension").ok_or_else(|| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Reading comprehension prompt not found".to_string(),
            )
        })?;

        // Generate new reading content using the generic generate_content method
        let contents: ReadingContents = state
            .generate_content(
                prompt_config,
                "ReadingContents",
                "A reading comprehension passage with questions",
            )
            .await
            .map_err(|e| e.into_status())?;

        // Store it for future use
        state
            .store_timed_object(&contents, ContentType::Reading)
            .await
            .map_err(|e| e.into_status())?;

        contents
    };

    Ok(Json(contents))
}
