//! RPC arms in the `backup.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::AppState;
use crate::auth::{Role, Session};

fn is_data_source(source: &str) -> bool {
    let Ok(relative) = std::path::Path::new(source).strip_prefix("/fs") else {
        return false;
    };
    let mut components = relative.components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn profile_requires_admin(profile: &nasty_backup::BackupProfile) -> bool {
    profile.sources.iter().any(|source| !is_data_source(source))
}

fn backup_read_requires_unscoped(method: &str) -> bool {
    matches!(
        method,
        "backup.profile.list"
            | "backup.schedule.list"
            | "backup.profile.get"
            | "backup.status"
            | "backup.snapshots"
            | "backup.jobs.list"
            | "backup.jobs.get"
    )
}

fn backup_read_access_error(method: &str, session: &Session) -> Option<&'static str> {
    (backup_read_requires_unscoped(method)
        && (session.filesystem.is_some() || session.owner.is_some()))
    .then_some("backup management requires an unscoped session")
}

fn profile_access_error(
    session: &Session,
    profile: &nasty_backup::BackupProfile,
) -> Option<&'static str> {
    if session.filesystem.is_some() || session.owner.is_some() {
        return Some("backup management requires an unscoped session");
    }
    if profile_requires_admin(profile) && session.role != Role::Admin {
        return Some("an admin session is required for system-state backup profiles");
    }
    None
}

async fn require_profile_access(
    state: &AppState,
    session: &Session,
    id: &str,
) -> Result<(), String> {
    let profile = state
        .backups
        .get_profile(id)
        .await
        .map_err(|error| error.to_rpc_error())?;
    profile_access_error(session, &profile).map_or(Ok(()), |message| Err(message.to_string()))
}

#[derive(Deserialize)]
struct RestoreParams {
    id: String,
    snapshot_id: String,
    dest: String,
    #[serde(default)]
    allow_overwrite: bool,
}

