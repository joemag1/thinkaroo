use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use std::path::PathBuf;

use crate::ServiceError;

/// S3 bucket name for storing objects
const S3_BUCKET_NAME: &str = "thinkaroo-reading-stories";

/// Base directory for disk storage
const DISK_STORAGE_BASE: &str = "./storage";

/// Represents a stored object with its key
#[derive(Debug, Clone)]
pub struct StoredObject {
    pub key: String,
}

/// Storage trait for abstracting basic object storage operations
///
/// This trait provides a common interface for put, get, and list operations,
/// allowing implementations using different backends (S3, local disk, etc.)
#[async_trait]
pub trait ObjectStore: Clone + Send + Sync {
    /// Stores an object with the given key and data
    ///
    /// # Arguments
    /// * `key` - The key/path for the object
    /// * `data` - The raw bytes to store
    ///
    /// # Returns
    /// * `Ok(())` - If the object was successfully stored
    /// * `Err(ServiceError)` - If storage operations fail
    async fn put_object(&self, key: &str, data: Vec<u8>) -> Result<(), ServiceError>;

    /// Retrieves an object by its key
    ///
    /// # Arguments
    /// * `key` - The key/path of the object to retrieve
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The raw bytes of the object
    /// * `Err(ServiceError)` - If the object doesn't exist or retrieval fails
    async fn get_object(&self, key: &str) -> Result<Vec<u8>, ServiceError>;

    /// Lists all objects with the given prefix
    ///
    /// # Arguments
    /// * `prefix` - The prefix to filter objects by
    ///
    /// # Returns
    /// * `Ok(Vec<StoredObject>)` - A list of objects matching the prefix
    /// * `Err(ServiceError)` - If listing fails
    async fn list_objects(&self, prefix: &str) -> Result<Vec<StoredObject>, ServiceError>;
}

/// S3-based storage implementation
#[derive(Clone)]
pub struct S3ObjectStore {
    client: S3Client,
}

impl S3ObjectStore {
    /// Creates a new S3Storage instance
    pub fn new(client: S3Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put_object(&self, key: &str, data: Vec<u8>) -> Result<(), ServiceError> {
        self.client
            .put_object()
            .bucket(S3_BUCKET_NAME)
            .key(key)
            .body(data.into())
            .content_type("application/json")
            .send()
            .await?;

        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<Vec<u8>, ServiceError> {
        let get_output = self
            .client
            .get_object()
            .bucket(S3_BUCKET_NAME)
            .key(key)
            .send()
            .await?;

        let body_bytes = get_output.body.collect().await?.into_bytes();
        Ok(body_bytes.to_vec())
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<StoredObject>, ServiceError> {
        let list_output = self
            .client
            .list_objects_v2()
            .bucket(S3_BUCKET_NAME)
            .prefix(prefix)
            .send()
            .await?;

        let objects = list_output
            .contents()
            .iter()
            .filter_map(|obj| {
                obj.key().map(|k| StoredObject {
                    key: k.to_string(),
                })
            })
            .collect();

        Ok(objects)
    }
}

/// Disk-based storage implementation
#[derive(Clone)]
pub struct DiskObjectStore {
    base_path: PathBuf,
}

impl DiskObjectStore {
    /// Creates a new DiskStorage instance with the default base path
    pub fn new() -> Self {
        Self {
            base_path: PathBuf::from(DISK_STORAGE_BASE),
        }
    }

    /// Creates a new DiskStorage instance with a custom base path
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Converts a storage key to a file path
    fn key_to_path(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }

    /// Converts a file path back to a storage key
    fn path_to_key(&self, path: &PathBuf) -> Option<String> {
        path.strip_prefix(&self.base_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
    }
}

impl Default for DiskObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ObjectStore for DiskObjectStore {
    async fn put_object(&self, key: &str, data: Vec<u8>) -> Result<(), ServiceError> {
        let file_path = self.key_to_path(key);

        // Create parent directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&file_path, data).await?;

        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<Vec<u8>, ServiceError> {
        let file_path = self.key_to_path(key);

        Ok(tokio::fs::read(&file_path).await?)
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<StoredObject>, ServiceError> {
        let search_path = self.key_to_path(prefix);

        // If the search path doesn't exist, return empty list
        if !search_path.exists() {
            return Ok(Vec::new());
        }

        let mut objects = Vec::new();

        // Recursively walk the directory
        let mut walk_stack = vec![search_path.clone()];

        while let Some(current_path) = walk_stack.pop() {
            if current_path.is_dir() {
                let mut entries = tokio::fs::read_dir(&current_path).await?;

                loop {
                    match entries.next_entry().await {
                        Ok(Some(entry)) => {
                            let path = entry.path();
                            if path.is_dir() {
                                walk_stack.push(path);
                            } else if let Some(key) = self.path_to_key(&path) {
                                objects.push(StoredObject { key });
                            }
                        }
                        Ok(None) => break,
                        Err(e) => return Err(ServiceError::IoError(e)),
                    }
                }
            } else if let Some(key) = self.path_to_key(&current_path) {
                objects.push(StoredObject { key });
            }
        }

        Ok(objects)
    }
}
