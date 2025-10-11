use axum::{extract::State, Json};
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ServiceError;

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
}

impl Default for Reading {
    fn default() -> Self {
        // This will be initialized asynchronously in practice
        // For now, this is a placeholder that will panic if called synchronously
        unimplemented!("Use Reading::new() async constructor instead")
    }
}

impl Reading {
    /// Creates a new Reading instance with an initialized S3 client
    pub async fn new() -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let s3_client = S3Client::new(&config);

        Self { s3_client }
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

    /// Generates a new reading story (stub implementation)
    async fn generate_story(&self) -> Result<ReadingContents, ServiceError> {
        // TODO: Implement AI-based story generation
        unimplemented!("Story generation not yet implemented")
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
