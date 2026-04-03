//! CADMATIC adapter configuration.

use thiserror::Error;

/// Errors during configuration building.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Required field '{0}' is missing")]
    MissingField(&'static str),
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, String),
}

/// Authentication method for the CADMATIC REST API.
#[derive(Debug, Clone)]
pub enum CadmaticAuth {
    /// HTTP Basic authentication (username + password).
    Basic { username: String, password: String },
    /// Bearer token authentication.
    Bearer(String),
    /// No authentication (for development/local instances).
    None,
}

impl CadmaticAuth {
    /// Returns the `Authorization` header value.
    pub fn header_value(&self) -> Option<String> {
        match self {
            Self::Basic { username, password } => {
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{password}"));
                Some(format!("Basic {encoded}"))
            }
            Self::Bearer(token) => Some(format!("Bearer {token}")),
            Self::None => None,
        }
    }
}

/// CADMATIC API version string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadmaticApiVersion {
    V1,
    V2,
}

impl CadmaticApiVersion {
    pub fn path_prefix(&self) -> &'static str {
        match self {
            Self::V1 => "/api/v1",
            Self::V2 => "/api/v2",
        }
    }
}

/// Configuration for the CADMATIC adapter.
#[derive(Debug, Clone)]
pub struct CadmaticConfig {
    /// Base URL of the CADMATIC REST API (e.g. `http://cadmatic-server:8080`).
    pub base_url: String,
    /// CADMATIC project identifier.
    pub project_id: String,
    /// Short PMEF project code for @id generation (e.g. `"eaf-2026"`).
    pub project_code: String,
    /// Authentication method.
    pub auth: CadmaticAuth,
    /// API version. Default: V1.
    pub api_version: CadmaticApiVersion,
    /// HTTP request timeout in seconds. Default: 30.
    pub timeout_secs: u64,
    /// Maximum number of retry attempts on transient errors. Default: 3.
    pub max_retries: u32,
    /// Fetch 3DDX geometry during export. Default: false (semantic data only).
    pub include_geometry: bool,
    /// Batch size for component fetching. Default: 200.
    pub component_batch_size: usize,
    /// Unit area / process unit ID for isPartOf references.
    pub unit_area: String,
}

impl CadmaticConfig {
    /// Create a builder.
    pub fn builder() -> CadmaticConfigBuilder {
        CadmaticConfigBuilder::default()
    }

    /// Construct the full URL for an API path.
    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}{}{path}",
            self.base_url.trim_end_matches('/'),
            self.api_version.path_prefix()
        )
    }
}

/// Builder for [`CadmaticConfig`].
#[derive(Debug, Default)]
pub struct CadmaticConfigBuilder {
    base_url: Option<String>,
    project_id: Option<String>,
    project_code: Option<String>,
    auth: Option<CadmaticAuth>,
    api_version: Option<CadmaticApiVersion>,
    timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    include_geometry: Option<bool>,
    component_batch_size: Option<usize>,
    unit_area: Option<String>,
}

impl CadmaticConfigBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into()); self
    }
    pub fn project_id(mut self, id: impl Into<String>) -> Self {
        self.project_id = Some(id.into()); self
    }
    pub fn project_code(mut self, code: impl Into<String>) -> Self {
        self.project_code = Some(code.into()); self
    }
    pub fn bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(CadmaticAuth::Bearer(token.into())); self
    }
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(CadmaticAuth::Basic { username: username.into(), password: password.into() }); self
    }
    pub fn no_auth(mut self) -> Self {
        self.auth = Some(CadmaticAuth::None); self
    }
    pub fn api_version(mut self, v: CadmaticApiVersion) -> Self {
        self.api_version = Some(v); self
    }
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs); self
    }
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = Some(n); self
    }
    pub fn include_geometry(mut self, yes: bool) -> Self {
        self.include_geometry = Some(yes); self
    }
    pub fn component_batch_size(mut self, n: usize) -> Self {
        self.component_batch_size = Some(n); self
    }
    pub fn unit_area(mut self, area: impl Into<String>) -> Self {
        self.unit_area = Some(area.into()); self
    }

    pub fn build(self) -> Result<CadmaticConfig, ConfigError> {
        let base_url = self.base_url.ok_or(ConfigError::MissingField("base_url"))?;
        let project_id = self.project_id.ok_or(ConfigError::MissingField("project_id"))?;
        let project_code = self.project_code
            .unwrap_or_else(|| project_id.to_lowercase().replace(' ', "-"));

        Ok(CadmaticConfig {
            base_url,
            project_id: project_id.clone(),
            project_code,
            auth: self.auth.unwrap_or(CadmaticAuth::None),
            api_version: self.api_version.unwrap_or(CadmaticApiVersion::V1),
            timeout_secs: self.timeout_secs.unwrap_or(30),
            max_retries: self.max_retries.unwrap_or(3),
            include_geometry: self.include_geometry.unwrap_or(false),
            component_batch_size: self.component_batch_size.unwrap_or(200),
            unit_area: self.unit_area.unwrap_or_else(|| "U-100".to_owned()),
        })
    }
}

// Re-export base64 used in CadmaticAuth
use base64::Engine as _;
