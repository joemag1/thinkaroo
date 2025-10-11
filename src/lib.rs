pub mod prompts;
pub mod reading;

use axum::http::StatusCode;
use aws_smithy_types::byte_stream::error::Error as ByteStreamError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("S3 error: {0}")]
    S3Error(String),

    #[error("OpenAI API error: {0}")]
    OpenAIError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Byte stream error: {0}")]
    ByteStreamError(#[from] ByteStreamError),
}

impl<E> From<aws_sdk_s3::error::SdkError<E>> for ServiceError
where
    E: std::error::Error + 'static,
{
    fn from(err: aws_sdk_s3::error::SdkError<E>) -> Self {
        ServiceError::S3Error(err.to_string())
    }
}

impl ServiceError {
    pub fn into_status(self) -> (StatusCode, String) {
        match self {
            ServiceError::S3Error(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Internal server error".to_string(),
            ),
            ServiceError::OpenAIError(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "AI service unavailable".to_string(),
            ),
            ServiceError::ConfigError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration error".to_string(),
            ),
            ServiceError::JsonError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Data parsing error".to_string(),
            ),
            ServiceError::Utf8Error(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Data encoding error".to_string(),
            ),
            ServiceError::IoError(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "I/O error".to_string(),
            ),
            ServiceError::ByteStreamError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Stream error".to_string(),
            ),
        }
    }
}
