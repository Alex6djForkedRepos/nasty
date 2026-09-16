//! Persistent POSIX-cron schedules and admission decisions for bcachefs scrub.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use cron::Schedule;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{FilesystemError, FilesystemService};

const SCRUB_SCHEDULES_PATH: &str = "/var/lib/nasty/scrub-schedules.json";
pub const TICK_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ScrubScheduleUpdate {
    pub name: String,
    /// Five-field POSIX cron in UTC. Null or whitespace disables scheduling.
    #[serde(deserialize_with = "deserialize_required_schedule")]
    #[schemars(required, schema_with = "nullable_string_schema")]
    pub schedule: Option<String>,
}

fn deserialize_required_schedule<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

fn nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    <Option<String>>::json_schema(generator)
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScrubScheduleStatus {
    pub name: String,
    pub schedule: Option<String>,
    /// Next nominal cron occurrence after evaluation time in RFC3339 UTC, or
    /// null for manual-only/error state. A retained due occurrence may run
    /// earlier once another pool's scrub finishes.
    pub next_run_at: Option<String>,
    pub schedule_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum ScrubScheduleError {
    #[error("invalid scrub schedule: {0}")]
    Invalid(String),
    #[error("scrub schedule state is unavailable until the engine restarts: {0}")]
    Unavailable(String),
    #[error("failed to persist scrub schedules: {0}")]
    Persist(String),
    #[error("scrub schedule update was applied, but persistence durability is not guaranteed: {0}")]
    AppliedButNotDurable(String),
    #[error(transparent)]
    Filesystem(#[from] FilesystemError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredSchedule {
    schedule: String,
    /// A unique revision, rather than the expression itself, so A -> B -> A
    /// between scheduler ticks still resets the cursor.
    version: String,
    /// Successful update's wall-clock anchor. The scheduler considers an
    /// occurrence after this instant even if its next poll crosses a boundary.
    updated_at: DateTime<Utc>,
}

enum StoreState {
    Available(HashMap<String, StoredSchedule>),
    Unavailable(String),
}

#[derive(Clone)]
pub struct ScrubScheduleService {
    path: Arc<PathBuf>,
    state: Arc<Mutex<StoreState>>,
    update_admission: Arc<Mutex<()>>,
}

impl Default for ScrubScheduleService {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrubScheduleService {
    pub fn new() -> Self {
        Self::new_at(SCRUB_SCHEDULES_PATH)
    }

    fn new_at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let state = match load_store(&path) {
            Ok(records) => StoreState::Available(records),
            Err(error) => {
                warn!(path = %path.display(), "Scrub schedule state is unavailable: {error}");
                StoreState::Unavailable(error)
            }
        };
        Self {
            path: Arc::new(path),
            state: Arc::new(Mutex::new(state)),
            update_admission: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get(
        &self,
        name: &str,
        filesystem_uuid: &str,
    ) -> Result<ScrubScheduleStatus, ScrubScheduleError> {
        let state = self.state.lock().await;
        let records = available(&state)?;
        Ok(status_from_record(
            name,
            records.get(filesystem_uuid),
            Utc::now(),
        ))
    }

    #[cfg(test)]
    async fn update(
        &self,
        filesystem_uuid: &str,
        update: ScrubScheduleUpdate,
    ) -> Result<ScrubScheduleStatus, ScrubScheduleError> {
        let _update_admission = self.update_admission.lock().await;
        self.update_locked(filesystem_uuid, update).await
    }

    /// Hold the update/cleanup ordering gate before resolving the name. If a
    /// destroy cleanup won first the lookup fails; if this update won first,
    /// the later cleanup removes the newly written UUID record.
    pub async fn update_for_filesystem(
        &self,
        filesystems: &FilesystemService,
        update: ScrubScheduleUpdate,
    ) -> Result<ScrubScheduleStatus, ScrubScheduleError> {
        let _update_admission = self.update_admission.lock().await;
        let filesystem = filesystems.get(&update.name).await?;
        self.update_locked(&filesystem.uuid, update).await
    }

    async fn update_locked(
        &self,
        filesystem_uuid: &str,
        update: ScrubScheduleUpdate,
    ) -> Result<ScrubScheduleStatus, ScrubScheduleError> {
        let mut state = self.state.lock().await;
        let records = available_mut(&mut state)?;
        let normalized = normalize_optional(update.schedule.as_deref())?;
        let mut replacement = records.clone();
        match normalized {
            Some(schedule) => {
                replacement.insert(
                    filesystem_uuid.to_string(),
                    StoredSchedule {
                        schedule,
                        version: uuid::Uuid::new_v4().to_string(),
                        updated_at: Utc::now(),
                    },
                );
            }
            None => {
                replacement.remove(filesystem_uuid);
            }
        }

        let persistence = write_store(&self.path, &replacement).await;
        install_persisted_replacement(records, replacement, persistence)?;
        Ok(status_from_record(
            &update.name,
            records.get(filesystem_uuid),
            Utc::now(),
        ))
    }

    /// Best-effort callers log failures after the filesystem mutation has
    /// already succeeded. An unavailable/corrupt store is never overwritten.
    pub async fn remove(&self, filesystem_uuid: &str) -> Result<(), ScrubScheduleError> {
        let _update_admission = self.update_admission.lock().await;
        let mut state = self.state.lock().await;
        let records = available_mut(&mut state)?;
        if !records.contains_key(filesystem_uuid) {
            return Ok(());
        }
        let mut replacement = records.clone();
        replacement.remove(filesystem_uuid);
        let persistence = write_store(&self.path, &replacement).await;
        install_persisted_replacement(records, replacement, persistence)
    }

    /// Operations rows remain visible when schedule persistence failed.
    pub async fn status_for_operation(
        &self,
        name: &str,
        filesystem_uuid: &str,
    ) -> ScrubScheduleStatus {
        let state = self.state.lock().await;
        match &*state {
            StoreState::Available(records) => {
                status_from_record(name, records.get(filesystem_uuid), Utc::now())
            }
            StoreState::Unavailable(error) => ScrubScheduleStatus {
                name: name.to_string(),
                schedule: None,
                next_run_at: None,
                schedule_error: Some(error.clone()),
            },
        }
    }

    async fn snapshot(&self) -> Result<HashMap<String, StoredSchedule>, ScrubScheduleError> {
        let state = self.state.lock().await;
        Ok(available(&state)?.clone())
    }
}

fn available(state: &StoreState) -> Result<&HashMap<String, StoredSchedule>, ScrubScheduleError> {
    match state {
        StoreState::Available(records) => Ok(records),
        StoreState::Unavailable(error) => Err(ScrubScheduleError::Unavailable(error.clone())),
    }
}

fn available_mut(
    state: &mut StoreState,
) -> Result<&mut HashMap<String, StoredSchedule>, ScrubScheduleError> {
    match state {
        StoreState::Available(records) => Ok(records),
        StoreState::Unavailable(error) => Err(ScrubScheduleError::Unavailable(error.clone())),
    }
}

fn load_store(path: &Path) -> Result<HashMap<String, StoredSchedule>, String> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(format!("read {} failed: {error}", path.display())),
    };
    let records: HashMap<String, StoredSchedule> = serde_json::from_str(&contents)
        .map_err(|error| format!("parse {} failed: {error}", path.display()))?;
    for (filesystem_uuid, record) in &records {
        if record.version.is_empty() {
            return Err(format!(
                "parse {} failed: schedule for UUID {filesystem_uuid} has no version",
                path.display()
            ));
        }
        let normalized = parse_posix_schedule(&record.schedule, Utc::now())
            .map_err(|error| format!("parse {} failed: {error}", path.display()))?
            .0;
        if normalized != record.schedule {
            return Err(format!(
                "parse {} failed: schedule for UUID {filesystem_uuid} is not normalized",
                path.display()
            ));
        }
    }
    Ok(records)
}

async fn write_store(
    path: &Path,
    records: &HashMap<String, StoredSchedule>,
) -> Result<(), PersistFailure> {
    let bytes = serde_json::to_vec_pretty(records)
        .map_err(|error| PersistFailure::before_commit(error.to_string()))?;
    let parent = path.parent().ok_or_else(|| {
        PersistFailure::before_commit(format!("{} has no parent directory", path.display()))
    })?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| PersistFailure::before_commit(error.to_string()))?;
    let temp = parent.join(format!(".scrub-schedules.{}.tmp", uuid::Uuid::new_v4()));
    let precommit = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await?;
        file.write_all(&bytes).await?;
        file.sync_all().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    if let Err(error) = precommit {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(PersistFailure::before_commit(error.to_string()));
    }
    if let Err(error) = tokio::fs::rename(&temp, path).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(PersistFailure::before_commit(error.to_string()));
    }
    let directory = tokio::fs::File::open(parent)
        .await
        .map_err(|error| PersistFailure::after_commit(error.to_string()))?;
    directory
        .sync_all()
        .await
        .map_err(|error| PersistFailure::after_commit(error.to_string()))
}

