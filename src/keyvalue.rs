use async_trait::async_trait;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::ServiceError;

/// DynamoDB table name for key-value storage
const DYNAMODB_TABLE_NAME: &str = "thinkaroo-data";

/// Primary key attribute name in DynamoDB
const PRIMARY_KEY_ATTR: &str = "pk";

/// Represents a column with a name and binary value
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub value: Vec<u8>,
}

impl Column {
    /// Creates a new Column
    pub fn new(name: String, value: Vec<u8>) -> Self {
        Self { name, value }
    }
}

/// KeyValueStore trait for abstracting key-value storage operations
///
/// This trait provides a common interface for put and get operations,
/// allowing implementations using different backends (DynamoDB, in-memory, etc.)
#[async_trait]
pub trait KeyValueStore: Clone + Send + Sync {
    /// Stores columns associated with a key
    ///
    /// # Arguments
    /// * `key` - The primary key for the item
    /// * `columns` - The columns to store (name and binary value pairs)
    ///
    /// # Returns
    /// * `Ok(())` - If the item was successfully stored
    /// * `Err(ServiceError)` - If storage operations fail
    async fn put(&self, key: String, columns: Vec<Column>) -> Result<(), ServiceError>;

    /// Retrieves specific columns for a key
    ///
    /// # Arguments
    /// * `key` - The primary key for the item
    /// * `column_names` - The names of columns to retrieve
    ///
    /// # Returns
    /// * `Ok(Vec<Column>)` - The retrieved columns (may be empty if key doesn't exist)
    /// * `Err(ServiceError)` - If retrieval fails
    async fn get(&self, key: String, column_names: Vec<String>) -> Result<Vec<Column>, ServiceError>;
}

/// DynamoDB-based key-value store implementation
#[derive(Clone)]
pub struct DynamoKeyValueStore {
    client: DynamoDbClient,
}

impl DynamoKeyValueStore {
    /// Creates a new DynamoKeyValueStore instance
    pub fn new(client: DynamoDbClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl KeyValueStore for DynamoKeyValueStore {
    async fn put(&self, key: String, columns: Vec<Column>) -> Result<(), ServiceError> {
        let mut item = HashMap::new();

        // Add primary key
        item.insert(
            PRIMARY_KEY_ATTR.to_string(),
            AttributeValue::S(key),
        );

        // Add all columns as binary attributes
        for column in columns {
            item.insert(
                column.name,
                AttributeValue::B(column.value.into()),
            );
        }

        self.client
            .put_item()
            .table_name(DYNAMODB_TABLE_NAME)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| ServiceError::DynamoDbError(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, key: String, column_names: Vec<String>) -> Result<Vec<Column>, ServiceError> {
        // Build primary key for get_item
        let mut key_map = HashMap::new();
        key_map.insert(
            PRIMARY_KEY_ATTR.to_string(),
            AttributeValue::S(key),
        );

        let result = self
            .client
            .get_item()
            .table_name(DYNAMODB_TABLE_NAME)
            .set_key(Some(key_map))
            .send()
            .await
            .map_err(|e| ServiceError::DynamoDbError(e.to_string()))?;

        let mut columns = Vec::new();

        if let Some(item) = result.item {
            for column_name in column_names {
                if let Some(attr_value) = item.get(&column_name) {
                    if let Some(bytes) = attr_value.as_b().ok() {
                        columns.push(Column::new(
                            column_name,
                            bytes.clone().into_inner(),
                        ));
                    }
                }
            }
        }

        Ok(columns)
    }
}

/// In-memory key-value store implementation for testing and development
#[derive(Clone)]
pub struct MemoryKeyValueStore {
    data: Arc<RwLock<HashMap<String, HashMap<String, Vec<u8>>>>>,
}

impl MemoryKeyValueStore {
    /// Creates a new MemoryKeyValueStore instance
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryKeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueStore for MemoryKeyValueStore {
    async fn put(&self, key: String, columns: Vec<Column>) -> Result<(), ServiceError> {
        let mut data = self.data.write().await;

        let item = data.entry(key).or_insert_with(HashMap::new);

        for column in columns {
            item.insert(column.name, column.value);
        }

        Ok(())
    }

    async fn get(&self, key: String, column_names: Vec<String>) -> Result<Vec<Column>, ServiceError> {
        let data = self.data.read().await;

        let mut columns = Vec::new();

        if let Some(item) = data.get(&key) {
            for column_name in column_names {
                if let Some(value) = item.get(&column_name) {
                    columns.push(Column::new(column_name, value.clone()));
                }
            }
        }

        Ok(columns)
    }
}
