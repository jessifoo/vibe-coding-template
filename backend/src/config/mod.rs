//! Application configuration module.
//!
//! Handles loading and validating all environment variables and configuration
//! settings. Uses strong typing to prevent misconfiguration at runtime.

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::env;

/// Global application settings, loaded once at startup.
pub static SETTINGS: Lazy<Settings> = Lazy::new(|| {
    Settings::from_env().unwrap_or_else(|e| {
        tracing::error!("Failed to load settings: {}", e);
        panic!("Failed to load settings: {}", e);
    })
});

/// Application settings loaded from environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Application environment (development, staging, production)
    pub environment: Environment,

    /// Server configuration
    pub server: ServerConfig,

    /// CORS configuration
    pub cors: CorsConfig,

    /// Supabase configuration
    pub supabase: SupabaseConfig,

    /// LLM provider configuration
    pub llm: LlmConfig,

    /// Qdrant vector database configuration
    pub qdrant: QdrantConfig,
}

/// Application environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
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

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
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

/// CORS configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins for CORS
    pub origins: Vec<String>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: vec!["http://localhost:3000".to_string()],
        }
    }
}

/// Supabase configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SupabaseConfig {
    /// Supabase project URL
    pub url: String,

    /// Supabase service role key (for backend operations)
    pub service_key: String,

    /// Supabase anon key (for client operations)
    pub anon_key: Option<String>,

    /// JWT secret for token verification
    pub jwt_secret: Option<String>,
}

/// LLM provider configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LlmConfig {
    /// OpenAI API key
    pub openai_api_key: Option<String>,

    /// Anthropic API key
    pub anthropic_api_key: Option<String>,
}

impl LlmConfig {
    /// Check if OpenAI is configured.
    #[must_use]
    pub fn has_openai(&self) -> bool {
        self.openai_api_key.as_ref().is_some_and(|k| !k.is_empty())
    }

    /// Check if Anthropic is configured.
    #[must_use]
    pub fn has_anthropic(&self) -> bool {
        self.anthropic_api_key
            .as_ref()
            .is_some_and(|k| !k.is_empty())
    }
}

/// Qdrant vector database configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct QdrantConfig {
    /// Qdrant server URL
    pub url: Option<String>,

    /// Qdrant API key
    pub api_key: Option<String>,

    /// Default collection name
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

impl Settings {
    /// Load settings from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing or invalid.
    pub fn from_env() -> Result<Self, SettingsError> {
        // Load .env file if present (ignore errors if not found)
        let _ = dotenvy::dotenv();

        // Parse environment
        let environment = env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string())
            .parse()
            .unwrap_or(Environment::Development);

        // Parse server config
        let server = ServerConfig {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .map_err(|_| SettingsError::InvalidValue("PORT".to_string()))?,
        };

        // Parse CORS origins
        let cors = CorsConfig {
            origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        };

        // Parse Supabase config (required)
        let supabase = SupabaseConfig {
            url: env::var("SUPABASE_URL")
                .map_err(|_| SettingsError::MissingEnvVar("SUPABASE_URL".to_string()))?,
            service_key: env::var("SUPABASE_SERVICE_KEY")
                .map_err(|_| SettingsError::MissingEnvVar("SUPABASE_SERVICE_KEY".to_string()))?,
            anon_key: env::var("SUPABASE_ANON_KEY").ok(),
            jwt_secret: env::var("SUPABASE_JWT_SECRET").ok(),
        };

        // Parse LLM config (optional)
        let llm = LlmConfig {
            openai_api_key: env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty()),
        };

        // Parse Qdrant config (optional)
        let qdrant = QdrantConfig {
            url: env::var("QDRANT_URL").ok().filter(|s| !s.is_empty()),
            api_key: env::var("QDRANT_API_KEY").ok().filter(|s| !s.is_empty()),
            collection_name: env::var("QDRANT_COLLECTION_NAME")
                .unwrap_or_else(|_| "default_collection".to_string()),
        };

        Ok(Self {
            environment,
            server,
            cors,
            supabase,
            llm,
            qdrant,
        })
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

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_parsing_full_names() {
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
    fn test_environment_parsing_short_names() {
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
    fn test_environment_parsing_case_insensitive() {
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
    fn test_environment_parsing_invalid() {
        assert!("invalid".parse::<Environment>().is_err());
        assert!("".parse::<Environment>().is_err());
        assert!("test".parse::<Environment>().is_err());
    }

    #[test]
    fn test_environment_display() {
        assert_eq!(Environment::Development.to_string(), "development");
        assert_eq!(Environment::Staging.to_string(), "staging");
        assert_eq!(Environment::Production.to_string(), "production");
    }

    #[test]
    fn test_environment_default() {
        assert_eq!(Environment::default(), Environment::Development);
    }

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8000);
    }

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();
        assert_eq!(config.origins, vec!["http://localhost:3000".to_string()]);
    }

    #[test]
    fn test_qdrant_config_default() {
        let config = QdrantConfig::default();
        assert!(config.url.is_none());
        assert!(config.api_key.is_none());
        assert_eq!(config.collection_name, "default_collection");
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(config.openai_api_key.is_none());
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn test_llm_config_has_openai_none() {
        let config = LlmConfig {
            openai_api_key: None,
            anthropic_api_key: None,
        };
        assert!(!config.has_openai());
    }

    #[test]
    fn test_llm_config_has_openai_empty() {
        let config = LlmConfig {
            openai_api_key: Some(String::new()),
            anthropic_api_key: None,
        };
        assert!(!config.has_openai());
    }

    #[test]
    fn test_llm_config_has_openai_set() {
        let config = LlmConfig {
            openai_api_key: Some("sk-test".to_string()),
            anthropic_api_key: None,
        };
        assert!(config.has_openai());
    }

    #[test]
    fn test_llm_config_has_anthropic_none() {
        let config = LlmConfig {
            openai_api_key: None,
            anthropic_api_key: None,
        };
        assert!(!config.has_anthropic());
    }

    #[test]
    fn test_llm_config_has_anthropic_empty() {
        let config = LlmConfig {
            openai_api_key: None,
            anthropic_api_key: Some(String::new()),
        };
        assert!(!config.has_anthropic());
    }

    #[test]
    fn test_llm_config_has_anthropic_set() {
        let config = LlmConfig {
            openai_api_key: None,
            anthropic_api_key: Some("sk-ant-test".to_string()),
        };
        assert!(config.has_anthropic());
    }

    #[test]
    fn test_settings_error_display() {
        let err = SettingsError::MissingEnvVar("TEST_KEY".to_string());
        assert_eq!(
            err.to_string(),
            "Missing required environment variable: TEST_KEY"
        );

        let err = SettingsError::InvalidValue("bad port".to_string());
        assert_eq!(err.to_string(), "Invalid configuration value: bad port");
    }

    #[test]
    fn test_environment_equality() {
        assert_eq!(Environment::Development, Environment::Development);
        assert_ne!(Environment::Development, Environment::Production);
        assert_ne!(Environment::Staging, Environment::Production);
    }
}
