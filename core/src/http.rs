//! Client HTTP minimale con timeout globale, condivisibile fra thread.

use std::time::Duration;

pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new(timeout_s: u64) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_s)))
            .user_agent("van-allen-dashboard/0.1 (+https://services.swpc.noaa.gov)")
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .build(),
            )
            .build();
        Self {
            agent: config.into(),
        }
    }

    pub fn text(&self, url: &str) -> Result<String, String> {
        let mut resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("{url}: {e}"))?;
        resp.body_mut()
            .read_to_string()
            .map_err(|e| format!("{url}: corpo: {e}"))
    }

    pub fn json(&self, url: &str) -> Result<serde_json::Value, String> {
        let text = self.text(url)?;
        serde_json::from_str(&text).map_err(|e| format!("{url}: json: {e}"))
    }
}

impl Clone for Http {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
        }
    }
}
