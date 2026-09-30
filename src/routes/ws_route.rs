use actix_web::{Scope, web};

use crate::ws;

pub fn ws_route() -> Scope {
    web::scope("/ws").route("", web::get().to(ws::handler::ws_handler))
}
