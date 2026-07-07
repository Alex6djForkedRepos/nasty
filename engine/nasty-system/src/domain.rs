//! Active Directory domain join state and configuration.
//!
//! This module manages the lifecycle of NASty membership in an AD realm:
//! - Realm validation (DNS names only, no local workgroups)
//! - NetBIOS workgroup derivation from the realm's first label
//! - UID range allocation for domain users (must avoid local account collision)
//! - Persistent storage of join configuration in `/var/lib/nasty/domain/config.json`

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Errors returned by domain operations.
#[derive(Debug, Error)]
pub enum DomainError {
    /// Validation failed (bad realm format, out-of-range idmap, etc.).
    #[error("validation error: {0}")]
    Validation(String),
    /// Preflight check failed (domain tools missing, network unreachable, etc.).
    #[error("preflight check failed: {0}")]
    Preflight(String),
    /// Already joined to a domain.
    #[error("already joined to a domain")]
    AlreadyJoined,
    /// Not currently joined to a domain.
    #[error("not joined to a domain")]
    NotJoined,
    /// A domain command (kinit, net ads, etc.) failed.
    #[error("domain command failed: {0}")]
    CommandFailed(String),
    /// I/O error (file operations, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Persisted domain join configuration.
///
/// Presence of the config file (`/var/lib/nasty/domain/config.json`) indicates
/// the system is AD-joined.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainConfig {
    /// Active Directory realm (DNS name, uppercase; e.g., "CORP.EXAMPLE.COM").
    pub realm: String,
    /// NetBIOS workgroup name derived from realm (≤ 15 chars, uppercase).
    pub workgroup: String,
    /// Base UID for domain user mappings (must be ≥ 65536 to avoid local collisions).
    pub idmap_base: u32,
}

/// Default base UID for domain user mappings.
/// UIDs below this are reserved for local system accounts.
pub const DEFAULT_IDMAP_BASE: u32 = 100_000;

/// UID range span for domain users (DEFAULT_IDMAP_BASE to DEFAULT_IDMAP_BASE + IDMAP_RANGE_SPAN).
pub const IDMAP_RANGE_SPAN: u32 = 900_000;

/// Validate and normalize an Active Directory realm name.
///
/// Returns the normalized (uppercase) realm on success.
/// Rejects: empty strings, single-label names (not resolvable AD realms),
/// invalid DNS characters, or trailing/leading hyphens per label.
pub fn validate_realm(raw: &str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Validation("realm is empty".into()));
    }
    let labels: Vec<&str> = trimmed.split('.').collect();
    if labels.len() < 2 {
        return Err(DomainError::Validation(format!(
            "'{trimmed}' is not a DNS realm (expected e.g. CORP.EXAMPLE.COM)"
        )));
    }
    for label in &labels {
        let ok = !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !ok {
            return Err(DomainError::Validation(format!(
                "realm label '{label}' contains invalid characters"
            )));
        }
    }
    Ok(trimmed.to_ascii_uppercase())
}

