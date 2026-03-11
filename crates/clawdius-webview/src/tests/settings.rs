use crate::components::settings::{
    KeybindingConfig, ProviderConfig, ThemeConfig, ThemeMode, WebviewConfig,
};

#[test]
fn test_webview_config_default() {
    let config = WebviewConfig::default();

    assert_eq!(config.providers.len(), 2);
    assert_eq!(config.theme.mode, ThemeMode::Dark);
    assert_eq!(config.keybindings.send_message, "Enter");
}

#[test]
fn test_provider_config_serialization() {
    let provider = ProviderConfig {
        name: "test-provider".to_string(),
        api_key: Some("secret-key".to_string()),
        model: Some("test-model".to_string()),
        enabled: true,
        base_url: Some("https://api.example.com".to_string()),
    };

    let json = serde_json::to_string(&provider).unwrap();
    assert!(json.contains("test-provider"));
    assert!(json.contains("secret-key"));
    assert!(json.contains("test-model"));

    let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "test-provider");
    assert_eq!(deserialized.api_key, Some("secret-key".to_string()));
    assert!(deserialized.enabled);
}

#[test]
fn test_theme_mode_serialization() {
    let modes = vec![ThemeMode::Dark, ThemeMode::Light, ThemeMode::Custom];

    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: ThemeMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }
}

#[test]
fn test_theme_config() {
    let theme = ThemeConfig {
        mode: ThemeMode::Light,
        custom_colors: None,
    };

    let json = serde_json::to_string(&theme).unwrap();
    let deserialized: ThemeConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.mode, ThemeMode::Light);
    assert_eq!(deserialized.custom_colors, None);
}

#[test]
fn test_keybinding_config() {
    let keybindings = KeybindingConfig {
        send_message: "Ctrl+Enter".to_string(),
        new_session: "Ctrl+Shift+N".to_string(),
        open_settings: "Ctrl+Shift+,".to_string(),
        toggle_sidebar: "Ctrl+Shift+B".to_string(),
    };

    let json = serde_json::to_string(&keybindings).unwrap();
    let deserialized: KeybindingConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.send_message, "Ctrl+Enter");
    assert_eq!(deserialized.new_session, "Ctrl+Shift+N");
}

#[test]
fn test_webview_config_serialization() {
    let config = WebviewConfig {
        providers: vec![ProviderConfig {
            name: "anthropic".to_string(),
            api_key: Some("key1".to_string()),
            model: Some("claude-3".to_string()),
            enabled: true,
            base_url: None,
        }],
        theme: ThemeConfig {
            mode: ThemeMode::Dark,
            custom_colors: None,
        },
        keybindings: KeybindingConfig {
            send_message: "Enter".to_string(),
            new_session: "Ctrl+N".to_string(),
            open_settings: "Ctrl+,".to_string(),
            toggle_sidebar: "Ctrl+B".to_string(),
        },
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: WebviewConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.providers.len(), 1);
    assert_eq!(deserialized.providers[0].name, "anthropic");
    assert_eq!(deserialized.theme.mode, ThemeMode::Dark);
}

#[test]
fn test_config_equality() {
    let config1 = WebviewConfig::default();
    let config2 = WebviewConfig::default();

    assert_eq!(config1, config2);
}

#[test]
fn test_provider_config_with_optional_fields() {
    let minimal_provider = ProviderConfig {
        name: "minimal".to_string(),
        api_key: None,
        model: None,
        enabled: false,
        base_url: None,
    };

    let json = serde_json::to_string(&minimal_provider).unwrap();
    let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "minimal");
    assert_eq!(deserialized.api_key, None);
    assert!(!deserialized.enabled);
}
