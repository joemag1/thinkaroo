use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
};
use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{prompts, reading::ReadingContents, ServiceError};

/// S3 bucket name for storing timed objects
const S3_BUCKET_NAME: &str = "thinkaroo-reading-stories";

/// Maximum number of objects to store per hour before reusing existing ones
const MAX_OBJECTS_PER_HOUR: usize = 16;

/// Content type enum for organizing S3 objects by type
#[derive(Debug, Clone, Copy)]
pub enum ContentType {
    Reading,
}

impl ContentType {
    /// Returns the string prefix for this content type
    fn prefix(&self) -> &'static str {
        match self {
            ContentType::Reading => "reading",
        }
    }
}

/// Application-wide state that can be shared across all routes
/// Contains AWS clients (S3, DynamoDB, Bedrock) and OpenAI client
#[derive(Clone)]
pub struct AppState {
    /// S3 client for object storage operations
    pub s3_client: S3Client,

    /// DynamoDB client for database operations
    pub dynamodb_client: DynamoDbClient,

    /// Bedrock client for AWS AI model interactions
    pub bedrock_client: BedrockClient,

    /// OpenAI client for OpenAI API interactions
    pub openai_client: OpenAIClient<async_openai::config::OpenAIConfig>,
}

impl AppState {
    /// Creates a new AppState with all clients initialized
    ///
    /// This function loads AWS credentials from the default credential chain
    /// and creates instances of all required service clients.
    ///
    /// # Example
    /// ```no_run
    /// use thinkaroo::state::AppState;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let state = AppState::new().await;
    ///     // Use state with your Axum router
    /// }
    /// ```
    pub async fn new() -> Self {
        // Load AWS configuration from environment
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

        // Initialize AWS clients
        let s3_client = S3Client::new(&config);
        let dynamodb_client = DynamoDbClient::new(&config);
        let bedrock_client = BedrockClient::new(&config);

        // Initialize OpenAI client (uses OPENAI_API_KEY environment variable)
        let openai_client = OpenAIClient::new();

        Self {
            s3_client,
            dynamodb_client,
            bedrock_client,
            openai_client,
        }
    }

    /// Gets a random timed object from S3 for the current hour
    ///
    /// This method implements a time-based caching strategy where objects are organized
    /// by content type and hourly time slots. Returns `None` if the current hour's folder
    /// has fewer than MAX_OBJECTS_PER_HOUR objects, indicating that more content should
    /// be generated. Otherwise, returns a random existing object from the current hour.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize from S3. Must implement Deserialize.
    ///
    /// # Arguments
    /// * `content_type` - The type of content being requested (e.g., Reading)
    ///
    /// # Returns
    /// * `Ok(Some(T))` - A random object from the current hour's cache
    /// * `Ok(None)` - No cached object available (generate new content)
    /// * `Err(ServiceError)` - If S3 operations fail
    ///
    /// # Example
    /// ```no_run
    /// use thinkaroo::state::{AppState, ContentType};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct MyContent {
    ///     data: String,
    /// }
    ///
    /// # async fn example(state: AppState) -> Result<(), thinkaroo::ServiceError> {
    /// let content: Option<MyContent> = state
    ///     .get_timed_object(ContentType::Reading)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_timed_object<T>(
        &self,
        content_type: ContentType,
    ) -> Result<Option<T>, ServiceError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let now = Utc::now();
        let folder_path = Self::format_timed_prefix(&now, content_type);

        // List all objects in the current hour's folder for this content type
        let list_output = self
            .s3_client
            .list_objects_v2()
            .bucket(S3_BUCKET_NAME)
            .prefix(&folder_path)
            .send()
            .await?;

        let object_count = list_output.contents().len();

        if object_count >= MAX_OBJECTS_PER_HOUR {
            // Pick a random object from existing ones
            let random_index = rand::random::<usize>() % object_count;
            let object = &list_output.contents()[random_index];
            let key = object
                .key()
                .ok_or_else(|| ServiceError::S3Error("Object key is missing".to_string()))?;

            // Fetch and parse the object
            let get_output = self
                .s3_client
                .get_object()
                .bucket(S3_BUCKET_NAME)
                .key(key)
                .send()
                .await?;

            let body_bytes = get_output.body.collect().await?.into_bytes();
            let contents: T = serde_json::from_slice(body_bytes.as_ref())?;

            Ok(Some(contents))
        } else {
            // Need to generate new content
            Ok(None)
        }
    }

    /// Stores an object in S3 with a time-based key
    ///
    /// Objects are stored with keys in the format:
    /// `{content_type_prefix}/{YYYY-MM-DD-HH}/{guid}.json`
    ///
    /// # Arguments
    /// * `object` - The object to store (must be serializable)
    /// * `content_type` - The type of content being stored
    ///
    /// # Returns
    /// * `Ok(())` - If the object was successfully stored
    /// * `Err(ServiceError)` - If serialization or S3 operations fail
    pub async fn store_timed_object<T>(
        &self,
        object: &T,
        content_type: ContentType,
    ) -> Result<(), ServiceError>
    where
        T: Serialize,
    {
        let now = Utc::now();
        let folder_path = Self::format_timed_prefix(&now, content_type);
        let guid = Uuid::new_v4();
        let key = format!("{}{}.json", folder_path, guid);

        let json_data = serde_json::to_string(object)?;

        self.s3_client
            .put_object()
            .bucket(S3_BUCKET_NAME)
            .key(&key)
            .body(json_data.into_bytes().into())
            .content_type("application/json")
            .send()
            .await?;

        Ok(())
    }

    /// Formats the S3 prefix with content type and timestamp
    ///
    /// Format: `{content_type_prefix}/{YYYY-MM-DD-HH}/`
    ///
    /// # Arguments
    /// * `dt` - The datetime to format
    /// * `content_type` - The content type for the prefix
    ///
    /// # Returns
    /// A formatted string like "reading/2025-10-11-14/"
    fn format_timed_prefix(dt: &DateTime<Utc>, content_type: ContentType) -> String {
        format!("{}/{}/", content_type.prefix(), dt.format("%Y-%m-%d-%H"))
    }

    /// Generates a new reading story using OpenAI
    pub async fn generate_story(&self) -> Result<ReadingContents, ServiceError> {
        // Get the reading comprehension prompt
        let prompt_config = prompts::get_prompt("reading_comprehension").ok_or_else(|| {
            ServiceError::ConfigError("Reading comprehension prompt not found".to_string())
        })?;

        // Create chat completion request with system context and user prompt
        let request = CreateChatCompletionRequestArgs::default()
            .model(&prompt_config.model)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(prompt_config.system_context.clone())
                    .build()
                    .map_err(|e| {
                        ServiceError::OpenAIError(format!("Failed to build system message: {}", e))
                    })?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt_config.prompt.text.clone())
                    .build()
                    .map_err(|e| {
                        ServiceError::OpenAIError(format!("Failed to build user message: {}", e))
                    })?
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
