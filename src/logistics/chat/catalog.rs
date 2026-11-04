//! Which API operations chat can use, generated from the OpenAPI spec so a
//! request can't reach anything but a documented route with its declared
//! parameters.
//!
//! Every operation must be listed in [`EXPOSED`] or [`EXCLUDED`]; a test fails
//! otherwise. So a new route needs a decision, and it and whatever it returns
//! can't reach the model by default.

use crate::logistics::server::routes::{ApiDoc, config_api_routes};
use actix_web::http::Method;
use actix_web::{App, test};
use serde_json::{Value, json};
use std::sync::OnceLock;
use utoipa::OpenApi;
use uuid::Uuid;

/// Stops one huge list from filling the conversation. The model is told when
/// this happens, so it can say the list was cut off.
const MAX_RESULT_CHARS: usize = 30_000;
const MAX_SCHEMA_DEPTH: usize = 8;

pub const EXPOSED: &[&str] = &[
    "add_customer",
    "add_driver",
    "add_godown_stock",
    "add_vehicle",
    "add_vehicle_document",
    "add_vehicle_maintenance",
    "add_vendor",
    "assign_vehicle_driver",
    "assign_vehicle_hire",
    "auth_me",
    "create_dispatch_invoice",
    "create_godown",
    "create_trip",
    "delete_customer",
    "delete_driver",
    "delete_godown",
    "delete_godown_stock",
    "delete_org_user",
    "delete_vehicle",
    "delete_vehicle_document",
    "delete_vehicle_maintenance",
    "delete_vendor",
    "dispatch_stock",
    "edit_vehicle",
    "get_agent_settings",
    "get_customer_billing",
    "get_daily_digest",
    "get_dispatch_invoice",
    "get_dispatch_summary",
    "get_godown",
    "get_ops_report",
    "get_ops_report_summary",
    "get_org",
    "get_reorder_suggestions",
    "get_trip",
    "list_agent_actions",
    "list_agent_runs",
    "list_customers",
    "list_dispatch_notifications",
    "list_dispatches",
    "list_drivers",
    "list_godowns",
    "list_org_invoices",
    "list_org_notifications",
    "list_org_trips",
    "list_org_users",
    "list_org_vehicle_documents",
    "list_org_vehicle_maintenance",
    "list_orgs",
    "list_stock_transfers",
    "list_vehicle_documents",
    "list_vehicle_hires",
    "list_vehicle_maintenance",
    "list_vehicles",
    "list_vendor_payments",
    "list_vendors",
    "pay_invoice",
    "record_vehicle_maintenance_mileage",
    "record_vendor_payment",
    "transfer_godown_stock",
    "unpair_driver_device",
    "update_customer_location",
    "update_dispatch_status",
    "update_driver",
    "update_godown",
    "update_godown_location",
    "update_godown_stock",
    "update_invoice",
    "update_org",
    "update_org_location",
    "update_org_user",
    "update_vehicle_document",
    "update_vehicle_location",
    "update_vehicle_maintenance",
    "update_vendor",
];

/// Operations chat can't use, and why.
pub const EXCLUDED: &[(&str, &str)] = &[
    ("auth_login", "takes a password"),
    ("user_login", "takes a password"),
    ("auth_orgs", "sign-in page only"),
    ("create_org", "takes a password"),
    ("add_org_user", "takes a password"),
    ("rotate_driver_device_token", "returns a credential"),
    ("rotate_vehicle_tracker_key", "returns a credential"),
    (
        "get_driver_device_pairing",
        "phone pairing is done on the driver's page",
    ),
    ("report_driver_location", "for the driver's phone"),
    ("driver_me", "for the driver's phone"),
    ("track_vehicle_location", "for GPS trackers"),
    ("latest_driver_app", "for the driver's phone"),
    ("download_driver_app", "binary download"),
    ("upload_file", "the chat composer uploads files itself"),
    ("get_uploaded_file", "binary download"),
    ("delete_org", "deletes the whole organization"),
    (
        "run_agents_now",
        "agent controls stay on the Agent Inbox page",
    ),
    (
        "update_agent_settings",
        "agent controls stay on the Agent Inbox page",
    ),
    (
        "approve_agent_action",
        "agent controls stay on the Agent Inbox page",
    ),
    (
        "reject_agent_action",
        "agent controls stay on the Agent Inbox page",
    ),
    ("ask_assistant", "chat answers questions itself"),
    ("reindex_assistant", "maintenance"),
    ("health_check", "not useful to people"),
    ("list_chat_conversations", "chat's own routes"),
    ("create_chat_conversation", "chat's own routes"),
    ("get_chat_conversation", "chat's own routes"),
    ("delete_chat_conversation", "chat's own routes"),
    ("send_chat_message", "chat's own routes"),
    ("confirm_chat_proposal", "only a person can confirm"),
    ("cancel_chat_proposal", "only a person can cancel"),
];

