use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::auth;
use crate::db::repository;
use crate::error::AppError;
use crate::AppState;

const DEFAULT_STARTING_CHIPS: u64 = 10_000;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// POST /api/auth/register
pub async fn register(
    state: web::Data<AppState>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    let username = body.username.trim();
    let password = &body.password;

    if username.is_empty() || username.len() > 32 {
        return Err(AppError::BadRequest(
            "Username must be 1-32 characters".to_string(),
        ));
    }
    if password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".to_string(),
        ));
    }
    if password.len() > 72 {
        return Err(AppError::BadRequest(
            "Password cannot exceed 72 characters".to_string(),
        ));
    }

    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;

    let user =
        repository::create_user(&state.db, username, &password_hash, DEFAULT_STARTING_CHIPS)
            .await?;

    let user_id = user
        .id
        .map(|id| id.to_hex())
        .unwrap_or_default();

    let token = auth::create_token(&user_id, username, &state.config.jwt_secret)?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "token": token,
        "username": username,
        "chips": user.chips,
    })))
}

/// POST /api/auth/login
pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    let username = body.username.trim();
    let password = &body.password;

    let user = repository::find_user_by_username(&state.db, username)
        .await?
        .ok_or_else(|| AppError::Auth("Invalid username or password".to_string()))?;

    let valid = bcrypt::verify(password, &user.password_hash)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {e}")))?;

    if !valid {
        return Err(AppError::Auth("Invalid username or password".to_string()));
    }

    let user_id = user
        .id
        .map(|id| id.to_hex())
        .unwrap_or_default();

    let token = auth::create_token(&user_id, &user.username, &state.config.jwt_secret)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "token": token,
        "username": user.username,
        "chips": user.chips,
    })))
}
