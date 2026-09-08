//! RPC arms in the `smb.*` domain. Extracted from the historical
//! 231-arm `match` in `router.rs`. Returns `Some(response)` when the
//! method matches, `None` when it falls through to another domain.

#![allow(unused_imports, unused_variables)]

use nasty_common::{ErrorCode, Request, Response};
use serde::Deserialize;

use super::*;
use crate::AppState;
use crate::auth::Session;

fn is_smb_identity_mutation(method: &str) -> bool {
    matches!(
        method,
        "smb.user.create"
            | "smb.user.delete"
            | "smb.user.set_password"
            | "smb.group.create"
            | "smb.group.delete"
            | "smb.group.add_member"
            | "smb.group.remove_member"
    )
}

pub(super) async fn try_route(
    req: &Request,
    state: &AppState,
    session: &Session,
) -> Option<Response> {
    if is_smb_identity_mutation(&req.method)
        && let Some(response) =
            require_unscoped_mutation(req, session, "global_smb_identity_mutation")
    {
        return Some(response);
    }

    Some(match req.method.as_str() {
        "smb.user.list" => match state.smb.list_users().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "smb.user.create" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            match parse_params::<nasty_sharing::smb::CreateSmbUserRequest>(req) {
                Ok(p) => match state.smb.create_user(p).await {
                    Ok(u) => ok(req, u),
                    Err(e) => err(req, e),
                },
                Err(e) => invalid(req, e),
            }
        }
        "smb.user.delete" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            match require_str(req, "username") {
                Ok(username) => match state.smb.delete_user(username).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(r) => r,
            }
        }
        "smb.user.set_password" => {
            if let Some(r) =
                require_protocol(state, req, nasty_system::protocol::Protocol::Smb).await
            {
                return Some(r);
            }
            #[derive(Deserialize)]
            struct P {
                username: String,
                password: String,
            }
            match parse_params::<P>(req) {
                Ok(p) => match state.smb.set_user_password(&p.username, &p.password).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(e) => invalid(req, e),
            }
        }
        "smb.group.list" => match state.smb.list_groups().await {
            Ok(v) => ok(req, v),
            Err(e) => err(req, e),
        },
        "smb.group.create" => match require_str(req, "name") {
            Ok(name) => match state.smb.create_group(name).await {
                Ok(g) => ok(req, g),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "smb.group.delete" => match require_str(req, "name") {
            Ok(name) => match state.smb.delete_group(name).await {
                Ok(()) => ok(req, "ok"),
                Err(e) => err(req, e),
            },
            Err(r) => r,
        },
        "smb.group.add_member" => {
            #[derive(Deserialize)]
            struct P {
                group: String,
                user: String,
            }
            match parse_params::<P>(req) {
                Ok(p) => match state.smb.add_group_member(&p.group, &p.user).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(e) => invalid(req, e),
            }
        }
        "smb.group.remove_member" => {
            #[derive(Deserialize)]
            struct P {
                group: String,
                user: String,
            }
            match parse_params::<P>(req) {
                Ok(p) => match state.smb.remove_group_member(&p.group, &p.user).await {
                    Ok(()) => ok(req, "ok"),
                    Err(e) => err(req, e),
                },
                Err(e) => invalid(req, e),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::is_smb_identity_mutation;
    use crate::auth::{EndpointAccess, Role, Session, authorize_session};

    fn session(role: Role, filesystem: Option<&str>, owner: Option<&str>) -> Session {
        Session {
            token: "token".into(),
            username: "user".into(),
            role,
            file_principal: None,
            filesystem: filesystem.map(str::to_string),
            owner: owner.map(str::to_string),
            created_at: 0,
            must_change_password: false,
            client_ip: None,
        }
    }

    #[test]
    fn all_smb_identity_mutations_are_scope_gated() {
        for method in [
            "smb.user.create",
            "smb.user.delete",
            "smb.user.set_password",
            "smb.group.create",
            "smb.group.delete",
            "smb.group.add_member",
            "smb.group.remove_member",
        ] {
            assert!(
                is_smb_identity_mutation(method),
                "missing gate for {method}"
            );
        }
        assert!(!is_smb_identity_mutation("smb.user.list"));
        assert!(!is_smb_identity_mutation("smb.group.list"));
    }

    #[test]
    fn smb_identity_mutations_allow_only_unscoped_operators_and_admins() {
        let access = EndpointAccess::UnscopedMutation;
        assert!(authorize_session(&session(Role::Operator, None, None), access).is_ok());
        assert!(authorize_session(&session(Role::Admin, None, None), access).is_ok());
        assert!(authorize_session(&session(Role::Operator, Some("tank"), None), access).is_err());
        assert!(authorize_session(&session(Role::Operator, None, Some("alice")), access).is_err());
        assert!(authorize_session(&session(Role::Admin, Some("tank"), None), access).is_err());
        assert!(authorize_session(&session(Role::Admin, None, Some("alice")), access).is_err());
        assert!(authorize_session(&session(Role::ReadOnly, None, None), access).is_err());
    }
}
