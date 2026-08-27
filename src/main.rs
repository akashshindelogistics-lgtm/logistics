mod logistics;

use actix_web::{App, HttpServer};
use logistics::server::routes::config_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Logistics System REST API server at http://127.0.0.1:8080...");

    HttpServer::new(|| App::new().configure(config_routes))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
