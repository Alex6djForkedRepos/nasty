//! RPC arms in the `apps.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::AppState;
use crate::auth::{Role, Session};

fn simple_requires_admin(req: &nasty_apps::InstallAppRequest) -> bool {
    req.allow_unsafe
        || req.network.as_deref() == Some("host")
        || req
            .registry_credential_ids
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
}

fn app_requires_admin(app: &nasty_apps::App) -> bool {
    app.kind == "compose"
        || app.unsafe_mode
        || app.network.as_deref() == Some("host")
        || !app.registry_credential_ids.is_empty()
}

fn preflight_access_error(req: &Request, session: &Session) -> Option<Response> {
    if req.method == "apps.fix_volume_perms" {
        return require_root_equivalent(req, session, "host_volume_ownership_change");
    }
    if req.method.starts_with("apps.registry_credentials.") {
        return if req.method.ends_with(".list") {
            require_root_equivalent_read(req, session, "registry_credentials_read")
        } else {
            require_root_equivalent(req, session, "registry_credentials_mutation")
        };
    }
    None
}

#[derive(Deserialize)]
struct InspectImageParams {
    image: String,
    #[serde(default)]
    registry_credential_id: Option<String>,
}

async fn existing_app_requires_admin(state: &AppState, name: &str) -> Result<bool, String> {
    let app = state
        .apps
        .get(name)
        .await
        .map_err(|error| error.to_string())?;
    Ok(app_requires_admin(&app))
}

async fn existing_app_access_error(
    req: &Request,
    state: &AppState,
    session: &Session,
    name: &str,
) -> Option<Response> {
    match existing_app_requires_admin(state, name).await {
        Ok(false) => {
            if session.filesystem.is_some() || session.owner.is_some() {
                let config = match state.apps.get_config(name).await {
                    Ok(config) => config,
                    Err(error) => return Some(err(req, error)),
                };
                if let Err(error) = authorize_app_paths(
                    state,
                    session,
                    config
                        .volumes
                        .into_iter()
                        .map(|volume| volume.host_path)
                        .collect(),
                )
                .await
                {
                    return Some(err(req, error));
                }
            }
            None
        }
        Ok(true) => require_root_equivalent(req, session, "unsafe_existing_app"),
        Err(error) => Some(invalid(req, error)),
    }
}

async fn authorize_app_paths(
    state: &AppState,
    session: &Session,
    paths: Vec<String>,
) -> Result<(), String> {
    if session.filesystem.is_none() && session.owner.is_none() {
        return Ok(());
    }
    for path in &paths {
        if path.starts_with('/') {
            super::share::authorize_path_source(state, session, path).await?;
        }
    }
    Ok(())
}

async fn simple_registry_access_error(
    req: &Request,
    state: &AppState,
    session: &Session,
    image: &str,
) -> Option<Response> {
    match state
        .apps
        .image_has_matching_registry_credentials(image)
        .await
    {
        Ok(true) => require_root_equivalent(req, session, "authenticated_registry_pull"),
        Ok(false) => None,
        Err(error) => Some(err(req, error)),
    }
}

pub(crate) async fn published_firewall_ports(
    state: &AppState,
) -> Result<Vec<nasty_system::firewall::PublishedAppPort>, String> {
    let apps = state
        .apps
        .list()
        .await
        .map_err(|e| format!("list apps for firewall: {e}"))?;
    let mut published: Vec<nasty_system::firewall::PublishedAppPort> = apps
        .iter()
        .flat_map(|app| {
            app.ports
                .iter()
                .map(move |port| nasty_system::firewall::PublishedAppPort {
                    app: app.name.clone(),
                    host_port: port.host_port,
                    container_port: port.container_port,
                    transport: port.protocol.to_ascii_lowercase(),
                })
        })
        .collect();
    published.sort_by(|a, b| {
        a.host_port
            .cmp(&b.host_port)
            .then_with(|| a.app.cmp(&b.app))
    });
    Ok(published)
}

pub(crate) async fn sync_published_firewall_ports(state: &AppState) -> Result<(), String> {
    let _sync = state.app_firewall_sync.lock().await;
    let published = published_firewall_ports(state).await?;
    state.firewall.set_published_app_ports(published).await
}

