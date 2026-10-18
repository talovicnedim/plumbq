use std::sync::Arc;

use plumbq_assistant::settings::{self, AssistantSettings, ProviderConfig, ProviderKind};
use plumbq_assistant::tools::{QuickAction, Tools};
use plumbq_assistant::{Error, key_name, provider};
use plumbq_core::app_secrets;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, blocking};
use crate::mcp::McpState;

/// Settings plus what the settings page shows about each provider.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStatus {
    pub settings: AssistantSettings,
    pub providers: Vec<ProviderStatus>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub has_key: bool,
    pub needs_key: bool,
    /// Runs on this machine or the local network: nothing leaves it.
    pub local: bool,
    /// `Assistant · Claude Sonnet 5.5`.
    pub actor: String,
}

/// A provider that isn't saved yet, and whether it runs locally (cloud ones need a one-time
/// "what gets sent" confirm before they are saved).
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDraft {
    pub provider: ProviderConfig,
    pub local: bool,
}

/// Looks up each provider's key in the keychain, so it runs off the async runtime.
async fn status(mcp: &McpState) -> Result<AssistantStatus, AppError> {
    let data_dir = mcp.data_dir().to_path_buf();
    blocking(move || {
        let settings = settings::load(&data_dir)?;
        let providers = settings
            .providers
            .iter()
            .map(|p| ProviderStatus {
                id: p.id.clone(),
                has_key: app_secrets::has(&key_name(&p.id)),
                needs_key: p.kind.needs_key(),
                local: p.is_local(),
                actor: p.actor(),
            })
            .collect();
        Ok(AssistantStatus {
            settings,
            providers,
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn assistant_status(mcp: State<'_, Arc<McpState>>) -> Result<AssistantStatus, AppError> {
    status(&mcp).await
}

/// Saves the chat settings; a removed provider's key leaves the keychain too.
#[tauri::command]
#[specta::specta]
pub async fn assistant_update(
    mcp: State<'_, Arc<McpState>>,
    settings: AssistantSettings,
) -> Result<AssistantStatus, AppError> {
    let data_dir = mcp.data_dir().to_path_buf();
    blocking(move || {
        for gone in settings::update(&data_dir, &settings)? {
            app_secrets::remove(&key_name(&gone))?;
        }
        Ok(())
    })
    .await?;
    status(&mcp).await
}

/// A new provider of `kind` with its default URL and model; not saved yet.
#[tauri::command]
#[specta::specta]
pub fn assistant_provider_new(kind: ProviderKind, name: String) -> ProviderDraft {
    let provider = ProviderConfig::new(kind, &name);
    let local = provider.is_local();
    ProviderDraft { provider, local }
}

/// Stores a provider's API key in the keychain; an empty key removes it.
#[tauri::command]
#[specta::specta]
pub async fn assistant_key_set(
    mcp: State<'_, Arc<McpState>>,
    provider_id: String,
    key: String,
) -> Result<AssistantStatus, AppError> {
    let name = key_name(&provider_id);
    blocking(move || Ok(app_secrets::set(&name, key.trim())?)).await?;
    status(&mcp).await
}

#[tauri::command]
#[specta::specta]
pub async fn assistant_models(
    mcp: State<'_, Arc<McpState>>,
    provider_id: String,
) -> Result<Vec<String>, AppError> {
    let (config, key) = provider_and_key(&mcp, &provider_id).await?;
    Ok(provider::models(&config, key.as_deref()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn assistant_test(
    mcp: State<'_, Arc<McpState>>,
    provider_id: String,
) -> Result<String, AppError> {
    let (config, key) = provider_and_key(&mcp, &provider_id).await?;
    Ok(provider::test(&config, key.as_deref()).await?)
}

/// The MCP server's prompt templates, offered as quick actions.
#[tauri::command]
#[specta::specta]
pub async fn assistant_quick_actions(
    mcp: State<'_, Arc<McpState>>,
) -> Result<Vec<QuickAction>, AppError> {
    let tools = Tools::connect(plumbq_mcp::in_app(mcp.http_server()), "Assistant").await?;
    let actions = tools.quick_actions().await;
    tools.close().await;
    Ok(actions?)
}

/// A quick action's prompt text, to send as the user's message. Its arguments come from the
/// open request (`request_id`), if any.
#[tauri::command]
#[specta::specta]
pub async fn assistant_quick_action(
    mcp: State<'_, Arc<McpState>>,
    name: String,
    request_id: Option<String>,
) -> Result<String, AppError> {
    let tools = Tools::connect(plumbq_mcp::in_app(mcp.http_server()), "Assistant").await?;
    let text = quick_action_text(&tools, &name, request_id.as_deref()).await;
    tools.close().await;
    text
}

async fn quick_action_text(
    tools: &Tools,
    name: &str,
    request_id: Option<&str>,
) -> Result<String, AppError> {
    let arguments = tools
        .quick_actions()
        .await?
        .into_iter()
        .find(|action| action.name == name)
        .map(|action| action.arguments)
        .unwrap_or_default();
    let args = Tools::quick_action_args(&arguments, request_id);
    Ok(tools.quick_action(name, args).await?)
}

/// A saved provider and its key.
async fn provider_and_key(
    mcp: &McpState,
    id: &str,
) -> Result<(ProviderConfig, Option<String>), AppError> {
    let config = settings::load(mcp.data_dir())?
        .providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| Error::Invalid(format!("no provider {id}")))?;
    let key = provider_key(&config).await?;
    Ok((config, key))
}

/// The provider's key from the keychain, read off the async runtime because keychain calls
/// block.
pub(super) async fn provider_key(config: &ProviderConfig) -> Result<Option<String>, AppError> {
    let name = key_name(&config.id);
    blocking(move || Ok(app_secrets::get(&name)?)).await
}
