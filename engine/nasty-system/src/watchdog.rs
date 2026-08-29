use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Write;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::sync::Mutex;

const STATE_DIR: &str = "/var/lib/nasty";
const STATE_PATH: &str = "/var/lib/nasty/watchdog.json";
const CONFIG_PATH: &str = "/var/lib/nasty/watchdog.conf";
const SERVICE: &str = "nasty-watchdog.service";
const MAX_PING_HOSTS: usize = 16;

pub(crate) static WATCHDOG_LIFECYCLE_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchdogConfig {
    /// Maximum allowed one-minute load average. Zero disables load checks.
    #[serde(default)]
    pub max_load_1: u32,
    /// Maximum allowed five-minute load average. Zero disables load checks.
    #[serde(default)]
    pub max_load_5: u32,
    /// Maximum allowed fifteen-minute load average. Zero disables load checks.
    #[serde(default)]
    pub max_load_15: u32,
    /// Minimum reclaimable memory in MiB. Zero disables the memory check.
    #[serde(default)]
    pub min_memory_mib: u64,
    /// Numeric IPv4 addresses that must all respond to ICMP ping.
    #[serde(default)]
    pub ping_hosts: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchdogConfigUpdate {
    pub max_load_1: Option<u32>,
    pub max_load_5: Option<u32>,
    pub max_load_15: Option<u32>,
    pub min_memory_mib: Option<u64>,
    pub ping_hosts: Option<Vec<String>>,
}

pub struct WatchdogService;

impl Default for WatchdogService {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchdogService {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_config(&self) -> WatchdogConfig {
        load_config().await
    }

    pub async fn update_config(
        &self,
        update: WatchdogConfigUpdate,
    ) -> Result<WatchdogConfig, String> {
        let _guard = WATCHDOG_LIFECYCLE_LOCK.lock().await;
        let previous = load_config_strict().await?;
        let mut candidate = previous.clone();

        if let Some(value) = update.max_load_1 {
            candidate.max_load_1 = value;
        }
        if let Some(value) = update.max_load_5 {
            candidate.max_load_5 = value;
        }
        if let Some(value) = update.max_load_15 {
            candidate.max_load_15 = value;
        }
        if let Some(value) = update.min_memory_mib {
            candidate.min_memory_mib = value;
        }
        if let Some(value) = update.ping_hosts {
            candidate.ping_hosts = normalize_ping_hosts(value)?;
        }

        validate(&candidate)?;
        let was_active = crate::protocol::systemctl_is_active(SERVICE).await;
        if was_active {
            preflight(&candidate).await?;
        }
        let candidate_config = render_config(&candidate)?;
        let previous_config = render_config(&previous)?;

        write_config_content(&candidate_config).await?;
        if let Err(error) = save_config(&candidate).await {
            let rollback = write_config_content(&previous_config).await;
            return Err(match rollback {
                Ok(()) => format!("failed to persist watchdog configuration: {error}"),
                Err(rollback_error) => format!(
                    "failed to persist watchdog configuration: {error}; \
                     failed to restore daemon configuration: {rollback_error}"
                ),
            });
        }

        if was_active && let Err(restart_error) = restart_and_verify().await {
            let mut rollback_errors = Vec::new();
            if let Err(error) = write_config_content(&previous_config).await {
                rollback_errors.push(format!("restore daemon config: {error}"));
            }
            if let Err(error) = save_config(&previous).await {
                rollback_errors.push(format!("restore persisted config: {error}"));
            }
            if let Err(error) = restart_and_verify().await {
                rollback_errors.push(format!("restart previous config: {error}"));
            }

            if rollback_errors.is_empty() {
                return Err(format!(
                    "failed to restart watchdog with the new configuration: {restart_error}; \
                     the previous configuration was restored"
                ));
            }
            return Err(format!(
                "failed to restart watchdog with the new configuration: {restart_error}; \
                 rollback was incomplete: {}",
                rollback_errors.join("; ")
            ));
        }

        Ok(candidate)
    }
}

pub async fn load_config() -> WatchdogConfig {
    nasty_common::load_singleton_or_recover(STATE_PATH).await
}

async fn load_config_strict() -> Result<WatchdogConfig, String> {
    match tokio::fs::read_to_string(STATE_PATH).await {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {STATE_PATH}: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(WatchdogConfig::default()),
        Err(error) => Err(format!("failed to read {STATE_PATH}: {error}")),
    }
}

pub async fn write_config_file(config: &WatchdogConfig) -> Result<(), String> {
    let _guard = WATCHDOG_LIFECYCLE_LOCK.lock().await;
    write_config_file_locked(config).await
}

pub(crate) async fn write_config_file_locked(config: &WatchdogConfig) -> Result<(), String> {
    validate(config)?;
    write_config_content(&render_config(config)?).await
}

/// Refuse to arm checks that are already failing. Besides catching mistakes at
/// enable time, this prevents a watchdog-triggered reboot from immediately
/// arming the same unavailable ping target during boot restoration.
pub async fn preflight(config: &WatchdogConfig) -> Result<(), String> {
    validate(config)?;

    if config.max_load_1 != 0 {
        let loadavg = tokio::fs::read_to_string("/proc/loadavg")
            .await
            .map_err(|error| format!("failed to read current load average: {error}"))?;
        let current: Vec<f64> = loadavg
            .split_whitespace()
            .take(3)
            .map(|value| value.parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|error| format!("failed to parse current load average: {error}"))?;
        if current.len() != 3 {
            return Err("failed to read all current load averages".into());
        }
        for (label, actual, limit) in [
            ("1-minute", current[0], config.max_load_1),
            ("5-minute", current[1], config.max_load_5),
            ("15-minute", current[2], config.max_load_15),
        ] {
            if actual.floor() > f64::from(limit) {
                return Err(format!(
                    "cannot arm watchdog: current {label} load {actual:.2} exceeds threshold {limit}"
                ));
            }
        }
    }

    if config.min_memory_mib != 0 {
        let meminfo = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .map_err(|error| format!("failed to read current memory availability: {error}"))?;
        let mut reclaimable_kib = 0_u64;
        for line in meminfo.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            if matches!(key, "MemFree" | "Buffers" | "Cached") {
                let value = rest
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| format!("missing value for {key} in /proc/meminfo"))?
                    .parse::<u64>()
                    .map_err(|error| format!("invalid value for {key}: {error}"))?;
                reclaimable_kib = reclaimable_kib.saturating_add(value);
            }
        }
        let minimum_kib = config
            .min_memory_mib
            .checked_mul(1024)
            .ok_or_else(|| "minimum memory value is too large".to_string())?;
        if reclaimable_kib < minimum_kib {
            return Err(format!(
                "cannot arm watchdog: reclaimable memory is {} MiB, below the configured {} MiB minimum",
                reclaimable_kib / 1024,
                config.min_memory_mib
            ));
        }
    }

    let mut probes = tokio::task::JoinSet::new();
    for host in &config.ping_hosts {
        let host = host.clone();
        probes.spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(4),
                tokio::process::Command::new("ping")
                    .args(["-n", "-c", "1", "-W", "2", &host])
                    .output(),
            )
            .await;
            match result {
                Ok(Ok(output)) if output.status.success() => Ok(()),
                Ok(Ok(_)) => Err(format!("ping target {host} is not reachable")),
                Ok(Err(error)) => Err(format!("failed to probe ping target {host}: {error}")),
                Err(_) => Err(format!("ping target {host} timed out")),
            }
        });
    }
    while let Some(result) = probes.join_next().await {
        result.map_err(|error| format!("ping preflight task failed: {error}"))??;
    }

    Ok(())
}

