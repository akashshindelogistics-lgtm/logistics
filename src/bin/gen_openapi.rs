use logistics_system::logistics::server::routes::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let spec = ApiDoc::openapi()
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI spec to JSON");
    println!("{}", spec);
}
