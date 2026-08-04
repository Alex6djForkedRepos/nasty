//! Physical block-device I/O scheduler management.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::disk_type::IdentityResolver;

const STATE_PATH: &str = "/var/lib/nasty/io-scheduler-overrides.json";
const LEGACY_FS_STATE_PATH: &str = "/var/lib/nasty/fs-state.json";
const SYS_CLASS_BLOCK: &str = "/sys/class/block";

/// Serializes scheduler mutations and state-file migration.
static MUTATION_LOCK: Mutex<()> = Mutex::const_new(());

type Overrides = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IoSchedulerState {
    /// Scheduler currently selected by the kernel.
    pub active: String,
    /// Schedulers advertised by this device's queue.
    pub available: Vec<String>,
    /// Scheduler NASty will restore at startup, if managed.
    pub configured: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct IoSchedulerUpdate {
    /// A whole-disk path, partition path, or `/dev/disk/*` alias.
    pub path: String,
    /// Scheduler to manage, or `None` to stop managing without changing it.
    pub scheduler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IoSchedulerResult {
    /// Canonical whole-device path that owns the queue.
    pub path: String,
    /// Stable identity used for persistence.
    pub stable_id: String,
    /// Durability of `stable_id`: `hardware`, `slot`, or `volatile`.
    pub id_kind: String,
    pub io_scheduler: IoSchedulerState,
}

#[derive(Debug, Clone)]
struct ResolvedDevice {
    name: String,
    path: String,
    scheduler_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyClaim {
    stable_id: String,
    scheduler: String,
}

/// Set or clear management for a physical device queue.
pub async fn set(update: IoSchedulerUpdate) -> Result<IoSchedulerResult, String> {
    let _guard = MUTATION_LOCK.lock().await;
    let device = resolve_device(&update.path).await?;
    let resolver = IdentityResolver::new().await;
    let (stable_id, id_kind) = resolver.resolve(&device.name);
    let mut overrides = load_overrides().await;

    let io_scheduler = match update.scheduler {
        None => {
            let mut state = read_state(&device, None).await?;
            if overrides.remove(&stable_id).is_some() {
                save_overrides(&overrides).await?;
            }
            state.configured = None;
            info!(
                "Stopped managing I/O scheduler for {} (stable id '{}')",
                device.path, stable_id
            );
            state
        }
        Some(scheduler) => {
            let advertised = read_state(&device, overrides.get(&stable_id).cloned()).await?;
            if !advertised.available.iter().any(|value| value == &scheduler) {
                return Err(format!(
                    "I/O scheduler '{scheduler}' is not available for {}; advertised values: {}",
                    device.path,
                    advertised.available.join(", ")
                ));
            }

            let previous_active = advertised.active;
            let mut state = write_and_confirm(&device, &scheduler).await?;
            overrides.insert(stable_id.clone(), scheduler.clone());
            if let Err(save_error) = save_overrides(&overrides).await {
                let rollback = write_and_confirm(&device, &previous_active).await;
                return Err(match rollback {
                    Ok(_) => format!(
                        "persist I/O scheduler override: {save_error}; restored active scheduler '{previous_active}'"
                    ),
                    Err(rollback_error) => format!(
                        "persist I/O scheduler override: {save_error}; additionally failed to restore active scheduler '{previous_active}': {rollback_error}"
                    ),
                });
            }

            state.configured = Some(scheduler.clone());
            info!(
                "Set I/O scheduler for {} (stable id '{}') to '{}'",
                device.path, stable_id, scheduler
            );
            state
        }
    };

    Ok(IoSchedulerResult {
        path: device.path,
        stable_id,
        id_kind: id_kind.to_string(),
        io_scheduler,
    })
}

/// Read scheduler state for the whole devices in an `lsblk` listing pass.
pub(crate) async fn states_for_device_names(
    resolver: &IdentityResolver,
    names: &[String],
) -> HashMap<String, IoSchedulerState> {
    let overrides = load_overrides().await;
    let mut states = HashMap::new();
    for name in names {
        let configured = overrides.get(&resolver.resolve(name).0).cloned();
        let device = ResolvedDevice {
            name: name.clone(),
            path: format!("/dev/{name}"),
            scheduler_path: Path::new(SYS_CLASS_BLOCK)
                .join(name)
                .join("queue/scheduler"),
        };
        if let Ok(state) = read_state(&device, configured).await {
            states.insert(name.clone(), state);
        }
    }
    states
}

/// Restore every currently resolvable persisted override. Failures are isolated
/// per disk so one missing or changed device never blocks engine startup.
pub async fn restore() {
    let _guard = MUTATION_LOCK.lock().await;
    let overrides = load_overrides().await;
    if overrides.is_empty() {
        return;
    }

    let resolver = IdentityResolver::new().await;
    let devices = match enumerate_queue_owners().await {
        Ok(devices) => devices,
        Err(error) => {
            warn!("Cannot enumerate block devices for I/O scheduler restore: {error}");
            return;
        }
    };
    let mut matched = BTreeSet::new();

    for device in devices.values() {
        let (stable_id, _) = resolver.resolve(&device.name);
        let Some(scheduler) = overrides.get(&stable_id) else {
            continue;
        };
        matched.insert(stable_id.clone());
        match write_and_confirm(device, scheduler).await {
            Ok(_) => info!(
                "Restored I/O scheduler '{}' on {} (stable id '{}')",
                scheduler, device.path, stable_id
            ),
            Err(error) => warn!("I/O scheduler restore failed for {}: {error}", device.path),
        }
    }

    for stable_id in overrides.keys() {
        if !matched.contains(stable_id) {
            warn!("No usable block device found for I/O scheduler override '{stable_id}'");
        }
    }
}

/// Migrate concrete filesystem-level scheduler settings to physical stable IDs.
/// The new state is durably renamed into place before legacy fields are removed.
pub async fn migrate_legacy() {
    let _guard = MUTATION_LOCK.lock().await;
    if let Err(error) = migrate_legacy_inner().await {
        warn!("Legacy I/O scheduler migration failed: {error}");
    }
}

async fn migrate_legacy_inner() -> Result<(), String> {
    let content = match tokio::fs::read_to_string(LEGACY_FS_STATE_PATH).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read {LEGACY_FS_STATE_PATH}: {error}")),
    };
    let mut state: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("parse {LEGACY_FS_STATE_PATH}: {error}"))?;
    let entries = state
        .as_object()
        .ok_or_else(|| format!("{LEGACY_FS_STATE_PATH} is not a JSON object"))?;
    if !entries.values().any(|options| {
        options
            .get("io_scheduler")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.is_empty())
    }) {
        return Ok(());
    }

    match tokio::process::Command::new("udevadm")
        .args(["settle", "--timeout=30"])
        .status()
        .await
    {
        Ok(status) if status.success() => {}
        Ok(status) => warn!(
            "udevadm settle exited {status} before legacy I/O scheduler migration; continuing"
        ),
        Err(error) => warn!(
            "udevadm settle failed before legacy I/O scheduler migration: {error}; continuing"
        ),
    }

    let resolver = IdentityResolver::new().await;
    let mut claims = Vec::new();
    let mut claims_by_entry = BTreeMap::new();
    let mut fully_resolved_entries = BTreeSet::new();

    for (name, options) in entries {
        let Some(scheduler) = options
            .get("io_scheduler")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let mut entry_claims = Vec::new();
        let mut fully_resolved = false;
        if let Some(devices) = options.get("devices").and_then(serde_json::Value::as_array) {
            fully_resolved = !devices.is_empty();
            for value in devices {
                let Some(path) = value.as_str() else {
                    fully_resolved = false;
                    continue;
                };
                let Ok(device) = resolve_device(path).await else {
                    fully_resolved = false;
                    continue;
                };
                entry_claims.push(LegacyClaim {
                    stable_id: resolver.resolve(&device.name).0,
                    scheduler: scheduler.to_string(),
                });
            }
        }
        if fully_resolved {
            fully_resolved_entries.insert(name.clone());
        }
        claims.extend(entry_claims.iter().cloned());
        claims_by_entry.insert(name.clone(), entry_claims);
    }

    if claims.is_empty() {
        return Ok(());
    }

    let existing = load_overrides().await;
    let (merged, conflicts) = merge_legacy_claims(&existing, &claims);
    if merged != existing {
        save_overrides(&merged).await?;
    }

    let migrated_entries =
        completed_legacy_entries(fully_resolved_entries, &claims_by_entry, &conflicts);
    if migrated_entries.is_empty() {
        return Ok(());
    }

    remove_migrated_fields(&mut state, &migrated_entries);
    atomic_write_json(LEGACY_FS_STATE_PATH, &state).await?;
    info!(
        "Migrated legacy I/O scheduler settings from {} filesystem entries",
        migrated_entries.len()
    );
    Ok(())
}