pub(super) async fn try_route(
    req: &Request,
    state: &AppState,
    session: &Session,
) -> Option<Response> {
    if let Some(response) = preflight_access_error(req, session) {
        return Some(response);
    }
    let response = match req.method.as_str() {
        "apps.status" => ok(req, state.apps.status().await),
        "apps.enable" => {
            if let Some(response) = require_root_equivalent(req, session, "apps_runtime_enable") {
                return Some(response);
            }
            let p: nasty_apps::EnableAppsRequest = parse_params(req).unwrap_or_default();
            match state.apps.enable(p).await {
                Ok(()) => ok(req, "ok"),
                Err(e) => err(req, e),
            }
        }
        "apps.disable" => {
            if let Some(response) = require_root_equivalent(req, session, "apps_runtime_disable") {
                return Some(response);
            }
            match state.apps.disable().await {
                Ok(()) => ok(req, "ok"),
                Err(e) => err(req, e),
            }
        }
        "apps.list" => match state.apps.list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "apps.stats" => match state.apps.stats().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "apps.get" => match require_str(req, "name") {
            Ok(name) => match state.apps.get(name).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "apps.inspect" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.inspect(name).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.install" => match parse_params::<nasty_apps::InstallAppRequest>(req) {
            Ok(p) => {
                let app_registry_guard = state.apps.app_registry_read_guard().await;
                if simple_requires_admin(&p)
                    && let Some(response) =
                        require_root_equivalent(req, session, "unsafe_app_payload")
                {
                    return Some(response);
                }
                if p.registry_credential_ids.as_ref().is_none_or(Vec::is_empty)
                    && let Some(response) =
                        simple_registry_access_error(req, state, session, &p.image).await
                {
                    return Some(response);
                }
                if let Err(error) = authorize_app_paths(
                    state,
                    session,
                    p.volumes
                        .iter()
                        .map(|volume| volume.host_path.clone())
                        .collect(),
                )
                .await
                {
                    return Some(err(req, error));
                }
                // Refresh Caddy TLS automation when the install opts into
                // a subdomain ingress — install() wires the route but the
                // TLS layer is reached from here (see the matching note in
                // app_deploy.rs::deploy_simple).
                let chose_subdomain = p
                    .subdomain
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
                match state
                    .apps
                    .install_with_app_registry_guard(p, app_registry_guard)
                    .await
                {
                    Ok(v) => {
                        if chose_subdomain {
                            tokio::spawn(nasty_system::settings::reapply_tls_from_disk());
                        }
                        ok(req, v)
                    }
                    Err(e) => err(req, e),
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.update" => match parse_params::<nasty_apps::InstallAppRequest>(req) {
            Ok(p) => {
                let app_registry_guard = state.apps.app_registry_read_guard().await;
                if let Some(response) =
                    existing_app_access_error(req, state, session, &p.name).await
                {
                    return Some(response);
                }
                if simple_requires_admin(&p)
                    && let Some(response) =
                        require_root_equivalent(req, session, "unsafe_app_payload")
                {
                    return Some(response);
                }
                if p.registry_credential_ids.as_ref().is_none_or(Vec::is_empty)
                    && let Some(response) =
                        simple_registry_access_error(req, state, session, &p.image).await
                {
                    return Some(response);
                }
                if let Err(error) = authorize_app_paths(
                    state,
                    session,
                    p.volumes
                        .iter()
                        .map(|volume| volume.host_path.clone())
                        .collect(),
                )
                .await
                {
                    return Some(err(req, error));
                }
                match state
                    .apps
                    .update_with_app_registry_guard(p, app_registry_guard)
                    .await
                {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.inspect_image" => match parse_params::<InspectImageParams>(req) {
            Ok(params) => {
                let _registry_guard = state.apps.registry_credentials_read_guard().await;
                if params.registry_credential_id.is_some()
                    && let Some(response) =
                        require_root_equivalent_read(req, session, "authenticated_image_inspect")
                {
                    return Some(response);
                }
                if params.registry_credential_id.is_none()
                    && let Some(response) =
                        simple_registry_access_error(req, state, session, &params.image).await
                {
                    return Some(response);
                }
                match state
                    .apps
                    .inspect_image(&params.image, params.registry_credential_id.as_deref())
                    .await
                {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.registry_credentials.list" => match state.apps.registry_credentials_list().await {
            Ok(credentials) => ok(req, credentials),
            Err(error) => err(req, error),
        },
        "apps.registry_credentials.create" => {
            match parse_params::<nasty_apps::CreateRegistryCredentialRequest>(req) {
                Ok(params) => match state.apps.registry_credentials_create(params).await {
                    Ok(credential) => ok(req, credential),
                    Err(error) => err(req, error),
                },
                Err(error) => invalid(req, error),
            }
        }
        "apps.registry_credentials.update" => {
            match parse_params::<nasty_apps::UpdateRegistryCredentialRequest>(req) {
                Ok(params) => match state.apps.registry_credentials_update(params).await {
                    Ok(credential) => ok(req, credential),
                    Err(error) => err(req, error),
                },
                Err(error) => invalid(req, error),
            }
        }
        "apps.registry_credentials.delete" => match require_str(req, "id") {
            Ok(id) => match state.apps.registry_credentials_delete(id).await {
                Ok(()) => ok(req, "ok"),
                Err(error) => err(req, error),
            },
            Err(response) => response,
        },
        "apps.check_ports" => match parse_params(req) {
            Ok(p) => ok(req, state.apps.check_ports(p).await),
            Err(e) => invalid(req, e),
        },
        "apps.check_devices" => match parse_params(req) {
            Ok(p) => ok(req, state.apps.check_devices(p).await),
            Err(e) => invalid(req, e),
        },
        "apps.check_volumes" => match parse_params(req) {
            Ok(p) => ok(req, state.apps.check_volumes(p).await),
            Err(e) => invalid(req, e),
        },
        "apps.check_compose" => match parse_params(req) {
            Ok(p) => ok(req, state.apps.check_compose(p).await),
            Err(e) => invalid(req, e),
        },
        "apps.appdata.status" => ok(req, state.apps.appdata_relocate_status().await),
        "apps.appdata.relocate" => {
            if let Some(response) = require_root_equivalent(req, session, "appdata_relocation") {
                return Some(response);
            }
            match require_str(req, "filesystem") {
                Ok(fs) => match state.apps.appdata_relocate(fs).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(r) => r,
            }
        }
        "apps.fix_volume_perms" => match parse_params(req) {
            Ok(p) => match state.apps.fix_volume_perms(p).await {
                Ok(()) => ok(req, serde_json::json!({"ok": true})),
                Err(e) => err(req, e),
            },
            Err(e) => invalid(req, e),
        },
        "apps.config" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.get_config(name).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.remove" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.remove(name).await {
                    Ok(()) => {
                        // Cover the case where the removed app had a
                        // subdomain ingress — Caddy stops trying to renew
                        // the now-orphaned cert. The internal remove path
                        // in nasty-apps clears the route via the admin API
                        // but doesn't know about the TLS-automation layer.
                        tokio::spawn(nasty_system::settings::reapply_tls_from_disk());
                        ok(req, "ok")
                    }
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.stop" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.stop(name).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.start" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.start(name).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.restart" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.restart(name).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.pull" => match require_str(req, "name") {
            Ok(name) => {
                let app_registry_guard = state.apps.app_registry_read_guard().await;
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.get(name).await {
                    Ok(app) if app.kind == "simple" && app.registry_credential_ids.is_empty() => {
                        if let Some(response) =
                            simple_registry_access_error(req, state, session, &app.image).await
                        {
                            return Some(response);
                        }
                    }
                    Ok(_) => {}
                    Err(error) => return Some(err(req, error)),
                }
                match state
                    .apps
                    .pull_with_app_registry_guard(name, app_registry_guard)
                    .await
                {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.prune" => {
            if let Some(response) = require_unscoped_mutation(req, session, "global_apps_prune") {
                return Some(response);
            }
            match state.apps.prune().await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            }
        }
        "apps.exec_command" => match require_str(req, "name") {
            Ok(name) => match state.apps.exec_command(name).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "apps.logs" => {
            let name = match require_str(req, "name") {
                Ok(n) => n,
                Err(r) => return Some(r),
            };
            let tail = req
                .params
                .as_ref()
                .and_then(|p| p.get("tail"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            match state.apps.logs(name, tail).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            }
        }
        "apps.container.logs" => {
            let container_id = match require_str(req, "container_id") {
                Ok(n) => n,
                Err(r) => return Some(r),
            };
            let tail = req
                .params
                .as_ref()
                .and_then(|p| p.get("tail"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            match state.apps.container_logs(container_id, tail).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            }
        }
        "apps.compose.install" => match parse_params::<nasty_apps::InstallComposeRequest>(req) {
            Ok(p) => {
                if let Some(response) = require_root_equivalent(req, session, "compose_lifecycle") {
                    return Some(response);
                }
                if let Err(error) =
                    crate::app_deploy::validate_compose(&p.compose_file, &p.name, true)
                {
                    return Some(invalid(req, error));
                }
                match state.apps.compose_install(p).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.compose.update" => match parse_params::<nasty_apps::InstallComposeRequest>(req) {
            Ok(p) => {
                if let Some(response) = require_root_equivalent(req, session, "compose_lifecycle") {
                    return Some(response);
                }
                if let Err(error) =
                    crate::app_deploy::validate_compose(&p.compose_file, &p.name, true)
                {
                    return Some(invalid(req, error));
                }
                match state.apps.compose_update(p).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.compose.remove" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = require_root_equivalent(req, session, "compose_lifecycle") {
                    return Some(response);
                }
                match state.apps.compose_remove(name).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        "apps.compose.get" => {
            if let Some(response) = require_root_equivalent(req, session, "compose_source_read") {
                return Some(response);
            }
            match require_str(req, "name") {
                Ok(name) => match state.apps.compose_get(name).await {
                    Ok(v) => ok(req, v),
                    Err(e) => err(req, e),
                },
                Err(r) => r,
            }
        }
        "apps.compose.logs" => {
            let name = match require_str(req, "name") {
                Ok(n) => n,
                Err(r) => return Some(r),
            };
            let tail = req
                .params
                .as_ref()
                .and_then(|p| p.get("tail"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            match state.apps.compose_logs(name, tail).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            }
        }
        "apps.compose.set_startup" => {
            match parse_params::<nasty_apps::SetComposeStartupRequest>(req) {
                Ok(p) => {
                    if let Some(response) =
                        existing_app_access_error(req, state, session, &p.name).await
                    {
                        return Some(response);
                    }
                    match state
                        .apps
                        .compose_set_startup(&p.name, p.managed, p.order, p.delay_secs)
                        .await
                    {
                        Ok(()) => ok(req, "ok"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "apps.compose.startup.list" => ok(req, state.apps.compose_list_startup().await),
        "apps.ingress.list" => match state.apps.ingress_list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        // Every route Caddy is serving (engine-owned + static), powering
        // the Ingress overview page so the operator can see at a glance
        // what's exposed and where each row came from — without shelling
        // in to read the live Caddy config.
        //
        // We enrich host-match rows with their on-disk cert info here
        // (rather than inside nasty-apps' walker) because the cert
        // directory lives in nasty-system's domain — nasty-apps doesn't
        // depend on nasty-system, and reaching across crates just for
        // a single optional field would invert the dep graph.
        "apps.caddy.routes" => match state.apps.list_caddy_routes().await {
            Ok(mut rows) => {
                for row in &mut rows {
                    if row.match_kind != "host" {
                        continue;
                    }
                    if let Some(info) =
                        nasty_system::settings::cert_info_for_host(&row.match_value).await
                    {
                        row.cert = Some(nasty_apps::HostCert {
                            issuer: info.issuer,
                            issued: info.issued,
                            expires: info.expires,
                            expires_in_days: info.expires_in_days,
                            path: info.path,
                        });
                    }
                }
                ok(req, rows)
            }
            Err(e) => err(req, e),
        },
        "apps.ingress.set" => match parse_params::<nasty_apps::SetIngressRequest>(req) {
            Ok(p) => {
                if let Some(response) =
                    existing_app_access_error(req, state, session, &p.name).await
                {
                    return Some(response);
                }
                let _reservation = crate::ingress_conflict::lock_hostname_reservations().await;
                // Gate the set on a subdomain-conflict check — catches the
                // "two apps claim the same hostname" / "app subdomain ==
                // WebUI hostname" cases that Caddy would silently let the
                // most recent one win. Empty subdomain (path-prefix mode)
                // short-circuits past the check inside find_subdomain_conflict.
                let conflict = match &p.subdomain {
                    Some(s) => {
                        crate::ingress_conflict::find_subdomain_conflict(state, &p.name, s).await
                    }
                    None => None,
                };
                if let Some(reason) = conflict {
                    err(req, format!("subdomain conflict: {reason}"))
                } else {
                    match state.apps.ingress_set(p).await {
                        Ok(v) => {
                            // Refresh Caddy's TLS automation so a new
                            // subdomain gets a cert immediately. Spawn
                            // so the RPC reply isn't blocked by the
                            // admin-API round-trip.
                            tokio::spawn(nasty_system::settings::reapply_tls_from_disk());
                            ok(req, v)
                        }
                        Err(e) => err(req, e),
                    }
                }
            }
            Err(e) => invalid(req, e),
        },
        // Best-effort lookup used by the WebUI's subdomain dialog to
        // surface a live "in use by X" hint as the operator types,
        // before they click Save. Returns the conflict reason or an
        // empty string when the choice is clear. Read-only.
        "apps.ingress.check_conflict" => 'arm: {
            let name = match require_str(req, "name") {
                Ok(s) => s,
                Err(r) => break 'arm r,
            };
            let subdomain = match require_str(req, "subdomain") {
                Ok(s) => s,
                Err(r) => break 'arm r,
            };
            let reason = crate::ingress_conflict::find_subdomain_conflict(state, name, subdomain)
                .await
                .unwrap_or_default();
            ok(req, reason)
        }
        "apps.ingress.remove" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) = existing_app_access_error(req, state, session, name).await {
                    return Some(response);
                }
                match state.apps.ingress_remove(name).await {
                    Ok(()) => {
                        // Reapply so Caddy stops trying to renew the cert
                        // for the now-orphaned subdomain. Same fire-and-
                        // forget pattern as the set arm above.
                        tokio::spawn(nasty_system::settings::reapply_tls_from_disk());
                        ok(req, "ok")
                    }
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },

        // ── Managed Docker networks ──────────────────────────
        "apps.networks.list" => match state.apps.network_list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "apps.networks.create" => match parse_params::<nasty_apps::ManagedNetwork>(req) {
            Ok(spec) => {
                if let Some(response) =
                    require_unscoped_mutation(req, session, "global_apps_network_create")
                {
                    return Some(response);
                }
                let ifaces = crate::system_network_ifaces(state).await;
                // Resolve the management interface from the caller's peer so we
                // can refuse a host shim on it (lockout guard, #448).
                let mgmt = match session.client_ip.as_deref() {
                    Some(peer) => nasty_system::network::mgmt_iface_for_peer(peer).await,
                    None => None,
                };
                if spec.host_shim && spec.parent.as_deref() == mgmt.as_deref() {
                    err(
                        req,
                        "refusing a host shim on the management interface (would risk lockout)"
                            .to_string(),
                    )
                } else {
                    match state.apps.network_create(spec.clone(), &ifaces).await {
                        Ok(()) => {
                            if spec.host_shim {
                                // Apply the host-side shim; on failure undo the
                                // Docker network so we don't leave a half-state.
                                match crate::add_macvlan_shim(state, &spec, mgmt.as_deref()).await {
                                    Ok(()) => ok(req, "ok"),
                                    Err(e) => {
                                        let _ = state.apps.network_remove(&spec.name).await;
                                        err(req, format!("host shim failed: {e}"))
                                    }
                                }
                            } else {
                                ok(req, "ok")
                            }
                        }
                        Err(e) => err(req, e),
                    }
                }
            }
            Err(e) => invalid(req, e),
        },
        "apps.networks.remove" => match require_str(req, "name") {
            Ok(name) => {
                if let Some(response) =
                    require_unscoped_mutation(req, session, "global_apps_network_remove")
                {
                    return Some(response);
                }
                match state.apps.network_remove(name).await {
                    Ok(()) => {
                        // Tear down the host shim too (best-effort).
                        if let Err(e) = crate::remove_macvlan_shim(state, name).await {
                            tracing::warn!("apps: failed to remove macvlan shim for '{name}': {e}");
                        }
                        ok(req, "ok")
                    }
                    Err(e) => err(req, e),
                }
            }
            Err(r) => r,
        },
        _ => return None,
    };
    let changes_published_ports = matches!(
        req.method.as_str(),
        "apps.enable"
            | "apps.disable"
            | "apps.install"
            | "apps.update"
            | "apps.remove"
            | "apps.pull"
            | "apps.compose.install"
            | "apps.compose.update"
            | "apps.compose.remove"
            | "apps.compose.set_startup"
    );
    if changes_published_ports && let Err(e) = sync_published_firewall_ports(state).await {
        if response.error.is_none() {
            return Some(err(
                req,
                format!("app state changed but firewall synchronization failed: {e}"),
            ));
        }
        tracing::warn!("app firewall reconciliation after failed request also failed: {e}");
    }
    Some(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_request(extra: serde_json::Value) -> nasty_apps::InstallAppRequest {
        let mut value = serde_json::json!({
            "name": "safe-app",
            "image": "example/app:latest"
        });
        value.as_object_mut().unwrap().extend(
            extra
                .as_object()
                .expect("test request extension must be an object")
                .clone(),
        );
        serde_json::from_value(value).expect("valid test request")
    }

    fn existing_app(extra: serde_json::Value) -> nasty_apps::App {
        let mut value = serde_json::json!({
            "name": "safe-app",
            "image": "example/app:latest",
            "status": "running",
            "created": "2026-01-01T00:00:00Z",
            "kind": "simple"
        });
        value.as_object_mut().unwrap().extend(
            extra
                .as_object()
                .expect("test app extension must be an object")
                .clone(),
        );
        serde_json::from_value(value).expect("valid test app")
    }

    #[test]
    fn simple_admin_gate_covers_unsafe_mounts_host_networking_and_registry_credentials() {
        assert!(!simple_requires_admin(&simple_request(serde_json::json!(
            {}
        ))));
        assert!(simple_requires_admin(&simple_request(serde_json::json!({
            "allow_unsafe": true
        }))));
        assert!(simple_requires_admin(&simple_request(serde_json::json!({
            "network": "host"
        }))));
        assert!(simple_requires_admin(&simple_request(serde_json::json!({
            "registry_credential_ids": ["credential-id"]
        }))));
    }

    #[test]
    fn existing_app_access_gates_privileged_apps_as_admin() {
        assert!(!app_requires_admin(&existing_app(serde_json::json!({}))));
        assert!(app_requires_admin(&existing_app(serde_json::json!({
            "kind": "compose"
        }))));
        assert!(app_requires_admin(&existing_app(serde_json::json!({
            "unsafe_mode": true
        }))));
        assert!(app_requires_admin(&existing_app(serde_json::json!({
            "network": "host"
        }))));
        assert!(app_requires_admin(&existing_app(serde_json::json!({
            "registry_credential_ids": ["credential-id"]
        }))));
    }

    #[test]
    fn registry_credentials_require_an_unscoped_admin() {
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

        for method in [
            "apps.registry_credentials.list",
            "apps.registry_credentials.create",
            "apps.registry_credentials.update",
            "apps.registry_credentials.delete",
        ] {
            let request: Request = serde_json::from_value(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": {}
            }))
            .unwrap();
            assert!(preflight_access_error(&request, &session(Role::Admin, false)).is_none());
            assert!(preflight_access_error(&request, &session(Role::Admin, true)).is_some());
            assert!(preflight_access_error(&request, &session(Role::Operator, false)).is_some());
        }
    }

    #[test]
    fn volume_permission_repair_requires_root_equivalent_access() {
        fn session(role: Role, filesystem: bool, owner: bool) -> Session {
            Session {
                token: "token".into(),
                username: "user".into(),
                role,
                file_principal: None,
                filesystem: filesystem.then(|| "tank".into()),
                owner: owner.then(|| "token-a".into()),
                created_at: 0,
                must_change_password: false,
                client_ip: None,
            }
        }

        let malformed: Request = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "apps.fix_volume_perms",
            "params": "not an object"
        }))
        .unwrap();
        assert!(preflight_access_error(&malformed, &session(Role::Admin, false, false)).is_none());
        assert!(preflight_access_error(&malformed, &session(Role::Admin, true, false)).is_some());
        assert!(preflight_access_error(&malformed, &session(Role::Admin, false, true)).is_some());
        assert!(
            preflight_access_error(&malformed, &session(Role::Operator, false, false)).is_some()
        );

        let unrelated = Request {
            method: "apps.check_volumes".into(),
            ..malformed
        };
        assert!(preflight_access_error(&unrelated, &session(Role::Admin, true, true)).is_none());
    }
}