#[derive(Debug)]
struct PersistFailure {
    message: String,
    committed: bool,
}

impl PersistFailure {
    fn before_commit(message: String) -> Self {
        Self {
            message,
            committed: false,
        }
    }

    fn after_commit(message: String) -> Self {
        Self {
            message,
            committed: true,
        }
    }
}

fn install_persisted_replacement(
    records: &mut HashMap<String, StoredSchedule>,
    replacement: HashMap<String, StoredSchedule>,
    persistence: Result<(), PersistFailure>,
) -> Result<(), ScrubScheduleError> {
    match persistence {
        Ok(()) => {
            *records = replacement;
            Ok(())
        }
        Err(error) => {
            if error.committed {
                *records = replacement;
                warn!(
                    "Scrub schedule update was applied, but persistence durability is not guaranteed: {}",
                    error.message
                );
                return Err(ScrubScheduleError::AppliedButNotDurable(error.message));
            }
            Err(ScrubScheduleError::Persist(error.message))
        }
    }
}

fn normalize_optional(schedule: Option<&str>) -> Result<Option<String>, ScrubScheduleError> {
    let Some(schedule) = schedule else {
        return Ok(None);
    };
    if schedule.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(parse_posix_schedule(schedule, Utc::now())?.0))
}

fn status_from_record(
    name: &str,
    record: Option<&StoredSchedule>,
    now: DateTime<Utc>,
) -> ScrubScheduleStatus {
    let Some(record) = record else {
        return ScrubScheduleStatus {
            name: name.to_string(),
            schedule: None,
            next_run_at: None,
            schedule_error: None,
        };
    };
    match parse_posix_schedule(&record.schedule, now) {
        Ok((_, schedule)) => ScrubScheduleStatus {
            name: name.to_string(),
            schedule: Some(record.schedule.clone()),
            next_run_at: schedule.next_after(now).map(rfc3339),
            schedule_error: None,
        },
        Err(error) => ScrubScheduleStatus {
            name: name.to_string(),
            schedule: Some(record.schedule.clone()),
            next_run_at: None,
            schedule_error: Some(error.to_string()),
        },
    }
}

