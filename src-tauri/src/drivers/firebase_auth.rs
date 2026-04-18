use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::DriverError;

const TOKEN_EXPIRY_SECS: u64 = 3600;
const REFRESH_MARGIN_SECS: u64 = 60;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccount {
    pub project_id: String,
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

impl ServiceAccount {
    pub fn from_json(json: &str) -> Result<Self, DriverError> {
        serde_json::from_str(json)
            .map_err(|e| DriverError::ConnectionFailed(format!("Invalid service account JSON: {}", e)))
    }
}

#[derive(Debug, Serialize)]
struct JwtClaims {
    iss: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: u64,
}

struct CachedToken {
    token: String,
    obtained_at: Instant,
}

pub struct FirebaseAuth {
    client_email: String,
    token_uri: String,
    project_id: String,
    /// RSA private key parsed once at construction. Token fetches reuse this
    /// without re-parsing the PEM each time.
    encoding_key: EncodingKey,
    cached_token: Mutex<Option<CachedToken>>,
    http: reqwest::Client,
}

impl FirebaseAuth {
    pub fn new(service_account: ServiceAccount) -> Result<Self, DriverError> {
        let encoding_key = EncodingKey::from_rsa_pem(service_account.private_key.as_bytes())
            .map_err(|e| DriverError::ConnectionFailed(format!("Invalid RSA private key: {}", e)))?;

        Ok(Self {
            client_email: service_account.client_email,
            token_uri: service_account.token_uri,
            project_id: service_account.project_id,
            encoding_key,
            cached_token: Mutex::new(None),
            http: reqwest::Client::new(),
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub async fn access_token(&self, scopes: &[&str]) -> Result<String, DriverError> {
        {
            let cache = self.cached_token.lock().unwrap();
            if let Some(ref cached) = *cache {
                let elapsed = cached.obtained_at.elapsed();
                if elapsed < Duration::from_secs(TOKEN_EXPIRY_SECS - REFRESH_MARGIN_SECS) {
                    return Ok(cached.token.clone());
                }
            }
        }

        let token = self.fetch_token(scopes).await?;

        {
            let mut cache = self.cached_token.lock().unwrap();
            *cache = Some(CachedToken {
                token: token.clone(),
                obtained_at: Instant::now(),
            });
        }

        Ok(token)
    }

    async fn fetch_token(&self, scopes: &[&str]) -> Result<String, DriverError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = JwtClaims {
            iss: self.client_email.clone(),
            scope: scopes.join(" "),
            aud: self.token_uri.clone(),
            iat: now,
            exp: now + TOKEN_EXPIRY_SECS,
        };

        let header = Header::new(Algorithm::RS256);
        let assertion = encode(&header, &claims, &self.encoding_key)
            .map_err(|e| DriverError::ConnectionFailed(format!("JWT signing failed: {}", e)))?;

        let resp = self
            .http
            .post(&self.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &assertion),
            ])
            .send()
            .await
            .map_err(|e| DriverError::ConnectionFailed(format!("Token request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DriverError::ConnectionFailed(format!(
                "Token exchange failed ({}): {}",
                status, body
            )));
        }

        let token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| DriverError::ConnectionFailed(format!("Token parse failed: {}", e)))?;

        Ok(token_resp.access_token)
    }
}

/// Blob stored in connection string for Firebase engines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseConnBlob {
    pub auth_json: String,
    pub project_id: String,
    pub database_url: String,
    pub firestore_db_id: String,
}

impl FirebaseConnBlob {
    pub fn encode(&self) -> Result<String, DriverError> {
        let json = serde_json::to_string(self)
            .map_err(|e| DriverError::ConnectionFailed(format!("Failed to serialize blob: {}", e)))?;
        Ok(format!("firebase://{}", URL_SAFE_NO_PAD.encode(json.as_bytes())))
    }

