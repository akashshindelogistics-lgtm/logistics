//! The chat inbox: a person asks for something in plain language and the model
//! does it through the same API the dashboard uses.
//!
//! This differs from `agent/` in whose authority is used. An agent acts alone,
//! under the org's autonomy mode. Chat acts for the signed-in person: it calls
//! the real route handlers with their own token, so their role, the org scope,
//! validation and notifications all work exactly as if they'd clicked through
//! the dashboard. Reads run straight away. A write only happens when the person
//! confirms a card whose contents the server wrote from the real request, so a
//! bad model answer or a prompt injection in customer data can't change anything
//! on its own.
//!
//! See `docs/chat-inbox.md`.

pub mod catalog;
pub mod runtime;
pub mod store;

#[cfg(test)]
mod tests;
