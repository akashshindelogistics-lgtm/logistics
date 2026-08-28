mod logistics;

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use logistics::server::routes::config_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Logistics System REST API server at http://127.0.0.1:8080...");

    // Permissive CORS: lets the published Swagger UI (or any other origin) call
    // this server directly when it's running locally. Auth uses bearer tokens,
    // not cookies, so a wide-open origin policy carries no credential risk.
    HttpServer::new(|| App::new().wrap(Cors::permissive()).configure(config_routes))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
