//! The built-in chat (bring your own key or a local model). It acts only through the app's MCP
//! tools in-process ([`tools`]); provider details stay in the adapter modules.

pub mod agent;
mod anthropic;
pub mod chats;
mod error;
mod gemini;
mod http;
mod openai;
pub mod provider;
pub mod settings;
pub mod tools;
pub mod turn;
pub mod types;

pub use error::{Error, Result};

/// The keychain name of a provider's API key.
pub fn key_name(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

#[cfg(test)]
mod tests;
