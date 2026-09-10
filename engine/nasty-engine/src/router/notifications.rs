//! RPC arms in the `notifications.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::auth::{Role, Session};

fn preflight_access_error(req: &Request, session: &Session) -> Option<Response> {
    matches!(
        req.method.as_str(),
        "notifications.config.get"
            | "notifications.config.update"
            | "notifications.test"
            | "notifications.test_saved"
    )
    .then(|| require_root_equivalent(req, session, "global_notification_management"))
    .flatten()
}

pub(super) async fn try_route(req: &Request, session: &Session) -> Option<Response> {
    if let Some(response) = preflight_access_error(req, session) {
        return Some(response);
    }
    Some(match req.method.as_str() {
        "notifications.config.get" => ok(
            req,
            nasty_system::notifications::NotificationConfig::load().redacted(),
        ),
        "notifications.config.update" => {
            match parse_params::<nasty_system::notifications::NotificationConfig>(req) {
                Ok(config) => match config.apply_update().await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(e) => err(req, e),
            }
        }
        "notifications.test" => match parse_params::<nasty_system::notifications::ChannelType>(req)
        {
            Ok(channel) => match nasty_system::notifications::test_channel(&channel).await {
                Ok(msg) => ok(req, msg),
                Err(e) => err(req, e),
            },
            Err(e) => err(req, e),
        },
        // Test a saved channel by id — resolves sealed secrets server-side
        // so the WebUI never has to send a redacted/real secret back.
        "notifications.test_saved" => match parse_params::<TestSavedRequest>(req) {
            Ok(p) => match nasty_system::notifications::test_saved_channel(&p.id).await {
                Ok(msg) => ok(req, msg),
                Err(e) => err(req, e),
            },
            Err(e) => err(req, e),
        },
        _ => return None,
    })
}

#[derive(Deserialize)]
struct TestSavedRequest {
    id: String,
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
    async fn notification_configuration_and_delivery_require_root_equivalent_access() {
        for method in [
            "notifications.config.get",
            "notifications.config.update",
            "notifications.test",
            "notifications.test_saved",
        ] {
            let request: Request = serde_json::from_value(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
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
            ] {
                let response = try_route(&request, &denied).await.unwrap();
                assert_eq!(response.error.unwrap().message, expected, "{method}");
            }
            assert!(
                preflight_access_error(&request, &session(Role::Admin, false, false)).is_none(),
                "{method}"
            );
        }

        let unrelated: Request = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "notifications.unknown"
        }))
        .unwrap();
        assert!(preflight_access_error(&unrelated, &session(Role::Admin, true, true)).is_none());
    }
}
