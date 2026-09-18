use actix_cors::Cors;
use actix_web::{App, HttpServer};
use logistics_system::logistics::auth::auth::ensure_jwt_secret_configured;
use logistics_system::logistics::notification::delay_alerts;
use logistics_system::logistics::server::routes::config_routes;

/// How often the delay-alert background scan wakes up and checks every
/// `IN_TRANSIT` dispatch across every org for one running behind schedule.
/// See `docs/delay-alerts.md`.
const DELAY_ALERT_SCAN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Fail fast on a release build that has no usable JWT_SECRET, instead of
    // 500-ing on the first login.
    ensure_jwt_secret_configured();

    // Runs for the lifetime of the process alongside the HTTP server — not
    // per-request, so a slow scan (or a transient DB hiccup, logged and
    // retried next tick) never blocks a single request.
    tokio::spawn(async {
        loop {
            tokio::time::sleep(DELAY_ALERT_SCAN_INTERVAL).await;
            if let Err(err) = delay_alerts::scan_and_alert().await {
                eprintln!("delay-alert scan failed: {err}");
            }
        }
    });

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