fn remove_migrated_fields(state: &mut serde_json::Value, migrated_entries: &[String]) {
    let Some(entries) = state.as_object_mut() else {
        return;
    };
    for name in migrated_entries {
        if let Some(options) = entries
            .get_mut(name)
            .and_then(serde_json::Value::as_object_mut)
        {
            options.remove("io_scheduler");
        }
    }
}

fn completed_legacy_entries(
    fully_resolved_entries: BTreeSet<String>,
    claims_by_entry: &BTreeMap<String, Vec<LegacyClaim>>,
    conflicts: &BTreeSet<String>,
) -> Vec<String> {
    fully_resolved_entries
        .into_iter()
        .filter(|name| {
            claims_by_entry.get(name).is_some_and(|entry_claims| {
                entry_claims
                    .iter()
                    .all(|claim| !conflicts.contains(&claim.stable_id))
            })
        })
        .collect()
}

fn merge_legacy_claims(
    existing: &Overrides,
    claims: &[LegacyClaim],
) -> (Overrides, BTreeSet<String>) {
    let mut merged = existing.clone();
    let mut conflicts = BTreeSet::new();
    let mut by_disk: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for claim in claims {
        by_disk
            .entry(&claim.stable_id)
            .or_default()
            .insert(&claim.scheduler);
    }

    for (stable_id, schedulers) in by_disk {
        if existing.contains_key(stable_id) {
            continue;
        }
        if schedulers.len() == 1 {
            merged.insert(
                stable_id.to_string(),
                schedulers.into_iter().next().unwrap().to_string(),
            );
        } else {
            merged.remove(stable_id);
            conflicts.insert(stable_id.to_string());
            warn!(
                "Conflicting legacy I/O scheduler claims for stable disk '{}'; leaving it unmanaged",
                stable_id
            );
        }
    }
    (merged, conflicts)
}