/// Writes that move money. Shown as a warning card, like deletes.
const DANGEROUS: &[&str] = &["pay_invoice", "record_vendor_payment", "update_invoice"];

#[derive(Debug, Clone)]
pub struct Operation {
    pub id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub path_params: Vec<String>,
    pub query_params: Vec<String>,
    /// The resolved request body schema, if the operation takes one.
    pub body_schema: Option<Value>,
    pub parameters: Value,
}

impl Operation {
    pub fn is_read(&self) -> bool {
        self.method == "GET"
    }

    pub fn is_dangerous(&self) -> bool {
        self.method == "DELETE" || DANGEROUS.contains(&self.id.as_str())
    }

    /// `/api/orgs/{id}/...` always means the caller's own org, so the model
    /// doesn't supply it.
    fn org_scoped(&self) -> bool {
        self.path == "/api/orgs/{id}" || self.path.starts_with("/api/orgs/{id}/")
    }

    /// The path parameters the model has to supply.
    pub fn required_path_params(&self) -> Vec<&str> {
        self.path_params
            .iter()
            .map(String::as_str)
            .filter(|p| !(self.org_scoped() && *p == "id"))
            .collect()
    }

    pub fn title(&self) -> String {
        let words = self.id.replace('_', " ");
        let mut chars = words.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => words,
        }
    }

    /// What the model can see of this operation.
    pub fn describe(&self) -> Value {
        json!({
            "operation_id": self.id,
            "kind": if self.is_read() { "read" } else { "write, needs the user's confirmation" },
            "method": self.method,
            "path": self.path,
            "path_params": self.required_path_params(),
            "parameters": self.parameters,
            "body_schema": self.body_schema,
        })
    }
}

/// Replace `$ref`s with what they point at, so the model gets one whole schema.
fn resolve_refs(value: &Value, spec: &Value, depth: usize) -> Value {
    if depth > MAX_SCHEMA_DEPTH {
        return json!({"type": "object"});
    }
    match value {
        Value::Object(map) => {
            if let Some(target) = map.get("$ref").and_then(Value::as_str) {
                let pointer = target.trim_start_matches('#');
                return match spec.pointer(pointer) {
                    Some(found) => resolve_refs(found, spec, depth + 1),
                    None => json!({"type": "object"}),
                };
            }
            Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), resolve_refs(v, spec, depth)))
                    .collect(),
            )
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| resolve_refs(v, spec, depth)).collect())
        }
        other => other.clone(),
    }
}

/// Every operation in the API, sorted by id, so the prompt built from it is
/// byte-identical between requests and stays cached.
pub fn spec_operations() -> Vec<Operation> {
    let spec = serde_json::to_value(ApiDoc::openapi()).unwrap_or(Value::Null);
    let mut operations = Vec::new();
    for (path, methods) in spec["paths"].as_object().into_iter().flatten() {
        for (method, op) in methods.as_object().into_iter().flatten() {
            let Some(id) = op["operationId"].as_str() else {
                continue;
            };
            let parameters = op["parameters"].as_array().cloned().unwrap_or_default();
            let names_in = |location: &str| -> Vec<String> {
                parameters
                    .iter()
                    .filter(|p| p["in"] == location)
                    .filter_map(|p| p["name"].as_str().map(str::to_string))
                    .collect()
            };
            operations.push(Operation {
                id: id.to_string(),
                method: method.to_uppercase(),
                path: path.clone(),
                summary: op["summary"].as_str().unwrap_or_default().to_string(),
                path_params: names_in("path"),
                query_params: names_in("query"),
                body_schema: op["requestBody"]["content"]["application/json"]
                    .get("schema")
                    .map(|s| resolve_refs(s, &spec, 0)),
                parameters: resolve_refs(&Value::Array(parameters.clone()), &spec, 0),
            });
        }
    }
    operations.sort_by(|a, b| a.id.cmp(&b.id));
    operations
}

