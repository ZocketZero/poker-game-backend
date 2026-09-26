use actix_web::{App, HttpServer, Responder, web};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().route("/hi", web::get().to(hi)))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
async fn hi() -> impl Responder {
    "Hi"
}