async fn resolve_device(path: &str) -> Result<ResolvedDevice, String> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| format!("resolve block device path '{path}': {error}"))?;
    let metadata = tokio::fs::metadata(&canonical).await.map_err(|error| {
        format!(
            "inspect block device path '{}': {error}",
            canonical.display()
        )
    })?;
    if !metadata.file_type().is_block_device() {
        return Err(format!("path '{path}' is not a block device"));
    }
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("block device path '{path}' has no valid device name"))?;
    resolve_sysfs_name(name).await
}

async fn resolve_sysfs_name(name: &str) -> Result<ResolvedDevice, String> {
    let class_path = Path::new(SYS_CLASS_BLOCK).join(name);
    let target = tokio::fs::canonicalize(&class_path)
        .await
        .map_err(|error| format!("resolve {}: {error}", class_path.display()))?;
    let is_partition = tokio::fs::metadata(target.join("partition")).await.is_ok();
    let owner_target = queue_owner_target(&target, is_partition)?;
    let owner_name = owner_target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("cannot determine queue owner for /dev/{name}"))?
        .to_string();
    let scheduler_path = Path::new(SYS_CLASS_BLOCK)
        .join(&owner_name)
        .join("queue/scheduler");
    tokio::fs::metadata(&scheduler_path)
        .await
        .map_err(|error| {
            format!(
                "{} has no scheduler queue: {error}",
                scheduler_path.display()
            )
        })?;

    Ok(ResolvedDevice {
        path: format!("/dev/{owner_name}"),
        name: owner_name,
        scheduler_path,
    })
}

