//! RPC arms in the `share.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{BlockVolumeId, ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::AppState;
use crate::auth::{Role, Session};

pub(super) async fn try_route(
    req: &Request,
    state: &AppState,
    session: &Session,
) -> Option<Response> {
    let resp = route_inner(req, state, session).await?;
    // iSCSI/NVMe-oF firewall rules follow the configured portal ports
    // (#602). Resync after any mutation that can change them; gate
    // refusals and errors return early above or carry `error`, so only
    // real changes pay the recompute.
    let changes_portals = matches!(
        req.method.as_str(),
        "share.iscsi.create"
            | "share.iscsi.delete"
            | "share.iscsi.add_portal"
            | "share.iscsi.remove_portal"
            | "share.iscsi.set_portals"
            | "share.nvmeof.create"
            | "share.nvmeof.delete"
            | "share.nvmeof.add_port"
            | "share.nvmeof.remove_port"
    );
    if changes_portals && let Err(e) = sync_portal_firewall_ports(state).await {
        if resp.error.is_none() {
            return Some(err(
                req,
                format!("share changed but firewall port synchronization failed: {e}"),
            ));
        }
        tracing::warn!("portal firewall reconciliation after failed request also failed: {e}");
    }
    Some(resp)
}

/// Recompute the iSCSI and NVMe-oF firewall port sets from the
/// configured portals. Defaults to the protocol's standard port while
/// no targets exist, so enabling the protocol opens what the next
/// `create` will bind. NVMe-oF RDMA listeners are excluded — RoCE
/// rides udp/4791 under the separate `rdma` rule, and native IB never
/// traverses netfilter.
pub(crate) async fn portal_firewall_ports(
    state: &AppState,
) -> Result<
    (
        Vec<nasty_system::firewall::PortSpec>,
        Vec<nasty_system::firewall::PortSpec>,
    ),
    String,
> {
    use nasty_system::firewall::{PortSpec, Transport};
    use std::collections::BTreeSet;

    let tcp = |port: u16| PortSpec {
        port,
        to: None,
        transport: Transport::Tcp,
        source: None,
        iface: None,
    };

    let mut iscsi_ports: BTreeSet<u16> = BTreeSet::new();
    let targets = state
        .iscsi
        .list()
        .await
        .map_err(|e| format!("list iSCSI targets: {e}"))?;
    iscsi_ports.extend(
        targets
            .iter()
            .flat_map(|target| target.portals.iter().map(|portal| portal.port)),
    );
    if iscsi_ports.is_empty() {
        iscsi_ports.insert(3260);
    }

    let mut nvmeof_ports: BTreeSet<u16> = BTreeSet::new();
    let subsystems = state
        .nvmeof
        .list()
        .await
        .map_err(|e| format!("list NVMe-oF subsystems: {e}"))?;
    nvmeof_ports.extend(subsystems.iter().flat_map(|subsystem| {
        subsystem
            .ports
            .iter()
            .filter(|port| port.transport == "tcp")
            .filter_map(|port| port.service_id.parse::<u16>().ok())
    }));
    if nvmeof_ports.is_empty() {
        nvmeof_ports.insert(4420);
    }
    Ok((
        iscsi_ports.into_iter().map(tcp).collect(),
        nvmeof_ports.into_iter().map(tcp).collect(),
    ))
}

pub(crate) async fn sync_portal_firewall_ports(state: &AppState) -> Result<(), String> {
    let _sync = state.portal_firewall_sync.lock().await;
    let (iscsi_ports, nvmeof_ports) = portal_firewall_ports(state).await?;
    state
        .firewall
        .set_portal_ports(iscsi_ports, nvmeof_ports)
        .await
}

fn resource_matches_scope(
    filesystem: &str,
    owner: Option<&str>,
    filesystem_filter: Option<&str>,
    owner_filter: Option<&str>,
) -> bool {
    filesystem_filter.is_none_or(|filter| filter == filesystem)
        && owner_filter.is_none_or(|filter| owner == Some(filter))
}

fn session_allows_raw_block_sources(session: &Session) -> bool {
    session.role == Role::Admin && !session_is_scoped(session)
}

fn session_allows_empty_block_destination(session: &Session) -> bool {
    session.role == Role::Operator && !session_is_scoped(session)
}

fn block_authorization_error(req: &Request, session: &Session, error: String) -> Response {
    if error == "access denied" {
        crate::auth::audit(
            "permission_denied",
            &session.username,
            session.client_ip.as_deref().unwrap_or("unknown"),
            &format!(
                "method={} role={:?} reason=block_source_policy",
                req.method, session.role
            ),
        );
    }
    err(req, error)
}