/// Derive a NetBIOS workgroup name from an AD realm.
///
/// Takes the first DNS label and uppercases it, truncating to 15 chars
/// (NetBIOS limit). The realm is assumed already validated.
pub fn derive_workgroup(realm: &str) -> String {
    let first = realm.split('.').next().unwrap_or(realm);
    first
        .chars()
        .take(15)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Validate an idmap base UID.
///
/// Rejects values below 65536 to ensure domain UIDs never collide
/// with local system accounts (which typically occupy 0–65535).
pub fn validate_idmap_base(base: u32) -> Result<(), DomainError> {
    if base < 65_536 {
        return Err(DomainError::Validation(format!(
            "idmap base {base} is too low — must be at least 65536 so domain \
             UIDs can never collide with local accounts"
        )));
    }
    Ok(())
}

/// Path to the Samba ADS configuration fragment.
pub const DOMAIN_SMB_CONF_PATH: &str = "/etc/samba/nasty-domain.conf";

/// Path to the Kerberos configuration.
pub const KRB5_CONF_PATH: &str = "/etc/samba/nasty-krb5.conf";

/// Render the `[global]`-scope Samba configuration block for Active Directory.
///
/// Produces configuration suitable for `/etc/samba/nasty-domain.conf`.
/// Realm and workgroup are safe to interpolate — `validate_realm` guarantees
/// they contain no shell/config-injection characters before a `DomainConfig` can exist.
pub fn render_domain_smb_conf(cfg: &DomainConfig) -> String {
    let base = cfg.idmap_base;
    let end = base + IDMAP_RANGE_SPAN - 1;
    format!(
        "# Managed by NASty — Active Directory member configuration.\n\
         # Rendered at domain join; emptied at leave. Do not edit manually.\n\
         security = ADS\n\
         realm = {realm}\n\
         workgroup = {wg}\n\
         kerberos method = secrets and keytab\n\
         winbind refresh tickets = yes\n\
         winbind offline logon = yes\n\
         winbind enum users = no\n\
         winbind enum groups = no\n\
         idmap config * : backend = tdb\n\
         idmap config * : range = 65000-65535\n\
         idmap config {wg} : backend = rid\n\
         idmap config {wg} : range = {base}-{end}\n\
         template shell = /run/current-system/sw/bin/nologin\n\
         template homedir = /var/empty\n",
        realm = cfg.realm,
        wg = cfg.workgroup,
    )
}

/// Render the Kerberos configuration.
///
/// Produces configuration suitable for `/etc/samba/nasty-krb5.conf`.
/// Realm is safe to interpolate — `validate_realm` guarantees it contains
/// no shell/config-injection characters.
pub fn render_krb5_conf(realm: &str) -> String {
    format!(
        "# Managed by NASty — rendered at domain join.\n\
         [libdefaults]\n\
         \tdefault_realm = {realm}\n\
         \tdns_lookup_realm = false\n\
         \tdns_lookup_kdc = true\n\
         \trdns = false\n",
    )
}

/// Path to the resolved(1) drop-in that routes AD-zone DNS queries to the
/// domain controllers without replacing the box's own resolvers.
pub const RESOLVED_DROPIN_PATH: &str = "/etc/systemd/resolved.conf.d/nasty-ad.conf";

/// Extract domain controller hostnames from `resolvectl query --type=SRV
/// _ldap._tcp.<realm>` output (e.g. lines like
/// "_ldap._tcp.corp.example.com IN SRV 0 100 389 dc1.corp.example.com").
fn parse_resolvectl_srv(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|l| l.contains(" SRV "))
        .filter_map(|l| l.split_whitespace().last())
        .map(|t| t.trim_end_matches('.').to_string())
        .collect()
}

/// Extract IPs from `resolvectl query <host>` output (lines like
/// "dc1.corp.example.com: 10.0.0.5").
///
/// Not yet called — feeds `render_resolved_dropin` from the join flow in a
/// later task, once `preflight`'s discovered DC hostnames are resolved to
/// IPs.
#[allow(dead_code)]
fn parse_resolvectl_addresses(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|l| l.split_once(": "))
        .map(|(_, addr)| addr.trim().to_string())
        .filter(|a| a.parse::<std::net::IpAddr>().is_ok())
        .collect()
}

/// Render the systemd-resolved drop-in that routes AD-zone DNS queries to
/// the domain controllers, per-domain (`Domains=~<realm>`) — the box's own
/// resolvers are left untouched for everything else (spec join-flow step 6).
pub fn render_resolved_dropin(realm: &str, dc_ips: &[String]) -> String {
    format!(
        "# Managed by NASty — routes AD-zone DNS queries to the domain\n\
         # controllers without replacing the box's resolvers. Removed at leave.\n\
         [Resolve]\n\
         DNS={}\n\
         Domains=~{}\n",
        dc_ips.join(" "),
        realm.to_ascii_lowercase(),
    )
}