fn queue_owner_target(target: &Path, is_partition: bool) -> Result<&Path, String> {
    if is_partition {
        target
            .parent()
            .ok_or_else(|| format!("partition sysfs path {} has no parent", target.display()))
    } else {
        Ok(target)
    }
}

async fn enumerate_queue_owners() -> Result<BTreeMap<String, ResolvedDevice>, String> {
    let mut entries = tokio::fs::read_dir(SYS_CLASS_BLOCK)
        .await
        .map_err(|error| format!("read {SYS_CLASS_BLOCK}: {error}"))?;
    let mut devices = BTreeMap::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| format!("read {SYS_CLASS_BLOCK}: {error}"))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(device) = resolve_sysfs_name(&name).await {
            devices.entry(device.name.clone()).or_insert(device);
        }
    }
    Ok(devices)
}

async fn read_state(
    device: &ResolvedDevice,
    configured: Option<String>,
) -> Result<IoSchedulerState, String> {
    let content = tokio::fs::read_to_string(&device.scheduler_path)
        .await
        .map_err(|error| format!("read {}: {error}", device.scheduler_path.display()))?;
    let (active, available) = parse_scheduler_content(&content)?;
    Ok(IoSchedulerState {
        active,
        available,
        configured,
    })
}

fn parse_scheduler_content(content: &str) -> Result<(String, Vec<String>), String> {
    let mut active = None;
    let mut available = Vec::new();
    for token in content.split_whitespace() {
        let bracketed = token.starts_with('[') && token.ends_with(']');
        let value = if bracketed {
            &token[1..token.len() - 1]
        } else {
            token
        };
        if value.is_empty() || token.starts_with('[') != token.ends_with(']') {
            return Err(format!("invalid scheduler queue content: {content:?}"));
        }
        if bracketed && active.replace(value.to_string()).is_some() {
            return Err(format!(
                "multiple active schedulers in queue content: {content:?}"
            ));
        }
        if !available.iter().any(|candidate| candidate == value) {
            available.push(value.to_string());
        }
    }
    let active =
        active.ok_or_else(|| format!("no active scheduler in queue content: {content:?}"))?;
    Ok((active, available))
}

async fn write_and_confirm(
    device: &ResolvedDevice,
    scheduler: &str,
) -> Result<IoSchedulerState, String> {
    tokio::fs::write(&device.scheduler_path, scheduler)
        .await
        .map_err(|error| format!("write {}: {error}", device.scheduler_path.display()))?;
    let state = read_state(device, None).await?;
    if state.active != scheduler {
        return Err(format!(
            "kernel did not activate I/O scheduler '{scheduler}' on {}; active scheduler is '{}'",
            device.path, state.active
        ));
    }
    Ok(state)
}

async fn load_overrides() -> Overrides {
    nasty_common::load_singleton_or_recover(STATE_PATH).await
}

async fn save_overrides(overrides: &Overrides) -> Result<(), String> {
    atomic_write_json(STATE_PATH, overrides).await
}

