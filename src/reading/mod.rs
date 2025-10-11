use async_openai::{
    types::{ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client as OpenAIClient,
};
use axum::{extract::State, Json};
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{prompts, ServiceError};

const S3_BUCKET_NAME: &str = "thinkaroo-reading-stories";
const MAX_STORIES_PER_HOUR: usize = 16;

#[derive(Serialize, Deserialize, Clone)]
pub struct ReadingContents {
    pub title: String,
    pub story: String,
    pub questions: Vec<String>,
}

#[derive(Clone)]
pub struct Reading {
    s3_client: S3Client,
    openai_client: OpenAIClient<async_openai::config::OpenAIConfig>,
}

impl Default for Reading {
    fn default() -> Self {
        // This will be initialized asynchronously in practice
        // For now, this is a placeholder that will panic if called synchronously
        unimplemented!("Use Reading::new() async constructor instead")
    }
}

impl Reading {
    /// Creates a new Reading instance with initialized S3 and OpenAI clients
    pub async fn new() -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let s3_client = S3Client::new(&config);

        // OpenAI client will use OPENAI_API_KEY environment variable
        let openai_client = OpenAIClient::new();

        Self {
            s3_client,
            openai_client,
        }
    }

    /// Gets a reading story from S3, generating a new one if needed
    pub async fn get_from_s3(&self) -> Result<ReadingContents, ServiceError> {
        let now = Utc::now();
        let folder_path = Self::format_s3_folder_path(&now);

        // List all stories in the current hour's folder
        let list_output = self.s3_client
            .list_objects_v2()
            .bucket(S3_BUCKET_NAME)
            .prefix(&folder_path)
            .send()
            .await?;

        let object_count = list_output.contents().len();

        if object_count >= MAX_STORIES_PER_HOUR {
            // Pick a random story from existing ones
            let random_index = rand::random::<usize>() % object_count;
            let object = &list_output.contents()[random_index];
            let key = object.key().ok_or_else(|| {
                ServiceError::S3Error("Object key is missing".to_string())
            })?;

            // Fetch and parse the story
            let get_output = self.s3_client
                .get_object()
                .bucket(S3_BUCKET_NAME)
                .key(key)
                .send()
                .await?;

            let body_bytes = get_output.body.collect().await?.into_bytes();
            let contents: ReadingContents = serde_json::from_slice(body_bytes.as_ref())?;

            Ok(contents)
        } else {
            // Generate a new story
            let contents = self.generate_story().await?;

            // Store it in S3 with a random GUID
            let guid = Uuid::new_v4();
            let key = format!("{}{}.json", folder_path, guid);

            let json_data = serde_json::to_string(&contents)?;

            self.s3_client
                .put_object()
                .bucket(S3_BUCKET_NAME)
                .key(&key)
                .body(json_data.into_bytes().into())
                .content_type("application/json")
                .send()
                .await?;

            Ok(contents)
        }
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

    /// Formats the S3 folder path as YYYY-MM-DD-HH/
    fn format_s3_folder_path(dt: &DateTime<Utc>) -> String {
        format!("{}/", dt.format("%Y-%m-%d-%H"))
    }}


pub async fn reading_contents(
    State(reading): State<Reading>,
) -> Result<Json<ReadingContents>, (axum::http::StatusCode, String)> {
    let contents = reading
        .get_from_s3()
        .await
        .map_err(|e| e.into_status())?;

    Ok(Json(contents))
}
