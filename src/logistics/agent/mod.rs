//! Agents: Claude working through the same domain code as the dashboard.
//!
//! The model only ever asks for things. [`tools`] decides what it can ask for,
//! [`store::decide`] decides what happens to a request given the org's autonomy
//! mode, and the domain models still enforce their own rules. None of that
//! depends on what the model was told, so a bad model answer or a prompt
//! injection in customer data can't widen what an agent may do.
//!
//! See `docs/agentic-workflow.md`.

pub mod model;
pub mod runtime;
pub mod store;
pub mod tools;

#[cfg(test)]
mod tests;
