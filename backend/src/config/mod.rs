//! Application configuration.
//!
//! Loads and validates all settings from environment variables at startup.
//! Uses strong typing to catch misconfiguration at compile time rather than runtime.

use std::env;
use std::sync::LazyLock;

/// Global application settings, loaded once at startup.
///
/// # Panics
///
/// Panics if required environment variables (`SUPABASE_URL`, `SUPABASE_SERVICE_KEY`)
/// are missing.
#[allow(clippy::expect_used)]
pub static SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    Settings::from_env().unwrap_or_else(|e| {
        tracing::error!("Failed to load settings: {e}");
        panic!("Failed to load settings: {e}");
    })
});

/// Top-level application settings.
#[derive(Debug, Clone)]
pub struct Settings {
    pub environment: Environment,
    pub server: ServerConfig,
    pub cors: CorsConfig,
    pub supabase: SupabaseConfig,
    pub llm: LlmConfig,
    pub qdrant: QdrantConfig,
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// Deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    #[default]
    Development,
    Staging,
    Production,
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Staging => write!(f, "staging"),
            Self::Production => write!(f, "production"),
        }
    }
}

impl std::str::FromStr for Environment {
    type Err = SettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(SettingsError::InvalidValue(format!(
                "Invalid environment: {s}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

/// HTTP server settings.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8000,
        }
    }
}

/// CORS origin allowlist.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub origins: Vec<String>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: vec!["http://localhost:3000".to_string()],
        }
    }
}

/// Supabase connection details (required).
#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub service_key: String,
    pub anon_key: Option<String>,
    pub jwt_secret: Option<String>,
}

/// LLM provider API keys (optional).
#[derive(Debug, Clone, Default)]
pub struct LlmConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
}

impl LlmConfig {
    /// Returns `true` if a non-empty `OpenAI` key is configured.
    pub fn has_openai(&self) -> bool {
        self.openai_api_key.as_ref().is_some_and(|k| !k.is_empty())
    }

    /// Returns `true` if a non-empty Anthropic key is configured.
    pub fn has_anthropic(&self) -> bool {
        self.anthropic_api_key
            .as_ref()
            .is_some_and(|k| !k.is_empty())
    }
}

/// Qdrant vector-database connection (optional).
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: Option<String>,
    pub api_key: Option<String>,
    pub collection_name: String,
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: None,
            api_key: None,
            collection_name: "default_collection".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl Settings {
    /// Build settings from environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError`] when required variables are missing or invalid.
    pub fn from_env() -> Result<Self, SettingsError> {
        let _ = dotenvy::dotenv(); // Load .env if present

        Ok(Self {
            environment: parse_environment(),
            server: parse_server()?,
            cors: parse_cors(),
            supabase: parse_supabase()?,
            llm: parse_llm(),
            qdrant: parse_qdrant(),
        })
    }
}