async fn atomic_write_json<T: Serialize + ?Sized>(path: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize state for {path}: {error}"))?;
    if let Some(parent) = Path::new(path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let temporary = format!("{path}.tmp");
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|error| format!("create {temporary}: {error}"))?;
    file.write_all(&bytes)
        .await
        .map_err(|error| format!("write {temporary}: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("sync {temporary}: {error}"))?;
    drop(file);
    tokio::fs::rename(&temporary, path)
        .await
        .map_err(|error| format!("rename {temporary} to {path}: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dynamic_scheduler_names_and_active_value() {
        let (active, available) =
            parse_scheduler_content("none mq-deadline [bfq] custom-scheduler\n").unwrap();
        assert_eq!(active, "bfq");
        assert_eq!(
            available,
            vec!["none", "mq-deadline", "bfq", "custom-scheduler"]
        );
    }

    #[test]
    fn rejects_scheduler_content_without_an_active_value() {
        assert!(parse_scheduler_content("none mq-deadline bfq").is_err());
    }

    #[test]
    fn partition_queue_owner_uses_sysfs_parent_without_name_heuristics() {
        let nvme = Path::new("/sys/devices/pci/block/nvme12n34/nvme12n34p56");
        assert_eq!(
            queue_owner_target(nvme, true).unwrap(),
            Path::new("/sys/devices/pci/block/nvme12n34")
        );
        let digit_ending_disk = Path::new("/sys/devices/virtual/block/md127");
        assert_eq!(
            queue_owner_target(digit_ending_disk, false).unwrap(),
            digit_ending_disk
        );
    }

    #[test]
    fn migration_deduplicates_identical_claims() {
        let claims = vec![
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "bfq".into(),
            },
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "bfq".into(),
            },
        ];
        assert_eq!(
            merge_legacy_claims(&Overrides::new(), &claims),
            (
                Overrides::from([("disk-a".into(), "bfq".into())]),
                BTreeSet::new()
            )
        );
    }

    #[test]
    fn migration_leaves_conflicting_disk_unmanaged() {
        let claims = vec![
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "none".into(),
            },
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "bfq".into(),
            },
        ];
        assert_eq!(
            merge_legacy_claims(&Overrides::new(), &claims),
            (Overrides::new(), BTreeSet::from(["disk-a".into()]))
        );
    }

    #[test]
    fn existing_override_wins_over_legacy_claims() {
        let existing = Overrides::from([("disk-a".into(), "kyber".into())]);
        let claims = vec![
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "none".into(),
            },
            LegacyClaim {
                stable_id: "disk-a".into(),
                scheduler: "bfq".into(),
            },
        ];
        assert_eq!(
            merge_legacy_claims(&existing, &claims),
            (existing, BTreeSet::new())
        );
    }

    #[test]
    fn migration_only_completes_fully_resolved_nonconflicting_entries() {
        let claims_by_entry = BTreeMap::from([
            (
                "complete".into(),
                vec![LegacyClaim {
                    stable_id: "disk-a".into(),
                    scheduler: "bfq".into(),
                }],
            ),
            (
                "conflicting".into(),
                vec![LegacyClaim {
                    stable_id: "disk-b".into(),
                    scheduler: "none".into(),
                }],
            ),
            (
                "partial".into(),
                vec![LegacyClaim {
                    stable_id: "disk-c".into(),
                    scheduler: "bfq".into(),
                }],
            ),
        ]);

        assert_eq!(
            completed_legacy_entries(
                BTreeSet::from(["complete".into(), "conflicting".into()]),
                &claims_by_entry,
                &BTreeSet::from(["disk-b".into()]),
            ),
            vec!["complete"]
        );
    }

    #[tokio::test]
    async fn rejects_non_block_device_paths() {
        let error = resolve_device("/dev/null").await.unwrap_err();
        assert!(error.contains("is not a block device"));
    }

    #[test]
    fn migration_removes_only_selected_legacy_fields() {
        let mut state = serde_json::json!({
            "tank": {
                "uuid": "uuid-a",
                "devices": ["/dev/sda1"],
                "io_scheduler": "bfq",
                "future_option": {"nested": true}
            },
            "offline": {
                "devices": ["/dev/missing"],
                "io_scheduler": "none"
            },
            "top_level_future": [1, 2, 3]
        });

        remove_migrated_fields(&mut state, &["tank".to_string()]);

        assert!(state["tank"].get("io_scheduler").is_none());
        assert_eq!(state["tank"]["future_option"]["nested"], true);
        assert_eq!(state["offline"]["io_scheduler"], "none");
        assert_eq!(state["top_level_future"], serde_json::json!([1, 2, 3]));
    }
}
