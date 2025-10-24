use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
    Client as OpenAIClient,
};
use schemars::schema_for;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{keyvalue::KeyValueStore, prompts::PromptConfig, storage::ObjectStore, ServiceError};

/// Maximum number of objects to store per hour before reusing existing ones
const MAX_OBJECTS_PER_HOUR: usize = 16;

/// Content type enum for organizing storage objects by type
#[derive(Debug, Clone, Copy)]
pub enum ContentType {
    Reading,
}

impl ContentType {
    /// Returns the string prefix for this content type
    pub fn prefix(&self) -> &'static str {
        match self {
            ContentType::Reading => "reading",
        }
    }
}

/// Application-wide state that can be shared across all routes
/// Generic over the storage implementations to allow different backends
#[derive(Clone)]
pub struct AppState<S: ObjectStore, K: KeyValueStore> {
    /// Object storage backend for blob storage operations
    pub object_store: S,

    /// Key-value store backend for database operations
    pub kv_store: K,

    /// OpenAI client for OpenAI API interactions
    pub openai_client: OpenAIClient<async_openai::config::OpenAIConfig>,
}

impl<S: ObjectStore, K: KeyValueStore> AppState<S, K> {
    /// Creates a new AppState with all clients initialized
    ///
    /// # Arguments
    /// * `object_store` - The object storage implementation to use
    /// * `kv_store` - The key-value store implementation to use
    ///
    /// # Example
    /// ```no_run
    /// use thinkaroo::state::AppState;
    /// use thinkaroo::storage::S3ObjectStore;
    /// use thinkaroo::keyvalue::DynamoKeyValueStore;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    ///     let object_store = S3ObjectStore::new(aws_sdk_s3::Client::new(&config));
    ///     let kv_store = DynamoKeyValueStore::new(aws_sdk_dynamodb::Client::new(&config));
    ///     let state = AppState::new(object_store, kv_store).await;
    ///     // Use state with your Axum router
    /// }
    /// ```
    pub async fn new(object_store: S, kv_store: K) -> Self {
        // Initialize OpenAI client (uses OPENAI_API_KEY environment variable)
        let openai_client = OpenAIClient::new();

        Self {
            object_store,
            kv_store,
            openai_client,
        }
    }

    /// Gets a random timed object from storage for the current hour
    ///
    /// This method implements a time-based caching strategy where objects are organized
    /// by content type and hourly time slots. Returns `None` if the current hour's folder
    /// has fewer than MAX_OBJECTS_PER_HOUR objects, indicating that more content should
    /// be generated. Otherwise, returns a random existing object from the current hour.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize from storage. Must implement Deserialize.
    ///
    /// # Arguments
    /// * `content_type` - The type of content being requested (e.g., Reading)
    ///
    /// # Returns
    /// * `Ok(Some(T))` - A random object from the current hour's cache
    /// * `Ok(None)` - No cached object available (generate new content)
    /// * `Err(ServiceError)` - If storage operations fail
    ///
    /// # Example
    /// ```no_run
    /// use thinkaroo::state::{AppState, ContentType};
    /// use thinkaroo::storage::S3ObjectStore;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct MyContent {
    ///     data: String,
    /// }
    ///
    /// # async fn example<S: thinkaroo::storage::ObjectStore>(state: AppState<S>) -> Result<(), thinkaroo::ServiceError> {
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
        let objects = self.object_store.list_objects(&folder_path).await?;
        let object_count = objects.len();

        if object_count >= MAX_OBJECTS_PER_HOUR {
            // Pick a random object from existing ones
            let random_index = rand::random::<usize>() % object_count;
            let key = &objects[random_index].key;

            // Fetch and parse the object
            let body_bytes = self.object_store.get_object(key).await?;
            let contents: T = serde_json::from_slice(&body_bytes)?;

            Ok(Some(contents))
        } else {
            // Need to generate new content
            Ok(None)
        }
    }

    /// Stores an object in storage with a time-based key
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
    /// * `Err(ServiceError)` - If serialization or storage operations fail
    pub async fn store_timed_object<T>(
        &self,
        object: &T,
        content_type: ContentType,
    ) -> Result<(), ServiceError>
    where
        T: Serialize + Sync,
    {
        let now = Utc::now();
        let folder_path = Self::format_timed_prefix(&now, content_type);
        let guid = Uuid::new_v4();
        let key = format!("{}{}.json", folder_path, guid);

        let json_data = serde_json::to_string(object)?;

        self.object_store.put_object(&key, json_data.into_bytes()).await?;

        Ok(())
    }

    /// Formats the storage prefix with content type and timestamp
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

    /// Generates content using OpenAI with structured JSON output
    ///
    /// This method uses OpenAI's structured output feature to generate content
    /// that strictly adheres to the provided type's JSON schema.
    ///
    /// # Type Parameters
    /// * `T` - The type of content to generate. Must implement Serialize, Deserialize, and JsonSchema.
    ///
    /// # Arguments
    /// * `prompt_config` - The prompt configuration containing model, system context, and user prompt
    /// * `schema_name` - A name for the JSON schema (e.g., "ReadingContents")
    /// * `schema_description` - A description of what the schema represents
    ///
    /// # Returns
    /// * `Ok(T)` - The generated content parsed into type T
    /// * `Err(ServiceError)` - If generation or parsing fails
    pub async fn generate_content<T>(
        &self,
        prompt_config: &PromptConfig,
        schema_name: &str,
        schema_description: &str,
    ) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Serialize + schemars::JsonSchema,
    {
        // Generate JSON schema for the type T
        let schema = schema_for!(T);
        let schema_value = serde_json::to_value(schema).map_err(|e| {
            ServiceError::ConfigError(format!("Failed to serialize schema: {}", e))
        })?;

        // Create response format with JSON schema
        let response_format = ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                description: Some(schema_description.to_string()),
                name: schema_name.to_string(),
                schema: Some(schema_value),
                strict: Some(true),
            },
        };

        // Create chat completion request with system context and user prompt
        let request = CreateChatCompletionRequestArgs::default()
            .model(&prompt_config.model)
            .response_format(response_format)
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

        // Parse the JSON response into the target type
        let result: T = serde_json::from_str(content)?;

        Ok(result)
    }
}