fn path_source_matches_scope(
    requested: &std::path::Path,
    candidates: &[(std::path::PathBuf, String, Option<String>)],
    filesystem_filter: Option<&str>,
    owner_filter: Option<&str>,
) -> bool {
    let containing_matches = candidates
        .iter()
        .filter(|(path, _, _)| requested.starts_with(path))
        .max_by_key(|(path, _, _)| path.components().count())
        .is_some_and(|(_, filesystem, owner)| {
            resource_matches_scope(
                filesystem,
                owner.as_deref(),
                filesystem_filter,
                owner_filter,
            )
        });
    let nested_matches = candidates
        .iter()
        .filter(|(path, _, _)| path.starts_with(requested))
        .all(|(_, filesystem, owner)| {
            resource_matches_scope(
                filesystem,
                owner.as_deref(),
                filesystem_filter,
                owner_filter,
            )
        });
    containing_matches && nested_matches
}

pub(super) async fn authorize_path_source(
    state: &AppState,
    session: &Session,
    requested: &str,
) -> Result<String, String> {
    if session.filesystem.is_none() && session.owner.is_none() {
        return Ok(requested.to_string());
    }

    let canonical = tokio::fs::canonicalize(requested)
        .await
        .map_err(|_| "access denied".to_string())?;
    let subvolumes = state
        .subvolumes
        .list_all(None, None)
        .await
        .map_err(|error| error.to_string())?;
    let mut candidates = Vec::with_capacity(subvolumes.len());
    for subvolume in subvolumes {
        if let Ok(path) = tokio::fs::canonicalize(&subvolume.path).await {
            candidates.push((path, subvolume.filesystem, subvolume.owner));
        }
    }

    if !path_source_matches_scope(
        &canonical,
        &candidates,
        session.filesystem.as_deref(),
        session.owner.as_deref(),
    ) {
        return Err("access denied".to_string());
    }
    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| "access denied".to_string())
}

async fn authorize_block_source(
    state: &AppState,
    session: &Session,
    device_path: &str,
) -> Result<Option<BlockVolumeId>, String> {
    if session_allows_raw_block_sources(session) {
        return state
            .subvolumes
            .block_volume_id_for_device(device_path)
            .await
            .map_err(|error| error.to_string());
    }

    let matched = state
        .subvolumes
        .list_all(None, None)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|subvolume| {
            subvolume.subvolume_type == nasty_storage::subvolume::SubvolumeType::Block
                && subvolume.block_device.as_deref() == Some(device_path)
        })
        .ok_or_else(|| "access denied".to_string())?;
    if !resource_matches_scope(
        &matched.filesystem,
        matched.owner.as_deref(),
        session.filesystem.as_deref(),
        session.owner.as_deref(),
    ) {
        return Err("access denied".to_string());
    }
    matched.block_volume_id.map(Some).ok_or_else(|| {
        format!("managed block device {device_path} has no stable bcachefs identity")
    })
}

async fn authorize_fileio_source(
    state: &AppState,
    session: &Session,
    requested: &str,
) -> Result<String, String> {
    if session_allows_raw_block_sources(session) {
        return Ok(requested.to_string());
    }

    let canonical = canonical_fileio_source(requested).await?;
    if !canonical.starts_with("/fs") {
        return Err("access denied".to_string());
    }

    if session_is_scoped(session) {
        let subvolumes = state
            .subvolumes
            .list_all(None, None)
            .await
            .map_err(|error| error.to_string())?;
        let mut candidates = Vec::with_capacity(subvolumes.len());
        for subvolume in subvolumes {
            if let Ok(path) = tokio::fs::canonicalize(&subvolume.path).await {
                candidates.push((path, subvolume.filesystem, subvolume.owner));
            }
        }
        if !path_source_matches_scope(
            &canonical,
            &candidates,
            session.filesystem.as_deref(),
            session.owner.as_deref(),
        ) {
            return Err("access denied".to_string());
        }
    }

    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| "access denied".to_string())
}

async fn canonical_fileio_source(requested: &str) -> Result<std::path::PathBuf, String> {
    let requested_path = std::path::Path::new(requested);
    let canonical = if requested_path.exists() {
        tokio::fs::canonicalize(requested_path)
            .await
            .map_err(|_| "access denied".to_string())?
    } else {
        let name = requested_path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "access denied".to_string())?;
        let parent = requested_path
            .parent()
            .ok_or_else(|| "access denied".to_string())?;
        tokio::fs::canonicalize(parent)
            .await
            .map_err(|_| "access denied".to_string())?
            .join(name)
    };
    Ok(canonical)
}