pub(super) async fn try_route(
    req: &Request,
    state: &AppState,
    session: &Session,
) -> Option<Response> {
    if let Some(message) = backup_read_access_error(&req.method, session) {
        return Some(err(req, message));
    }
    Some(match req.method.as_str() {
        "backup.profile.list" => ok(req, state.backups.list_profiles().await),
        "backup.schedule.list" => ok(req, state.backups.list_schedule(chrono::Utc::now()).await),
        "backup.profile.get" => match require_str(req, "id") {
            Ok(id) => match state.backups.get_profile(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e.to_rpc_error()),
            },
            Err(r) => r,
        },
        "backup.profile.create" => match parse_params::<nasty_backup::BackupProfile>(req) {
            Ok(p) => match profile_access_error(session, &p) {
                Some(message) => err(req, message),
                None => match state.backups.create_profile(p).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e.to_rpc_error()),
                },
            },
            Err(e) => err(req, e),
        },
        "backup.profile.update" => {
            let id = match require_str(req, "id") {
                Ok(s) => s.to_string(),
                Err(r) => return Some(r),
            };
            match parse_params::<nasty_backup::BackupProfile>(req) {
                Ok(p) => match require_profile_access(state, session, &id).await {
                    Err(e) => err(req, e),
                    Ok(()) => match profile_access_error(session, &p) {
                        Some(message) => err(req, message),
                        None => match state.backups.update_profile(&id, p).await {
                            Ok(v) => ok(req, v),
                            Err(e) => err(req, e.to_rpc_error()),
                        },
                    },
                },
                Err(e) => err(req, e),
            }
        }
        "backup.profile.delete" => match require_str(req, "id") {
            Ok(id) => match require_profile_access(state, session, id).await {
                Ok(()) => match state.backups.delete_profile(id).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e.to_rpc_error()),
                },
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "backup.status" => ok(req, state.backups.status().await),
        // The init / run / check RPCs return a BackupJob handle now.
        // Long-running ops would otherwise blow through the 10 s
        // WebSocket request timeout in the WebUI client — observed
        // with `backup.repo.init` against a remote REST target
        // taking 32 s. Clients poll backup.jobs.get / backup.jobs.list
        // to watch the Pending → Running → Succeeded|Failed transition.
        "backup.repo.init" => match require_str(req, "id") {
            Ok(id) => match require_profile_access(state, session, id).await {
                Ok(()) => match state.backups.start_init_repo(id).await {
                    Ok(job) => ok(req, job),
                    Err(e) => err(req, e.to_string()),
                },
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "backup.run" => match require_str(req, "id") {
            Ok(id) => match require_profile_access(state, session, id).await {
                Ok(()) => match state.backups.start_run_backup(id).await {
                    Ok(job) => ok(req, job),
                    Err(e) => err(req, e.to_string()),
                },
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "backup.snapshots" => match require_str(req, "id") {
            Ok(id) => match state.backups.list_snapshots(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e.to_rpc_error()),
            },
            Err(r) => r,
        },
        "backup.restore" => match parse_params::<RestoreParams>(req) {
            Ok(p) => match require_profile_access(state, session, &p.id).await {
                Ok(()) => match state
                    .backups
                    .start_restore(&p.id, &p.snapshot_id, &p.dest, p.allow_overwrite)
                    .await
                {
                    Ok(job) => ok(req, job),
                    Err(e) => err(req, e.to_rpc_error()),
                },
                Err(e) => err(req, e),
            },
            Err(e) => err(req, e),
        },
        "backup.repo.check" => match require_str(req, "id") {
            Ok(id) => match require_profile_access(state, session, id).await {
                Ok(()) => match state.backups.start_check_repo(id).await {
                    Ok(job) => ok(req, job),
                    Err(e) => err(req, e.to_string()),
                },
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "backup.jobs.list" => {
            // Optional `profile_id` filter — empty / missing returns all.
            let profile_id = req
                .params
                .as_ref()
                .and_then(|p| p.get("profile_id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            ok(req, state.backups.jobs().list(profile_id).await)
        }
        "backup.jobs.get" => match require_str(req, "id") {
            Ok(job_id) => match state.backups.jobs().get(job_id).await {
                Some(job) => ok(req, job),
                None => err(req, format!("backup job not found: {job_id}")),
            },
            Err(r) => r,
        },
        "backup.secrets_status" => ok(req, state.backups.secrets_status().await),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        backup_read_access_error, backup_read_requires_unscoped, is_data_source,
        profile_access_error,
    };
    use crate::auth::{Role, Session};

    fn profile(sources: &[&str]) -> nasty_backup::BackupProfile {
        serde_json::from_value(serde_json::json!({
            "id": "profile",
            "name": "Profile",
            "enabled": true,
            "sources": sources,
            "target": { "type": "local", "path": "/fs/tank/backups" }
        }))
        .unwrap()
    }

    fn session(role: Role, scoped: bool) -> Session {
        Session {
            token: "token".into(),
            username: "user".into(),
            role,
            file_principal: None,
            filesystem: scoped.then(|| "tank".into()),
            owner: None,
            created_at: 0,
            must_change_password: false,
            client_ip: None,
        }
    }

    #[test]
    fn data_sources_are_absolute_children_of_fs() {
        assert!(is_data_source("/fs/tank"));
        assert!(is_data_source("/fs/tank/appdata"));
        assert!(!is_data_source("/fs"));
        assert!(!is_data_source("/fs/../etc"));
        assert!(!is_data_source("/var/lib/nasty"));
        assert!(!is_data_source("relative/path"));
    }

    #[test]
    fn system_profiles_require_an_unscoped_admin() {
        let data = profile(&["/fs/tank/appdata"]);
        let system = profile(&["/var/lib/nasty", "/etc/nixos"]);

        assert!(profile_access_error(&session(Role::Operator, false), &data).is_none());
        assert!(profile_access_error(&session(Role::Operator, false), &system).is_some());
        assert!(profile_access_error(&session(Role::Admin, false), &system).is_none());
        assert!(profile_access_error(&session(Role::Admin, true), &system).is_some());
    }

    #[test]
    fn profile_derived_backup_reads_require_unscoped_sessions() {
        for method in [
            "backup.profile.list",
            "backup.schedule.list",
            "backup.profile.get",
            "backup.status",
            "backup.snapshots",
            "backup.jobs.list",
            "backup.jobs.get",
        ] {
            assert!(backup_read_requires_unscoped(method), "{method}");
        }
        assert!(!backup_read_requires_unscoped("backup.secrets_status"));
        assert!(!backup_read_requires_unscoped("backup.profile.create"));
    }

    #[test]
    fn backup_read_scope_rejects_filesystem_and_owner_credentials() {
        let unscoped = session(Role::ReadOnly, false);
        let filesystem_scoped = session(Role::Admin, true);
        let mut owner_scoped = session(Role::Admin, false);
        owner_scoped.owner = Some("token-a".to_string());

        assert!(backup_read_access_error("backup.profile.list", &unscoped).is_none());
        assert!(backup_read_access_error("backup.profile.list", &filesystem_scoped).is_some());
        assert!(backup_read_access_error("backup.profile.list", &owner_scoped).is_some());
        assert!(backup_read_access_error("backup.secrets_status", &owner_scoped).is_none());
    }
}
