use actix_web::{Scope, web};

use crate::auth;

pub fn auth_routes() -> Scope {
    web::scope("/api/auth")
        .route("/register", web::post().to(auth::auth_handlers::register))
        .route("/login", web::post().to(auth::auth_handlers::login))
}
