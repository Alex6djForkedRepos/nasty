//! RPC arms in the `bcachefs.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::AppState;
use crate::auth::{Role, Session};

fn bcachefs_scope_denied(session: &Session, requested: Option<&str>) -> bool {
    session.owner.is_some()
        || matches!(
            (session.filesystem.as_deref(), requested),
            (Some(scope), Some(requested)) if requested != scope
        )
}

fn is_bcachefs_read(method: &str) -> bool {
    matches!(
        method,
        "bcachefs.usage" | "bcachefs.top" | "bcachefs.timestats"
    )
}

pub(super) async fn try_route(
    req: &Request,
    state: &AppState,
    session: &Session,
) -> Option<Response> {
    if is_bcachefs_read(&req.method) && bcachefs_scope_denied(session, str_param(req, "name")) {
        return Some(err(req, "access denied"));
    }
    Some(match req.method.as_str() {
        "bcachefs.usage" => match require_str(req, "name") {
            Ok(name) => match state.filesystems.bcachefs_usage(name).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "bcachefs.top" => match require_str(req, "name") {
            Ok(name) => match state.filesystems.bcachefs_top(name).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "bcachefs.timestats" => match require_str(req, "name") {
            Ok(name) => match state.filesystems.bcachefs_timestats(name).await {
                Ok(v) => ok(req, v),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{bcachefs_scope_denied, is_bcachefs_read};
    use crate::auth::{Role, Session};

    fn session(filesystem: Option<&str>, owner: Option<&str>) -> Session {
        Session {
            token: "token".into(),
            username: "user".into(),
            role: Role::ReadOnly,
            file_principal: None,
            filesystem: filesystem.map(str::to_string),
            owner: owner.map(str::to_string),
            created_at: 0,
            must_change_password: false,
            client_ip: None,
        }
    }

    #[test]
    fn diagnostics_require_a_matching_filesystem_and_no_owner_scope() {
        for method in ["bcachefs.usage", "bcachefs.top", "bcachefs.timestats"] {
            assert!(is_bcachefs_read(method), "{method}");
        }
        assert!(!is_bcachefs_read("system.status"));
        assert!(!bcachefs_scope_denied(&session(None, None), Some("tank")));
        assert!(!bcachefs_scope_denied(
            &session(Some("tank"), None),
            Some("tank")
        ));
        assert!(bcachefs_scope_denied(
            &session(Some("tank"), None),
            Some("other")
        ));
        assert!(bcachefs_scope_denied(
            &session(Some("tank"), Some("token-a")),
            Some("tank")
        ));
    }
}
