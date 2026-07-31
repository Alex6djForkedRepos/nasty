use std::sync::Arc;

use rand::RngExt;
use serde::Serialize;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::AppState;

const TELEMETRY_URL: &str = "https://nasty-telemetry.nasty-project.workers.dev/api/report";
const TELEMETRY_ID_PATH: &str = "/var/lib/nasty/telemetry-id";
const TELEMETRY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

#[derive(Serialize)]
struct Report {
    instance_id: String,
    drives: usize,
    total_bytes: u64,
    used_bytes: u64,
    version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    vms: usize,
    apps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    smb_shares: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nfs_exports: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iscsi_luns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nvmeof_namespaces: Option<usize>,
    arch: &'static str,
}

/// Short git SHA this engine was built from. `None` for dev cargo
/// builds outside Nix where `NASTY_GIT_SHA` wasn't injected. Matches
/// the 7-char form used elsewhere (see `nasty-system::update`).
/// Reused by `/health` and `--version` so all three report the same
/// commit from the same compile-time source.
pub(crate) fn build_commit() -> Option<String> {
    let raw = option_env!("NASTY_GIT_SHA")?.trim();
    if raw.is_empty() || raw == "unknown" {
        return None;
    }
    Some(raw[..7.min(raw.len())].to_string())
}

/// Get or create the persistent instance ID.
async fn instance_id() -> Option<String> {
    if let Ok(id) = tokio::fs::read_to_string(TELEMETRY_ID_PATH).await {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Err(e) = tokio::fs::write(TELEMETRY_ID_PATH, &id).await {
        warn!("Failed to write telemetry ID: {e}");
        return None;
    }
    info!("Generated telemetry instance ID");
    Some(id)
}

/// Collect current stats from mounted bcachefs filesystems.
async fn collect_report(state: &AppState) -> Option<Report> {
    let id = instance_id().await?;

    let filesystems = match state.filesystems.list().await {
        Ok(fs) => fs,
        Err(e) => {
            debug!("Failed to list filesystems for telemetry: {e}");
            return None;
        }
    };
    let mounted: Vec<_> = filesystems.iter().filter(|fs| fs.mounted).collect();

    if mounted.is_empty() {
        debug!("No mounted bcachefs filesystems, skipping telemetry report");
        return None;
    }

    let mut drives: usize = 0;
    let mut total_bytes: u64 = 0;
    let mut used_bytes: u64 = 0;

    for fs in &mounted {
        drives += fs.devices.len();
        total_bytes += fs.total_bytes;
        used_bytes += fs.used_bytes;
    }

    let vms = state.vms.list().await.map(|v| v.len()).unwrap_or(0);
    let apps = state.apps.list().await.map(|a| a.len()).unwrap_or(0);
    let (smb, nfs, iscsi, nvmeof) = tokio::join!(
        state.smb.list_strict(),
        state.nfs.list_strict(),
        state.iscsi.list(),
        state.nvmeof.list(),
    );
    let smb_shares = smb
        .inspect_err(|e| debug!("Failed to count SMB shares for telemetry: {e}"))
        .ok()
        .map(|shares| shares.len());
    let nfs_exports = nfs
        .inspect_err(|e| debug!("Failed to count NFS exports for telemetry: {e}"))
        .ok()
        .map(|exports| exports.len());
    let iscsi_luns = iscsi
        .inspect_err(|e| debug!("Failed to count iSCSI LUNs for telemetry: {e}"))
        .ok()
        .map(|targets| targets.iter().map(|target| target.luns.len()).sum());
    let nvmeof_namespaces = nvmeof
        .inspect_err(|e| debug!("Failed to count NVMe-oF namespaces for telemetry: {e}"))
        .ok()
        .map(|subsystems| {
            subsystems
                .iter()
                .map(|subsystem| subsystem.namespaces.len())
                .sum()
        });

    Some(Report {
        instance_id: id,
        drives,
        total_bytes,
        used_bytes,
        version: env!("CARGO_PKG_VERSION"),
        commit: build_commit(),
        vms,
        apps,
        smb_shares,
        nfs_exports,
        iscsi_luns,
        nvmeof_namespaces,
        arch: std::env::consts::ARCH,
    })
}

/// Send a telemetry report. Returns true on success.
pub async fn send_report(state: &AppState) -> bool {
    if !state.settings.get().await.telemetry_enabled {
        debug!("Telemetry disabled, skipping report");
        return false;
    }

    let report = match collect_report(state).await {
        Some(r) => r,
        None => return false,
    };

    debug!(
        "Sending telemetry: drives={}, total={}B, used={}B, vms={}, apps={}, smb={:?}, nfs={:?}, iscsi={:?}, nvmeof={:?}, arch={}, version={}, commit={:?}",
        report.drives,
        report.total_bytes,
        report.used_bytes,
        report.vms,
        report.apps,
        report.smb_shares,
        report.nfs_exports,
        report.iscsi_luns,
        report.nvmeof_namespaces,
        report.arch,
        report.version,
        report.commit
    );

    match state
        .metrics_client
        .post(TELEMETRY_URL)
        .json(&report)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            debug!("Telemetry report sent successfully");
            true
        }
        Ok(resp) => {
            debug!("Telemetry report rejected: {}", resp.status());
            false
        }
        Err(e) => {
            debug!("Telemetry report failed: {e}");
            false
        }
    }
}

