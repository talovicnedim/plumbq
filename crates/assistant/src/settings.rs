//! Chat settings in `assistant.json` in the app data dir: providers (keys live in the keychain),
//! the active one, and what may be shared with them.

use std::fs;
use std::path::{Path, PathBuf};

use plumbq_core::{fs_util, guard};
use plumbq_mcp::McpSettings;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const FILE: &str = "assistant.json";

/// The start of every actor name the built-in chat uses, so the activity log can tell the chat
/// from AI apps connected over MCP.
pub const CHAT_ACTOR: &str = "Chat";

/// The current Claude models, newest first; the default comes first.
pub const CLAUDE_MODELS: [&str; 3] = ["claude-sonnet-5-5", "claude-opus-5-5", "claude-haiku-5-5"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    Gemini,
    Ollama,
    LmStudio,
    /// Any server speaking the OpenAI chat completions API ("Custom": OpenRouter, Groq, vLLM…).
    /// Settings from before OpenRouter folded into it still load, keeping their URL and key.
    #[serde(alias = "openRouter")]
    OpenAiCompatible,
}

impl ProviderKind {
    /// Where the provider listens unless the user changes it.
    pub fn default_base_url(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "https://api.anthropic.com/v1",
            ProviderKind::OpenAi => "https://api.openai.com/v1",
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            ProviderKind::Ollama => "http://localhost:11434/v1",
            ProviderKind::LmStudio => "http://localhost:1234/v1",
            ProviderKind::OpenAiCompatible => "https://openrouter.ai/api/v1",
        }
    }

    pub fn needs_key(self) -> bool {
        matches!(
            self,
            ProviderKind::Anthropic | ProviderKind::OpenAi | ProviderKind::Gemini
        )
    }

    /// The model a new provider of this kind starts with; empty means "pick one".
    pub fn default_model(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => CLAUDE_MODELS[0],
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// Generated; also the keychain name of its API key.
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
    pub model: String,
    /// The provider's default when unset.
    #[serde(default)]
    pub temperature: Option<f32>,
}

impl ProviderConfig {
    pub fn new(kind: ProviderKind, name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            kind,
            name: name.to_string(),
            base_url: kind.default_base_url().to_string(),
            model: kind.default_model().to_string(),
            temperature: None,
        }
    }

    /// Local when the endpoint is this machine or a private network: nothing leaves it. The host
    /// is parsed as a URL, so `10.evil.com` or `192.168.1.1@evil.com` don't pass as addresses.
    pub fn is_local(&self) -> bool {
        guard::url_host(&self.base_url).is_some_and(|host| guard::is_private_host(&host))
    }

    /// How the chat introduces itself in approvals and the activity log.
    pub fn actor(&self) -> String {
        format!("{CHAT_ACTOR} · {}", model_label(&self.model))
    }
}

/// `claude-sonnet-5-5` → `Claude Sonnet 5.5`; other models as written.
pub fn model_label(model: &str) -> String {
    let Some(rest) = model.strip_prefix("claude-") else {
        return model.to_string();
    };
    let mut parts = rest.split('-');
    let family = parts.next().unwrap_or_default();
    let version: Vec<&str> = parts
        .take_while(|p| p.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let mut chars = family.chars();
    let family = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
        None => return model.to_string(),
    };
    format!("Claude {family} {}", version.join("."))
        .trim_end()
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSettings {
    pub providers: Vec<ProviderConfig>,
    pub active_provider: Option<String>,
    /// Response bodies go to the model; off means tools return status, headers and sizes only.
    pub share_bodies: bool,
    /// Provider ids whose "what gets sent" notice the user accepted.
    pub acknowledged: Vec<String>,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            active_provider: None,
            share_bodies: true,
            acknowledged: Vec::new(),
        }
    }
}

impl AssistantSettings {
    pub fn active(&self) -> Option<&ProviderConfig> {
        let id = self.active_provider.as_deref()?;
        self.providers.iter().find(|p| p.id == id)
    }

    /// The provider the chat may send to: AI is on (`ai`, the app's AI settings), a model is
    /// picked, and for a cloud provider the user accepted the "what gets sent" notice.
    /// Checked here, not only in the UI.
    pub fn chat_provider(&self, ai: &McpSettings) -> Result<&ProviderConfig> {
        if ai.turned_off {
            return Err(Error::Invalid(
                "AI is turned off; turn it on in the AI dialog".into(),
            ));
        }
        let config = self
            .active()
            .ok_or_else(|| Error::Invalid("pick a model first (AI dialog → Chat)".into()))?;
        if !config.is_local() && !self.acknowledged.contains(&config.id) {
            return Err(Error::NeedsConfirmation(format!(
                "confirm what goes to {} first (AI dialog → Chat)",
                config.name
            )));
        }
        Ok(config)
    }
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

pub fn load(data_dir: &Path) -> Result<AssistantSettings> {
    match fs::read_to_string(path(data_dir)) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(AssistantSettings::default()),
        Err(err) => Err(err.into()),
    }
}

/// Saves `new`. Returns the ids of providers it no longer has: the caller removes their keys
/// from the keychain.
pub fn update(data_dir: &Path, new: &AssistantSettings) -> Result<Vec<String>> {
    let old = load(data_dir)?;
    save(data_dir, new)?;
    Ok(old
        .providers
        .into_iter()
        .filter(|o| !new.providers.iter().any(|p| p.id == o.id))
        .map(|gone| gone.id)
        .collect())
}

pub fn save(data_dir: &Path, settings: &AssistantSettings) -> Result<()> {
    for provider in &settings.providers {
        if provider.name.trim().is_empty() || provider.base_url.trim().is_empty() {
            return Err(Error::Invalid(
                "every provider needs a name and a base URL".into(),
            ));
        }
    }
    fs_util::write_private(&path(data_dir), serde_json::to_string_pretty(settings)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