/// Parse `net ads info`'s "Server time:" line into unix seconds.
/// Format observed: "Server time: Tue, 07 Jul 2026 12:34:56 UTC".
/// Hand-rolled (days-since-epoch arithmetic) to avoid a chrono dep for
/// one line of output; only UTC/GMT zones are accepted — anything else
/// returns None and preflight skips the skew check rather than
/// mis-judging it.
fn parse_net_ads_server_time(output: &str) -> Option<i64> {
    let line = output
        .lines()
        .find(|l| l.trim_start().starts_with("Server time:"))?;
    let rest = line.split_once(':')?.1.trim(); // "Tue, 07 Jul 2026 12:34:56 UTC"
    let rest = rest.split_once(',').map(|(_, r)| r.trim()).unwrap_or(rest);
    let mut parts = rest.split_whitespace(); // 07 Jul 2026 12:34:56 UTC
    let day: i64 = parts.next()?.parse().ok()?;
    let month = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let mut hms = parts.next()?.split(':');
    let (h, m, s): (i64, i64, i64) = (
        hms.next()?.parse().ok()?,
        hms.next()?.parse().ok()?,
        hms.next()?.parse().ok()?,
    );
    if !matches!(parts.next(), Some("UTC") | Some("GMT")) {
        return None;
    }
    // Days since 1970-01-01 (civil-from-days, Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3_600 + m * 60 + s)
}

