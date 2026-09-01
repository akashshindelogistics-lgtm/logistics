use actix_cors::Cors;
use actix_web::{App, HttpServer};
use logistics_system::logistics::auth::auth::ensure_jwt_secret_configured;
use logistics_system::logistics::server::routes::config_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Fail fast on a release build that has no usable JWT_SECRET, instead of
    // 500-ing on the first login.
    ensure_jwt_secret_configured();

    // Bind host/port come from the environment so the same binary runs behind a
    // reverse proxy in a container (`0.0.0.0`, `PORT`) and directly on a
    // developer's machine (the `127.0.0.1:8080` defaults).
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    println!("Starting Logistics System REST API server at http://{host}:{port}...");

    // Permissive CORS: lets the published Swagger UI (or any other origin) call
    // this server directly. Auth uses bearer tokens, not cookies, so a wide-open
    // origin policy carries no credential risk.
    HttpServer::new(|| App::new().wrap(Cors::permissive()).configure(config_routes))
        .bind((host.as_str(), port))?
        .run()
        .await
}
