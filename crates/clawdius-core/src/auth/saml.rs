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
    pub fn new(config: SamlConfig) -> Self {
        Self { config }
    }

    pub fn get_redirect_url(&self) -> String {
        self.config.sso_url.clone()
    }
}
