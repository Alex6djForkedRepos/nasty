# API Role Review

Date: 2026-09-07

## Scope

Static review of all 329 methods registered in
`engine/nasty-engine/src/registry/methods.rs`:

- 125 `Any`
- 80 `Operator`
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

Status: fixed by requiring an unscoped Admin for notification configuration
reads and writes and for both supplied and saved-channel test delivery. The
WebUI hides the notification controls when the session lacks that access.

`notifications.config.get` masks dedicated secret fields, but webhook URLs and
arbitrary headers can themselves contain credentials. The same root-equivalent
boundary now covers global configuration mutation and outbound test delivery.

- `engine/nasty-system/src/notifications.rs:57-71`
- `engine/nasty-system/src/notifications.rs:167-214`
- `engine/nasty-engine/src/router/notifications.rs:14-34`
- `engine/nasty-engine/src/registry/methods.rs:2066-2092`
- `webui/src/routes/settings/+page.svelte:290-297`
- `webui/src/routes/settings/+page.svelte:821-832`

### `audit.list` returns every user's records

Status: fixed by requiring an unscoped Admin before reading the global audit
log. `audit.mine` remains available as the self-scoped alternative.

- `engine/nasty-engine/src/router/audit.rs:13-49`
- `engine/nasty-engine/src/registry/methods.rs:1324-1341`

### `apps.fix_volume_perms` can recursively chown broad host paths

Status: fixed by requiring an unscoped Admin before parsing the request. The
service also requires an existing canonical non-symlink target and invokes
`chown` with no-dereference and preserve-root safeguards.

- `engine/nasty-engine/src/router/apps.rs:22-26`
- `engine/nasty-engine/src/router/apps.rs:122-129`
- `engine/nasty-apps/src/lib.rs:839-868`
- `engine/nasty-apps/src/lib.rs:5294-5324`

### `firmware.check` is not a pure read

Status: fixed by requiring an unscoped Admin for the host-wide LVFS metadata
refresh. Other management roles retain access to the pure `firmware.devices`
inventory.

- `engine/nasty-system/src/firmware.rs:133-174`
- `engine/nasty-engine/src/router/system.rs:803-820`

## Over-Restricted

- Status: fixed. `share.iscsi.set_portals` is now Operator-level like the
  equivalent add/remove operations; existing target-scope, raw-backing, and
  iSER authorization checks still apply.
- Status: fixed. `system.network.pending` and `system.network.nm_preview` are
  management-role reads, including for scoped credentials; standard `User`
  accounts remain deny-by-default.

## Additional API Defects

- Status: fixed. `apps.compose.get` is registered and documented as returning
  the `ComposeContent` object containing `compose_file` and `env_file`, with a
  registry regression test covering that schema contract.
- Status: fixed. `MethodRole::Any` and generated API documentation now define
  `any` as any authenticated management role (`Admin`, `Operator`, or
  `ReadOnly`). Standard `User` API access remains a separate explicit allowlist.

## Existing Coverage

The router tests verify that registry role declarations and central dispatcher
allowlists agree. They do not verify response redaction, payload-dependent
privilege, or comprehensive filesystem/owner scope enforcement. The external
RBAC test covers basic filesystem and subvolume isolation but not the findings
above.
