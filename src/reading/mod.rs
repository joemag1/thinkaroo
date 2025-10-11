use std::time::Duration;
use axum::Json;
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
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

/// Gets a reading story from S3, generating a new one if needed
pub async fn get_from_s3(s3_client: &S3Client) -> Result<ReadingContents, ServiceError> {
    let now = Utc::now();
    let folder_path = format_s3_folder_path(&now);

    // List all stories in the current hour's folder
    let list_output = s3_client
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
        let get_output = s3_client
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
        let contents = generate_story().await?;

        // Store it in S3 with a random GUID
        let guid = Uuid::new_v4();
        let key = format!("{}{}.json", folder_path, guid);

        let json_data = serde_json::to_string(&contents)?;

        s3_client
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

/// Formats the S3 folder path as YYYY-MM-DD-HH/
fn format_s3_folder_path(dt: &DateTime<Utc>) -> String {
    format!("{}/", dt.format("%Y-%m-%d-%H"))
}

/// Generates a new reading story (stub implementation)
async fn generate_story() -> Result<ReadingContents, ServiceError> {
    // TODO: Implement AI-based story generation
    unimplemented!("Story generation not yet implemented")
}

pub async fn reading_contents() -> Json<ReadingContents> {
    // todo: remove once we load actual contents.
    sleep(Duration::from_secs(5)).await;

    // Placeholder implementation - will be replaced with AI generation later
    let contents = ReadingContents {
        title: "A story to behold".into(),
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
