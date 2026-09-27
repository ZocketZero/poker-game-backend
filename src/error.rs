use actix_web::{HttpResponse, ResponseError};

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Game error: {0}")]
    Game(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let body = serde_json::json!({ "error": self.to_string() });
        match self {
            AppError::Auth(_) => HttpResponse::Unauthorized().json(body),
            AppError::BadRequest(_) => HttpResponse::BadRequest().json(body),
            AppError::NotFound(_) => HttpResponse::NotFound().json(body),
            AppError::Conflict(_) => HttpResponse::Conflict().json(body),
            AppError::Game(_) => HttpResponse::UnprocessableEntity().json(body),
            AppError::Internal(_) => HttpResponse::InternalServerError().json(body),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::Internal(format!("Database error: {e}"))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::Auth(format!("Token error: {e}"))
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::Internal(format!("Hashing error: {e}"))
    }
}