fn parse_environment() -> Environment {
    env::var("ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_string())
        .parse()
        .unwrap_or(Environment::Development)
}

fn parse_server() -> Result<ServerConfig, SettingsError> {
    Ok(ServerConfig {
        host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
        port: env::var("PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse()
            .map_err(|_| SettingsError::InvalidValue("PORT".to_string()))?,
    })
}

fn parse_cors() -> CorsConfig {
    CorsConfig {
        origins: env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

fn parse_supabase() -> Result<SupabaseConfig, SettingsError> {
    Ok(SupabaseConfig {
        url: required_env("SUPABASE_URL")?,
        service_key: required_env("SUPABASE_SERVICE_KEY")?,
        anon_key: optional_env("SUPABASE_ANON_KEY"),
        jwt_secret: optional_env("SUPABASE_JWT_SECRET"),
    })
}

fn parse_llm() -> LlmConfig {
    LlmConfig {
        openai_api_key: nonempty_env("OPENAI_API_KEY"),
        anthropic_api_key: nonempty_env("ANTHROPIC_API_KEY"),
    }
}

fn parse_qdrant() -> QdrantConfig {
    QdrantConfig {
        url: nonempty_env("QDRANT_URL"),
        api_key: nonempty_env("QDRANT_API_KEY"),
        collection_name: env::var("QDRANT_COLLECTION_NAME")
            .unwrap_or_else(|_| "default_collection".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Env-var helpers
// ---------------------------------------------------------------------------

fn required_env(key: &str) -> Result<String, SettingsError> {
    env::var(key).map_err(|_| SettingsError::MissingEnvVar(key.to_string()))
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key).ok()
}

fn nonempty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Configuration loading errors.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Environment --------------------------------------------------------

    #[test]
    fn environment_parses_full_names() {
        assert_eq!(
            "development".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "staging".parse::<Environment>().unwrap(),
            Environment::Staging
        );
        assert_eq!(
            "production".parse::<Environment>().unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn environment_parses_short_names() {
        assert_eq!(
            "dev".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "stage".parse::<Environment>().unwrap(),
            Environment::Staging
        );
        assert_eq!(
            "prod".parse::<Environment>().unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn environment_parses_case_insensitively() {
        assert_eq!(
            "DEVELOPMENT".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "Production".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert_eq!(
            "DEV".parse::<Environment>().unwrap(),
            Environment::Development
        );
    }

    #[test]
    fn environment_rejects_invalid_input() {
        assert!("invalid".parse::<Environment>().is_err());
        assert!("".parse::<Environment>().is_err());
        assert!("test".parse::<Environment>().is_err());
    }

    #[test]
    fn environment_display_roundtrips() {
        for env in [
            Environment::Development,
            Environment::Staging,
            Environment::Production,
        ] {
            assert_eq!(env.to_string().parse::<Environment>().unwrap(), env);
        }
    }

    #[test]
    fn environment_default_is_development() {
        assert_eq!(Environment::default(), Environment::Development);
    }

    // -- Defaults -----------------------------------------------------------

    #[test]
    fn server_config_default_values() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 8000);
    }

    #[test]
    fn cors_config_default_allows_localhost() {
        let cfg = CorsConfig::default();
        assert_eq!(cfg.origins, vec!["http://localhost:3000"]);
    }

    #[test]
    fn qdrant_config_default_values() {
        let cfg = QdrantConfig::default();
        assert!(cfg.url.is_none());
        assert!(cfg.api_key.is_none());
        assert_eq!(cfg.collection_name, "default_collection");
    }

    #[test]
    fn llm_config_default_is_empty() {
        let cfg = LlmConfig::default();
        assert!(!cfg.has_openai());
        assert!(!cfg.has_anthropic());
    }

    // -- LlmConfig ----------------------------------------------------------

    #[test]
    fn llm_config_detects_openai_presence() {
        let with_key = LlmConfig {
            openai_api_key: Some("sk-test".into()),
            ..LlmConfig::default()
        };
        let empty_key = LlmConfig {
            openai_api_key: Some(String::new()),
            ..LlmConfig::default()
        };
        let no_key = LlmConfig::default();

        assert!(with_key.has_openai());
        assert!(!empty_key.has_openai());
        assert!(!no_key.has_openai());
    }

    #[test]
    fn llm_config_detects_anthropic_presence() {
        let with_key = LlmConfig {
            anthropic_api_key: Some("sk-ant".into()),
            ..LlmConfig::default()
        };
        let empty_key = LlmConfig {
            anthropic_api_key: Some(String::new()),
            ..LlmConfig::default()
        };
        let no_key = LlmConfig::default();

        assert!(with_key.has_anthropic());
        assert!(!empty_key.has_anthropic());
        assert!(!no_key.has_anthropic());
    }

    // -- SettingsError ------------------------------------------------------

    #[test]
    fn settings_error_displays_correctly() {
        assert_eq!(
            SettingsError::MissingEnvVar("KEY".into()).to_string(),
            "Missing required environment variable: KEY",
        );
        assert_eq!(
            SettingsError::InvalidValue("bad".into()).to_string(),
            "Invalid configuration value: bad",
        );
    }
}