pub(super) async fn fileio_source_is_managed(requested: &str) -> bool {
    match canonical_fileio_source(requested).await {
        Ok(path) => path.starts_with("/fs"),
        Err(_) => fileio_source_is_lexically_managed(requested),
    }
}

fn fileio_source_is_lexically_managed(requested: &str) -> bool {
    let path = std::path::Path::new(requested);
    path.is_absolute()
        && path.starts_with("/fs")
        && !path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
}

async fn authorize_block_destination(
    state: &AppState,
    session: &Session,
    identities: &[Option<BlockVolumeId>],
) -> Result<(), String> {
    if session_allows_raw_block_sources(session) {
        return Ok(());
    }
    if identities.is_empty() {
        return if session_allows_empty_block_destination(session) {
            Ok(())
        } else {
            Err("access denied".to_string())
        };
    }
    if identities.iter().any(Option::is_none) {
        return Err("access denied".to_string());
    }

    let subvolumes = state
        .subvolumes
        .list_all(None, None)
        .await
        .map_err(|error| error.to_string())?;
    for identity in identities.iter().flatten() {
        let subvolume = subvolumes
            .iter()
            .find(|subvolume| subvolume.block_volume_id.as_ref() == Some(identity))
            .ok_or_else(|| "access denied".to_string())?;
        if !resource_matches_scope(
            &subvolume.filesystem,
            subvolume.owner.as_deref(),
            session.filesystem.as_deref(),
            session.owner.as_deref(),
        ) {
            return Err("access denied".to_string());
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
async fn authorized_iscsi_target(
    req: &Request,
    state: &AppState,
    session: &Session,
    target_id: &str,
) -> Result<nasty_sharing::iscsi::IscsiTarget, Response> {
    let target = state
        .iscsi
        .get(target_id)
        .await
        .map_err(|error| err(req, error))?;

    let mut root_reason = None;
    for lun in &target.luns {
        match lun.backstore_type.as_str() {
            "block" if lun.backing_volume.is_none() || lun.backing_volume_unresolved => {
                root_reason = Some("raw_iscsi_destination");
            }
            "fileio" if !fileio_source_is_managed(&lun.backstore_path).await => {
                root_reason = Some("unmanaged_iscsi_fileio_destination");
            }
            "block" | "fileio" => {}
            _ => root_reason = Some("unknown_iscsi_backstore"),
        }
    }
    if let Some(reason) = root_reason
        && let Some(response) = require_root_equivalent(req, session, reason)
    {
        return Err(response);
    }

    if session_is_scoped(session) {
        for lun in target
            .luns
            .iter()
            .filter(|lun| lun.backstore_type == "fileio")
        {
            authorize_fileio_source(state, session, &lun.backstore_path)
                .await
                .map_err(|error| block_authorization_error(req, session, error))?;
        }
        let identities: Vec<_> = target
            .luns
            .iter()
            .filter(|lun| lun.backstore_type == "block")
            .map(|lun| lun.backing_volume.clone())
            .collect();
        if target.luns.is_empty() || !identities.is_empty() {
            authorize_block_destination(state, session, &identities)
                .await
                .map_err(|error| block_authorization_error(req, session, error))?;
        }
    }
    Ok(target)
}

#[allow(clippy::result_large_err)]
async fn authorized_nvmeof_subsystem(
    req: &Request,
    state: &AppState,
    session: &Session,
    subsystem_id: &str,
) -> Result<nasty_sharing::nvmeof::NvmeofSubsystem, Response> {
    let subsystem = state
        .nvmeof
        .get(subsystem_id)
        .await
        .map_err(|error| err(req, error))?;
    if subsystem
        .namespaces
        .iter()
        .any(|namespace| namespace.backing_volume.is_none() || namespace.backing_volume_unresolved)
        && let Some(response) = require_root_equivalent(req, session, "raw_nvmeof_destination")
    {
        return Err(response);
    }
    if session_is_scoped(session) {
        let identities: Vec<_> = subsystem
            .namespaces
            .iter()
            .map(|namespace| namespace.backing_volume.clone())
            .collect();
        authorize_block_destination(state, session, &identities)
            .await
            .map_err(|error| block_authorization_error(req, session, error))?;
    }
    Ok(subsystem)
}

fn session_is_scoped(session: &Session) -> bool {
    session.filesystem.is_some() || session.owner.is_some()
}

fn smb_extra_params_require_admin(
    existing: Option<&std::collections::HashMap<String, String>>,
    requested: Option<&std::collections::HashMap<String, String>>,
) -> bool {
    requested
        .or(existing)
        .is_some_and(|params| !params.is_empty())
}

async fn quiesce_iscsi_after_failed_repair(state: &AppState) -> Option<String> {
    let mut failures = Vec::new();
    if let Err(error) = state
        .protocols
        .quiesce(nasty_system::protocol::Protocol::Iscsi)
        .await
    {
        failures.push(error);
    }
    if let Err(error) = state
        .firewall
        .close(nasty_system::protocol::Protocol::Iscsi)
        .await
    {
        failures.push(error);
    }
    (!failures.is_empty()).then(|| failures.join("; "))
}

async fn quiesce_nvmeof_after_failed_repair(state: &AppState) -> Option<String> {
    let mut failures = Vec::new();
    if let Err(error) = state.nvmeof.quiesce().await {
        failures.push(error.to_string());
    }
    if let Err(error) = state
        .firewall
        .close(nasty_system::protocol::Protocol::Nvmeof)
        .await
    {
        failures.push(error);
    }
    (!failures.is_empty()).then(|| failures.join("; "))
}

async fn route_inner(req: &Request, state: &AppState, session: &Session) -> Option<Response> {
    let is_block_share_mutation = (req.method.starts_with("share.iscsi.")
        || req.method.starts_with("share.nvmeof."))
        && !req.method.ends_with(".list")
        && !req.method.ends_with(".get");
    let _block_share_guard = if is_block_share_mutation {
        Some(state.block_share_mutation.lock().await)
    } else {
        None
    };

    Some(match req.method.as_str() {
        "share.nfs.list" => match state.nfs.list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "share.nfs.get" => match require_str(req, "id") {
            Ok(id) => match state.nfs.get(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "share.nfs.create" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nfs).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nfs::CreateNfsShareRequest>(req) {
                Ok(mut p) => match authorize_path_source(state, session, &p.path).await {
                    Ok(path) => {
                        p.path = path;
                        match state.nfs.create(p).await {
                            Ok(v) => ok(req, v),
                            Err(e) => err(req, e),
                        }
                    }
                    Err(e) => err(req, e),
                },
                Err(e) => invalid(req, e),
            }
        }
        "share.nfs.update" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nfs).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nfs::UpdateNfsShareRequest>(req) {
                Ok(p) => {
                    let share = match state.nfs.get(&p.id).await {
                        Ok(share) => share,
                        Err(error) => return Some(err(req, error)),
                    };
                    if let Err(error) = authorize_path_source(state, session, &share.path).await {
                        return Some(err(req, error));
                    }
                    match state.nfs.update(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nfs.delete" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nfs).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nfs::DeleteNfsShareRequest>(req) {
                Ok(p) => {
                    let share = match state.nfs.get(&p.id).await {
                        Ok(share) => share,
                        Err(error) => return Some(err(req, error)),
                    };
                    if let Err(error) = authorize_path_source(state, session, &share.path).await {
                        return Some(err(req, error));
                    }
                    match state.nfs.delete(p).await {
                        Ok(()) => ok(req, "ok"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.smb.list" => match state.smb.list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "share.smb.get" => match require_str(req, "id") {
            Ok(id) => match state.smb.get(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "share.smb.create" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::smb::CreateSmbShareRequest>(req) {
                Ok(mut p) => {
                    if smb_extra_params_require_admin(None, p.extra_params.as_ref())
                        && let Some(response) =
                            require_root_equivalent(req, session, "raw_samba_parameters")
                    {
                        return Some(response);
                    }
                    if session_is_scoped(session)
                        && state.smb.list().await.is_ok_and(|shares| {
                            shares
                                .iter()
                                .any(|share| share.name.eq_ignore_ascii_case(&p.name))
                        })
                    {
                        return Some(err(req, "access denied"));
                    }
                    match authorize_path_source(state, session, &p.path).await {
                        Ok(path) => {
                            p.path = path;
                            match state.smb.create(p).await {
                                Ok(v) => ok(req, v),
                                Err(e) => err(req, e),
                            }
                        }
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.smb.update" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::smb::UpdateSmbShareRequest>(req) {
                Ok(p) => {
                    let share = match state.smb.get(&p.id).await {
                        Ok(share) => share,
                        Err(error) => return Some(err(req, error)),
                    };
                    if smb_extra_params_require_admin(
                        Some(&share.extra_params),
                        p.extra_params.as_ref(),
                    ) && let Some(response) =
                        require_root_equivalent(req, session, "raw_samba_parameters")
                    {
                        return Some(response);
                    }
                    if let Err(error) = authorize_path_source(state, session, &share.path).await {
                        return Some(err(req, error));
                    }
                    match state.smb.update(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.smb.delete" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::smb::DeleteSmbShareRequest>(req) {
                Ok(p) => {
                    let share = match state.smb.get(&p.id).await {
                        Ok(share) => share,
                        Err(error) => return Some(err(req, error)),
                    };
                    if let Err(error) = authorize_path_source(state, session, &share.path).await {
                        return Some(err(req, error));
                    }
                    match state.smb.delete(p).await {
                        Ok(()) => ok(req, "ok"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.list" => match state.iscsi.list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "share.iscsi.get" => match require_str(req, "id") {
            Ok(id) => match state.iscsi.get(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "share.iscsi.create" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::CreateTargetRequest>(req) {
                Ok(mut p) => {
                    if session_is_scoped(session) {
                        if p.device_path.is_none() {
                            return Some(err(req, "access denied"));
                        }
                        let iqn = format!("iqn.2137-04.storage.nasty:{}", p.name);
                        if state.iscsi.list().await.is_ok_and(|targets| {
                            targets
                                .iter()
                                .any(|target| target.iqn.eq_ignore_ascii_case(&iqn))
                        }) {
                            return Some(err(req, "access denied"));
                        }
                    }
                    if p.portals
                        .as_deref()
                        .is_some_and(|ps| ps.iter().any(|portal| portal.iser))
                        && let Some(r) = require_rdma(req, "ib_isert").await
                    {
                        return Some(r);
                    }
                    if let Some(ref device_path) = p.device_path {
                        match authorize_block_source(state, session, device_path).await {
                            Ok(identity) => {
                                if identity.is_none()
                                    && let Some(response) =
                                        require_root_equivalent(req, session, "raw_iscsi_source")
                                {
                                    return Some(response);
                                }
                                p.backing_volume = identity;
                            }
                            Err(error) => {
                                return Some(block_authorization_error(req, session, error));
                            }
                        }
                    }
                    if let Some(ref dp) = p.device_path
                        && let Some(conflict) =
                            check_block_device_conflict(state, dp, "iscsi").await
                    {
                        return Some(err(req, conflict));
                    }
                    match state.iscsi.create(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.delete" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::DeleteTargetRequest>(req) {
                Ok(p) => {
                    if let Err(response) = authorized_iscsi_target(req, state, session, &p.id).await
                    {
                        return Some(response);
                    }
                    match state.iscsi.delete(p).await {
                        Ok(()) => ok(req, "ok"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.add_lun" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::AddLunRequest>(req) {
                Ok(mut p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    let fileio = p.backstore_type.as_deref() == Some("fileio")
                        || (p.backstore_type.is_none()
                            && std::path::Path::new(&p.backstore_path)
                                .metadata()
                                .is_ok_and(|metadata| metadata.is_file()));
                    if fileio {
                        match authorize_fileio_source(state, session, &p.backstore_path).await {
                            Ok(path) => p.backstore_path = path,
                            Err(error) => {
                                return Some(block_authorization_error(req, session, error));
                            }
                        }
                    } else {
                        match authorize_block_source(state, session, &p.backstore_path).await {
                            Ok(identity) => {
                                if identity.is_none()
                                    && let Some(response) =
                                        require_root_equivalent(req, session, "raw_iscsi_source")
                                {
                                    return Some(response);
                                }
                                p.backing_volume = identity;
                            }
                            Err(error) => {
                                return Some(block_authorization_error(req, session, error));
                            }
                        }
                    }
                    if let Some(conflict) =
                        check_block_device_conflict(state, &p.backstore_path, "iscsi").await
                    {
                        err(req, conflict)
                    } else {
                        match state.iscsi.add_lun(p).await {
                            Ok(v) => ok(req, v),
                            Err(e) => err(req, e),
                        }
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.remove_lun" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::RemoveLunRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    match state.iscsi.remove_lun(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.repair_lun" => {
            if session.role != Role::Admin || session_is_scoped(session) {
                return Some(err(req, "unscoped Admin session required"));
            }
            match parse_params::<nasty_sharing::iscsi::RepairLunRequest>(req) {
                Ok(p) => {
                    if let Some(conflict) =
                        check_block_device_conflict(state, &p.device_path, "iscsi").await
                    {
                        return Some(err(req, conflict));
                    }
                    match state
                        .subvolumes
                        .block_volume_id_for_device(&p.device_path)
                        .await
                    {
                        Ok(Some(identity)) => match state.iscsi.repair_lun(p, identity).await {
                            Ok((repaired, previous)) => {
                                let activation = match state
                                    .protocols
                                    .quiesce(nasty_system::protocol::Protocol::Iscsi)
                                    .await
                                {
                                    Ok(()) => {
                                        super::fs::reconcile_block_shares_under_lock(state).await
                                    }
                                    Err(error) => Err(format!(
                                        "failed to stop the existing iSCSI export: {error}"
                                    )),
                                };
                                match activation {
                                    Ok(()) => ok(req, repaired.redacted()),
                                    Err(activation_error) => {
                                        let rollback_error =
                                            state.iscsi.restore_target_state(&previous).await.err();
                                        let cleanup_error =
                                            quiesce_iscsi_after_failed_repair(state).await;
                                        let mut message = format!(
                                            "LUN repair could not be activated: {activation_error}"
                                        );
                                        if let Some(error) = rollback_error {
                                            message.push_str(&format!(
                                                "; persisted-state rollback failed: {error}"
                                            ));
                                        } else {
                                            message.push_str("; persisted state was rolled back");
                                        }
                                        if let Some(error) = cleanup_error {
                                            message.push_str(&format!(
                                                "; safe export cleanup failed: {error}"
                                            ));
                                        }
                                        err(req, message)
                                    }
                                }
                            }
                            Err(e) => err(req, e),
                        },
                        Ok(None) => err(req, "selected device is not a managed block subvolume"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.add_acl" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::AddAclRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    match state.iscsi.add_acl(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.remove_acl" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::RemoveAclRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    match state.iscsi.remove_acl(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.add_portal" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::AddPortalRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    if p.iser
                        && let Some(r) = require_rdma(req, "ib_isert").await
                    {
                        return Some(r);
                    }
                    match state.iscsi.add_portal(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.set_portals" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::SetPortalsRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    if p.portals.iter().any(|portal| portal.iser)
                        && let Some(r) = require_rdma(req, "ib_isert").await
                    {
                        return Some(r);
                    }
                    match state.iscsi.set_portals(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.iscsi.remove_portal" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Iscsi).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::iscsi::RemovePortalRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_iscsi_target(req, state, session, &p.target_id).await
                    {
                        return Some(response);
                    }
                    match state.iscsi.remove_portal(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.list" => match state.nvmeof.list().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "share.nvmeof.get" => match require_str(req, "id") {
            Ok(id) => match state.nvmeof.get(id).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "share.nvmeof.create" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::CreateSubsystemRequest>(req) {
                Ok(mut p) => {
                    if session_is_scoped(session) {
                        if p.device_path.is_none() {
                            return Some(err(req, "access denied"));
                        }
                        let nqn = format!("nqn.2137-04.storage.nasty:{}", p.name);
                        if state.nvmeof.list().await.is_ok_and(|subsystems| {
                            subsystems.iter().any(|subsystem| subsystem.nqn == nqn)
                        }) {
                            return Some(err(req, "access denied"));
                        }
                    }
                    if let Some(ref device_path) = p.device_path {
                        match authorize_block_source(state, session, device_path).await {
                            Ok(identity) => {
                                if identity.is_none()
                                    && let Some(response) =
                                        require_root_equivalent(req, session, "raw_nvmeof_source")
                                {
                                    return Some(response);
                                }
                                p.backing_volume = identity;
                            }
                            Err(error) => {
                                return Some(block_authorization_error(req, session, error));
                            }
                        }
                    }
                    if let Some(ref device_path) = p.device_path
                        && let Some(conflict) =
                            check_block_device_conflict(state, device_path, "nvmeof").await
                    {
                        return Some(err(req, conflict));
                    }
                    match state.nvmeof.create(p).await {
                        Ok(v) => {
                            // If Tailscale is connected, add a port for its IP too
                            if !v.ports.is_empty() {
                                let ts = state.tailscale.get().await;
                                if ts.connected
                                    && let Some(ref ip) = ts.ip
                                    && let Err(e) = state
                                        .nvmeof
                                        .add_port(nasty_sharing::nvmeof::AddPortRequest {
                                            subsystem_id: v.id.clone(),
                                            transport: Some("tcp".to_string()),
                                            addr: Some(ip.clone()),
                                            service_id: Some(4420),
                                            addr_family: Some("ipv4".to_string()),
                                        })
                                        .await
                                {
                                    tracing::warn!(
                                        "auto-add Tailscale port for '{}' on {ip} failed: {e}",
                                        v.nqn
                                    );
                                }
                            }
                            ok(req, v)
                        }
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.delete" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::DeleteSubsystemRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.id).await
                    {
                        return Some(response);
                    }
                    match state.nvmeof.delete(p).await {
                        Ok(()) => ok(req, "ok"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.add_namespace" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::AddNamespaceRequest>(req) {
                Ok(mut p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    match authorize_block_source(state, session, &p.device_path).await {
                        Ok(identity) => {
                            if identity.is_none()
                                && let Some(response) =
                                    require_root_equivalent(req, session, "raw_nvmeof_source")
                            {
                                return Some(response);
                            }
                            p.backing_volume = identity;
                        }
                        Err(error) => {
                            return Some(block_authorization_error(req, session, error));
                        }
                    }
                    if let Some(conflict) =
                        check_block_device_conflict(state, &p.device_path, "nvmeof").await
                    {
                        err(req, conflict)
                    } else {
                        match state.nvmeof.add_namespace(p).await {
                            Ok(v) => ok(req, v),
                            Err(e) => err(req, e),
                        }
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.remove_namespace" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::RemoveNamespaceRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    match state.nvmeof.remove_namespace(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.repair_namespace" => {
            if session.role != Role::Admin || session_is_scoped(session) {
                return Some(err(req, "unscoped Admin session required"));
            }
            match parse_params::<nasty_sharing::nvmeof::RepairNamespaceRequest>(req) {
                Ok(p) => {
                    if let Some(conflict) =
                        check_block_device_conflict(state, &p.device_path, "nvmeof").await
                    {
                        return Some(err(req, conflict));
                    }
                    match state
                        .subvolumes
                        .block_volume_id_for_device(&p.device_path)
                        .await
                    {
                        Ok(Some(identity)) => {
                            match state.nvmeof.repair_namespace(p, identity).await {
                                Ok((repaired, previous)) => {
                                    let activation = match state.nvmeof.quiesce().await {
                                        Ok(()) => {
                                            super::fs::reconcile_block_shares_under_lock(state)
                                                .await
                                        }
                                        Err(error) => Err(format!(
                                            "failed to stop the existing NVMe-oF export: {error}"
                                        )),
                                    };
                                    match activation {
                                        Ok(()) => ok(req, repaired),
                                        Err(activation_error) => {
                                            let rollback_error = state
                                                .nvmeof
                                                .restore_subsystem_state(&previous)
                                                .await
                                                .err();
                                            let cleanup_error =
                                                quiesce_nvmeof_after_failed_repair(state).await;
                                            let mut message = format!(
                                                "namespace repair could not be activated: {activation_error}"
                                            );
                                            if let Some(error) = rollback_error {
                                                message.push_str(&format!(
                                                    "; persisted-state rollback failed: {error}"
                                                ));
                                            } else {
                                                message
                                                    .push_str("; persisted state was rolled back");
                                            }
                                            if let Some(error) = cleanup_error {
                                                message.push_str(&format!(
                                                    "; safe export cleanup failed: {error}"
                                                ));
                                            }
                                            err(req, message)
                                        }
                                    }
                                }
                                Err(e) => err(req, e),
                            }
                        }
                        Ok(None) => err(req, "selected device is not a managed block subvolume"),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.add_port" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::AddPortRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    if p.transport.as_deref() == Some("rdma")
                        && let Some(r) = require_rdma(req, "nvmet-rdma").await
                    {
                        return Some(r);
                    }
                    match state.nvmeof.add_port(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.remove_port" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::RemovePortRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    match state.nvmeof.remove_port(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.add_host" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::AddHostRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    match state.nvmeof.add_host(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        "share.nvmeof.remove_host" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Nvmeof).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::nvmeof::RemoveHostRequest>(req) {
                Ok(p) => {
                    if let Err(response) =
                        authorized_nvmeof_subsystem(req, state, session, &p.subsystem_id).await
                    {
                        return Some(response);
                    }
                    match state.nvmeof.remove_host(p).await {
                        Ok(v) => ok(req, v),
                        Err(e) => err(req, e),
                    }
                }
                Err(e) => invalid(req, e),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn candidate(
        path: &str,
        filesystem: &str,
        owner: Option<&str>,
    ) -> (PathBuf, String, Option<String>) {
        (
            PathBuf::from(path),
            filesystem.to_string(),
            owner.map(str::to_string),
        )
    }

    fn session(role: Role, filesystem: Option<&str>, owner: Option<&str>) -> Session {
        Session {
            token: "token".to_string(),
            username: "tester".to_string(),
            role,
            file_principal: None,
            filesystem: filesystem.map(str::to_string),
            owner: owner.map(str::to_string),
            created_at: 0,
            must_change_password: false,
            client_ip: Some("127.0.0.1".to_string()),
        }
    }

    #[test]
    fn raw_block_sources_require_unscoped_admin() {
        assert!(session_allows_raw_block_sources(&session(
            Role::Admin,
            None,
            None
        )));
        assert!(!session_allows_raw_block_sources(&session(
            Role::Operator,
            None,
            None
        )));
        assert!(!session_allows_raw_block_sources(&session(
            Role::Admin,
            Some("tank"),
            None
        )));
        assert!(!session_allows_raw_block_sources(&session(
            Role::Admin,
            None,
            Some("token-a")
        )));
    }

    #[test]
    fn samba_raw_parameters_use_the_effective_update_value() {
        let existing = std::collections::HashMap::from([(
            "root preexec".to_string(),
            "/bin/true".to_string(),
        )]);
        let cleared = std::collections::HashMap::new();

        assert!(smb_extra_params_require_admin(Some(&existing), None));
        assert!(smb_extra_params_require_admin(None, Some(&existing)));
        assert!(!smb_extra_params_require_admin(
            Some(&existing),
            Some(&cleared)
        ));
    }

    #[test]
    fn offline_fileio_sources_use_a_component_safe_fs_fallback() {
        assert!(fileio_source_is_lexically_managed(
            "/fs/offline/subvolume/disk.img"
        ));
        assert!(!fileio_source_is_lexically_managed(
            "/fs/offline/../etc/passwd"
        ));
        assert!(!fileio_source_is_lexically_managed(
            "/filesystem/offline/disk.img"
        ));
        assert!(!fileio_source_is_lexically_managed("relative/disk.img"));
    }

    #[test]
    fn unscoped_operator_can_seed_an_empty_target_with_a_managed_source() {
        assert!(session_allows_empty_block_destination(&session(
            Role::Operator,
            None,
            None
        )));
        assert!(!session_allows_empty_block_destination(&session(
            Role::Operator,
            Some("tank"),
            None
        )));
        assert!(!session_allows_empty_block_destination(&session(
            Role::ReadOnly,
            None,
            None
        )));
    }

    #[test]
    fn path_scope_accepts_owned_subvolume_and_descendant() {
        let candidates = [candidate("/fs/first/owned", "first", Some("token-a"))];
        for path in ["/fs/first/owned", "/fs/first/owned/folder"] {
            assert!(path_source_matches_scope(
                Path::new(path),
                &candidates,
                Some("first"),
                Some("token-a"),
            ));
        }
    }

    #[test]
    fn path_scope_respects_component_boundaries() {
        let candidates = [candidate("/fs/first/data", "first", Some("token-a"))];
        assert!(!path_source_matches_scope(
            Path::new("/fs/first/data2"),
            &candidates,
            Some("first"),
            Some("token-a"),
        ));
    }

    #[test]
    fn deepest_subvolume_controls_path_ownership() {
        let candidates = [
            candidate("/fs/first/parent", "first", Some("token-a")),
            candidate("/fs/first/parent/foreign", "first", Some("token-b")),
        ];
        assert!(!path_source_matches_scope(
            Path::new("/fs/first/parent/foreign/file"),
            &candidates,
            Some("first"),
            Some("token-a"),
        ));
    }

    #[test]
    fn parent_export_rejects_nested_foreign_subvolume() {
        let candidates = [
            candidate("/fs/first/parent", "first", Some("token-a")),
            candidate("/fs/first/parent/foreign", "first", Some("token-b")),
        ];
        assert!(!path_source_matches_scope(
            Path::new("/fs/first/parent"),
            &candidates,
            Some("first"),
            Some("token-a"),
        ));
    }

    #[test]
    fn resource_scope_requires_filesystem_and_owner() {
        assert!(resource_matches_scope(
            "first",
            Some("token-a"),
            Some("first"),
            Some("token-a"),
        ));
        assert!(!resource_matches_scope(
            "second",
            Some("token-a"),
            Some("first"),
            Some("token-a"),
        ));
        assert!(!resource_matches_scope(
            "first",
            None,
            Some("first"),
            Some("token-a"),
        ));
        assert!(resource_matches_scope("first", None, Some("first"), None));
    }
}
