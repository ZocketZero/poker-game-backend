mod auth_route;
mod ws_route;

use actix_web::{Scope, web};

pub fn routes() -> Scope {
    web::scope("")
        // Auth endpoint
        .service(auth_route::auth_routes())
        // WebSocket endpoint
        .service(ws_route::ws_route())
}