fn validate(config: &WatchdogConfig) -> Result<(), String> {
    let loads = [config.max_load_1, config.max_load_5, config.max_load_15];
    let enabled_loads = loads.iter().filter(|&&value| value != 0).count();
    if enabled_loads != 0 && enabled_loads != loads.len() {
        return Err(
            "max_load_1, max_load_5, and max_load_15 must all be set or all be zero".into(),
        );
    }
    if let Some(value) = loads.into_iter().find(|&value| value == 1) {
        return Err(format!(
            "load thresholds must be zero (disabled) or at least 2; got {value}"
        ));
    }
    if config.ping_hosts.len() > MAX_PING_HOSTS {
        return Err(format!(
            "at most {MAX_PING_HOSTS} watchdog ping hosts may be configured"
        ));
    }
    for host in &config.ping_hosts {
        parse_ping_host(host)?;
    }
    memory_pages(config.min_memory_mib)?;
    Ok(())
}

fn normalize_ping_hosts(hosts: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for host in hosts {
        let trimmed = host.trim();
        if trimmed.is_empty() {
            continue;
        }
        let address = parse_ping_host(trimmed)?.to_string();
        if seen.insert(address.clone()) {
            normalized.push(address);
        }
    }
    if normalized.len() > MAX_PING_HOSTS {
        return Err(format!(
            "at most {MAX_PING_HOSTS} watchdog ping hosts may be configured"
        ));
    }
    Ok(normalized)
}