fn exposed() -> &'static [Operation] {
    static OPERATIONS: OnceLock<Vec<Operation>> = OnceLock::new();
    OPERATIONS.get_or_init(|| {
        spec_operations()
            .into_iter()
            .filter(|o| EXPOSED.contains(&o.id.as_str()))
            .collect()
    })
}

pub fn find(id: &str) -> Option<&'static Operation> {
    exposed().iter().find(|o| o.id == id)
}

/// One line per operation, for the system prompt.
pub fn prompt_listing() -> String {
    exposed()
        .iter()
        .map(|o| {
            let summary = o.summary.lines().next().unwrap_or_default();
            format!("{} {} {} {}", o.id, o.method, o.path, summary)
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn param_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Build the request's URI. Values are percent-encoded, so one can't climb out
/// of its path segment into a route that isn't in the catalog.
pub fn build_uri(
    op: &Operation,
    org_id: Uuid,
    path_params: &Value,
    query: &Value,
) -> Result<String, String> {
    let mut uri = op.path.clone();
    if op.org_scoped() {
        uri = uri.replacen("{id}", &org_id.to_string(), 1);
    }
    for name in op.required_path_params() {
        let value = path_params
            .get(name)
            .and_then(param_text)
            .filter(|v| v != "." && v != "..")
            .ok_or_else(|| format!("path_params.{name} is required"))?;
        uri = uri.replace(&format!("{{{name}}}"), &encode(&value));
    }
    let pairs: Vec<String> = op
        .query_params
        .iter()
        .filter_map(|name| {
            let value = query.get(name).and_then(param_text)?;
            Some(format!("{}={}", encode(name), encode(&value)))
        })
        .collect();
    if !pairs.is_empty() {
        uri.push('?');
        uri.push_str(&pairs.join("&"));
    }
    Ok(uri)
}

/// Catch a missing field before asking anyone to confirm a request that would
/// only fail. The handler still validates everything else.
pub fn check_body(op: &Operation, body: &Value) -> Result<(), String> {
    let Some(schema) = &op.body_schema else {
        return Ok(());
    };
    if !body.is_object() {
        return Err("body must be a JSON object. Use describe_operation to see it".to_string());
    }
    let missing: Vec<&str> = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|field| body.get(*field).is_none_or(Value::is_null))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("body is missing: {}", missing.join(", ")))
    }
}

/// Response fields that are credentials. Some routes return them for the
/// dashboard to show, such as `tracker_key` on every vehicle. They never go to
/// the model or into stored conversations. A test fails if the spec gains a
/// field that looks like one of these and isn't accounted for.
pub const SECRET_FIELDS: &[&str] = &["device_token", "password", "token", "tracker_key"];

fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if SECRET_FIELDS.contains(&key.as_str()) {
                    *item = Value::String("[hidden]".to_string());
                } else {
                    scrub(item);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub),
        _ => {}
    }
}

/// Hide credentials in an API response. Not JSON means a plain error message,
/// which has nothing to hide.
pub fn redact(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) => {
            scrub(&mut value);
            value.to_string()
        }
        Err(_) => text.to_string(),
    }
}

/// Cut a result down to size, saying so if anything was cut.
pub fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_RESULT_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(MAX_RESULT_CHARS).collect();
    format!("{kept}\n[Cut off here: the result was longer than {MAX_RESULT_CHARS} characters]")
}

/// Run one request through the real route handlers, as the person whose token
/// this is, without a network hop.
///
/// `actix_web::test` is used because it's the public way to drive an `App`
/// in-process; there's nothing test-only about what it does.
pub async fn invoke(
    method: &str,
    uri: &str,
    authorization: &str,
    body: Option<&Value>,
) -> Result<(u16, String), String> {
    let app = test::init_service(App::new().configure(config_api_routes)).await;
    let method = Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut request = test::TestRequest::default()
        .method(method)
        .uri(uri)
        .insert_header(("Authorization", authorization));
    if let Some(body) = body {
        request = request.set_json(body);
    }
    let response = test::try_call_service(&app, request.to_request())
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let bytes = test::read_body(response).await;
    Ok((status, String::from_utf8_lossy(&bytes).to_string()))
}