    pub fn decode(conn_str: &str) -> Result<Self, DriverError> {
        let payload = conn_str
            .strip_prefix("firebase://")
            .ok_or_else(|| DriverError::ConnectionFailed("Expected firebase:// prefix".into()))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|e| DriverError::ConnectionFailed(format!("Base64 decode failed: {}", e)))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| DriverError::ConnectionFailed(format!("Blob JSON parse failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks in PR feedback fix: an invalid PEM key must fail at
    /// construction time so we don't re-parse it on every token fetch.
    #[test]
    fn invalid_private_key_fails_in_new_not_on_first_token_fetch() {
        let sa = ServiceAccount {
            project_id: "demo".into(),
            client_email: "x@y.iam".into(),
            private_key: "not a real PEM".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
        };
        match FirebaseAuth::new(sa) {
            Ok(_) => panic!("FirebaseAuth::new must reject invalid PEM up front"),
            Err(e) => assert!(
                e.to_string().contains("Invalid RSA private key"),
                "Unexpected error: {}",
                e
            ),
        }
    }

    /// Reproduces user bug: "Connection failed: Expected firebase:// prefix".
    /// The frontend's `buildConnectionString` for `firestore` emits a preview-only
    /// string like `firebase://<project-id>/(default)` (literal angle brackets,
    /// not a base64 blob). When the Test button forwards that string to the
    /// Rust `test_connection` command, decode fails.
    #[test]
    fn frontend_preview_string_for_firestore_currently_fails_to_decode() {
        let preview = "firebase://<my-project>/(default)";
        let result = FirebaseConnBlob::decode(preview);
        assert!(
            result.is_err(),
            "BUG REPRO: preview string should fail to decode as a blob"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Base64 decode failed"),
            "Expected base64 failure, got: {}",
            err
        );
    }

    /// Reproduces same bug for the Realtime DB engine, where the preview is the
    /// raw RTDB URL and has no `firebase://` prefix at all.
    #[test]
    fn frontend_preview_string_for_rtdb_currently_fails_to_decode() {
        let preview = "https://my-app-default-rtdb.firebaseio.com";
        let result = FirebaseConnBlob::decode(preview);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Expected firebase:// prefix"),
            "Expected prefix error, got: {}",
            err
        );
    }

    /// Proves the fix: the shape produced by `build_firebase_conn_str` (which
    /// the frontend now invokes for Firebase engines) decodes successfully.
    #[test]
    fn fix_blob_built_from_service_account_json_decodes() {
        let auth_json = r#"{"project_id":"my-fix-project","client_email":"x@y.iam","private_key":"k","token_uri":"https://oauth2.googleapis.com/token"}"#;

        let project_id = serde_json::from_str::<serde_json::Value>(auth_json)
            .ok()
            .and_then(|v| v.get("project_id").and_then(|p| p.as_str()).map(|s| s.to_string()))
            .unwrap_or_default();

        let blob = FirebaseConnBlob {
            auth_json: auth_json.to_string(),
            project_id,
            database_url: "https://my-fix-default-rtdb.firebaseio.com".to_string(),
            firestore_db_id: String::new(),
        };
        let conn_str = blob.encode().expect("encode should succeed");

        let decoded = FirebaseConnBlob::decode(&conn_str).expect("backend-built blob must decode");
        assert_eq!(decoded.project_id, "my-fix-project");
        assert_eq!(decoded.database_url, "https://my-fix-default-rtdb.firebaseio.com");
    }

    /// Sanity: a properly-encoded blob round-trips. After the fix, the frontend
    /// must produce strings of this shape before calling `test_connection`.
    #[test]
    fn properly_encoded_blob_roundtrips() {
        let blob = FirebaseConnBlob {
            auth_json: r#"{"project_id":"demo"}"#.to_string(),
            project_id: "demo".to_string(),
            database_url: "https://demo-default-rtdb.firebaseio.com".to_string(),
            firestore_db_id: "(default)".to_string(),
        };
        let encoded = blob.encode().expect("encode should succeed");
        assert!(encoded.starts_with("firebase://"));
        let decoded = FirebaseConnBlob::decode(&encoded).expect("should decode");
        assert_eq!(decoded.project_id, "demo");
        assert_eq!(decoded.firestore_db_id, "(default)");
    }
}