fn parse_ping_host(host: &str) -> Result<Ipv4Addr, String> {
    let address = host
        .parse::<Ipv4Addr>()
        .map_err(|_| format!("watchdog ping host must be a numeric IPv4 address: {host}"))?;
    if address.is_unspecified() || address.is_multicast() || address == Ipv4Addr::BROADCAST {
        return Err(format!(
            "watchdog ping host is not a usable unicast address: {host}"
        ));
    }
    Ok(address)
}

fn memory_pages(min_memory_mib: u64) -> Result<u64, String> {
    if min_memory_mib == 0 {
        return Ok(0);
    }
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return Err("failed to determine system memory page size".into());
    }
    let bytes = min_memory_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| "minimum memory value is too large".to_string())?;
    let pages = bytes.div_ceil(page_size as u64);
    if pages > i32::MAX as u64 {
        return Err("minimum memory value is too large for watchdog".into());
    }
    Ok(pages)
}

fn render_config(config: &WatchdogConfig) -> Result<String, String> {
    validate(config)?;
    let mut output = String::from(
        "# Managed by NASty - do not edit manually\n\
         interval = 10\n\
         realtime = yes\n\
         priority = 1\n\
         retry-timeout = 60\n\
         repair-maximum = 1\n\
         ping-count = 3\n",
    );
    writeln!(output, "max-load-1 = {}", config.max_load_1).unwrap();
    writeln!(output, "max-load-5 = {}", config.max_load_5).unwrap();
    writeln!(output, "max-load-15 = {}", config.max_load_15).unwrap();
    writeln!(
        output,
        "min-memory = {}",
        memory_pages(config.min_memory_mib)?
    )
    .unwrap();
    for host in &config.ping_hosts {
        writeln!(output, "ping = {host}").unwrap();
    }
    Ok(output)
}

async fn save_config(config: &WatchdogConfig) -> Result<(), String> {
    nasty_common::StateDir::new(STATE_DIR)
        .save("watchdog", config)
        .await
        .map_err(|error| error.to_string())
}

async fn write_config_content(content: &str) -> Result<(), String> {
    tokio::fs::create_dir_all(STATE_DIR)
        .await
        .map_err(|error| format!("failed to create {STATE_DIR}: {error}"))?;
    let temporary = format!("{CONFIG_PATH}.tmp");
    tokio::fs::write(&temporary, content)
        .await
        .map_err(|error| format!("failed to write {temporary}: {error}"))?;
    tokio::fs::rename(&temporary, CONFIG_PATH)
        .await
        .map_err(|error| format!("failed to replace {CONFIG_PATH}: {error}"))
}

async fn restart_and_verify() -> Result<(), String> {
    crate::protocol::systemctl("restart", SERVICE).await?;
    verify_running().await
}

pub(crate) async fn verify_running() -> Result<(), String> {
    tokio::time::sleep(Duration::from_secs(2)).await;
    if crate::protocol::systemctl_is_active(SERVICE).await {
        Ok(())
    } else {
        Err("watchdog service exited after restart".into())
    }
}

