pub struct SamlConfig {
    pub entity_id: String,
    pub sso_url: String,
    pub slo_url: Option<String>,
    pub certificate: String,
}

pub struct SamlClient {
    config: SamlConfig,
}

impl SamlClient {
    #[must_use]
    pub fn new(config: SamlConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn get_redirect_url(&self) -> String {
        self.config.sso_url.clone()
    }
}