/// Run a command, capturing stdout+stderr. Returns stdout on success;
/// on non-zero exit (or a spawn failure) returns `CommandFailed` carrying
/// stderr (or the spawn error).
async fn run_cmd(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, DomainError> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .await
        .map_err(|e| DomainError::CommandFailed(format!("failed to run {program}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DomainError::CommandFailed(stderr.trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Fail fast with actionable errors before Kerberos gets a chance to
/// produce cryptic ones. Checks, in order: the realm's LDAP SRV records
/// resolve; a DC answers `net ads info`; clock skew is within bounds.
///
/// Not yet called — wired up to the join flow in a later task; exercised
/// by the VM integration test in the meantime.
#[allow(dead_code)]
async fn preflight(realm: &str) -> Result<Vec<String>, DomainError> {
    let srv_name = format!("_ldap._tcp.{}", realm.to_ascii_lowercase());
    let out = run_cmd("resolvectl", &["query", "--type=SRV", &srv_name], &[]).await?;
    let dcs = parse_resolvectl_srv(&out);
    if dcs.is_empty() {
        return Err(DomainError::Preflight(format!(
            "no domain controllers found: DNS SRV lookup for {srv_name} returned \
             nothing. The box's DNS must be able to resolve the AD zone — point \
             it at (or forward to) the domain's DNS server."
        )));
    }
    let info = run_cmd(
        "net",
        &["ads", "info", "-S", &dcs[0], "--realm", realm],
        &[("KRB5_CONFIG", KRB5_CONF_PATH)],
    )
    .await
    .map_err(|e| {
        DomainError::Preflight(format!("domain controller {} did not answer: {e}", dcs[0]))
    })?;
    if let Some(server_time) = parse_net_ads_server_time(&info) {
        let local = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let skew = (server_time - local).abs();
        if skew > 240 {
            return Err(DomainError::Preflight(format!(
                "clock skew vs domain controller is {skew}s — Kerberos tolerates \
                 ~300s. Fix NTP before joining."
            )));
        }
    }
    Ok(dcs)
}

/// Service for managing domain join state.
pub struct DomainService;

const CONFIG_PATH: &str = "/var/lib/nasty/domain/config.json";

impl Default for DomainService {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainService {
    /// Create a new domain service instance.
    pub fn new() -> Self {
        Self
    }

    /// Load domain configuration from disk if it exists.
    pub async fn load_config() -> Option<DomainConfig> {
        Self::load_config_at(Path::new(CONFIG_PATH)).await
    }

    /// Persist domain configuration to disk.
    pub async fn save_config(config: &DomainConfig) -> Result<(), DomainError> {
        Self::save_config_at(Path::new(CONFIG_PATH), config).await
    }

    /// Clear domain configuration (leave domain).
    pub async fn clear_config() -> Result<(), DomainError> {
        Self::clear_config_at(Path::new(CONFIG_PATH)).await
    }

    /// Load domain configuration from an arbitrary path if it exists.
    ///
    /// Absence of the file, or unparseable contents, both mean "not joined" —
    /// this never panics on corrupt state.
    pub(crate) async fn load_config_at(path: &Path) -> Option<DomainConfig> {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => serde_json::from_str(&content).ok(),
            Err(_) => None,
        }
    }

    /// Persist domain configuration to an arbitrary path, creating parent dirs.
    pub(crate) async fn save_config_at(
        path: &Path,
        config: &DomainConfig,
    ) -> Result<(), DomainError> {
        let dir = path.parent().unwrap();
        tokio::fs::create_dir_all(dir).await?;
        let json =
            serde_json::to_string(config).map_err(|e| DomainError::Io(std::io::Error::other(e)))?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Clear domain configuration at an arbitrary path (leave domain).
    ///
    /// Idempotent: clearing an already-absent config is not an error.
    pub(crate) async fn clear_config_at(path: &Path) -> Result<(), DomainError> {
        match tokio::fs::remove_file(path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_realm_normalizes_and_accepts_dns_names() {
        assert_eq!(
            validate_realm("corp.example.com").unwrap(),
            "CORP.EXAMPLE.COM"
        );
        assert_eq!(validate_realm("  ad.lan ").unwrap(), "AD.LAN");
    }

    #[test]
    fn validate_realm_rejects_garbage() {
        // Single label: not a resolvable AD realm.
        assert!(validate_realm("WORKGROUP").is_err());
        assert!(validate_realm("").is_err());
        // Characters that could smuggle config or shell content.
        assert!(validate_realm("corp.example.com\ninclude=/etc/passwd").is_err());
        assert!(validate_realm("corp;rm -rf /.com").is_err());
        assert!(validate_realm("corp .example.com").is_err());
    }

    #[test]
    fn derive_workgroup_takes_first_label_netbios_truncated() {
        assert_eq!(derive_workgroup("CORP.EXAMPLE.COM"), "CORP");
        // NetBIOS names cap at 15 chars.
        assert_eq!(
            derive_workgroup("VERYLONGCOMPANYNAME.LAN"),
            "VERYLONGCOMPANY"
        );
    }

    #[test]
    fn validate_idmap_base_rejects_low_ranges() {
        // Must clear every local UID the engine can allocate.
        assert!(validate_idmap_base(3000).is_err());
        assert!(validate_idmap_base(65_535).is_err());
        assert!(validate_idmap_base(65_536).is_ok());
        assert!(validate_idmap_base(DEFAULT_IDMAP_BASE).is_ok());
    }

    #[tokio::test]
    async fn config_round_trips_and_clear_means_not_joined() {
        let dir = std::env::temp_dir().join(format!("nasty-domain-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("config.json");
        // Absent file == not joined.
        assert!(DomainService::load_config_at(&path).await.is_none());
        let cfg = DomainConfig {
            realm: "CORP.EXAMPLE.COM".into(),
            workgroup: "CORP".into(),
            idmap_base: 100_000,
        };
        // Save creates the parent dir and the file.
        DomainService::save_config_at(&path, &cfg)
            .await
            .expect("save");
        let loaded = DomainService::load_config_at(&path).await.expect("loaded");
        assert_eq!(loaded.realm, "CORP.EXAMPLE.COM");
        assert_eq!(loaded.workgroup, "CORP");
        assert_eq!(loaded.idmap_base, 100_000);
        // Corrupt JSON degrades to "not joined", never panics.
        tokio::fs::write(&path, b"{not json").await.unwrap();
        assert!(DomainService::load_config_at(&path).await.is_none());
        // Clear is idempotent.
        DomainService::save_config_at(&path, &cfg)
            .await
            .expect("save again");
        DomainService::clear_config_at(&path).await.expect("clear");
        assert!(DomainService::load_config_at(&path).await.is_none());
        DomainService::clear_config_at(&path)
            .await
            .expect("second clear: no panic"); // idempotent
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_domain_smb_conf_emits_ads_block() {
        let cfg = DomainConfig {
            realm: "CORP.EXAMPLE.COM".into(),
            workgroup: "CORP".into(),
            idmap_base: 100_000,
        };
        let conf = render_domain_smb_conf(&cfg);
        assert!(conf.contains("security = ADS"), "{conf}");
        assert!(conf.contains("realm = CORP.EXAMPLE.COM"), "{conf}");
        assert!(conf.contains("workgroup = CORP"), "{conf}");
        // Deterministic algorithmic mapping — same user, same UID, forever.
        assert!(conf.contains("idmap config CORP : backend = rid"), "{conf}");
        assert!(
            conf.contains("idmap config CORP : range = 100000-999999"),
            "{conf}"
        );
        // The default (*) range must not overlap the domain range.
        assert!(
            conf.contains("idmap config * : range = 65000-65535"),
            "{conf}"
        );
        // DC outage tolerance for recently-seen users.
        assert!(conf.contains("winbind offline logon = yes"), "{conf}");
        // Explicit namespaces — never ambiguous with local users.
        assert!(!conf.contains("winbind use default domain"), "{conf}");
        assert!(
            conf.contains("kerberos method = secrets and keytab"),
            "{conf}"
        );
    }

    #[test]
    fn render_krb5_conf_pins_realm_and_dns_lookup() {
        let conf = render_krb5_conf("CORP.EXAMPLE.COM");
        assert!(conf.contains("default_realm = CORP.EXAMPLE.COM"), "{conf}");
        // DCs are found via DNS SRV — no static kdc lines to go stale.
        assert!(conf.contains("dns_lookup_kdc = true"), "{conf}");
        assert!(conf.contains("rdns = false"), "{conf}");
    }

    #[test]
    fn parse_resolvectl_srv_extracts_targets() {
        // resolvectl output shape: "_ldap._tcp.corp.example.com IN SRV 0 100 389 dc1.corp.example.com"
        let out = "\
_ldap._tcp.corp.example.com IN SRV 0 100 389 dc1.corp.example.com\n\
_ldap._tcp.corp.example.com IN SRV 0 100 389 dc2.corp.example.com\n\n\
-- Information acquired via protocol DNS in 2.1ms.\n";
        assert_eq!(
            parse_resolvectl_srv(out),
            vec![
                "dc1.corp.example.com".to_string(),
                "dc2.corp.example.com".to_string()
            ]
        );
        assert!(parse_resolvectl_srv("-- no data --\n").is_empty());
    }

    #[test]
    fn parse_resolvectl_addresses_extracts_ips() {
        let out = "dc1.corp.example.com: 10.0.0.5\n\n-- Information acquired via protocol DNS in 1.2ms.\n";
        assert_eq!(
            parse_resolvectl_addresses(out),
            vec!["10.0.0.5".to_string()]
        );
    }

    #[test]
    fn render_resolved_dropin_routes_realm_to_dcs() {
        let conf =
            render_resolved_dropin("CORP.EXAMPLE.COM", &["10.0.0.5".into(), "10.0.0.6".into()]);
        assert!(conf.contains("[Resolve]"), "{conf}");
        assert!(conf.contains("DNS=10.0.0.5 10.0.0.6"), "{conf}");
        // Routing domain (~) — only AD-zone queries go to the DCs.
        assert!(conf.contains("Domains=~corp.example.com"), "{conf}");
    }

    #[test]
    fn parse_net_ads_server_time_reads_rfc2822_style() {
        // `net ads info` prints e.g. "Server time: Tue, 07 Jul 2026 12:34:56 UTC"
        let out = "LDAP server: 10.0.0.5\nServer time: Tue, 07 Jul 2026 12:34:56 UTC\n";
        // 2026-07-07T12:34:56Z — verified via:
        // `date -u -j -f "%Y-%m-%d %H:%M:%S" "2026-07-07 12:34:56" +%s` => 1783427696
        assert_eq!(parse_net_ads_server_time(out), Some(1783427696));
        assert_eq!(parse_net_ads_server_time("no time here"), None);
    }
}
