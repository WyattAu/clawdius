//! Internationalization (i18n) support for Clawdius
//!
//! This module provides multi-language support with:
//! - Lazy loading of translation files
//! - Pluralization support
//! - Variable interpolation
//! - Fallback to English for missing translations

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    Chinese,
    Japanese,
    Korean,
    German,
    French,
    Spanish,
    Italian,
    Portuguese,
    Russian,
}

impl Language {
    /// Get language code (ISO 639-1)
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Chinese => "zh",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::Italian => "it",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
        }
    }

    /// Get language name in English
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "Chinese",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::German => "German",
            Self::French => "French",
            Self::Spanish => "Spanish",
            Self::Italian => "Italian",
            Self::Portuguese => "Portuguese",
            Self::Russian => "Russian",
        }
    }

    /// Get language name in its native form
    #[must_use]
    pub fn native_name(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "中文",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::German => "Deutsch",
            Self::French => "Français",
            Self::Spanish => "Español",
            Self::Italian => "Italiano",
            Self::Portuguese => "Português",
            Self::Russian => "Русский",
        }
    }

    /// Parse from ISO 639-1 code
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "en" | "eng" => Some(Self::English),
            "zh" | "zho" | "chi" | "cn" => Some(Self::Chinese),
            "ja" | "jpn" | "jp" => Some(Self::Japanese),
            "ko" | "kor" | "kr" => Some(Self::Korean),
            "de" | "deu" | "ger" => Some(Self::German),
            "fr" | "fra" | "fre" => Some(Self::French),
            "es" | "spa" => Some(Self::Spanish),
            "it" | "ita" => Some(Self::Italian),
            "pt" | "por" => Some(Self::Portuguese),
            "ru" | "rus" => Some(Self::Russian),
            _ => None,
        }
    }

    /// Detect language from system locale
    #[must_use]
    pub fn detect() -> Self {
        // Try to get system language from environment
        if let Ok(lang) = std::env::var("LANG") {
            if let Some(detected) = Self::from_code(&lang[..2.min(lang.len())]) {
                return detected;
            }
        }
        if let Ok(lang) = std::env::var("LC_ALL") {
            if let Some(detected) = Self::from_code(&lang[..2.min(lang.len())]) {
                return detected;
            }
        }
        if let Ok(lang) = std::env::var("LC_MESSAGES") {
            if let Some(detected) = Self::from_code(&lang[..2.min(lang.len())]) {
                return detected;
            }
        }
        Self::default()
    }

    /// List all supported languages
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::English,
            Self::Chinese,
            Self::Japanese,
            Self::Korean,
            Self::German,
            Self::French,
            Self::Spanish,
            Self::Italian,
            Self::Portuguese,
            Self::Russian,
        ]
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.native_name(), self.name())
    }
}

/// Translation entry with optional plural forms
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationEntry {
    /// Single form or zero/one form for plurals
    #[serde(default)]
    pub one: Option<String>,
    /// Other/multiple form for plurals
    #[serde(default)]
    pub other: Option<String>,
    /// Simple translation (equivalent to one)
    #[serde(default)]
    pub message: Option<String>,
}

impl TranslationEntry {
    /// Get translation for count
    #[must_use]
    pub fn get(&self, count: Option<usize>) -> Option<&str> {
        match count {
            Some(1) => self.one.as_deref().or(self.message.as_deref()),
            Some(_) => self.other.as_deref().or(self.message.as_deref()),
            None => self.message.as_deref().or(self.one.as_deref()),
        }
    }
}

/// Translation file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranslationFile {
    /// Language code
    pub language: String,
    /// Translations map
    #[serde(flatten)]
    pub translations: HashMap<String, TranslationEntry>,
}

/// Global localization state
static LOCALIZATION: Lazy<RwLock<Localization>> = Lazy::new(|| {
    let lang = Language::detect();
    RwLock::new(Localization::new(lang))
});

/// Localization manager
pub struct Localization {
    current: Language,
    translations: HashMap<Language, TranslationFile>,
    loaded: bool,
}

impl Localization {
    /// Create new localization manager
    #[must_use]
    pub fn new(lang: Language) -> Self {
        Self {
            current: lang,
            translations: HashMap::new(),
            loaded: false,
        }
    }

