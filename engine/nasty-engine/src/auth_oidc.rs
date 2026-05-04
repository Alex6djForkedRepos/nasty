//! OpenID Connect client wrapper.
//!
//! Builds an `openidconnect` Core client from `OidcSettings`, generates the
//! authorization URL with PKCE + nonce, and exchanges an authorization code
//! for a verified ID token. Group extraction is dynamic so the operator can
//! point `groups_claim` at any custom claim the IdP emits.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::reqwest::async_http_client;
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use tokio::sync::RwLock;

use nasty_system::settings::OidcSettings;

/// Window during which a started OIDC login flow can be completed via callback.
const PENDING_TTL_SECS: u64 = 5 * 60;

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("OIDC is not enabled")]
    NotEnabled,
    #[error("OIDC is not fully configured: {0}")]
    NotConfigured(&'static str),
    #[error("OIDC discovery failed: {0}")]
    Discovery(String),
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("invalid state cookie or expired")]
    StateMismatch,
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    #[error("ID token verification failed: {0}")]
    TokenVerification(String),
    #[error("missing ID token in response")]
    MissingIdToken,
    #[error("login denied: no role mapping matched and no default role configured")]
    NoRoleMatch,
}

/// Identity returned from a successful callback. Username preference order is
/// `preferred_username` → `email` → `subject`.
#[derive(Debug, Clone)]
pub struct OidcIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub preferred_username: Option<String>,
    pub groups: Vec<String>,
}

struct Pending {
    pkce_verifier_secret: String,
    nonce: Nonce,
    expires_at: u64,
}

pub struct OidcClient {
    inner: CoreClient,
    settings: OidcSettings,
    pending: Arc<RwLock<HashMap<String, Pending>>>,
}

#[derive(Default, Clone)]
pub struct OidcHolder {
    inner: Arc<RwLock<Option<Arc<OidcClient>>>>,
}

impl OidcHolder {
    pub async fn current(&self) -> Option<Arc<OidcClient>> {
        self.inner.read().await.clone()
    }

    /// Rebuild the client from current settings. Called on engine startup
    /// and whenever settings change.
    pub async fn rebuild(&self, settings: &OidcSettings) -> Result<(), OidcError> {
        if !settings.enabled {
            *self.inner.write().await = None;
            return Ok(());
        }
        let client = OidcClient::from_settings(settings).await?;
        *self.inner.write().await = Some(Arc::new(client));
        Ok(())
    }
}

impl OidcClient {
    pub async fn from_settings(settings: &OidcSettings) -> Result<Self, OidcError> {
        let issuer = settings
            .issuer_url
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or(OidcError::NotConfigured("issuer_url"))?;
        let client_id = settings
            .client_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or(OidcError::NotConfigured("client_id"))?;
        let redirect = settings
            .redirect_uri
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or(OidcError::NotConfigured("redirect_uri"))?;

        let issuer_url =
            IssuerUrl::new(issuer.to_string()).map_err(|e| OidcError::Config(e.to_string()))?;
        let metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|e| OidcError::Discovery(e.to_string()))?;

        let client_secret = settings
            .client_secret
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| ClientSecret::new(s.to_string()));

        let inner = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.to_string()),
            client_secret,
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect.to_string()).map_err(|e| OidcError::Config(e.to_string()))?,
        );

        Ok(Self {
            inner,
            settings: settings.clone(),
            pending: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Build an authorization URL and stash the PKCE verifier + nonce keyed by
    /// the CSRF state value. Returns the URL to redirect the browser to.
    pub async fn authorize_url(&self) -> url::Url {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let mut builder = self.inner.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        for scope in &self.settings.scopes {
            builder = builder.add_scope(Scope::new(scope.clone()));
        }
        let (url, csrf, nonce) = builder.set_pkce_challenge(pkce_challenge).url();

        let now = unix_now();
        let mut pending = self.pending.write().await;
        pending.retain(|_, p| p.expires_at > now);
        pending.insert(
            csrf.secret().clone(),
            Pending {
                pkce_verifier_secret: pkce_verifier.secret().clone(),
                nonce,
                expires_at: now + PENDING_TTL_SECS,
            },
        );
        url
    }

    /// Look up a pending flow by the state value the IdP echoed back, exchange
    /// the code, validate the ID token (signature, audience, nonce, expiry),
    /// and return the resolved identity.
    pub async fn exchange_code(
        &self,
        state: &str,
        code: &str,
    ) -> Result<OidcIdentity, OidcError> {
        let pending = {
            let mut p = self.pending.write().await;
            p.remove(state).ok_or(OidcError::StateMismatch)?
        };
        if pending.expires_at <= unix_now() {
            return Err(OidcError::StateMismatch);
        }

        let token_response = self
            .inner
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier_secret))
            .request_async(async_http_client)
            .await
            .map_err(|e| OidcError::TokenExchange(e.to_string()))?;

        let id_token = token_response
            .id_token()
            .ok_or(OidcError::MissingIdToken)?;
        let verifier = self.inner.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &pending.nonce)
            .map_err(|e| OidcError::TokenVerification(e.to_string()))?;

        let issuer = claims.issuer().to_string();
        let subject = claims.subject().to_string();
        let preferred_username = claims.preferred_username().map(|p| p.as_str().to_string());
        let email = claims.email().map(|e| e.as_str().to_string());

        // The Core verifier consumes only standard claims. Re-parse the JWT
        // payload for the configured `groups_claim` — signature has already
        // been validated, so trusting the bytes is fine.
        let raw_payload = decode_jwt_payload(&id_token.to_string()).unwrap_or_default();
        let groups = extract_groups(&raw_payload, &self.settings.groups_claim);

        Ok(OidcIdentity {
            issuer,
            subject,
            email,
            preferred_username,
            groups,
        })
    }
}

/// Decode the middle (payload) segment of a JWS-compact-serialized JWT into
/// raw JSON. Returns Null on any structural failure.
fn decode_jwt_payload(jwt: &str) -> Option<serde_json::Value> {
    let mut parts = jwt.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Walk a dotted JSON path and return any string value(s) found at the leaf.
/// Accepts a single string or an array of strings. Anything else → empty list.
fn extract_groups(payload: &serde_json::Value, claim_path: &str) -> Vec<String> {
    let mut cur = payload.clone();
    for segment in claim_path.split('.') {
        cur = match cur {
            serde_json::Value::Object(mut o) => {
                o.remove(segment).unwrap_or(serde_json::Value::Null)
            }
            _ => return Vec::new(),
        };
    }
    match cur {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        serde_json::Value::String(s) => vec![s],
        _ => Vec::new(),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Map the ID-token groups list to a NASty role using the operator-provided
/// mapping table. Returns the configured default role when nothing matches,
/// or `None` when there is no default (login should be denied).
pub fn role_for_groups(groups: &[String], settings: &OidcSettings) -> Option<String> {
    for mapping in &settings.role_mappings {
        if groups.iter().any(|g| g == &mapping.group) {
            return Some(mapping.role.clone());
        }
    }
    settings.default_role.clone().filter(|s| !s.is_empty())
}

/// Quick non-network sanity check used by `auth.oidc.test` — dry-run a sample
/// claim payload against the configured mappings without bouncing the IdP.
pub fn dry_run_role(
    sample_claims: &serde_json::Value,
    settings: &OidcSettings,
) -> Result<Option<String>, String> {
    let groups = match sample_claims.get(&settings.groups_claim) {
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(_) => {
            return Err(format!(
                "claim `{}` is not a string or array of strings",
                settings.groups_claim
            ));
        }
        None => Vec::new(),
    };
    Ok(role_for_groups(&groups, settings))
}
