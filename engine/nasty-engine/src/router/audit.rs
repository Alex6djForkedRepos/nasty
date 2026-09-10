//! RPC arms in the `audit.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::auth::{Role, Session};

fn preflight_access_error(req: &Request, session: &Session) -> Option<Response> {
    (req.method == "audit.list")
        .then(|| require_root_equivalent_read(req, session, "global_audit_log_read"))
        .flatten()
}

pub(super) async fn try_route(req: &Request, session: &Session) -> Option<Response> {
    if let Some(response) = preflight_access_error(req, session) {
        return Some(response);
    }
    Some(match req.method.as_str() {
        "audit.list" => {
            let limit = req
                .params
                .as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as usize;
            ok(req, crate::auth::read_audit_log(limit).await)
        }
        "audit.mine" => {
            let limit = req
                .params
                .as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as usize;
            ok(
                req,
                crate::auth::read_audit_log_for_user(&session.username, limit).await,
            )
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{preflight_access_error, try_route};
    use crate::auth::{Role, Session};
    use nasty_common::Request;

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

    #[tokio::test]
    async fn global_audit_log_requires_root_equivalent_access() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "audit.list",
            "params": "not an object"
        }))
        .unwrap();

        for (denied, expected) in [
            (
                session(Role::Admin, true, false),
                "Scoped credentials cannot access this endpoint",
            ),
            (
                session(Role::Admin, false, true),
                "Scoped credentials cannot access this endpoint",
            ),
            (session(Role::Operator, false, false), "Permission denied"),
            (session(Role::ReadOnly, false, false), "Permission denied"),
            (session(Role::User, false, false), "Permission denied"),
        ] {
            let response = try_route(&request, &denied).await.unwrap();
            assert_eq!(response.error.unwrap().message, expected);
        }
        assert!(preflight_access_error(&request, &session(Role::Admin, false, false)).is_none());
    }

    #[test]
    fn personal_audit_log_remains_self_scoped() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "audit.mine",
            "params": {"limit": 20}
        }))
        .unwrap();

        assert!(preflight_access_error(&request, &session(Role::User, false, false)).is_none());
        assert!(preflight_access_error(&request, &session(Role::Admin, true, true)).is_none());
    }
}