    /// Load translations from embedded data
    pub fn load_embedded(&mut self) -> Result<()> {
        // Load embedded translations
        self.translations.insert(Language::English, load_english());
        self.translations.insert(Language::Chinese, load_chinese());
        self.translations
            .insert(Language::Japanese, load_japanese());
        self.translations.insert(Language::German, load_german());
        self.translations.insert(Language::French, load_french());
        self.loaded = true;
        Ok(())
    }

    /// Load translations from directory
    pub fn load_from_dir(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().is_some_and(|e| e == "json") {
                if let Some(lang_code) = file_path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(lang) = Language::from_code(lang_code) {
                        let content = std::fs::read_to_string(&file_path).with_context(|| {
                            format!("Failed to read translation file: {}", file_path.display())
                        })?;
                        let translation: TranslationFile = serde_json::from_str(&content)
                            .with_context(|| {
                                format!("Failed to parse translation file: {}", file_path.display())
                            })?;
                        self.translations.insert(lang, translation);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get translation for key
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.tn(key, None, &HashMap::new())
    }

    /// Get translation with interpolation
    #[must_use]
    pub fn ti(&self, key: &str, vars: &HashMap<&str, &str>) -> String {
        self.tn(key, None, vars)
    }

    /// Get translation with count (pluralization)
    #[must_use]
    pub fn tc(&self, key: &str, count: usize) -> String {
        let count_str = count.to_string();
        let mut vars = HashMap::new();
        vars.insert("count", count_str.as_str());
        self.tn(key, Some(count), &vars)
    }

    /// Get translation with count and interpolation
    #[must_use]
    pub fn tcn(&self, key: &str, count: usize, vars: &HashMap<&str, &str>) -> String {
        let count_str = count.to_string();
        let mut all_vars = HashMap::new();
        for (k, v) in vars {
            all_vars.insert(*k, *v);
        }
        all_vars.insert("count", count_str.as_str());
        self.tn(key, Some(count), &all_vars)
    }

    /// Internal translation function
    fn tn(&self, key: &str, count: Option<usize>, vars: &HashMap<&str, &str>) -> String {
        // Try current language first
        let result = self
            .translations
            .get(&self.current)
            .and_then(|t| t.translations.get(key))
            .and_then(|e| e.get(count).map(|s| s.to_string()));

        // Fall back to English
        let result = result.or_else(|| {
            self.translations
                .get(&Language::English)
                .and_then(|t| t.translations.get(key))
                .and_then(|e| e.get(count).map(|s| s.to_string()))
        });

        // Fall back to key
        let result = result.unwrap_or_else(|| key.to_string());

        // Interpolate variables
        self.interpolate(&result, vars)
    }

    /// Interpolate variables in string
    fn interpolate(&self, text: &str, vars: &HashMap<&str, &str>) -> String {
        let mut result = text.to_string();
        for (key, value) in vars {
            result = result.replace(&format!("{{{}}}", key), value);
            result = result.replace(&format!("{{{}:uppercase}}", key), &value.to_uppercase());
            result = result.replace(&format!("{{{}:lowercase}}", key), &value.to_lowercase());
        }
        result
    }

    /// Set current language
    pub fn set_language(&mut self, lang: Language) {
        self.current = lang;
    }

    /// Get current language
    #[must_use]
    pub fn language(&self) -> Language {
        self.current
    }

    /// Check if translations are loaded
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}

/// Get the global localization instance
#[must_use]
pub fn localization() -> &'static RwLock<Localization> {
    &LOCALIZATION
}

/// Initialize i18n with embedded translations
pub fn init() -> Result<()> {
    let mut loc = LOCALIZATION
        .write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire i18n lock: {}", e))?;
    loc.load_embedded()
}

/// Get translation for key (convenience function)
#[must_use]
pub fn t(key: &str) -> String {
    match LOCALIZATION.read() {
        Ok(loc) => loc.t(key),
        Err(_) => key.to_string(),
    }
}

/// Get translation with count (convenience function)
#[must_use]
pub fn tc(key: &str, count: usize) -> String {
    match LOCALIZATION.read() {
        Ok(loc) => loc.tc(key, count),
        Err(_) => key.to_string(),
    }
}

/// Get translation with interpolation (convenience function)
#[must_use]
pub fn ti(key: &str, vars: &HashMap<&str, &str>) -> String {
    match LOCALIZATION.read() {
        Ok(loc) => loc.ti(key, vars),
        Err(_) => key.to_string(),
    }
}

// Embedded translations

fn load_english() -> TranslationFile {
    TranslationFile {
        language: "en".to_string(),
        translations: [
            // General
            (
                "app.name",
                TranslationEntry {
                    message: Some("Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.tagline",
                TranslationEntry {
                    message: Some("High-Assurance Rust-Native Engineering Engine".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.version",
                TranslationEntry {
                    message: Some("Version".to_string()),
                    ..Default::default()
                },
            ),
            // Commands
            (
                "cmd.chat",
                TranslationEntry {
                    message: Some("Send a chat message to the LLM".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.init",
                TranslationEntry {
                    message: Some("Initialize Clawdius in a project".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.sessions",
                TranslationEntry {
                    message: Some("List and manage sessions".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.auto",
                TranslationEntry {
                    message: Some("Autonomous CI/CD mode - run without interaction".to_string()),
                    ..Default::default()
                },
            ),
            // Chat
            (
                "chat.thinking",
                TranslationEntry {
                    message: Some("Thinking...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.session",
                TranslationEntry {
                    message: Some("Session".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.provider",
                TranslationEntry {
                    message: Some("Provider".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.mode",
                TranslationEntry {
                    message: Some("Mode".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.tokens",
                TranslationEntry {
                    message: Some("Tokens".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.duration",
                TranslationEntry {
                    message: Some("Duration".to_string()),
                    ..Default::default()
                },
            ),
            // Auto mode
            (
                "auto.title",
                TranslationEntry {
                    message: Some("Clawdius Auto Mode".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.task",
                TranslationEntry {
                    message: Some("Task".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.max_iterations",
                TranslationEntry {
                    message: Some("Max iterations".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_enabled",
                TranslationEntry {
                    message: Some("Tests: enabled".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.auto_commit",
                TranslationEntry {
                    message: Some("Auto-commit: enabled".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.running_tests",
                TranslationEntry {
                    message: Some("Running tests...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_passed",
                TranslationEntry {
                    message: Some("Tests passed".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_failed",
                TranslationEntry {
                    message: Some("Tests failed".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committing",
                TranslationEntry {
                    message: Some("Committing changes...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committed",
                TranslationEntry {
                    message: Some("Changes committed".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.working",
                TranslationEntry {
                    message: Some("Working...".to_string()),
                    ..Default::default()
                },
            ),
            // Init
            (
                "init.success",
                TranslationEntry {
                    message: Some("Clawdius initialized successfully".to_string()),
                    ..Default::default()
                },
            ),
            (
                "init.config_created",
                TranslationEntry {
                    message: Some("Configuration file created".to_string()),
                    ..Default::default()
                },
            ),
            (
                "init.dirs_created",
                TranslationEntry {
                    message: Some("Directory structure created".to_string()),
                    ..Default::default()
                },
            ),
            // Errors
            (
                "error.generic",
                TranslationEntry {
                    message: Some("An error occurred".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.config_load",
                TranslationEntry {
                    message: Some("Failed to load configuration".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.api_key_missing",
                TranslationEntry {
                    message: Some("API key not configured".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.provider_not_found",
                TranslationEntry {
                    message: Some("Provider not found".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.session_not_found",
                TranslationEntry {
                    message: Some("Session not found".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.file_not_found",
                TranslationEntry {
                    message: Some("File not found".to_string()),
                    ..Default::default()
                },
            ),
            // Plurals
            (
                "files.changed",
                TranslationEntry {
                    one: Some("1 file changed".to_string()),
                    other: Some("{count} files changed".to_string()),
                    ..Default::default()
                },
            ),
            (
                "sessions.count",
                TranslationEntry {
                    one: Some("1 session".to_string()),
                    other: Some("{count} sessions".to_string()),
                    ..Default::default()
                },
            ),
            (
                "iterations.count",
                TranslationEntry {
                    one: Some("1 iteration".to_string()),
                    other: Some("{count} iterations".to_string()),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    }
}

fn load_chinese() -> TranslationFile {
    TranslationFile {
        language: "zh".to_string(),
        translations: [
            // General
            (
                "app.name",
                TranslationEntry {
                    message: Some("Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.tagline",
                TranslationEntry {
                    message: Some("高保障Rust原生工程引擎".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.version",
                TranslationEntry {
                    message: Some("版本".to_string()),
                    ..Default::default()
                },
            ),
            // Commands
            (
                "cmd.chat",
                TranslationEntry {
                    message: Some("向LLM发送聊天消息".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.init",
                TranslationEntry {
                    message: Some("在项目中初始化Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.sessions",
                TranslationEntry {
                    message: Some("列出和管理会话".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.auto",
                TranslationEntry {
                    message: Some("自主CI/CD模式 - 无交互运行".to_string()),
                    ..Default::default()
                },
            ),
            // Chat
            (
                "chat.thinking",
                TranslationEntry {
                    message: Some("思考中...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.session",
                TranslationEntry {
                    message: Some("会话".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.provider",
                TranslationEntry {
                    message: Some("提供商".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.mode",
                TranslationEntry {
                    message: Some("模式".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.tokens",
                TranslationEntry {
                    message: Some("令牌".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.duration",
                TranslationEntry {
                    message: Some("持续时间".to_string()),
                    ..Default::default()
                },
            ),
            // Auto mode
            (
                "auto.title",
                TranslationEntry {
                    message: Some("Clawdius自动模式".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.task",
                TranslationEntry {
                    message: Some("任务".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.max_iterations",
                TranslationEntry {
                    message: Some("最大迭代次数".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_enabled",
                TranslationEntry {
                    message: Some("测试：已启用".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.auto_commit",
                TranslationEntry {
                    message: Some("自动提交：已启用".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.running_tests",
                TranslationEntry {
                    message: Some("正在运行测试...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_passed",
                TranslationEntry {
                    message: Some("测试通过".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_failed",
                TranslationEntry {
                    message: Some("测试失败".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committing",
                TranslationEntry {
                    message: Some("正在提交更改...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committed",
                TranslationEntry {
                    message: Some("更改已提交".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.working",
                TranslationEntry {
                    message: Some("工作中...".to_string()),
                    ..Default::default()
                },
            ),
            // Init
            (
                "init.success",
                TranslationEntry {
                    message: Some("Clawdius初始化成功".to_string()),
                    ..Default::default()
                },
            ),
            (
                "init.config_created",
                TranslationEntry {
                    message: Some("配置文件已创建".to_string()),
                    ..Default::default()
                },
            ),
            (
                "init.dirs_created",
                TranslationEntry {
                    message: Some("目录结构已创建".to_string()),
                    ..Default::default()
                },
            ),
            // Errors
            (
                "error.generic",
                TranslationEntry {
                    message: Some("发生错误".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.config_load",
                TranslationEntry {
                    message: Some("加载配置失败".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.api_key_missing",
                TranslationEntry {
                    message: Some("API密钥未配置".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.provider_not_found",
                TranslationEntry {
                    message: Some("未找到提供商".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.session_not_found",
                TranslationEntry {
                    message: Some("未找到会话".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.file_not_found",
                TranslationEntry {
                    message: Some("未找到文件".to_string()),
                    ..Default::default()
                },
            ),
            // Plurals
            (
                "files.changed",
                TranslationEntry {
                    one: Some("1个文件已更改".to_string()),
                    other: Some("{count}个文件已更改".to_string()),
                    ..Default::default()
                },
            ),
            (
                "sessions.count",
                TranslationEntry {
                    one: Some("1个会话".to_string()),
                    other: Some("{count}个会话".to_string()),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    }
}

fn load_japanese() -> TranslationFile {
    TranslationFile {
        language: "ja".to_string(),
        translations: [
            // General
            (
                "app.name",
                TranslationEntry {
                    message: Some("Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.tagline",
                TranslationEntry {
                    message: Some("高信頼Rustネイティブエンジニアリングエンジン".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.version",
                TranslationEntry {
                    message: Some("バージョン".to_string()),
                    ..Default::default()
                },
            ),
            // Commands
            (
                "cmd.chat",
                TranslationEntry {
                    message: Some("LLMにチャットメッセージを送信".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.init",
                TranslationEntry {
                    message: Some("プロジェクトでClawdiusを初期化".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.sessions",
                TranslationEntry {
                    message: Some("セッションの一覧と管理".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.auto",
                TranslationEntry {
                    message: Some("自律CI/CDモード - 対話なしで実行".to_string()),
                    ..Default::default()
                },
            ),
            // Chat
            (
                "chat.thinking",
                TranslationEntry {
                    message: Some("思考中...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.session",
                TranslationEntry {
                    message: Some("セッション".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.provider",
                TranslationEntry {
                    message: Some("プロバイダー".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.mode",
                TranslationEntry {
                    message: Some("モード".to_string()),
                    ..Default::default()
                },
            ),
            // Auto mode
            (
                "auto.title",
                TranslationEntry {
                    message: Some("Clawdius自動モード".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.task",
                TranslationEntry {
                    message: Some("タスク".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.running_tests",
                TranslationEntry {
                    message: Some("テストを実行中...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_passed",
                TranslationEntry {
                    message: Some("テストが成功しました".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_failed",
                TranslationEntry {
                    message: Some("テストが失敗しました".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committing",
                TranslationEntry {
                    message: Some("変更をコミット中...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committed",
                TranslationEntry {
                    message: Some("変更がコミットされました".to_string()),
                    ..Default::default()
                },
            ),
            // Errors
            (
                "error.generic",
                TranslationEntry {
                    message: Some("エラーが発生しました".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.config_load",
                TranslationEntry {
                    message: Some("設定の読み込みに失敗しました".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.api_key_missing",
                TranslationEntry {
                    message: Some("APIキーが設定されていません".to_string()),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    }
}

fn load_german() -> TranslationFile {
    TranslationFile {
        language: "de".to_string(),
        translations: [
            // General
            (
                "app.name",
                TranslationEntry {
                    message: Some("Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.tagline",
                TranslationEntry {
                    message: Some("Hochsichere Rust-native Engineering-Engine".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.version",
                TranslationEntry {
                    message: Some("Version".to_string()),
                    ..Default::default()
                },
            ),
            // Commands
            (
                "cmd.chat",
                TranslationEntry {
                    message: Some("Chat-Nachricht an LLM senden".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.init",
                TranslationEntry {
                    message: Some("Clawdius in einem Projekt initialisieren".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.sessions",
                TranslationEntry {
                    message: Some("Sitzungen auflisten und verwalten".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.auto",
                TranslationEntry {
                    message: Some("Autonomer CI/CD-Modus - ohne Interaktion ausführen".to_string()),
                    ..Default::default()
                },
            ),
            // Chat
            (
                "chat.thinking",
                TranslationEntry {
                    message: Some("Denke nach...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.session",
                TranslationEntry {
                    message: Some("Sitzung".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.provider",
                TranslationEntry {
                    message: Some("Anbieter".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.mode",
                TranslationEntry {
                    message: Some("Modus".to_string()),
                    ..Default::default()
                },
            ),
            // Auto mode
            (
                "auto.title",
                TranslationEntry {
                    message: Some("Clawdius Auto-Modus".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.task",
                TranslationEntry {
                    message: Some("Aufgabe".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.running_tests",
                TranslationEntry {
                    message: Some("Tests werden ausgeführt...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_passed",
                TranslationEntry {
                    message: Some("Tests bestanden".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_failed",
                TranslationEntry {
                    message: Some("Tests fehlgeschlagen".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committing",
                TranslationEntry {
                    message: Some("Änderungen werden committet...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committed",
                TranslationEntry {
                    message: Some("Änderungen committet".to_string()),
                    ..Default::default()
                },
            ),
            // Errors
            (
                "error.generic",
                TranslationEntry {
                    message: Some("Ein Fehler ist aufgetreten".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.config_load",
                TranslationEntry {
                    message: Some("Konfiguration konnte nicht geladen werden".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.api_key_missing",
                TranslationEntry {
                    message: Some("API-Schlüssel nicht konfiguriert".to_string()),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    }
}

fn load_french() -> TranslationFile {
    TranslationFile {
        language: "fr".to_string(),
        translations: [
            // General
            (
                "app.name",
                TranslationEntry {
                    message: Some("Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.tagline",
                TranslationEntry {
                    message: Some("Moteur d'ingénierie Rust natif haute assurance".to_string()),
                    ..Default::default()
                },
            ),
            (
                "app.version",
                TranslationEntry {
                    message: Some("Version".to_string()),
                    ..Default::default()
                },
            ),
            // Commands
            (
                "cmd.chat",
                TranslationEntry {
                    message: Some("Envoyer un message de chat au LLM".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.init",
                TranslationEntry {
                    message: Some("Initialiser Clawdius dans un projet".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.sessions",
                TranslationEntry {
                    message: Some("Lister et gérer les sessions".to_string()),
                    ..Default::default()
                },
            ),
            (
                "cmd.auto",
                TranslationEntry {
                    message: Some("Mode CI/CD autonome - exécuter sans interaction".to_string()),
                    ..Default::default()
                },
            ),
            // Chat
            (
                "chat.thinking",
                TranslationEntry {
                    message: Some("Réflexion...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.session",
                TranslationEntry {
                    message: Some("Session".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.provider",
                TranslationEntry {
                    message: Some("Fournisseur".to_string()),
                    ..Default::default()
                },
            ),
            (
                "chat.mode",
                TranslationEntry {
                    message: Some("Mode".to_string()),
                    ..Default::default()
                },
            ),
            // Auto mode
            (
                "auto.title",
                TranslationEntry {
                    message: Some("Mode automatique Clawdius".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.task",
                TranslationEntry {
                    message: Some("Tâche".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.running_tests",
                TranslationEntry {
                    message: Some("Exécution des tests...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_passed",
                TranslationEntry {
                    message: Some("Tests réussis".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.tests_failed",
                TranslationEntry {
                    message: Some("Tests échoués".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committing",
                TranslationEntry {
                    message: Some("Validation des modifications...".to_string()),
                    ..Default::default()
                },
            ),
            (
                "auto.committed",
                TranslationEntry {
                    message: Some("Modifications validées".to_string()),
                    ..Default::default()
                },
            ),
            // Errors
            (
                "error.generic",
                TranslationEntry {
                    message: Some("Une erreur s'est produite".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.config_load",
                TranslationEntry {
                    message: Some("Échec du chargement de la configuration".to_string()),
                    ..Default::default()
                },
            ),
            (
                "error.api_key_missing",
                TranslationEntry {
                    message: Some("Clé API non configurée".to_string()),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_code() {
        assert_eq!(Language::from_code("en"), Some(Language::English));
        assert_eq!(Language::from_code("zh"), Some(Language::Chinese));
        assert_eq!(Language::from_code("ja"), Some(Language::Japanese));
        assert_eq!(Language::from_code("invalid"), None);
    }

    #[test]
    fn test_translation() {
        let mut loc = Localization::new(Language::English);
        loc.load_embedded().unwrap();

        assert_eq!(loc.t("app.name"), "Clawdius");
        assert_eq!(loc.t("chat.thinking"), "Thinking...");
    }

    #[test]
    fn test_chinese_translation() {
        let mut loc = Localization::new(Language::Chinese);
        loc.load_embedded().unwrap();

        assert_eq!(loc.t("chat.thinking"), "思考中...");
        assert_eq!(loc.t("auto.tests_passed"), "测试通过");
    }

    #[test]
    fn test_pluralization() {
        let mut loc = Localization::new(Language::English);
        loc.load_embedded().unwrap();

        assert_eq!(loc.tc("files.changed", 1), "1 file changed");
        assert_eq!(loc.tc("files.changed", 5), "5 files changed");
    }

    #[test]
    fn test_interpolation() {
        let mut loc = Localization::new(Language::English);
        loc.load_embedded().unwrap();

        // ti doesn't do pluralization - it just interpolates variables
        // For pluralization, use tc instead
        let mut vars = HashMap::new();
        vars.insert("count", "5");
        // ti uses the 'one' or 'message' form and just replaces {count}
        assert_eq!(loc.ti("files.changed", &vars), "1 file changed");

        // tc does pluralization based on count
        assert_eq!(loc.tc("files.changed", 5), "5 files changed");
    }

    #[test]
    fn test_fallback() {
        let mut loc = Localization::new(Language::Korean); // Korean has no translations embedded
        loc.load_embedded().unwrap();

        // Should fall back to English
        assert_eq!(loc.t("app.name"), "Clawdius");
    }
}