fn rfc3339(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Clone, Debug)]
struct PosixSchedule {
    schedules: Vec<Schedule>,
}

impl PosixSchedule {
    fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedules
            .iter()
            .filter_map(|schedule| schedule.after(&after).next())
            .min()
    }
}

fn parse_posix_schedule(
    expression: &str,
    now: DateTime<Utc>,
) -> Result<(String, PosixSchedule), ScrubScheduleError> {
    if expression.contains('?') {
        return Err(ScrubScheduleError::Invalid(
            "'?' is not supported in POSIX cron".into(),
        ));
    }
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(ScrubScheduleError::Invalid(format!(
            "expected exactly 5 fields (minute hour day month weekday), got {}",
            fields.len()
        )));
    }
    let normalized = fields.join(" ");
    let weekday = translate_weekday(fields[4])?;
    let expressions = if fields[2] != "*" && fields[4] != "*" {
        vec![
            format!(
                "0 {} {} {} {} *",
                fields[0], fields[1], fields[2], fields[3]
            ),
            format!("0 {} {} * {} {weekday}", fields[0], fields[1], fields[3]),
        ]
    } else {
        vec![format!(
            "0 {} {} {} {} {weekday}",
            fields[0], fields[1], fields[2], fields[3]
        )]
    };
    let schedules = expressions
        .into_iter()
        .map(|value| {
            Schedule::from_str(&value)
                .map_err(|error| ScrubScheduleError::Invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let schedule = PosixSchedule { schedules };
    if schedule.next_after(now).is_none() {
        return Err(ScrubScheduleError::Invalid(
            "schedule has no future occurrence".into(),
        ));
    }
    Ok((normalized, schedule))
}

/// cron 0.17 numbers Sunday as 1 through Saturday as 7. POSIX uses Sunday
/// 0 or 7, Monday 1, ..., Saturday 6, so numeric lists/ranges must be expanded
/// before translation (a direct endpoint substitution breaks `0-6`).
fn translate_weekday(field: &str) -> Result<String, ScrubScheduleError> {
    if field == "*" {
        return Ok("*".into());
    }
    let mut days = BTreeSet::new();
    for item in field.split(',') {
        if item.is_empty() {
            return Err(ScrubScheduleError::Invalid(
                "empty weekday list item".into(),
            ));
        }
        let (base, step) = match item.split_once('/') {
            Some((base, step)) => {
                if step.contains('/') {
                    return Err(ScrubScheduleError::Invalid("invalid weekday step".into()));
                }
                let step = step.parse::<u8>().map_err(|_| {
                    ScrubScheduleError::Invalid(format!("invalid weekday step '{step}'"))
                })?;
                if step == 0 || step > 7 {
                    return Err(ScrubScheduleError::Invalid(format!(
                        "weekday step must be between 1 and 7, got {step}"
                    )));
                }
                (base, step as usize)
            }
            None => (item, 1),
        };
        let raw_days: Vec<u8> = if base == "*" {
            (0..=6).collect()
        } else if let Some((start, end)) = base.split_once('-') {
            let start = posix_weekday(start)?;
            let end = posix_weekday(end)?;
            if start > end {
                return Err(ScrubScheduleError::Invalid(format!(
                    "invalid weekday range '{base}'"
                )));
            }
            (start..=end).collect()
        } else {
            let start = posix_weekday(base)?;
            (start..=if item.contains('/') { 7 } else { start }).collect()
        };
        for day in raw_days.into_iter().step_by(step) {
            days.insert(if day == 0 || day == 7 { 1 } else { day + 1 });
        }
    }
    if days.is_empty() {
        return Err(ScrubScheduleError::Invalid(
            "weekday field selects no days".into(),
        ));
    }
    Ok(days
        .into_iter()
        .map(|day| day.to_string())
        .collect::<Vec<_>>()
        .join(","))
}

fn posix_weekday(value: &str) -> Result<u8, ScrubScheduleError> {
    if let Ok(day) = value.parse::<u8>() {
        return (day <= 7).then_some(day).ok_or_else(|| {
            ScrubScheduleError::Invalid(format!("weekday must be between 0 and 7, got {day}"))
        });
    }
    let day = match value.to_ascii_lowercase().as_str() {
        "sun" | "sunday" => 0,
        "mon" | "monday" => 1,
        "tue" | "tues" | "tuesday" => 2,
        "wed" | "wednesday" => 3,
        "thu" | "thur" | "thurs" | "thursday" => 4,
        "fri" | "friday" => 5,
        "sat" | "saturday" => 6,
        _ => {
            return Err(ScrubScheduleError::Invalid(format!(
                "invalid weekday '{value}'"
            )));
        }
    };
    Ok(day)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoolState {
    uuid: String,
    name: String,
    mounted: bool,
    activity: ScrubActivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrubActivity {
    Idle,
    Running,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DueScrub {
    uuid: String,
    name: String,
    due_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct CursorEntry {
    version: String,
    after: DateTime<Utc>,
    pending: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct SchedulerCursor {
    entries: HashMap<String, CursorEntry>,
    started_at: DateTime<Utc>,
}

impl SchedulerCursor {
    pub fn new(started_at: DateTime<Utc>) -> Self {
        Self {
            entries: HashMap::new(),
            started_at,
        }
    }

    fn evaluate(
        &mut self,
        records: &HashMap<String, StoredSchedule>,
        pools: &[PoolState],
        now: DateTime<Utc>,
    ) -> Option<DueScrub> {
        self.entries.retain(|uuid, _| records.contains_key(uuid));
        let pool_by_uuid: HashMap<&str, &PoolState> = pools
            .iter()
            .map(|pool| (pool.uuid.as_str(), pool))
            .collect();

        for (uuid, record) in records {
            match self.entries.get_mut(uuid) {
                Some(cursor) if cursor.version != record.version => {
                    *cursor = CursorEntry {
                        version: record.version.clone(),
                        after: record.updated_at.max(self.started_at),
                        pending: None,
                    };
                }
                Some(_) => {}
                None => {
                    self.entries.insert(
                        uuid.clone(),
                        CursorEntry {
                            version: record.version.clone(),
                            after: record.updated_at.max(self.started_at),
                            pending: None,
                        },
                    );
                }
            }
            let cursor = self.entries.get_mut(uuid).expect("cursor inserted above");
            if cursor.pending.is_some() {
                cursor.after = now;
                continue;
            }
            let Ok((_, schedule)) = parse_posix_schedule(&record.schedule, now) else {
                cursor.after = now;
                continue;
            };
            if let Some(due_at) = schedule
                .next_after(cursor.after)
                .filter(|occurrence| *occurrence <= now)
            {
                cursor.pending = Some(due_at);
                cursor.after = now;
            }
        }

        // Missing/unmounted pools and a currently-running scrub's own due
        // occurrence are consumed. Other mounted pools retain their oldest due.
        for (uuid, cursor) in &mut self.entries {
            if cursor.pending.is_none() {
                continue;
            }
            match pool_by_uuid.get(uuid.as_str()) {
                None => cursor.pending = None,
                Some(pool) if !pool.mounted || pool.activity != ScrubActivity::Idle => {
                    cursor.pending = None
                }
                Some(_) => {}
            }
        }

        if pools
            .iter()
            .any(|pool| pool.activity != ScrubActivity::Idle)
        {
            return None;
        }

        let selected = self
            .entries
            .iter()
            .filter_map(|(uuid, cursor)| {
                let due_at = cursor.pending?;
                let pool = pool_by_uuid.get(uuid.as_str())?;
                Some(DueScrub {
                    uuid: uuid.clone(),
                    name: pool.name.clone(),
                    due_at,
                })
            })
            .min_by(|left, right| (left.due_at, &left.uuid).cmp(&(right.due_at, &right.uuid)))?;
        // Consumption happens before admission, so a failed scrub_start is not
        // retried every tick for the same occurrence.
        self.entries.get_mut(&selected.uuid)?.pending = None;
        Some(selected)
    }

    pub fn resume_after_restart(&mut self, now: DateTime<Utc>) {
        self.started_at = now;
        for cursor in self.entries.values_mut() {
            cursor.after = now;
            cursor.pending = None;
        }
    }
}

/// Runs forever; callers should supervise the task and restart it on panic.
pub async fn run_scheduler_loop(
    schedules: ScrubScheduleService,
    filesystems: FilesystemService,
    events: tokio::sync::broadcast::Sender<String>,
    cursor: Arc<Mutex<SchedulerCursor>>,
) {
    info!(interval = ?TICK_INTERVAL, "scrub scheduler started");
    loop {
        let listed = match filesystems.list().await {
            Ok(filesystems) => filesystems,
            Err(error) => {
                warn!(
                    "Scrub scheduler could not list filesystems; due occurrences will be skipped: {error}"
                );
                Vec::new()
            }
        };
        let mut pools = Vec::with_capacity(listed.len());
        for filesystem in listed {
            let running = match filesystems.scrub_running_known(&filesystem.name).await {
                Ok(running) => running,
                Err(error) => {
                    warn!(filesystem = %filesystem.name, "Scrub scheduler could not read scrub status; treating filesystem as unavailable: {error}");
                    pools.push(PoolState {
                        uuid: filesystem.uuid,
                        name: filesystem.name,
                        mounted: filesystem.mounted,
                        activity: ScrubActivity::Unknown,
                    });
                    continue;
                }
            };
            pools.push(PoolState {
                uuid: filesystem.uuid,
                name: filesystem.name,
                mounted: filesystem.mounted,
                activity: if running {
                    ScrubActivity::Running
                } else {
                    ScrubActivity::Idle
                },
            });
        }

        // Serialize only the schedule snapshot/decision/admission with updates.
        // Filesystem discovery above may shell out and must not block API writes.
        let _update_admission = schedules.update_admission.lock().await;
        let records = match schedules.snapshot().await {
            Ok(records) => records,
            Err(error) => {
                warn!("Scrub scheduler unavailable: {error}");
                drop(_update_admission);
                tokio::time::sleep(TICK_INTERVAL).await;
                continue;
            }
        };
        let now = Utc::now();
        let due = cursor.lock().await.evaluate(&records, &pools, now);
        if let Some(due) = due {
            match filesystems
                .scrub_start_scheduled(&due.name, &due.uuid)
                .await
            {
                Ok(()) => {
                    info!(filesystem = %due.name, scheduled_at = %due.due_at, "Scheduled scrub admitted");
                    let _ = events.send("filesystem".to_string());
                }
                Err(error) => {
                    warn!(filesystem = %due.name, scheduled_at = %due.due_at, "Scheduled scrub admission failed; occurrence consumed: {error}")
                }
            }
        }
        drop(_update_admission);
        tokio::time::sleep(TICK_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        ts_sec(year, month, day, hour, minute, 0)
    }

    fn ts_sec(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .unwrap()
    }

    fn record(schedule: &str, version: &str) -> StoredSchedule {
        record_at(schedule, version, ts(2026, 9, 12, 1, 0))
    }

    fn record_at(schedule: &str, version: &str, updated_at: DateTime<Utc>) -> StoredSchedule {
        StoredSchedule {
            schedule: schedule.into(),
            version: version.into(),
            updated_at,
        }
    }

    fn pool(uuid: &str, mounted: bool, running: bool) -> PoolState {
        PoolState {
            uuid: uuid.into(),
            name: format!("pool-{uuid}"),
            mounted,
            activity: if running {
                ScrubActivity::Running
            } else {
                ScrubActivity::Idle
            },
        }
    }

    fn unknown_pool(uuid: &str, mounted: bool) -> PoolState {
        PoolState {
            uuid: uuid.into(),
            name: format!("pool-{uuid}"),
            mounted,
            activity: ScrubActivity::Unknown,
        }
    }

    #[test]
    fn parser_requires_five_fields_and_rejects_question_mark_and_no_future() {
        let now = ts(2026, 9, 12, 0, 0);
        assert!(parse_posix_schedule("0 3 * * *", now).is_ok());
        assert!(parse_posix_schedule("0 0 3 * * *", now).is_err());
        assert!(parse_posix_schedule("0 3 ? * *", now).is_err());
        assert!(parse_posix_schedule("0 0 31 2 *", now).is_err());
        assert!(parse_posix_schedule("not cron", now).is_err());
    }

    #[test]
    fn update_requires_schedule_field_but_accepts_explicit_null() {
        assert!(
            serde_json::from_value::<ScrubScheduleUpdate>(serde_json::json!({ "name": "tank" }))
                .is_err()
        );
        let disabled = serde_json::from_value::<ScrubScheduleUpdate>(
            serde_json::json!({ "name": "tank", "schedule": null }),
        )
        .unwrap();
        assert_eq!(disabled.schedule, None);

        let schema = serde_json::to_value(schemars::schema_for!(ScrubScheduleUpdate)).unwrap();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|field| field == "schedule"));
        let schedule_types = schema["properties"]["schedule"]["type"].as_array().unwrap();
        assert!(schedule_types.iter().any(|kind| kind == "null"));
    }

    #[test]
    fn parser_translates_posix_sunday_and_numeric_ranges() {
        let before_sunday = ts(2026, 9, 12, 0, 0);
        for expression in ["0 0 * * 0", "0 0 * * 7"] {
            let (_, schedule) = parse_posix_schedule(expression, before_sunday).unwrap();
            assert_eq!(
                schedule.next_after(before_sunday),
                Some(ts(2026, 9, 13, 0, 0))
            );
        }
        let before_friday = ts(2026, 9, 10, 0, 0);
        let (_, schedule) = parse_posix_schedule("0 0 * * 5-7", before_friday).unwrap();
        assert_eq!(
            schedule.next_after(before_friday),
            Some(ts(2026, 9, 11, 0, 0))
        );
        let (_, schedule) = parse_posix_schedule("0 0 * * 0-2", before_friday).unwrap();
        assert_eq!(
            schedule.next_after(before_friday),
            Some(ts(2026, 9, 13, 0, 0))
        );
    }

    #[test]
    fn parser_uses_posix_dom_dow_union() {
        let start = ts(2026, 9, 12, 0, 0);
        let (_, schedule) = parse_posix_schedule("0 0 13 * 1", start).unwrap();
        assert_eq!(schedule.next_after(start), Some(ts(2026, 9, 13, 0, 0)));
        assert_eq!(
            schedule.next_after(ts(2026, 9, 13, 0, 0)),
            Some(ts(2026, 9, 14, 0, 0))
        );
    }

    #[test]
    fn wildcard_step_is_restricted_for_dom_dow_union() {
        let start = ts(2026, 11, 12, 0, 0);
        let (_, schedule) = parse_posix_schedule("0 0 13 * */2", start).unwrap();
        assert_eq!(schedule.next_after(start), Some(ts(2026, 11, 13, 0, 0)));
    }

    #[tokio::test]
    async fn persistence_round_trip_disable_and_uuid_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scrub-schedules.json");
        let service = ScrubScheduleService::new_at(&path);
        let status = service
            .update(
                "uuid-a",
                ScrubScheduleUpdate {
                    name: "tank".into(),
                    schedule: Some("  0  3  * * *  ".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(status.schedule.as_deref(), Some("0 3 * * *"));

        let reloaded = ScrubScheduleService::new_at(&path);
        assert_eq!(
            reloaded
                .get("tank", "uuid-a")
                .await
                .unwrap()
                .schedule
                .as_deref(),
            Some("0 3 * * *")
        );
        assert_eq!(
            reloaded.get("tank", "uuid-b").await.unwrap().schedule,
            None,
            "a reused name must not inherit another UUID's schedule"
        );
        reloaded.remove("uuid-a").await.unwrap();
        assert_eq!(reloaded.get("tank", "uuid-a").await.unwrap().schedule, None);

        reloaded
            .update(
                "uuid-b",
                ScrubScheduleUpdate {
                    name: "tank".into(),
                    schedule: Some(" \t ".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(reloaded.get("tank", "uuid-b").await.unwrap().schedule, None);
    }

    #[tokio::test]
    async fn corrupt_state_is_fail_closed_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scrub-schedules.json");
        std::fs::write(&path, "{broken").unwrap();
        let service = ScrubScheduleService::new_at(&path);
        assert!(service.get("tank", "uuid-a").await.is_err());
        assert!(
            service
                .status_for_operation("tank", "uuid-a")
                .await
                .schedule_error
                .is_some()
        );
        assert!(
            service
                .update(
                    "uuid-a",
                    ScrubScheduleUpdate {
                        name: "tank".into(),
                        schedule: Some("0 3 * * *".into()),
                    },
                )
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{broken");
    }

    #[test]
    fn committed_persistence_failure_installs_replacement_in_memory() {
        let old = record("0 2 * * *", "old");
        let new = record("0 3 * * *", "new");
        let mut records = HashMap::from([("a".into(), old.clone())]);
        let replacement = HashMap::from([("a".into(), new.clone())]);
        let error = install_persisted_replacement(
            &mut records,
            replacement,
            Err(PersistFailure::after_commit(
                "directory fsync failed".into(),
            )),
        )
        .unwrap_err();
        assert!(matches!(error, ScrubScheduleError::AppliedButNotDurable(_)));
        assert!(error.to_string().contains("update was applied"));
        assert_eq!(records.get("a"), Some(&new));

        let mut records = HashMap::from([("a".into(), old.clone())]);
        let replacement = HashMap::from([("a".into(), new)]);
        assert!(
            install_persisted_replacement(
                &mut records,
                replacement,
                Err(PersistFailure::before_commit("rename failed".into())),
            )
            .is_err()
        );
        assert_eq!(records.get("a"), Some(&old));
    }

    #[test]
    fn scheduler_does_not_catch_up_and_fires_exactly_once() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 9, 0));
        let records = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
        let pools = [pool("a", true, false)];
        assert_eq!(
            cursor.evaluate(&records, &pools, ts(2026, 9, 12, 9, 0)),
            None
        );
        assert!(
            cursor
                .evaluate(&records, &pools, ts(2026, 9, 13, 3, 0))
                .is_some()
        );
        assert_eq!(
            cursor.evaluate(&records, &pools, ts(2026, 9, 13, 3, 1)),
            None
        );
    }

    #[test]
    fn schedule_revision_change_resets_from_observation() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let pools = [pool("a", true, false)];
        let first = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
        cursor.evaluate(&first, &pools, ts(2026, 9, 12, 2, 0));
        let changed = HashMap::from([(
            "a".into(),
            record_at("0 3 * * *", "v2", ts(2026, 9, 13, 4, 0)),
        )]);
        assert_eq!(
            cursor.evaluate(&changed, &pools, ts(2026, 9, 13, 4, 0)),
            None
        );
    }

    #[test]
    fn newly_observed_schedule_uses_persisted_update_anchor() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let records = HashMap::from([(
            "a".into(),
            record_at("0 3 * * *", "v1", ts_sec(2026, 9, 12, 2, 59, 40)),
        )]);
        let pools = [pool("a", true, false)];
        assert_eq!(
            cursor
                .evaluate(&records, &pools, ts_sec(2026, 9, 12, 3, 0, 30))
                .map(|due| due.due_at),
            Some(ts(2026, 9, 12, 3, 0))
        );
    }

    #[test]
    fn worker_restart_does_not_replay_schedule_updated_while_down() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let pools = [pool("a", true, false)];
        let initial = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
        cursor.evaluate(&initial, &pools, ts(2026, 9, 12, 2, 30));

        cursor.resume_after_restart(ts(2026, 9, 12, 3, 1));
        let changed_while_down = HashMap::from([(
            "a".into(),
            record_at("0 3 * * *", "v2", ts_sec(2026, 9, 12, 2, 59, 40)),
        )]);
        assert_eq!(
            cursor.evaluate(&changed_while_down, &pools, ts(2026, 9, 12, 3, 1)),
            None
        );
    }

    #[test]
    fn unmounted_and_currently_running_pools_consume_their_occurrence() {
        for state in [pool("a", false, false), pool("a", true, true)] {
            let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
            let records = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
            cursor.evaluate(
                &records,
                std::slice::from_ref(&state),
                ts(2026, 9, 12, 2, 0),
            );
            assert_eq!(
                cursor.evaluate(
                    &records,
                    std::slice::from_ref(&state),
                    ts(2026, 9, 12, 3, 0)
                ),
                None
            );
            let idle = [pool("a", true, false)];
            assert_eq!(
                cursor.evaluate(&records, &idle, ts(2026, 9, 12, 3, 1)),
                None
            );
        }
    }

    #[test]
    fn global_running_defers_other_pool_until_completion() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let records = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
        let running = [pool("a", true, false), pool("manual", true, true)];
        cursor.evaluate(&records, &running, ts(2026, 9, 12, 2, 0));
        assert_eq!(
            cursor.evaluate(&records, &running, ts(2026, 9, 12, 3, 0)),
            None
        );
        let idle = [pool("a", true, false), pool("manual", true, false)];
        assert_eq!(
            cursor
                .evaluate(&records, &idle, ts(2026, 9, 12, 3, 1))
                .map(|due| due.uuid),
            Some("a".into())
        );
    }

    #[test]
    fn unknown_status_consumes_own_due_and_blocks_other_admission() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let records = HashMap::from([
            ("a".into(), record("0 3 * * *", "v1")),
            ("b".into(), record("0 3 * * *", "v1")),
        ]);
        let uncertain = [unknown_pool("a", true), pool("b", true, false)];
        assert_eq!(
            cursor.evaluate(&records, &uncertain, ts(2026, 9, 12, 3, 0)),
            None
        );

        let known = [pool("a", true, false), pool("b", true, false)];
        assert_eq!(
            cursor
                .evaluate(&records, &known, ts(2026, 9, 12, 3, 1))
                .map(|due| due.uuid),
            Some("b".into())
        );
        assert_eq!(
            cursor.evaluate(&records, &known, ts(2026, 9, 12, 3, 2)),
            None,
            "the uncertain pool's own occurrence was consumed"
        );
    }

    #[test]
    fn list_failure_consumes_due_occurrences_and_starts_none() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let records = HashMap::from([("a".into(), record("0 3 * * *", "v1"))]);
        assert_eq!(cursor.evaluate(&records, &[], ts(2026, 9, 12, 3, 0)), None);
        assert_eq!(
            cursor.evaluate(&records, &[pool("a", true, false)], ts(2026, 9, 12, 3, 1)),
            None
        );
    }

    #[test]
    fn oldest_due_starts_first_and_failed_admission_is_consumed() {
        let mut cursor = SchedulerCursor::new(ts(2026, 9, 12, 2, 0));
        let records = HashMap::from([
            ("a".into(), record("0 3 * * *", "v1")),
            ("b".into(), record("5 3 * * *", "v1")),
        ]);
        let pools = [pool("a", true, false), pool("b", true, false)];
        cursor.evaluate(&records, &pools, ts(2026, 9, 12, 2, 0));
        assert_eq!(
            cursor
                .evaluate(&records, &pools, ts(2026, 9, 12, 3, 10))
                .map(|due| due.uuid),
            Some("a".into())
        );
        // Treat the first result as a failed admission: it remains consumed,
        // while the other retained occurrence is admitted on the next tick.
        assert_eq!(
            cursor
                .evaluate(&records, &pools, ts(2026, 9, 12, 3, 11))
                .map(|due| due.uuid),
            Some("b".into())
        );
        assert_eq!(
            cursor.evaluate(&records, &pools, ts(2026, 9, 12, 3, 12)),
            None
        );
    }
}
