use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ── ACME cert status (global, in-memory) ─────────────────────

static ACME_STATUS: std::sync::OnceLock<std::sync::Mutex<AcmeStatus>> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AcmeStatus {
    /// "idle", "running", "success", "error"
    pub state: String,
    /// Human-readable message (error details, progress info)
    pub message: String,
    /// Domain the cert is for
    pub domain: Option<String>,
    /// When the cert expires, if known
    pub expires: Option<String>,
    /// When the cert was issued, if known
    pub issued: Option<String>,
    /// Certificate issuer (e.g. "Let's Encrypt")
    pub issuer: Option<String>,
    /// When the last attempt was made
    pub last_attempt: Option<String>,
}

impl Default for AcmeStatus {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            message: String::new(),
            domain: None,
            expires: None,
            issued: None,
            issuer: None,
            last_attempt: None,
        }
    }
}

fn set_acme_status(state: &str, message: &str, domain: Option<&str>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    let status = ACME_STATUS.get_or_init(|| std::sync::Mutex::new(AcmeStatus::default()));
    if let Ok(mut s) = status.lock() {
        s.state = state.to_string();
        s.message = message.to_string();
        if let Some(d) = domain {
            s.domain = Some(d.to_string());
        }
        s.last_attempt = Some(now);
    }
}

pub fn get_acme_status() -> AcmeStatus {
    let status = ACME_STATUS.get_or_init(|| std::sync::Mutex::new(AcmeStatus::default()));
    status.lock().map(|s| s.clone()).unwrap_or_default()
}

pub fn reset_acme_status() {
    set_acme_status("idle", "", None);
}

/// Re-apply the Caddy TLS snippet. Useful after a transient failure
/// (Caddy briefly down, ACME server flaking, etc.) — the user can hit
/// "retry" in the WebUI and we re-render + reload, which kicks Caddy's
/// internal ACME state machine back into action.
pub async fn retry_acme() -> Result<(), String> {
    let settings = load().await;
    tokio::spawn(async move {
        if let Err(e) = apply_caddy_tls(&settings).await {
            warn!("Caddy TLS reload failed: {e}");
        }
    });
    Ok(())
}

const STATE_PATH: &str = "/var/lib/nasty/settings.json";
const STATE_DIR: &str = "/var/lib/nasty";
const TLS_CERT_PATH: &str = "/var/lib/nasty/tls/cert.pem";

// Caddy reads this snippet from its main Caddyfile via `import` to add a
// hostname-bound vhost when ACME is enabled. Empty file = no extra vhost,
// Caddy serves the static-cert `:8443` block from the NixOS-managed config.
const CADDY_VHOSTS_PATH: &str = "/var/lib/nasty/caddy/vhosts.conf";

// Caddy reads DNS-01 provider credentials from this EnvironmentFile (the
// caddy.service unit references it). One KEY=VAL per line.
const CADDY_ACME_ENV_PATH: &str = "/var/lib/nasty/caddy/acme.env";