/// Spawn the daily telemetry background task.
pub fn spawn_daily(state: Arc<AppState>) {
    let h = tokio::spawn(async move {
        // Random initial delay (0-24h) to spread load across instances
        let jitter = rand::rng().random_range(0..TELEMETRY_INTERVAL.as_secs());
        debug!("Telemetry: first report in {}s", jitter);
        tokio::time::sleep(Duration::from_secs(jitter)).await;

        let mut ticker = interval(TELEMETRY_INTERVAL);
        loop {
            ticker.tick().await;
            send_report(&state).await;
        }
    });
    // Observer spawn — telemetry loop is supposed to run forever; if
    // it exits (cleanly or by panic) we want a single log line so the
    // user can see why telemetry stopped reporting.
    tokio::spawn(async move {
        match h.await {
            Ok(()) => tracing::warn!("telemetry loop exited unexpectedly"),
            Err(e) => tracing::warn!("telemetry loop panicked / cancelled: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_protocol_export_counts() {
        let value = serde_json::to_value(Report {
            instance_id: "instance".to_string(),
            drives: 2,
            total_bytes: 100,
            used_bytes: 50,
            version: "1.2.3",
            commit: None,
            vms: 1,
            apps: 2,
            smb_shares: Some(3),
            nfs_exports: Some(4),
            iscsi_luns: Some(5),
            nvmeof_namespaces: Some(6),
            arch: "x86_64",
        })
        .unwrap();

        assert_eq!(value["smb_shares"], 3);
        assert_eq!(value["nfs_exports"], 4);
        assert_eq!(value["iscsi_luns"], 5);
        assert_eq!(value["nvmeof_namespaces"], 6);
        assert!(value.get("commit").is_none());
    }

    #[test]
    fn report_omits_unobserved_protocol_counts() {
        let value = serde_json::to_value(Report {
            instance_id: "instance".to_string(),
            drives: 2,
            total_bytes: 100,
            used_bytes: 50,
            version: "1.2.3",
            commit: None,
            vms: 1,
            apps: 2,
            smb_shares: None,
            nfs_exports: None,
            iscsi_luns: None,
            nvmeof_namespaces: None,
            arch: "x86_64",
        })
        .unwrap();

        assert!(value.get("smb_shares").is_none());
        assert!(value.get("nfs_exports").is_none());
        assert!(value.get("iscsi_luns").is_none());
        assert!(value.get("nvmeof_namespaces").is_none());
    }
}
