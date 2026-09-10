# API Role Review

Date: 2026-09-07

## Scope

Static review of all 329 methods registered in
`engine/nasty-engine/src/registry/methods.rs`:

- 126 `Any`
- 79 `Operator`
- 124 `Admin`

The registry declarations match the central dispatcher. The findings below are
semantic authorization problems that the registry/dispatcher consistency tests
cannot detect, including credential scope, sensitive response data, and
payload-dependent privilege.

## Critical

### Scoped Admin credentials can escape their scope

Status: fixed in the current uncommitted worktree.

`auth.token.create` lets a filesystem-scoped Admin create an unscoped Admin
token. `auth.create_user` similarly lets a scoped Admin create an Admin user
whose interactive login is unscoped.

References:

- `engine/nasty-engine/src/router/mod.rs:443`
- `engine/nasty-engine/src/auth.rs:704`
- `engine/nasty-engine/src/auth.rs:884`

These operations must require an unscoped Admin session.

### Scoped Admin can alter resources outside its filesystem

Status: fixed in the current uncommitted worktree for filesystem-addressed
operations and global raw-device mutations.

Many Admin filesystem handlers do not check `session.filesystem`. A scoped
Admin can export or delete another filesystem's encryption key, destroy or
unmount filesystems, alter filesystem devices, and wipe arbitrary devices.

Affected operations include `fs.destroy`, `fs.key.export`, `fs.key.delete`,
`device.wipe`, filesystem device operations, mount/unmount, lock/unlock,
scrub/fsck, and reconciliation controls.

References:

- `engine/nasty-engine/src/router/fs.rs:74-475`
- `engine/nasty-engine/src/router/fs.rs:226`

Filesystem-addressed operations must enforce the configured filesystem scope.
Global raw-device operations must require root-equivalent access.

## High

### `system.settings.get` exposes secret-bearing settings

Status: fixed by redacting plaintext credentials and encrypted blobs from
settings API responses while preserving configured/unconfigured markers.

The `Any` method returns the unsanitized settings structure, including OIDC and
DNS credential fields.

- `engine/nasty-engine/src/router/system.rs:560`
- `engine/nasty-system/src/settings.rs:285-350`

### App read methods expose secrets to ReadOnly users

Status: fixed by making simple-app configuration and inspect reads
Operator-level with existing-app scope enforcement, and compose source reads
unscoped Admin-only.

`apps.config` returns environment values, `apps.inspect` returns raw Docker
inspect data, and `apps.compose.get` returns the stack `.env` file.

- `engine/nasty-engine/src/registry/methods.rs:2909-2913`
- `engine/nasty-engine/src/registry/methods.rs:2955-2961`
- `engine/nasty-engine/src/registry/methods.rs:3116-3122`
- `engine/nasty-apps/src/lib.rs:4047-4057`
- `engine/nasty-apps/src/lib.rs:4135-4181`
- `engine/nasty-apps/src/lib.rs:4744-4759`

### `system.logs` exposes journals to ReadOnly users

Status: fixed by requiring an unscoped Admin session for historical journal
reads, matching the existing live-stream authorization boundary.

The RPC is `Any`, while the equivalent live stream requires root-equivalent
access because journals can leak secrets, addresses, and audit details.

- `engine/nasty-engine/src/router/system.rs:258-301`
- `engine/nasty-engine/src/log_stream.rs:102-123`

### `firmware.update` is Operator-level

Status: fixed by requiring an unscoped Admin session before invoking fwupd.

The method performs a host firmware flash through `fwupdmgr update ... -y` and
should require an unscoped Admin.

- `engine/nasty-engine/src/registry/methods.rs:2138-2146`
- `engine/nasty-system/src/firmware.rs:203-228`

### Generic protocol toggles control system safety services

Status: fixed with payload-aware authorization for both enable and disable.

Operator-level `service.protocol.enable` and `service.protocol.disable` also
control SSH, NUT, watchdog, SMART, Avahi, and the backup REST server. System
service payloads now require an unscoped Admin, while share-protocol payloads
remain available to unscoped Operators and Admins.

- `engine/nasty-engine/src/registry/methods.rs:447-464`
- `engine/nasty-engine/src/router/service.rs:14-38`
- `engine/nasty-system/src/protocol.rs:16-54`

### Scoped Operator tokens can mutate global SMB identities

