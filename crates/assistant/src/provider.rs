//! The one entry point to every provider: picks the adapter for the wire format. Keys are
//! passed in (read from the keychain by the caller), so tests never touch the keychain.

use crate::settings::{CLAUDE_MODELS, ProviderConfig, ProviderKind};
use crate::types::{ChatRequest, ChatResponse};
use crate::{Error, Result, anthropic, gemini, openai};

fn require_key(config: &ProviderConfig, key: Option<&str>) -> Result<()> {
    if config.kind.needs_key() && key.is_none_or(|k| k.trim().is_empty()) {
        return Err(Error::Invalid(format!(
            "{} needs an API key (AI dialog → Chat)",
            config.name
        )));
    }
    if config.model.trim().is_empty() {
        return Err(Error::Invalid(format!(
            "pick a model for {} (AI dialog → Chat)",
            config.name
        )));
    }
    Ok(())
}

/// Streams a reply: `on_text` gets each piece of text as it arrives.
pub async fn chat(
    config: &ProviderConfig,
    key: Option<&str>,
    request: &ChatRequest<'_>,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<ChatResponse> {
    require_key(config, key)?;
    let base = config.base_url.as_str();
    match config.kind {
        ProviderKind::Anthropic => {
            anthropic::chat(base, key.unwrap_or_default(), request, on_text).await
        }
        ProviderKind::Gemini => gemini::chat(base, key.unwrap_or_default(), request, on_text).await,
        _ => openai::chat(base, key.filter(|k| !k.is_empty()), request, on_text).await,
    }
}

/// Models the provider offers, for the settings picker. Any name can still be typed.
pub async fn models(config: &ProviderConfig, key: Option<&str>) -> Result<Vec<String>> {
    if config.kind.needs_key() && key.is_none_or(|k| k.trim().is_empty()) {
        return Ok(suggested(config.kind));
    }
    let base = config.base_url.as_str();
    let mut found = match config.kind {
        ProviderKind::Anthropic => anthropic::models(base, key.unwrap_or_default()).await?,
        ProviderKind::Gemini => gemini::models(base, key.unwrap_or_default()).await?,
        _ => openai::models(base, key.filter(|k| !k.is_empty())).await?,
    };
    // Our recommended models first, then whatever else the provider lists.
    let mut out = suggested(config.kind);
    out.retain(|m| found.is_empty() || found.contains(m));
    found.retain(|m| !out.contains(m));
    out.extend(found);
    Ok(out)
}

/// Models offered before the provider is reachable.
pub fn suggested(kind: ProviderKind) -> Vec<String> {
    match kind {
        ProviderKind::Anthropic => CLAUDE_MODELS.iter().map(|m| m.to_string()).collect(),
        _ => Vec::new(),
    }
}

/// "Test connection": the provider answers and accepts the key.
pub async fn test(config: &ProviderConfig, key: Option<&str>) -> Result<String> {
    require_key(config, key)?;
    let models = models(config, key).await?;
    if !models.is_empty() && !models.contains(&config.model) {
        return Ok(format!(
            "Connected, but {} isn't in the provider's list of {} models",
            config.model,
            models.len()
        ));
    }
    Ok(format!("Connected ({} models available)", models.len()))
}
