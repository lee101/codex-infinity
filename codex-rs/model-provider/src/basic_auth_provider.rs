use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;

/// Basic auth provider for APIs that expect `Authorization: Basic <base64(api_key:)>` headers.
#[derive(Clone)]
pub struct BasicAuthProvider {
    api_key: String,
}

impl BasicAuthProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl AuthProvider for BasicAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let encoded = BASE64_STANDARD.encode(format!("{}:", self.api_key));
        if let Ok(header) = HeaderValue::from_str(&format!("Basic {encoded}")) {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn basic_auth_provider_adds_auth_header() {
        let auth = BasicAuthProvider::new("cursor-test-key".to_string());
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Basic Y3Vyc29yLXRlc3Qta2V5Og==")
        );
    }
}