/// Display unit for temperatures rendered in the WebUI. Internal storage
/// and alert thresholds are always in Celsius; this is presentational only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TempUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Settings {
    /// IANA timezone string applied to the system (e.g. `UTC`, `America/New_York`).
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// System hostname.
    pub hostname: Option<String>,
    /// Whether to display clocks in 24-hour format.
    #[serde(default = "default_clock_24h")]
    pub clock_24h: bool,
    /// Unit for displayed temperatures (CPU, disks, alert thresholds).
    /// Storage and alert evaluation always use Celsius internally — this
    /// only affects rendering in the WebUI.
    #[serde(default)]
    pub temp_unit: TempUnit,
    /// Domain name for Let's Encrypt TLS (e.g. "nasty.example.com"). Empty = self-signed.
    #[serde(default)]
    pub tls_domain: Option<String>,
    /// Email address for Let's Encrypt ACME notifications.
    #[serde(default)]
    pub tls_acme_email: Option<String>,
    /// Whether Let's Encrypt is enabled. Requires tls_domain and tls_acme_email.
    #[serde(default)]
    pub tls_acme_enabled: bool,
    /// ACME challenge type. Caddy's built-in ACME issuer handles all three:
    ///   - "tls-alpn"  → TLS-ALPN-01 (port 443)
    ///   - "http"      → HTTP-01 (port 80)
    ///   - "dns"       → DNS-01 via a DNS-provider plugin compiled into Caddy
    #[serde(default = "default_challenge_type")]
    pub tls_challenge_type: String,
    /// DNS provider code for DNS-01 challenge (e.g. "cloudflare", "route53").
    /// Must match a DNS plugin compiled into the Caddy binary.
    #[serde(default)]
    pub tls_dns_provider: Option<String>,
    /// DNS provider API credentials as KEY=VALUE lines.
    /// Written to a Caddy `EnvironmentFile` and referenced from the
    /// generated `tls` block via `{env.KEY}` placeholders.
    #[serde(default)]
    pub tls_dns_credentials: Option<String>,
    /// Use Let's Encrypt staging environment (for testing, avoids rate limits).
    #[serde(default)]
    pub tls_acme_staging: bool,
    /// Whether anonymous telemetry is enabled (drive count, storage capacity).
    #[serde(default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
    /// OpenID Connect single-sign-on configuration. Disabled by default.
    #[serde(default)]
    pub oidc: OidcSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OidcSettings {
    /// Master switch — when false, OIDC endpoints return 404 and no IdP traffic occurs.
    #[serde(default)]
    pub enabled: bool,
    /// IdP issuer URL (used for OIDC discovery, e.g. "https://auth.example.com").
    #[serde(default)]
    pub issuer_url: Option<String>,
    /// OAuth client_id registered with the IdP.
    #[serde(default)]
    pub client_id: Option<String>,
    /// OAuth client_secret. Returned as a placeholder over RPC; only the engine sees the real value.
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Absolute redirect URI registered with the IdP (e.g. "https://nasty.local/api/auth/oidc/callback").
    #[serde(default)]
    pub redirect_uri: Option<String>,
    /// OAuth scopes to request. Defaults to ["openid","profile","email","groups"].
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
    /// Name of the ID-token claim that carries the user's groups.
    #[serde(default = "default_oidc_groups_claim")]
    pub groups_claim: String,
    /// Group → role mappings. Evaluated in order; first match wins.
    #[serde(default)]
    pub role_mappings: Vec<OidcRoleMapping>,
    /// Role applied when no group mapping matches. None = deny login.
    #[serde(default)]
    pub default_role: Option<String>,
    /// When true, unknown OIDC subjects are auto-provisioned as local users on first login.
    #[serde(default = "default_true")]
    pub auto_provision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OidcRoleMapping {
    /// Group name (matched verbatim against entries in the configured groups claim).
    pub group: String,
    /// NASty role to assign: "admin", "operator", or "readonly".
    pub role: String,
}

impl Default for OidcSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            issuer_url: None,
            client_id: None,
            client_secret: None,
            redirect_uri: None,
            scopes: default_oidc_scopes(),
            groups_claim: default_oidc_groups_claim(),
            role_mappings: Vec::new(),
            default_role: None,
            auto_provision: true,
        }
    }
}

/// Sentinel returned in place of the OIDC client_secret over the API. When a
/// caller sends this back unchanged, the engine keeps the stored secret.
pub const OIDC_SECRET_PLACEHOLDER: &str = "<unchanged>";

/// Replace the client_secret on a copy of OidcSettings with `<set>` / `<unset>`,
/// suitable for returning to API callers without leaking the real value.
pub fn redact_oidc_secret(mut s: OidcSettings) -> OidcSettings {
    s.client_secret = match s.client_secret.as_deref() {
        Some(v) if !v.is_empty() => Some("<set>".into()),
        _ => Some("<unset>".into()),
    };
    s
}

fn default_oidc_scopes() -> Vec<String> {
    vec![
        "openid".into(),
        "profile".into(),
        "email".into(),
        "groups".into(),
    ]
}

fn default_oidc_groups_claim() -> String {
    "groups".to_string()
}