Status: fixed by requiring an unscoped Operator or Admin session for every SMB
user and group mutation.

The Operator allowlist permits user deletion, password reset, and group
membership changes. These handlers now reject filesystem- and owner-scoped API
tokens before processing the mutation.

- `engine/nasty-engine/src/router/mod.rs:100-110`
- `engine/nasty-engine/src/router/smb.rs:14-37`

### `apps.update` does not authorize the existing app

Status: fixed by authorizing the installed app and its existing volume paths
before validating or applying the replacement configuration.

The handler validates only replacement paths. A scoped Operator can replace an
app it does not own, and the backend removes the existing container before
reinstalling it.

- `engine/nasty-engine/src/router/apps.rs:204-227`
- `engine/nasty-apps/src/lib.rs:3498-3532`

### Scoped Admin can control the global backup REST server

Status: fixed by requiring an unscoped Admin session before reading or rotating
credentials and before changing the global REST server storage path.

`service.rest_server.configure`, `service.rest_server.credentials`, and
`service.rest_server.rotate_credentials` lack unscoped Admin checks.

- `engine/nasty-engine/src/router/service.rs:285-367`

## Medium

### Scoped reads return global resource inventories

Status: partially fixed. Share inventories and direct subvolume/dependency reads
now enforce filesystem and owner scope. Direct profile-derived backup reads,
global alert APIs, aggregate system status, and block-device inventory require
an unscoped session. Filesystem operations and diagnostics filter filesystem
scope and reject owner scope where attribution is unavailable.

Broader host telemetry and adjacent storage reads remain open, including global
system statistics, disk health, TLS host status, update build-directory mounts,
and the shared VM image inventory.

- `engine/nasty-engine/src/router/share.rs:530-539`
- `engine/nasty-engine/src/router/share.rs:607-616`
- `engine/nasty-engine/src/router/share.rs:709-718`
- `engine/nasty-engine/src/router/share.rs:1054-1063`
- `engine/nasty-engine/src/router/backup.rs:27-44`
- `engine/nasty-engine/src/router/backup.rs:81-88`
- `engine/nasty-engine/src/router/fs.rs:64-85`
- `engine/nasty-engine/src/router/bcachefs.rs:14-35`
- `engine/nasty-engine/src/router/subvolume.rs:14-30`
- `engine/nasty-engine/src/router/alerts.rs:14-29`
- `engine/nasty-engine/src/router/system.rs:985-994`

### Notification webhook credentials are incompletely redacted

`notifications.config.get` masks dedicated secret fields but leaves webhook
URLs and arbitrary headers visible to ReadOnly callers.

- `engine/nasty-system/src/notifications.rs:57-71`
- `engine/nasty-system/src/notifications.rs:167-214`

### `audit.list` returns every user's records

The method is `Any`; `audit.mine` already provides a self-scoped alternative.

- `engine/nasty-engine/src/router/audit.rs:20-39`

### `apps.fix_volume_perms` can recursively chown broad host paths

The method accepts absolute paths outside `/fs` except for a small denylist. A
scoped Admin can therefore modify ownership outside its assigned filesystem.

- `engine/nasty-engine/src/router/apps.rs:267-273`
- `engine/nasty-apps/src/lib.rs:772-826`
- `engine/nasty-apps/src/lib.rs:5253-5294`

### `firmware.check` is not a pure read

The `Any` method forces a host-wide LVFS metadata refresh before returning
available updates.

- `engine/nasty-system/src/firmware.rs:133-174`

## Over-Restricted

- `share.iscsi.set_portals` is Admin while equivalent add/remove operations are
  Operator: `engine/nasty-engine/src/registry/methods.rs:1039-1057`.
- `system.network.pending` and `system.network.nm_preview` are pure reads but
  Admin-only: `engine/nasty-engine/src/registry/methods.rs:1935-1953`.

## Additional API Defects

- `apps.compose.get` is documented as returning a string but returns a
  `ComposeContent` object containing `compose_file` and `env_file`.
- `MethodRole::Any` says "any authenticated user", but most such methods are
  unavailable to `Role::User`, which has a separate explicit allowlist.

## Existing Coverage

The router tests verify that registry role declarations and central dispatcher
allowlists agree. They do not verify response redaction, payload-dependent
privilege, or comprehensive filesystem/owner scope enforcement. The external
RBAC test covers basic filesystem and subvolume isolation but not the findings
above.