pub(crate) async fn stop_and_verify() -> Result<(), String> {
    let _ = crate::protocol::systemctl("stop", SERVICE).await;
    if wait_until_stopped(Duration::from_secs(2)).await.is_err() {
        let output = tokio::process::Command::new("systemctl")
            .args(["kill", "--kill-who=all", "--signal=SIGKILL", SERVICE])
            .output()
            .await
            .map_err(|error| format!("failed to force-stop watchdog: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "failed to force-stop watchdog: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        // A SIGKILL is considered a service failure and may queue Restart=.
        // Stop again to cancel that job before checking the exact unit state.
        let _ = crate::protocol::systemctl("stop", SERVICE).await;
        wait_until_stopped(Duration::from_secs(3)).await?;
    }

    // Clear a failed/start-limited state so a later explicit enable starts cleanly.
    let _ = crate::protocol::systemctl("reset-failed", SERVICE).await;
    wait_until_stopped(Duration::from_secs(1)).await
}

async fn wait_until_stopped(timeout: Duration) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match unit_state().await? {
            (load, _) if load == "not-found" => return Ok(()),
            (_, active) if matches!(active.as_str(), "inactive" | "failed") => return Ok(()),
            (load, active) if tokio::time::Instant::now() >= deadline => {
                return Err(format!(
                    "watchdog did not stop (LoadState={load}, ActiveState={active})"
                ));
            }
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn unit_state() -> Result<(String, String), String> {
    let output = tokio::process::Command::new("systemctl")
        .args(["show", SERVICE, "-p", "LoadState", "-p", "ActiveState"])
        .output()
        .await
        .map_err(|error| format!("failed to query watchdog service state: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to query watchdog service state: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut load = None;
    let mut active = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("LoadState=") {
            load = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("ActiveState=") {
            active = Some(value.to_string());
        }
    }
    match (load, active) {
        (Some(load), Some(active)) => Ok((load, active)),
        _ => Err("systemctl returned incomplete watchdog service state".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_disable_configurable_checks_and_hardware_access() {
        let config = WatchdogConfig::default();
        let rendered = render_config(&config).unwrap();
        assert!(rendered.contains("max-load-1 = 0\n"));
        assert!(rendered.contains("max-load-5 = 0\n"));
        assert!(rendered.contains("max-load-15 = 0\n"));
        assert!(rendered.contains("min-memory = 0\n"));
        assert!(!rendered.contains("\nping = "));
        assert!(!rendered.contains("watchdog-device"));
    }

    #[test]
    fn renders_load_memory_and_ping_checks() {
        let config = WatchdogConfig {
            max_load_1: 24,
            max_load_5: 18,
            max_load_15: 12,
            min_memory_mib: 64,
            ping_hosts: vec!["192.0.2.1".into(), "198.51.100.1".into()],
        };
        let rendered = render_config(&config).unwrap();
        assert!(rendered.contains("max-load-1 = 24\n"));
        assert!(rendered.contains(&format!("min-memory = {}\n", memory_pages(64).unwrap())));
        assert!(rendered.contains("ping = 192.0.2.1\n"));
        assert!(rendered.contains("ping = 198.51.100.1\n"));
    }

    #[test]
    fn rejects_partial_or_too_low_load_thresholds() {
        let partial = WatchdogConfig {
            max_load_1: 24,
            ..Default::default()
        };
        assert!(validate(&partial).is_err());

        let too_low = WatchdogConfig {
            max_load_1: 2,
            max_load_5: 1,
            max_load_15: 2,
            ..Default::default()
        };
        assert!(validate(&too_low).is_err());
    }

    #[test]
    fn normalizes_and_deduplicates_ipv4_targets() {
        let hosts = normalize_ping_hosts(vec![
            " 192.0.2.1 ".into(),
            "192.0.2.1".into(),
            "198.51.100.2".into(),
            String::new(),
        ])
        .unwrap();
        assert_eq!(hosts, vec!["192.0.2.1", "198.51.100.2"]);
        assert!(normalize_ping_hosts(vec!["example.com".into()]).is_err());
        assert!(normalize_ping_hosts(vec!["2001:db8::1".into()]).is_err());
        assert!(normalize_ping_hosts(vec!["255.255.255.255".into()]).is_err());
    }

    #[test]
    fn missing_fields_load_as_disabled_defaults() {
        let config: WatchdogConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, WatchdogConfig::default());
    }
}
