use async_openai::{
    types::{ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{prompts, state::{AppState, ContentType}, ServiceError};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReadingContents {
    pub title: String,
    pub story: String,
    pub questions: Vec<String>,
}

impl AppState {
    /// Gets a reading story from S3, generating a new one if needed
    pub async fn get_reading_contents(&self) -> Result<ReadingContents, ServiceError> {
        // Try to get an existing cached story
        if let Some(contents) = self.get_timed_object(ContentType::Reading).await? {
            return Ok(contents);
        }

        // No cached story available, generate a new one
        let contents = self.generate_story().await?;

        // Store it for future use
        self.store_timed_object(&contents, ContentType::Reading).await?;

        Ok(contents)
    }

    /// Generates a new reading story using OpenAI
    async fn generate_story(&self) -> Result<ReadingContents, ServiceError> {
        // Get the reading comprehension prompt
        let prompt_config = prompts::get_prompt("reading_comprehension")
            .ok_or_else(|| ServiceError::ConfigError("Reading comprehension prompt not found".to_string()))?;

        // Create chat completion request with system context and user prompt
        let request = CreateChatCompletionRequestArgs::default()
            .model(&prompt_config.model)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(prompt_config.system_context.clone())
                    .build()
                    .map_err(|e| ServiceError::OpenAIError(format!("Failed to build system message: {}", e)))?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt_config.prompt.text.clone())
                    .build()
                    .map_err(|e| ServiceError::OpenAIError(format!("Failed to build user message: {}", e)))?
                    .into(),
            ])
            .build()
            .map_err(|e| ServiceError::OpenAIError(format!("Failed to build request: {}", e)))?;

        // Call OpenAI API
        let response = self
            .openai_client
            .chat()
            .create(request)
            .await
            .map_err(|e| ServiceError::OpenAIError(format!("OpenAI API call failed: {}", e)))?;

        // Extract the content from the response
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| ServiceError::OpenAIError("No content in OpenAI response".to_string()))?;

        // Parse the JSON response
        let reading_contents: ReadingContents = serde_json::from_str(content)?;

        Ok(reading_contents)
    }
}


pub async fn reading_contents(
    State(state): State<AppState>,
) -> Result<Json<ReadingContents>, (axum::http::StatusCode, String)> {
    let contents = state
        .get_reading_contents()
        .await
        .map_err(|e| e.into_status())?;

    Ok(Json(contents))
}