fn default_true() -> bool {
    true
}

fn default_challenge_type() -> String {
    "tls-alpn".to_string()
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_clock_24h() -> bool {
    true
}

fn default_telemetry_enabled() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            hostname: None,
            clock_24h: default_clock_24h(),
            temp_unit: TempUnit::default(),
            tls_domain: None,
            tls_acme_email: None,
            tls_acme_enabled: false,
            tls_challenge_type: default_challenge_type(),
            tls_dns_provider: None,
            tls_dns_credentials: None,
            tls_acme_staging: false,
            telemetry_enabled: default_telemetry_enabled(),
            oidc: OidcSettings::default(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SettingsUpdate {
    /// New IANA timezone to apply (optional).
    pub timezone: Option<String>,
    /// New hostname to set (optional).
    pub hostname: Option<String>,
    /// Whether to use 24-hour clock display (optional).
    pub clock_24h: Option<bool>,
    /// Display unit for temperatures (optional).
    pub temp_unit: Option<TempUnit>,
    /// Domain name for Let's Encrypt TLS (set to empty string to disable).
    pub tls_domain: Option<String>,
    /// Email address for ACME notifications.
    pub tls_acme_email: Option<String>,
    /// Enable/disable Let's Encrypt.
    pub tls_acme_enabled: Option<bool>,
    /// Challenge type: "tls-alpn" or "dns".
    pub tls_challenge_type: Option<String>,
    /// DNS provider code.
    pub tls_dns_provider: Option<String>,
    /// DNS API credentials (KEY=VALUE per line).
    pub tls_dns_credentials: Option<String>,
    /// Use staging environment.
    pub tls_acme_staging: Option<bool>,
    /// Enable/disable anonymous telemetry.
    pub telemetry_enabled: Option<bool>,
}

pub struct SettingsService {
    state: Arc<RwLock<Settings>>,
}

impl SettingsService {
    pub async fn new() -> Self {
        let mut settings = load().await;
        // Seed hostname from the running system if not yet persisted.
        // This picks up whatever the installer configured (networking.hostName)
        // so the settings page shows the real hostname from day one.
        if settings.hostname.is_none()
            && let Ok(name) = tokio::fs::read_to_string("/proc/sys/kernel/hostname").await
        {
            let name = name.trim().to_string();
            if !name.is_empty() {
                settings.hostname = Some(name);
                let _ = save(&settings).await;
            }
        }
        Self {
            state: Arc::new(RwLock::new(settings)),
        }
    }

    pub async fn get(&self) -> Settings {
        self.state.read().await.clone()
    }

    /// Replace the OIDC configuration. If `incoming.client_secret` is the literal
    /// placeholder `"<unchanged>"`, the previously-stored secret is preserved.
    pub async fn set_oidc(&self, mut incoming: OidcSettings) -> Result<OidcSettings, String> {
        let mut settings = self.state.write().await;
        if incoming.client_secret.as_deref() == Some(OIDC_SECRET_PLACEHOLDER) {
            incoming.client_secret = settings.oidc.client_secret.clone();
        }
        if incoming.scopes.is_empty() {
            incoming.scopes = default_oidc_scopes();
        }
        settings.oidc = incoming.clone();
        save(&settings).await.map_err(|e| e.to_string())?;
        Ok(redact_oidc_secret(incoming))
    }

    pub async fn update(&self, update: SettingsUpdate) -> Result<Settings, String> {
        let mut settings = self.state.write().await;
        if let Some(tz) = update.timezone {
            apply_timezone(&tz).await?;
            settings.timezone = tz;
        }
        if let Some(name) = update.hostname {
            apply_hostname(&name).await?;
            settings.hostname = Some(name);
        }
        if let Some(h24) = update.clock_24h {
            settings.clock_24h = h24;
        }
        if let Some(unit) = update.temp_unit {
            settings.temp_unit = unit;
        }
        let mut tls_changed = false;
        if let Some(domain) = update.tls_domain {
            let domain = if domain.trim().is_empty() {
                None
            } else {
                Some(domain.trim().to_string())
            };
            if settings.tls_domain != domain {
                settings.tls_domain = domain;
                tls_changed = true;
            }
        }
        if let Some(email) = update.tls_acme_email {
            let email = if email.trim().is_empty() {
                None
            } else {
                Some(email.trim().to_string())
            };
            if settings.tls_acme_email != email {
                settings.tls_acme_email = email;
                tls_changed = true;
            }
        }
        if let Some(enabled) = update.tls_acme_enabled
            && settings.tls_acme_enabled != enabled
        {
            settings.tls_acme_enabled = enabled;
            tls_changed = true;
        }
        if let Some(ct) = update.tls_challenge_type
            && settings.tls_challenge_type != ct
        {
            settings.tls_challenge_type = ct;
            tls_changed = true;
        }
        if let Some(provider) = update.tls_dns_provider {
            let provider = if provider.trim().is_empty() {
                None
            } else {
                Some(provider.trim().to_string())
            };
            if settings.tls_dns_provider != provider {
                settings.tls_dns_provider = provider;
                tls_changed = true;
            }
        }
        if let Some(creds) = update.tls_dns_credentials {
            let creds = if creds.trim().is_empty() {
                None
            } else {
                Some(creds.trim().to_string())
            };
            if settings.tls_dns_credentials != creds {
                settings.tls_dns_credentials = creds;
                tls_changed = true;
            }
        }
        if let Some(staging) = update.tls_acme_staging
            && settings.tls_acme_staging != staging
        {
            settings.tls_acme_staging = staging;
            tls_changed = true;
        }
        if let Some(telemetry) = update.telemetry_enabled {
            settings.telemetry_enabled = telemetry;
        }
        save(&settings).await.map_err(|e| e.to_string())?;
        if tls_changed {
            // Always re-apply on a TLS change — even when ACME is now
            // disabled the snippet must flip back to the static-cert
            // form so Caddy stops serving the old ACME-issued cert.
            let s = settings.clone();
            tokio::spawn(async move {
                if let Err(e) = apply_caddy_tls(&s).await {
                    warn!("Caddy TLS reload failed: {e}");
                }
            });
        }
        Ok(settings.clone())
    }
}

pub async fn list_timezones() -> Result<Vec<String>, String> {
    let output = tokio::process::Command::new("timedatectl")
        .args(["list-timezones"])
        .output()
        .await
        .map_err(|e| format!("timedatectl: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

async fn apply_hostname(name: &str) -> Result<(), String> {
    // NixOS has /etc as read-only — set the kernel hostname via /proc.
    // Persistence is via /var/lib/nasty/settings.json, read at boot by
    // nasty-apply-hostname.service.
    tokio::fs::write("/proc/sys/kernel/hostname", name.as_bytes())
        .await
        .map_err(|e| format!("failed to set kernel hostname: {e}"))?;

    // Also expose the name to the wrapper flake so `nixos-rebuild
    // switch` (which defaults to looking up `nixosConfigurations.<kernel-hostname>`)
    // resolves to our system. The flake at /etc/nixos/flake.nix imports
    // ./hostname.nix when present and falls back to "nasty" otherwise,
    // so writing this file is best-effort — failures are logged but
    // don't fail the apply (e.g. fresh installs before rebootstrap, or
    // if /etc/nixos isn't writable for some reason).
    write_hostname_nix(name).await;

    Ok(())
}

/// Write `/etc/nixos/hostname.nix` with the current hostname as a Nix
/// string literal. Read by the wrapper flake to alias
/// `nixosConfigurations.<hostname>` to the same system attr as `nasty`.
async fn write_hostname_nix(name: &str) {
    let nixos_dir = std::path::Path::new("/etc/nixos");
    if !nixos_dir.exists() {
        // Fresh install before rebootstrap, or running outside a normal
        // NixOS layout (tests). Nothing to do.
        return;
    }
    let path = nixos_dir.join("hostname.nix");
    let content = format!("{}\n", to_nix_string(name));
    if let Err(e) = tokio::fs::write(&path, content).await {
        warn!("could not write {}: {e}", path.display());
    }
}

/// Render a Rust string as a Nix double-quoted string literal, escaping
/// the characters that have special meaning inside `"..."`. The hostname
/// has been validated upstream (RFC1123-ish), but escape defensively so
/// any future relaxation can't smuggle Nix syntax into the flake.
fn to_nix_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

async fn apply_timezone(tz: &str) -> Result<(), String> {
    let output = tokio::process::Command::new("timedatectl")
        .args(["set-timezone", tz])
        .output()
        .await
        .map_err(|e| format!("timedatectl: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("failed to set timezone: {stderr}"));
    }
    Ok(())
}

async fn load() -> Settings {
    match tokio::fs::read_to_string(STATE_PATH).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

async fn save(settings: &Settings) -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all(STATE_DIR).await?;
    let json = serde_json::to_string_pretty(settings).unwrap();
    tokio::fs::write(STATE_PATH, json).await?;
    // Contains OIDC client_secret and ACME/DNS provider config.
    tokio::fs::set_permissions(STATE_PATH, std::fs::Permissions::from_mode(0o600)).await?;
    Ok(())
}

// ── Caddy-driven ACME ────────────────────────────────────────
//
// Caddy's built-in ACME issuer handles HTTP-01, TLS-ALPN-01 and
// (with the right plugin compiled in) DNS-01 — including renewals,
// retries, OCSP stapling, and the multi-stepped state machine that
// used to live in this file as a 250-line `lego` subprocess wrapper.
//
// The engine's job here is reduced to:
//   1. Render a Caddy snippet from `Settings` describing the desired
//      TLS state (static cert vs hostname-bound ACME vhost).
//   2. Write it to `CADDY_VHOSTS_PATH`, where the NixOS-managed
//      Caddyfile imports it.
//   3. Reload Caddy (`systemctl reload`) so it picks up the snippet.
//   4. Read back what Caddy ends up serving so the WebUI can show
//      issuer / expiry / status.
//
// Caddy stores ACME-issued certs under
// `/var/lib/caddy/.local/share/caddy/certificates/<endpoint>/<sans>/<sans>.crt`,
// where `<endpoint>` is the ACME directory URL with `/` → `-` (e.g.
// `acme-v02.api.letsencrypt.org-directory`). We resolve the actual
// path lazily by globbing — relying on the exact URL-encoding rules
// would be brittle across Caddy versions.

const CADDY_DATA_DIR: &str = "/var/lib/caddy";

/// Render the Caddy snippet that should live at `CADDY_VHOSTS_PATH`.
///
/// When ACME is disabled or no domain is set, returns the empty string —
/// the NixOS-managed `:8443` block (with the static self-signed cert)
/// is the only vhost in play. When ACME is enabled, returns a single
/// hostname-bound vhost that Caddy will issue + serve a real cert for,
/// while the static `:8443` block stays as the IP-fallback for users
/// hitting the box by address.
fn caddy_vhosts_snippet(settings: &Settings, https_port: u16) -> String {
    if !settings.tls_acme_enabled {
        return String::new();
    }
    let Some(domain) = settings.tls_domain.as_deref() else {
        return String::new();
    };
    if domain.trim().is_empty() {
        return String::new();
    }

    // Email is optional for Caddy's ACME issuer (Let's Encrypt requires
    // one for renewal notices but won't reject without). Render an empty
    // arg if missing rather than failing the whole reload.
    let email = settings
        .tls_acme_email
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    let mut out = String::new();
    out.push_str(&format!("{domain}:{https_port} {{\n"));

    // The `tls` directive's first form is `tls EMAIL` (or just `tls`)
    // and triggers Caddy's automatic ACME flow for the matched
    // hostname(s). Anything inside `{ ... }` overrides the defaults.
    let needs_block = settings.tls_acme_staging
        || settings.tls_challenge_type == "dns"
        || settings.tls_challenge_type == "tls-alpn"
        || settings.tls_challenge_type == "http";

    if !needs_block {
        if email.is_empty() {
            out.push_str("    tls\n");
        } else {
            out.push_str(&format!("    tls {email}\n"));
        }
    } else {
        if email.is_empty() {
            out.push_str("    tls {\n");
        } else {
            out.push_str(&format!("    tls {email} {{\n"));
        }

        // Restrict the challenge mechanism. Caddy defaults to trying
        // both HTTP-01 and TLS-ALPN-01; pinning lets the user pick one
        // when their network only allows one (e.g. port 80 firewalled).
        match settings.tls_challenge_type.as_str() {
            "http" => out.push_str("        protocols tls1.2 tls1.3\n"),
            "tls-alpn" => out.push_str("        protocols tls1.2 tls1.3\n"),
            _ => {}
        }

        // DNS-01 needs both: a `dns` directive naming the provider, and
        // (for most providers) one or more env-var references. Users
        // paste KEY=VAL lines into `tls_dns_credentials`; the engine
        // writes them to an EnvironmentFile that Caddy.service loads.
        if settings.tls_challenge_type == "dns"
            && let Some(provider) = settings.tls_dns_provider.as_deref()
        {
            // The provider arg is plugin-specific; most one-token
            // plugins (cloudflare, digitalocean, hetzner, …) accept
            // `{env.PROVIDER_TOKEN}` as their first argument. Multi-arg
            // providers (route53, …) want a sub-block; for now we emit
            // the simple form and rely on the plugin's env-var
            // discovery for the rest.
            out.push_str(&format!("        dns {provider}\n"));
        }

        // Issuer block: only needed for the staging override. Without
        // it, Caddy uses the production Let's Encrypt directory (its
        // default).
        if settings.tls_acme_staging {
            out.push_str("        issuer acme {\n");
            out.push_str("            ca https://acme-staging-v02.api.letsencrypt.org/directory\n");
            out.push_str("        }\n");
            out.push_str("        issuer zerossl {\n");
            out.push_str("            ca https://acme-staging-v02.api.letsencrypt.org/directory\n");
            out.push_str("        }\n");
        }

        out.push_str("    }\n");
    }

    out.push_str("    import nasty_webui_routes\n");
    out.push_str("}\n");
    out
}

/// Render the Caddy `EnvironmentFile` content from `tls_dns_credentials`.
/// Empty when ACME-DNS is off — the file is recreated empty so a stale
/// value from a previous provider doesn't leak into the next run.
fn caddy_acme_env(settings: &Settings) -> String {
    if !settings.tls_acme_enabled
        || settings.tls_challenge_type != "dns"
        || settings.tls_dns_credentials.is_none()
    {
        return String::new();
    }
    settings
        .tls_dns_credentials
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string()
        + "\n"
}

/// Apply current settings to Caddy: render the snippet + env file,
/// write them, reload Caddy, then refresh the cached ACME status from
/// whatever Caddy ends up serving.
pub async fn apply_caddy_tls(settings: &Settings) -> Result<(), String> {
    let domain_for_status = settings.tls_domain.clone();

    if settings.tls_acme_enabled {
        set_acme_status(
            "running",
            "Reloading Caddy with ACME configuration...",
            domain_for_status.as_deref(),
        );
    }

    // Both files live in the same dir, owned by the caddy user/group so
    // Caddy can read them. Engine runs as root, so the create + chown
    // here are guaranteed to succeed even on first apply — but log any
    // failure anyway per the workspace-wide rule (see CONTRIBUTING.md).
    if let Some(parent) = std::path::Path::new(CADDY_VHOSTS_PATH).parent()
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        warn!("create_dir_all({}) failed: {e}", parent.display());
    }

    let snippet = caddy_vhosts_snippet(settings, 8443);
    tokio::fs::write(CADDY_VHOSTS_PATH, &snippet)
        .await
        .map_err(|e| format!("write {CADDY_VHOSTS_PATH}: {e}"))?;
    if let Err(e) =
        tokio::fs::set_permissions(CADDY_VHOSTS_PATH, std::fs::Permissions::from_mode(0o644)).await
    {
        warn!("chmod 644 {CADDY_VHOSTS_PATH} failed: {e}");
    }

    let env_content = caddy_acme_env(settings);
    tokio::fs::write(CADDY_ACME_ENV_PATH, &env_content)
        .await
        .map_err(|e| format!("write {CADDY_ACME_ENV_PATH}: {e}"))?;
    // 0640 + chown caddy: secrets, not world-readable.
    if let Err(e) =
        tokio::fs::set_permissions(CADDY_ACME_ENV_PATH, std::fs::Permissions::from_mode(0o640))
            .await
    {
        warn!("chmod 640 {CADDY_ACME_ENV_PATH} failed: {e}");
    }
    nasty_common::cmd::try_run("chown", &["root:caddy", CADDY_ACME_ENV_PATH]).await;

    // `systemctl reload caddy` directly rather than through try_run so
    // we can surface stderr to the caller (the WebUI shows it as an
    // error toast). Spawn / non-zero-exit logging happens here inline.
    let reload = tokio::process::Command::new("systemctl")
        .args(["reload", "caddy"])
        .output()
        .await
        .map_err(|e| {
            let m = format!("systemctl reload caddy: spawn failed: {e}");
            warn!("{m}");
            m
        })?;
    if !reload.status.success() {
        let stderr = String::from_utf8_lossy(&reload.stderr).to_string();
        let msg = format!("caddy reload failed: {stderr}");
        warn!("{msg}");
        set_acme_status("error", &msg, domain_for_status.as_deref());
        return Err(msg);
    }
    info!("caddy reloaded");

    // Push the status forward best-effort. A successful reload doesn't
    // mean the ACME issuance is complete — Caddy issues asynchronously
    // — so we fall back to "running" if no cert is on disk yet.
    refresh_acme_status_from_disk(settings).await;
    Ok(())
}

/// Locate the cert Caddy is currently serving for `domain`, falling back
/// to the static-cert path used when ACME is off.
async fn locate_serving_cert(settings: &Settings) -> Option<String> {
    if settings.tls_acme_enabled
        && let Some(domain) = settings.tls_domain.as_deref()
    {
        // `/var/lib/caddy/.local/share/caddy/certificates/<endpoint>/<domain>/<domain>.crt`
        // Endpoint dir name varies (production vs staging vs zerossl
        // fallback), so glob one level deep.
        let base = format!("{CADDY_DATA_DIR}/.local/share/caddy/certificates");
        if let Ok(mut endpoints) = tokio::fs::read_dir(&base).await {
            while let Ok(Some(ep)) = endpoints.next_entry().await {
                let candidate = format!("{}/{domain}/{domain}.crt", ep.path().display());
                if tokio::fs::metadata(&candidate).await.is_ok() {
                    return Some(candidate);
                }
            }
        }
        return None;
    }
    if tokio::fs::metadata(TLS_CERT_PATH).await.is_ok() {
        return Some(TLS_CERT_PATH.to_string());
    }
    None
}

/// Repopulate the cached ACME status struct from whatever cert Caddy is
/// (or isn't yet) serving. Cheap to call repeatedly — does at most one
/// directory listing + one cert parse.
pub async fn refresh_acme_status_from_disk(settings: &Settings) {
    let domain = settings.tls_domain.clone();
    if !settings.tls_acme_enabled {
        // Static-cert mode. Show the cert details if we have any so the
        // WebUI's "Active certificate" panel still has something to
        // render, but flag the state as idle so the page doesn't claim
        // ACME succeeded.
        let cert_info = read_cert_info(TLS_CERT_PATH).await;
        let status = ACME_STATUS.get_or_init(|| std::sync::Mutex::new(AcmeStatus::default()));
        if let Ok(mut s) = status.lock() {
            s.state = "idle".into();
            s.message = "Static certificate (ACME disabled)".into();
            s.domain = domain;
            s.expires = cert_info.expires;
            s.issued = cert_info.issued;
            s.issuer = cert_info.issuer;
        }
        return;
    }

    let Some(path) = locate_serving_cert(settings).await else {
        // ACME enabled but no cert on disk yet — still issuing.
        set_acme_status(
            "running",
            "Waiting for Caddy to obtain a certificate...",
            domain.as_deref(),
        );
        return;
    };
    let cert_info = read_cert_info(&path).await;
    let status = ACME_STATUS.get_or_init(|| std::sync::Mutex::new(AcmeStatus::default()));
    if let Ok(mut s) = status.lock() {
        s.state = "success".into();
        s.message = match domain.as_deref() {
            Some(d) => format!("Certificate active for {d}"),
            None => "Certificate active".into(),
        };
        s.domain = domain;
        s.expires = cert_info.expires;
        s.issued = cert_info.issued;
        s.issuer = cert_info.issuer;
    }
}

/// Engine-startup hook (replaces the old lego-driven `check_acme_renewal`).
/// Caddy auto-renews internally — there's nothing for us to *do* here, just
/// seed the cached status so the WebUI shows cert details immediately rather
/// than after the first user-triggered apply.
pub async fn check_acme_renewal() {
    let settings = load().await;
    refresh_acme_status_from_disk(&settings).await;
}

struct CertInfo {
    expires: Option<String>,
    issued: Option<String>,
    issuer: Option<String>,
}

/// Read certificate details from a PEM file.
async fn read_cert_info(cert_path: &str) -> CertInfo {
    let mut info = CertInfo {
        expires: None,
        issued: None,
        issuer: None,
    };
    let pem_data = match tokio::fs::read(cert_path).await {
        Ok(d) => d,
        Err(_) => return info,
    };
    let pem = match x509_parser::pem::parse_x509_pem(&pem_data) {
        Ok((_, pem)) => pem,
        Err(_) => return info,
    };
    let cert = match pem.parse_x509() {
        Ok(c) => c,
        Err(_) => return info,
    };
    let validity = cert.validity();
    info.issued = Some(
        validity
            .not_before
            .to_rfc2822()
            .unwrap_or_else(|_| validity.not_before.to_string()),
    );
    info.expires = Some(
        validity
            .not_after
            .to_rfc2822()
            .unwrap_or_else(|_| validity.not_after.to_string()),
    );
    for rdn in cert.issuer().iter() {
        for attr in rdn.iter() {
            let val = attr.as_str().unwrap_or_default();
            let oid = attr.attr_type();
            // OID 2.5.4.3 = CN, 2.5.4.10 = O
            if *oid == x509_parser::oid_registry::OID_X509_COMMON_NAME {
                info.issuer = Some(val.to_string());
                break;
            } else if *oid == x509_parser::oid_registry::OID_X509_ORGANIZATION_NAME
                && info.issuer.is_none()
            {
                info.issuer = Some(val.to_string());
            }
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::to_nix_string;

    #[test]
    fn nix_string_renders_a_plain_hostname_unchanged() {
        // The common case: a normal hostname has no special characters,
        // so the output is just `"name"`.
        assert_eq!(to_nix_string("nasty"), "\"nasty\"");
    }

    #[test]
    fn nix_string_renders_an_fqdn_unchanged() {
        // The motivating case from issue #95: user set hostname to
        // a dot-separated FQDN. Dots have no Nix-string semantics so
        // they just pass through.
        assert_eq!(to_nix_string("nasty.domain.xyz"), "\"nasty.domain.xyz\"",);
    }

    #[test]
    fn nix_string_escapes_quotes_backslashes_and_dollar() {
        // Defensive — hostnames have been validated upstream, but if
        // anything ever sneaks past validation we don't want it
        // smuggling Nix syntax into the flake. `${...}` interpolation
        // is the most dangerous: escaping the leading `$` is enough.
        assert_eq!(to_nix_string("a\"b"), r#""a\"b""#);
        assert_eq!(to_nix_string("a\\b"), r#""a\\b""#);
        assert_eq!(to_nix_string("a${x}b"), r#""a\${x}b""#);
    }

    #[test]
    fn nix_string_escapes_whitespace_control_chars() {
        assert_eq!(to_nix_string("a\nb"), r#""a\nb""#);
        assert_eq!(to_nix_string("a\tb"), r#""a\tb""#);
    }
}
