use super::*;
use crate::tests::temp_dir;

fn provider(kind: ProviderKind, base_url: &str) -> ProviderConfig {
    ProviderConfig {
        base_url: base_url.into(),
        ..ProviderConfig::new(kind, "test")
    }
}

#[test]
fn tells_local_from_cloud_endpoints() {
    assert!(provider(ProviderKind::Ollama, "http://localhost:11434/v1").is_local());
    assert!(provider(ProviderKind::LmStudio, "http://127.0.0.1:1234/v1").is_local());
    assert!(provider(ProviderKind::Ollama, "http://[::1]:11434/v1").is_local());
    assert!(provider(ProviderKind::OpenAiCompatible, "http://studio.local/v1").is_local());
    assert!(!provider(ProviderKind::OpenAi, "https://api.openai.com/v1").is_local());
    assert!(
        !provider(
            ProviderKind::OpenAiCompatible,
            "https://openrouter.ai/api/v1"
        )
        .is_local()
    );
    assert!(
        !provider(
            ProviderKind::OpenAiCompatible,
            "https://localhost.evil.com/v1"
        )
        .is_local()
    );
}

#[test]
fn only_private_addresses_count_as_local() {
    let local = |url: &str| provider(ProviderKind::OpenAiCompatible, url).is_local();
    assert!(local("http://10.0.0.5:8000/v1"));
    assert!(local("http://172.16.4.2/v1"));
    assert!(local("http://192.168.1.20:11434/v1"));
    assert!(local("http://169.254.10.1/v1"));
    assert!(local("http://[fd12::1]:8080/v1"));
    assert!(local("http://LocalHost:1234/v1"));
    assert!(!local("https://10.evil.com/v1"));
    assert!(!local("https://127.0.0.1.evil.com/v1"));
    assert!(!local("https://192.168.1.1@evil.com/v1"));
    assert!(!local("https://172.32.0.1/v1"));
    assert!(!local("https://8.8.8.8/v1"));
    assert!(!local("not a url"));
}

#[test]
fn labels_claude_models() {
    assert_eq!(model_label("claude-sonnet-5-5"), "Claude Sonnet 5.5");
    assert_eq!(model_label("claude-opus-5-5"), "Claude Opus 5.5");
    assert_eq!(model_label("llama3.1:8b"), "llama3.1:8b");
    assert_eq!(
        ProviderConfig::new(ProviderKind::Anthropic, "Claude").actor(),
        "Chat · Claude Sonnet 5.5"
    );
}

#[test]
fn open_router_settings_load_as_custom() {
    let dir = temp_dir("assistant-open-router");
    std::fs::write(
        dir.join(FILE),
        r#"{"providers": [{"id": "abc", "kind": "openRouter", "name": "OpenRouter",
            "baseUrl": "https://openrouter.ai/api/v1", "model": "x"}],
            "activeProvider": "abc", "shareBodies": true, "acknowledged": []}"#,
    )
    .unwrap();
    let loaded = load(&dir).unwrap();
    let provider = loaded.active().unwrap();
    assert_eq!(provider.kind, ProviderKind::OpenAiCompatible);
    assert_eq!(provider.base_url, "https://openrouter.ai/api/v1");
}

#[test]
fn saves_without_secrets_and_defaults_to_sharing_bodies() {
    let dir = temp_dir("assistant-settings");
    assert!(load(&dir).unwrap().share_bodies);
    let mut settings = AssistantSettings::default();
    let claude = ProviderConfig::new(ProviderKind::Anthropic, "Claude");
    settings.active_provider = Some(claude.id.clone());
    settings.providers.push(claude);
    save(&dir, &settings).unwrap();

    let loaded = load(&dir).unwrap();
    assert_eq!(
        loaded.active().map(|p| p.model.as_str()),
        Some("claude-sonnet-5-5")
    );
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(
        !text.to_lowercase().contains("key"),
        "no key fields: {text}"
    );
}

#[test]
fn the_chat_needs_ai_on_a_model_and_consent_for_the_cloud() {
    let claude = ProviderConfig::new(ProviderKind::Anthropic, "Claude");
    let ollama = ProviderConfig::new(ProviderKind::Ollama, "Ollama");
    let mut settings = AssistantSettings {
        active_provider: Some(claude.id.clone()),
        providers: vec![claude.clone(), ollama.clone()],
        ..AssistantSettings::default()
    };
    let ai = McpSettings::default();
    let err = settings.chat_provider(&ai).unwrap_err();
    assert_eq!(err.kind(), "needsConfirmation", "{err}");

    settings.acknowledged.push(claude.id.clone());
    assert_eq!(settings.chat_provider(&ai).unwrap().id, claude.id);
    let off = McpSettings {
        turned_off: true,
        ..McpSettings::default()
    };
    assert!(
        settings
            .chat_provider(&off)
            .unwrap_err()
            .to_string()
            .contains("turned off")
    );

    settings.active_provider = Some(ollama.id.clone());
    assert_eq!(
        settings.chat_provider(&ai).unwrap().id,
        ollama.id,
        "local needs no notice"
    );
    settings.active_provider = None;
    assert!(settings.chat_provider(&ai).is_err());
}

#[test]
fn settings_saved_with_the_old_ai_switch_still_load() {
    let dir = temp_dir("assistant-old-switch");
    std::fs::write(
        dir.join(FILE),
        r#"{"providers": [], "activeProvider": null, "shareBodies": false,
            "acknowledged": [], "turnedOff": true, "mcpWasOn": true}"#,
    )
    .unwrap();
    assert!(!load(&dir).unwrap().share_bodies);
}

#[test]
fn an_update_names_the_providers_it_removed() {
    let dir = temp_dir("assistant-update");
    let claude = ProviderConfig::new(ProviderKind::Anthropic, "Claude");
    let ollama = ProviderConfig::new(ProviderKind::Ollama, "Ollama");
    let both = AssistantSettings {
        providers: vec![claude.clone(), ollama.clone()],
        ..AssistantSettings::default()
    };
    assert!(update(&dir, &both).unwrap().is_empty());
    let one = AssistantSettings {
        providers: vec![ollama],
        ..AssistantSettings::default()
    };
    assert_eq!(update(&dir, &one).unwrap(), [claude.id]);
}
