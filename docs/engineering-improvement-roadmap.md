# NASty Engineering Improvement Roadmap

Status: Active, reconciled  
Initial review date: 2026-07-17  
Last reconciled: 2026-07-26 against core commit `3ea53193`  
Scope: Core appliance, WebUI, NixOS delivery, CSI, Helm chart, Go client,
kubectl plugin, telemetry, documentation, operational tools, and external tests.

## Reconciliation Ledger

This ledger is authoritative for current status. The detailed sections below
preserve the original review context and proposals; their numeric line
references are historical and may have moved. A finding is **Completed** only
when its full risk and proposal have been addressed, **Partial** when a useful
foundation or only part of the proposal has landed, and **Open** when the
substantive risk remains. Related repositories were checked on their current
GitHub default branches where local clones were stale.

Summary for the 48 numbered findings: **15 Completed**, **19 Partial**, and
**14 Open**. The six cross-cutting architecture initiatives are all **Partial**.

### Authentication And Core Data Safety

| ID | Status | Reconciliation and evidence |
| --- | --- | --- |
| AUTH-1 | **Partial** | Direct HTTP and specialized WebSockets now enforce forced-password restrictions via [#663](https://github.com/nasty-project/nasty/pull/663). Fresh installs still use documented `admin/admin`; one-time credentials and explicit root-password SSH disablement remain open. |
| AUTH-2 | **Completed** | Auth bootstraps only on a missing state file and uses durable atomic persistence after [#682](https://github.com/nasty-project/nasty/pull/682). Corrupt or unreadable existing state fails closed. |
| AUTH-3 | **Partial** | [#663](https://github.com/nasty-project/nasty/pull/663) added role, password-state, and filesystem-scope checks to direct HTTP and specialized endpoints. RPC and non-RPC routes still lack one generated policy covering authorization, audit, mutation, and event behavior. |
| AUTH-4 | **Partial** | Streaming app deployment is Admin-only, but Operator can still reach normal app, Compose, and VM RPC paths that permit unsafe container settings, arbitrary mounts, or raw QEMU arguments. The security validator is not shared across all deployment paths. |
| AUTH-5 | **Partial** | [#691](https://github.com/nasty-project/nasty/pull/691) revalidates the main WebSocket session before each message and on periodic pings. Revocation does not proactively close an idle socket; closure waits for the next message or ping. |
| DATA-1 | **Completed** | [#698](https://github.com/nasty-project/nasty/pull/698) persists `(filesystem UUID, bcachefs subvolume ID)` for each managed LUN and namespace, remaps entries independently, rejects name- and path-based legacy inference, validates persisted/live inventories strictly, and fails closed by quiescing unsafe exports. Regression coverage includes duplicate names, multi-LUN and multi-namespace mappings, partial or corrupt state, and failed NVMe namespace rewrites. |
| DATA-2 | **Completed** | [#699](https://github.com/nasty-project/nasty/pull/699) builds and validates a complete creation plan before disk writes, binds selected devices to kernel and partition identities, rejects aliases, overlaps, mounted descendants, active swap, read-only devices, holders, unsupported topology, and foreign filesystem/LVM/MD signatures, and revalidates immediately before partitioning, wiping, and formatting. `:free` now uses an exact GPT slot and geometry instead of selecting the last `lsblk` child, including 4Kn normalization. Regression coverage preserves the installer split-disk sibling-data workflow and exercises system-disk, swap, holder, signature, request, geometry, and identity-change failures. |
| DATA-3 | **Partial** | [#698](https://github.com/nasty-project/nasty/pull/698) introduces immutable managed block-volume IDs for iSCSI and NVMe-oF state and resolves recognized managed loop paths authoritatively in the engine. Normal share and VM APIs still accept raw host block paths, and there is no separately authorized, topology-checked expert-device operation. |
| DATA-4 | **Partial** | [#678](https://github.com/nasty-project/nasty/pull/678) added app, volume, snapshot, and clone identifier validation. Descriptor-relative cleanup and migration operations remain open. |
| DATA-5 | **Completed** | [#679](https://github.com/nasty-project/nasty/pull/679) distinguishes missing parents from ownership failures and requires explicit Admin authority for orphan snapshot cleanup. |
| DATA-6 | **Partial** | [#695](https://github.com/nasty-project/nasty/pull/695) refuses to replace a populated real `/var/lib/docker` directory or unexpected regular file and fails enablement before persisting state or starting Docker. A verified copy-and-switch migration with retained rollback data remains open. |
| DATA-7 | **Open** | App updates still replace live containers or Compose files before proving the replacement healthy, and some cleanup paths still use `docker compose down -v` without separate named-volume confirmation. |
| DATA-8 | **Completed** | [#675](https://github.com/nasty-project/nasty/pull/675) and [#677](https://github.com/nasty-project/nasty/pull/677) made core resize grow-only, unified quota conversion, and deferred metadata updates until storage changes succeed. |

### System, Release, And CSI Safety

| ID | Status | Reconciliation and evidence |
| --- | --- | --- |
| SYS-1 | **Completed** | [#664](https://github.com/nasty-project/nasty/pull/664) added the boot-time default-drop baseline, early initialization, checked transactional nftables replacement, persistence rollback, Docker DNAT handling, and VM coverage. |
| SYS-2 | **Completed** | [#666](https://github.com/nasty-project/nasty/pull/666) stages matching wrapper and lock files, verifies the candidate build, installs atomically, restores files and generation on failure, and health-checks engine and Caddy. |
| SYS-3 | **Partial** | [#666](https://github.com/nasty-project/nasty/pull/666) removed predictable temporary paths from primary wrapper updates. Admission still uses one fixed systemd unit, generation switching retains a fixed `/tmp` script, and durable operation IDs and cross-process locking are absent. |
| SYS-4 | **Completed** | [#668](https://github.com/nasty-project/nasty/pull/668) wires the wrapper bcachefs input into both userspace and DKMS derivations and adds a CI test proving pin changes affect both outputs. |
| SYS-5 | **Partial** | [#680](https://github.com/nasty-project/nasty/pull/680) restricts Mild discovery to published stable GitHub releases. New installs still default to the Nasty channel, and release automation creates the final tag as a prerelease before qualification. |
| SYS-6 | **Open** | Development ISO assets have commit-based filenames, but the installed wrapper still points to `v<Cargo version>`. Release CI does not prove tag, version, and HEAD agreement and still rewrites `flake.nix`. |
| SYS-7 | **Open** | The installer still partitions before proving UEFI mode, release and network availability, architecture, and immutable lock construction; partial-mount cleanup traps are absent. |
| CSI-1 | **Completed** | [nasty-csi #4](https://github.com/nasty-project/nasty-csi/pull/4) rejects shrink against authoritative capacity, while core independently rejects shrink through [#675](https://github.com/nasty-project/nasty/pull/675). |
| CSI-2 | **Completed** | [nasty-csi #5](https://github.com/nasty-project/nasty-csi/pull/5) removed node-side forced formatting; [#676](https://github.com/nasty-project/nasty/pull/676) initializes and verifies newly created block filesystems in core. |
| CSI-3 | **Completed** | [nasty-csi #5](https://github.com/nasty-project/nasty-csi/pull/5) preserves existing or uncertain NFS and SMB subvolumes when share creation fails. |
| CSI-4 | **Open** | Attachment state remains process-local, same-volume multi-node publication is not durably fenced, and CSI does not configure per-node NVMe host NQNs or managed iSCSI CHAP. |
| CSI-5 | **Open** | CSI names are still truncated without a hash or immutable CSI UUID, and same-name unmanaged NFS or SMB resources can be implicitly adopted. |
| CSI-6 | **Completed** | [nasty-csi #11](https://github.com/nasty-project/nasty-csi/pull/11), [core #681](https://github.com/nasty-project/nasty/pull/681), and [nasty-go #3](https://github.com/nasty-project/nasty-go/pull/3) corrected clone capacity, expansion, iSCSI unstage, pagination, and snapshot timestamps. |
| CSI-7 | **Open** | `nasty-go` still retries mutation requests after ambiguous connection failures without a stable operation ID, and core has no matching deduplication journal. |

### WebUI And Delivery

| ID | Status | Reconciliation and evidence |
| --- | --- | --- |
| UI-1 | **Completed** | [#696](https://github.com/nasty-project/nasty/pull/696) publishes uploads atomically, returns conflict by default for existing destinations, requires an explicit overwrite parameter and WebUI confirmation, and regression-tests collision, overwrite, and concurrent publication behavior. |
| UI-2 | **Completed** | [#696](https://github.com/nasty-project/nasty/pull/696) clears rollback state only after server acknowledgement, preserves confirmation errors and countdown state, serializes overlapping updates, and reloads authoritative pending transactions on reconnect with regression coverage. |
| UI-3 | **Completed** | [#696](https://github.com/nasty-project/nasty/pull/696) resolves clients at operation time, introduces a session generation and reset hooks, and resets domain, sharing, dialog, toast, and related session state after logout or client replacement. |
| UI-4 | **Partial** | Reconnect and backup-job refresh foundations exist through [#163](https://github.com/nasty-project/nasty/pull/163) and [#404](https://github.com/nasty-project/nasty/pull/404), while [#696](https://github.com/nasty-project/nasty/pull/696) adds a session generation and authoritative rollback recovery. There is still no global query invalidation or consistent reconnect-time durable-job rehydration. |
| UI-5 | **Open** | Pages still own independent clients, polling, request sequencing, and direct assignments. No shared typed query-key, cancellation, stale-data, or event-invalidation layer exists. |
| UI-6 | **Open** | Subvolume deletion still uses cached browser dependencies and sequential mutations. There is no fresh engine-issued impact plan, generation token, stale-plan rejection, or server-side commit. |
| UI-7 | **Open** | `withToast()` still converts errors to `undefined`, and callers can close forms or discard secrets after failed mutations. |
| UI-8 | **Partial** | Accessible dialog primitives and a collapsible sidebar exist, and [#696](https://github.com/nasty-project/nasty/pull/696) settles confirmation promises on replacement, dismissal, and session reset. Raw overlays, hover-only actions, and the lack of an off-canvas mobile drawer remain. |
| UI-9 | **Partial** | [#260](https://github.com/nasty-project/nasty/pull/260) improved timer cleanup and some hidden-tab behavior, and noVNC is lazy-loaded. Overlapping hidden polling, eager CodeMirror and xterm routes, and bundle or RPC budgets remain. |
| CI-1 | **Partial** | Appliance smoke foundations exist, [#697](https://github.com/nasty-project/nasty/pull/697) made the app smoke provision managed storage and tolerate asynchronous RPC events, and [#700](https://github.com/nasty-project/nasty/pull/700) added a gating Nix invariant for engine-owned iSCSI lifecycle during generation switches. Engine- and WebUI-only changes can still skip integration, workspace tests omit `--all-targets`, full installer/update/rollback/Secure Boot coverage is absent, and AD DC remains non-gating. |
| CI-2 | **Partial** | [#184](https://github.com/nasty-project/nasty/pull/184) added native ARM builds and release ISOs. Routine integration still lacks a complete aarch64 appliance smoke and cache-only closure verification. |
| CI-3 | **Partial** | Some cache pushes are post-test and main-only, but PR integration can still receive Cachix write credentials, actions use mutable tags, and releases lack published checksums, SBOMs, and attestations. |
| CI-4 | **Open** | There is no persisted-state compatibility policy, populated N-1 upgrade and rollback fixture, pre-activation state snapshot, or real GC root for labeled generations. |

### Ecosystem And Supporting Projects

| ID | Status | Reconciliation and evidence |
| --- | --- | --- |
| ECO-1 | **Open** | Helm still has driver-name, explicit-false, namespace, and metrics-default wiring inconsistencies. There is no values schema or golden rendering suite. |
| ECO-2 | **Open** | CSI and plugin dashboards remain unauthenticated, the plugin binds all interfaces, TLS verification defaults off, and chart exposure adds no authentication middleware. |
| ECO-3 | **Partial** | Core generates Markdown and OpenAPI from one registry, and [nasty-docs #1](https://github.com/nasty-project/nasty-docs/pull/1) added automated publication. Go DTOs and ecosystem constants remain hand-maintained, and runtime response contracts are not tested. |
| ECO-4 | **Open** | No signed cross-repository release manifest exists; repositories retain independent versions and local `nasty-go` replacements. |
| SUP-1 | **Open** | The external harness still cleans by `test-*`, can consume every reported unused device, disables TLS verification, interpolates root shell arguments, and lacks run ownership, dry-run, allowlists, and protocol restoration. |
| SUP-2 | **Partial** | [nasty-top #22](https://github.com/nasty-project/nasty-top/pull/22) added terminal restoration, interval validation, deadline-driven ticks, and elapsed-time rates. Slow storage subprocesses still lack deadlines, asynchronous execution, and last-good caching. |
| SUP-3 | **Partial** | [nasty-telemetry #3](https://github.com/nasty-project/nasty-telemetry/pull/3) added basic validation and rate limits. Authentication, aggregate correctness, retention, precise privacy disclosure, pinned dependencies, tests, and migrations remain. |
| SUP-4 | **Partial** | Generated Markdown and OpenAPI foundations exist through [nasty-docs #1](https://github.com/nasty-project/nasty-docs/pull/1), but Markdown still emits stale transport, cookie, and method examples, and runtime response contracts are not tested. |

### Cross-Cutting Architecture

| ID | Status | Reconciliation and evidence |
| --- | --- | --- |
| ARCH-1 | **Partial** | [#698](https://github.com/nasty-project/nasty/pull/698) gives managed iSCSI LUNs and NVMe-oF namespaces immutable filesystem/subvolume identities, while [#699](https://github.com/nasty-project/nasty/pull/699) binds filesystem-creation targets to kernel device, disk-sequence, and partition identities through execution. VM and expert-device APIs still accept raw host paths, and storage APIs still broadly exchange names. |
| ARCH-2 | **Partial** | Backup jobs and network transactions provide local IDs and phases, but there is no durable cross-subsystem operation journal or mutation deduplication. |
| ARCH-3 | **Partial** | Auth [#682](https://github.com/nasty-project/nasty/pull/682) and firewall persistence are durable, while [#698](https://github.com/nasty-project/nasty/pull/698) adds strict all-or-error directory reads for safety-critical share state. No shared writer exists and generic `StateDir::save` still lacks the complete durability contract. |
| ARCH-4 | **Partial** | Subvolume destinations and backup profiles have process-local keyed admission, and [#699](https://github.com/nasty-project/nasty/pull/699) serializes filesystem create, destroy, add-device, and wipe operations with one process-local block-mutation guard. There is no shared cross-process facility covering the listed subsystems. |
| ARCH-5 | **Partial** | Guest downloads and archives use descriptor-relative `openat2` boundaries after [#688](https://github.com/nasty-project/nasty/pull/688) and [#689](https://github.com/nasty-project/nasty/pull/689). Normal file mutations, sharing validation, backup restore, and migrations still use pathname validation. |
| ARCH-6 | **Partial** | A shared command helper exists, and [#698](https://github.com/nasty-project/nasty/pull/698) bounds `systemctl` execution with kill-on-drop for protocol lifecycle safety. The general helper still does not enforce deadlines, bounded output, process-group termination, redaction, or cancellation-safe cleanup, and direct command construction remains widespread. |

## Purpose

This document records findings from a read-only review of the NASty ecosystem.
It is a planning document, not a claim that every item has a working exploit or
field report. The highest-risk findings should first receive focused regression
tests or VM reproductions, followed by small, reviewable fixes.

The main themes are:

- Privileged operations accept mutable host paths and device names.
- Authorization differs between JSON-RPC, REST, file endpoints, and specialized
  WebSockets.
- Multi-step destructive operations lack atomic prepare/commit/rollback
  boundaries.
- CSI contains independent data-loss paths.
- Update and release guarantees do not always match actual behavior.
- Frontend state is often page-local and can become stale after reconnects or
  overlapping requests.

## Priority And Effort

- P0: Potential release blocker involving data loss, host compromise, or a
  fail-open security boundary.
- P1: High-impact reliability, security, or operational correctness issue.
- P2: Product quality, performance, maintainability, or ecosystem maturity.
- S: Usually one or two focused development days.
- M: Several days or a cross-component change.
- L: Architectural work requiring staged delivery.

## P0: Authentication And Authorization

### AUTH-1: Replace known bootstrap credentials

Fresh systems create and document `admin/admin`. The forced-password-change
restriction is enforced by the central RPC dispatcher, but specialized
endpoints such as the root terminal authenticate independently.

References:

- `engine/nasty-engine/src/auth.rs:223-238`
- `engine/nasty-engine/src/router/mod.rs:408-420`
- `engine/nasty-engine/src/terminal.rs:261-299`
- `nixos/iso.nix:438-442`

Proposal:

- Generate a random, one-time setup credential during installation.
- Display it only on the local console.
- Apply the forced-password-change restriction to every HTTP and WebSocket
  endpoint.
- Disable root password SSH by default.

Effort: M

Verification:

- A fresh session can only change its password or log out.
- Terminal, file mutations, VM console, deployment, and log streaming reject
  the bootstrap session.
- `admin/admin` and root password SSH do not work on a fresh appliance.

### AUTH-2: Fail closed on corrupt authentication state

The generic state loader can return defaults after read or parse failures.
Auth initialization then recreates the known default administrator. Auth state
is also written by truncating the live file.

References:

- `engine/nasty-common/src/state.rs:38-84`
- `engine/nasty-engine/src/auth.rs:210-238`
- `engine/nasty-engine/src/auth.rs:1375-1385`

Proposal:

- Bootstrap only when auth state is definitively absent.
- Refuse startup or authentication when an existing auth file is unreadable or
  corrupt.
- Use the durable state writer described under ARCH-3.

Effort: M

### AUTH-3: Apply authorization to non-RPC endpoints

The HTTP bearer helper discards the returned session. File upload, delete,
mkdir, rename, copy, edit, restore, and VM image upload therefore do not enforce
role or filesystem scope through the central gate.

References:

- `engine/nasty-engine/src/main.rs:1577-1602`
- `engine/nasty-engine/src/main.rs:1616-2771`
- `engine/nasty-engine/src/main.rs:1025-1125`

Proposal:

- Define one endpoint policy containing minimum role, password-change
  allowance, filesystem scope, mutation classification, audit policy, and
  event behavior.
- Apply it to JSON-RPC, REST, file endpoints, terminal, logs, app deployment,
  and specialized WebSockets.
- Generate a role/scope test matrix from the policy.

Effort: L

### AUTH-4: Treat Operator as non-root

Operators can access root-equivalent Compose, unsafe Docker mounts, and raw
QEMU arguments. The streaming app deploy path recognizes this and requires
Admin, but RPC paths do not consistently do so.

References:

- `engine/nasty-engine/src/router/apps.rs:54-82`
- `engine/nasty-engine/src/app_deploy.rs:184-208`
- `engine/nasty-apps/src/lib.rs:392-444`
- `engine/nasty-vm/src/lib.rs:297-339`

Proposal:

- Make privileged containers, host namespaces, Docker socket access, arbitrary
  host mounts, and raw QEMU arguments Admin-only.
- Share one Compose security validator across every deployment path.
- Prefer typed VM options over raw command-line arguments.

Effort: M

### AUTH-5: Revalidate long-lived WebSocket authority

The main WebSocket keeps a cloned session after initial authentication. Logout,
user deletion, role changes, and token revocation do not immediately invalidate
that authority.

References:

- `engine/nasty-engine/src/main.rs:3493-3547`
- `engine/nasty-engine/src/auth.rs:694-703`

Proposal:

- Revalidate a session epoch before dispatching each message.
- Close the socket after logout or revocation.

Effort: M

## P0: Core Data Safety

### DATA-1: Restore block devices by immutable identity

Loop-device restoration is keyed by subvolume name. Names can collide across
filesystems, and iSCSI or NVMe-oF restoration can assign one device to every
LUN or namespace in a target.

References:

- `engine/nasty-storage/src/subvolume.rs:560-627`
- `engine/nasty-sharing/src/iscsi.rs:1094-1147`
- `engine/nasty-sharing/src/nvmeof.rs:325-349`

Proposal:

- Persist `(filesystem UUID, subvolume ID)` for each LUN and namespace.
- Restore each backing device independently.
- Stop deriving identity from a target name, IQN, or NQN.

Effort: M

Verification:

- Reboot with duplicate subvolume names on two pools.
- Reboot a target containing multiple LUNs or namespaces.
- Confirm every target maps to its original backing object.

### DATA-2: Make filesystem creation preflight non-destructive

Filesystem creation checks that paths exist and are not already bcachefs, but
does not comprehensively reject mounted filesystems, root/boot disks, swap,
LVM, MD members, holders, foreign signatures, descendants, or duplicate paths.
Some partition changes happen before later validation.

References:

- `engine/nasty-storage/src/filesystem.rs:1244-1469`

Proposal:

- Build a complete immutable execution plan before any write.
- Inventory mount, swap, holder, partition, root, boot, LVM, MD, and signature
  state.
- Reject the operation unless every selected device is proven safe.
- Revalidate identity immediately before format.

Effort: L

### DATA-3: Replace raw host block paths with managed IDs

VM, iSCSI, and NVMe-oF APIs can accept arbitrary block device paths, including
an OS disk or a storage member.

References:

- `engine/nasty-vm/src/lib.rs:386-405`
- `engine/nasty-sharing/src/iscsi.rs:571-665`
- `engine/nasty-sharing/src/nvmeof.rs:569-613`

Proposal:

- Accept managed block-volume IDs in normal APIs.
- Resolve paths only inside the storage layer.
- If a raw-device expert mode remains, make it a separate Admin-only operation
  with full identity and topology checks.

Effort: M

### DATA-4: Validate every filesystem path component

App names, snapshot names, and clone destinations can become path components
without one consistent validation rule. Cleanup can recursively delete derived
directories.

References:

- `engine/nasty-apps/src/lib.rs:4092-4193`
- `engine/nasty-engine/src/app_deploy.rs:398-562`
- `engine/nasty-storage/src/subvolume.rs:1384-1478`
- `engine/nasty-storage/src/subvolume.rs:1570-1605`

Proposal:

- Introduce checked single-component identifier types.
- Use a conservative app grammar such as
  `[a-z0-9][a-z0-9_-]{0,62}`.
- Reject separators, traversal, absolute paths, controls, and oversized names.
- Use descriptor-relative filesystem operations for cleanup.

Effort: S for validators, L for descriptor-safe operations

### DATA-5: Preserve ownership checks during snapshot deletion

Snapshot parent lookup ignores every error, including owner mismatch, before
continuing deletion.

Reference:

- `engine/nasty-storage/src/subvolume.rs:1446-1478`

Proposal:

- Distinguish NotFound from Forbidden.
- Require explicit Admin authorization for orphan cleanup.

Effort: S

### DATA-6: Do not delete an existing Docker data root

Enabling apps removes `/var/lib/docker` before replacing it with a symlink.

Reference:

- `engine/nasty-apps/src/lib.rs:6138-6166`

Proposal:

- Refuse a nonempty real directory by default.
- Provide an explicit migration that stops Docker, copies data, verifies the
  result, atomically switches paths, and retains rollback data.

Effort: M

### DATA-7: Make application updates rollback-safe

Simple updates remove the old container before proving the replacement works.
Compose updates overwrite configuration before successful startup, and some
cleanup paths use `down -v`, deleting named volumes.

References:

- `engine/nasty-apps/src/lib.rs:5272-5292`
- `engine/nasty-apps/src/lib.rs:5353-5617`

Proposal:

- Stage and validate replacements before switching.
- Preserve the prior manifest and configuration.
- Health-check before commit.
- Make named-volume deletion explicit and separately confirmed.

Effort: L

### DATA-8: Correct block resize units and forbid shrinking

Creation converts quota bytes to KiB, while resize passes bytes directly. The
metadata update can also occur after quota failure. Core block resize permits
`truncate` to a smaller value.

References:

- `engine/nasty-storage/src/subvolume.rs:1222-1284`

Proposal:

- Introduce one typed quota-unit helper.
- Reject every shrink request in core.
- Update metadata only after quota and backing-file changes succeed.

Effort: S

## P0: Firewall, Updates, And Release Safety

### SYS-1: Make firewall replacement transactional and fail closed

The NixOS firewall is disabled. The engine deletes the live nftables table and
then separately loads its replacement. Failure can leave no NASty firewall.
Firewall initialization also happens after several network-facing services are
restored.

References:

- `nixos/modules/nasty.nix:2442-2446`
- `engine/nasty-system/src/firewall.rs:878-920`
- `engine/nasty-engine/src/main.rs:386-442`

Proposal:

- Install a declarative boot-time default-drop baseline.
- Initialize the dynamic firewall before restoring network-facing services.
- Run `nft --check` and submit deletion/replacement in one transaction.
- Commit in-memory and persisted state only after live application succeeds.
- Cover Docker-published ports through an appropriate host/bridge chain.

Effort: M

### SYS-2: Make wrapper updates transactional

Version switching snapshots `flake.lock` only after updating it, so restoration
restores the failed target lock. `flake.nix` is not restored. Other update paths
also mutate live wrapper files before proving the candidate build works.

References:

- `engine/nasty-system/src/update.rs:609-625`
- `engine/nasty-system/src/update.rs:1408-1465`

Proposal:

- Copy both original files before mutation.
- Render, lock, and build in a root-owned staging directory.
- Atomically install the matching wrapper/lock pair only after successful build.
- Restore both files on failure.
- Health-check engine and Caddy after activation.

Effort: L

### SYS-3: Serialize update operations and remove predictable temp files

Update methods use check-then-start admission, one shared systemd unit, and
fixed paths under `/tmp`. Concurrent operations can stop or overwrite one
another, while local symlink races target root-run scripts.

References:

- `engine/nasty-system/src/update.rs:1024-1037`
- `engine/nasty-system/src/update.rs:1229-1505`
- `engine/nasty-system/src/update.rs:1677-1716`

Proposal:

- Use an in-process mutex plus a process-wide lock under `/run/nasty`.
- Create unique root-only operation directories.
- Give operations durable IDs and unique systemd units.

Effort: M

### SYS-4: Wire the wrapper bcachefs pin into the actual package

The installed wrapper declares an independent `bcachefs-tools` input but passes
`nasty.packages.${system}.bcachefs-tools`, which uses the nested input from the
NASty flake.

References:

- `nixos/system-flake/flake.nix.template:30-70`
- `nixos/system-flake/flake.nix.template:109-113`

Proposal:

- Add `nasty.inputs.bcachefs-tools.follows = "bcachefs-tools"`.
- Add a test proving that changing the wrapper ref changes the actual userspace
  tools and kernel module derivations.

Effort: M

### SYS-5: Do not expose prereleases to the stable channel

Release automation creates `v*` prereleases, while stable discovery uses Git
tags and does not check GitHub release state.

References:

- `.github/workflows/build-iso.yml:76-90`
- `engine/nasty-system/src/update.rs:1900-1964`

Proposal:

- Discover only published, non-draft, non-prerelease GitHub releases.
- Create the final stable tag only after candidate validation.
- Default new appliances to the Mild channel.

Effort: S

### SYS-6: Pin development ISOs to their exact source commit

The ISO wrapper points at `v<Cargo version>` even for branch or arbitrary
commit builds. The installed system can therefore differ from the ISO source or
fail when the synthetic tag does not exist.

References:

- `flake.nix:226-235`
- `.github/workflows/build-iso.yml:3-69`

Proposal:

- Verify release tag, Cargo version, and HEAD agree for release builds.
- Pin development ISOs to the exact source revision.
- Never rewrite `flake.nix` during release CI.

Effort: M

### SYS-7: Perform installer preflight before disk destruction

The installer partitions and formats before proving UEFI mode, network access,
release availability, or lock construction.

References:

- `nixos/iso.nix:227-304`
- `nixos/iso.nix:401-465`

Proposal:

- Validate UEFI, network, release source, architecture, disk identity, disk
  size, and static network fields before destructive confirmation.
- Ship or construct a complete immutable wrapper lock before partitioning.
- Add cleanup traps for partial mounts.

Effort: L

## P0: Kubernetes And CSI

### CSI-1: Reject block-volume shrink at both layers

CSI forwards a smaller requested capacity to core, where the backing image is
truncated.

References:

- `nasty-csi/pkg/driver/controller.go:1319-1357`
- `nasty-csi/pkg/driver/controller_nvmeof.go:450-462`
- `nasty-csi/pkg/driver/controller_iscsi.go:451-463`
- Core: `engine/nasty-storage/src/subvolume.rs:1222-1257`

Proposal:

- Compare against authoritative backend size in CSI.
- Reject shrink in core as a second safety boundary.

Effort: S

### CSI-2: Fail closed when filesystem probing is inconclusive

When `lsblk` returns no filesystem, any `blkid` error can be treated as proof
that the device is empty. The node then invokes forced formatting.

Reference:

- `nasty-csi/pkg/driver/node_device.go:355-571`

Proposal:

- Interpret only documented no-signature results as empty.
- Treat timeouts and unknown failures as inconclusive.
- Refuse formatting unless independent probes successfully confirm emptiness.

Effort: S

### CSI-3: Do not delete reused NFS/SMB subvolumes on share failure

The “new subvolume” boolean is inverted for existing resources. Cleanup can
delete a pre-existing subvolume after share creation fails.

References:

- `nasty-csi/pkg/driver/controller_nfs.go:301-416`
- `nasty-csi/pkg/driver/controller_smb.go:168-260`

Proposal:

- Track `subvolumeCreated` separately and set it only after successful create.
- Roll back using immutable resource IDs.

Effort: S

### CSI-4: Add durable attachment fencing

Controller attachment state is process-local. NVMe-oF permits any host and
iSCSI uses generated ACLs without authentication. A stale and replacement node
can mount one ext4/XFS filesystem concurrently.

References:

- `nasty-csi/pkg/driver/controller.go:726-786`
- Core: `engine/nasty-sharing/src/nvmeof.rs:363-387`
- Core: `engine/nasty-sharing/src/iscsi.rs:441-444`

Proposal:

- Persist backend attachment leases.
- Enforce single-writer behavior across controller replicas and restarts.
- Configure per-node NVMe host NQNs and iSCSI ACL/CHAP credentials.

Effort: L

### CSI-5: Make volume identity collision-resistant

Sanitized names are truncated to 63 characters without a hash suffix. Normal
provisioning can also implicitly adopt unmanaged same-name resources.

References:

- `nasty-csi/pkg/driver/template.go:185-216`
- `nasty-csi/pkg/driver/controller_nfs.go:356-374`

Proposal:

- Reserve space for a hash of the complete requested name.
- Persist an immutable CSI volume UUID.
- Reject unmanaged same-name resources unless using explicit adoption.

Effort: M

### CSI-6: Correct clone, expansion, unstage, and pagination contracts

Related issues:

- Snapshot restore reports requested capacity without resizing the clone.
- NVMe filesystem expansion returns `NodeExpansionRequired: false`.
- iSCSI unstage synthesizes an IQN rather than recovering the actual session.
- Negative snapshot pagination offsets can panic.
- Snapshot timestamps are generated at response time.

References:

- `nasty-csi/pkg/driver/controller_snapshot_clone.go:50-114`
- `nasty-csi/pkg/driver/controller_nvmeof.go:456-485`
- `nasty-csi/pkg/driver/node.go:181-210`
- `nasty-csi/pkg/driver/node_iscsi.go:673-730`
- `nasty-csi/pkg/driver/controller_snapshot_list.go:17-29`

Effort: M

### CSI-7: Make retries mutation-aware

`nasty-go` retries every request after connection errors without a stable
operation ID. A server can commit a mutation, lose the response, and receive the
mutation again.

Reference:

- `nasty-go/client.go:361-465`

Proposal:

- Retry reads freely.
- Retry mutations only when explicitly idempotent or after reconciliation.
- Add stable operation IDs and core-side deduplication.

Effort: L

## P1: WebUI Correctness And UX

### UI-1: Make uploads atomic and explicit about overwrite

Upload opens the destination with `File::create`, immediately truncating an
existing file. A failed upload then removes the destination, losing the old
file.

References:

- `engine/nasty-engine/src/main.rs:1745-1788`
- `webui/src/routes/files/+page.svelte:453-487`

Proposal:

- Upload to a unique sibling temporary file.
- Sync it before rename.
- Return 409 on collision by default.
- Require an explicit confirmed overwrite.

Effort: S

### UI-2: Preserve network rollback state on confirmation failure

The client clears the pending rollback in `finally`, including permission,
timeout, and transport failures. Reconnect does not always reload authoritative
rollback state.

References:

- `webui/src/lib/rollbackState.svelte.ts:108-123`
- `webui/src/routes/+layout.svelte:406-470`

Proposal:

- Clear local state only after successful server confirmation.
- Reload pending rollback on every reconnect.
- Display confirmation errors without dismissing the countdown.

Effort: S

### UI-3: Recreate shared clients after logout

Several shared stores cache `getClient()` at module initialization. After
`resetClient()`, those stores continue using the disconnected instance.

References:

- `webui/src/lib/client.ts:3-19`
- `webui/src/lib/domain.svelte.ts`
- `webui/src/lib/sharing/*.svelte.ts`

Proposal:

- Resolve the client at operation time or expose a stable facade with a
  replaceable transport.
- Clear session-specific shared state on logout.

Effort: S

### UI-4: Add authoritative reconnect invalidation

Most pages do not refetch after reconnect and can miss events emitted during an
outage. Backup polling treats transport failure as terminal.

Proposal:

- Add a reconnect generation store.
- Invalidate every engine-backed query after reconnect.
- Rehydrate durable jobs from server state.

Effort: M

### UI-5: Introduce a shared query layer

Many pages issue overlapping requests without cancellation or sequencing. Slow
responses can overwrite newer state, while event and explicit refresh paths can
duplicate work.

Proposal:

- Use typed query keys.
- Permit one in-flight request per key.
- Add abort/sequence protection.
- Keep last-good data with explicit stale/error status.
- Map engine events to query invalidation.
- Pause nonessential polling when the page is hidden.

Effort: L

### UI-6: Move destructive impact planning to the engine

Subvolume cascade deletion uses cached client-side dependencies and removes
shares one at a time before final deletion. Partial failure leaves surprising
state.

Reference:

- `webui/src/routes/subvolumes/+page.svelte:247-279`
- `webui/src/routes/subvolumes/+page.svelte:1668-1736`

Proposal:

- Return a fresh impact plan tied to stable identities and a generation token.
- Commit the plan server-side.
- Reject stale plans and report precise partial results.

Effort: L

### UI-7: Stop swallowing mutation errors

`withToast()` converts errors to `undefined`, but callers often close and reset
forms regardless of the result.

References:

- `webui/src/lib/toast.svelte.ts:51-67`
- `webui/src/routes/backups/+page.svelte:385-390`

Proposal:

- Rethrow after displaying the toast or return a required discriminated result.
- Keep forms and secrets intact after failure.

Effort: M

### UI-8: Standardize dialogs and mobile actions

Some overlays lack dialog semantics, focus trapping, and focus restoration.
Hover-only table actions are unreliable on touch devices, and the permanent
sidebar does not adapt well to narrow screens.

Proposal:

- Use the existing accessible dialog primitives everywhere.
- Settle confirmation promises on Escape, overlay, or close-button dismissal.
- Add an off-canvas mobile navigation drawer.
- Use touch- and keyboard-accessible row action menus.

Effort: L

### UI-9: Reduce polling and route bundle cost

Intervals continue in hidden tabs and can overlap. CodeMirror and xterm are
loaded with their full routes even when unused.

Proposal:

- Use completion-driven polling with visibility checks.
- Aggregate dashboard history requests.
- Lazy-load editor, terminal, and console modules.
- Add route bundle and RPC-count budgets to CI.

Effort: M

## P1: Delivery, CI, And Rollback

### CI-1: Run appliance integration for engine and WebUI changes

Current integration path filters can skip engine and WebUI-only changes. Some
large tests are non-gating, and installer/update/rollback/Secure Boot are not
covered end to end.

Proposal:

- Trigger appliance smoke for engine, WebUI, vendor, lock, and NixOS changes.
- Run `cargo test --workspace --all-targets`.
- Add required installer, update, rollback, firewall, and Secure Boot VM tests.
- Stabilize and gate the AD DC test.

Effort: L

### CI-2: Add aarch64 appliance parity

ARM CI realizes engine, WebUI, and bcachefs tools but not a complete appliance,
ISO, cloud image, or kernel-module closure.

Proposal:

- Build and cache full x86_64 and aarch64 appliance closures.
- Add native aarch64 smoke coverage.
- Verify clean cache-only builds schedule no Rust, npm, or DKMS compilation.

Effort: M

### CI-3: Protect cache and release credentials

PR integration configures Cachix write credentials. Release workflows use
mutable action tags and publish without checksums, SBOMs, or provenance.

Proposal:

- Make PR cache use read-only with no token.
- Push cache entries only from a trusted post-test job.
- Pin GitHub Actions by commit SHA.
- Publish checksums, SBOMs, and artifact attestations.

Effort: M

### CI-4: Test N-1 mutable-state rollback

Generation rollback changes the system closure but leaves `/var/lib/nasty` at
the newer schema. Labeled generations are not protected from garbage
collection.

Proposal:

- Define a backward-compatible persisted-state policy.
- Test populated N-1 upgrade and rollback fixtures.
- Snapshot mutable configuration before activation.
- Make pinned generations actual GC roots.

Effort: L

## P1: Helm, Plugin, And Cross-Repository Contracts

### ECO-1: Correct Helm value wiring

Known inconsistencies include `csiDriverName` versus `driverName`, explicit false
values being overridden, namespace split-brain, and metrics remaining enabled
when omitted.

References:

- `nasty-chart/values.yaml:45-46`
- `nasty-chart/templates/_helpers.tpl:119-286`
- `nasty-chart/templates/controller.yaml:43-62`
- `nasty-chart/templates/node.yaml:41-64`

Proposal:

- Add `values.schema.json`.
- Add golden rendering tests for custom driver names, namespaces, false values,
  disabled metrics, protocols, and upgrade scenarios.

Effort: S

### ECO-2: Secure dashboards and plugin TLS

CSI and plugin dashboards lack authentication. The plugin dashboard binds all
interfaces while printing a localhost URL. Plugin TLS verification defaults to
disabled.

References:

- `nasty-csi/pkg/dashboard/server.go:49-67`
- `nasty-plugin/cmd/kubectl-nasty/cmd_dashboard.go:115-158`
- `nasty-plugin/cmd/kubectl-nasty/main.go:60-66`

Proposal:

- Bind the plugin dashboard to loopback by default.
- Require authenticated proxying or middleware for chart exposure.
- Enable TLS verification by default and support a CA from file or Secret.

Effort: S

### ECO-3: Create one generated API contract

Core registry, runtime responses, Go DTOs, CSI context keys, plugin-generated
PVs, Markdown, and OpenAPI can drift independently.

Proposal:

- Use dedicated response DTOs instead of persistence structs.
- Generate clients and shared constants from the core schema.
- Contract-test every registered handler response.
- Test each supported CSI/plugin version against supported core releases.

Effort: L

### ECO-4: Publish a release manifest

Repositories are versioned independently and local replacements or moving
`nasty-go` checkouts make releases non-reproducible.

Proposal:

- Publish a signed manifest containing the exact core, Go client, CSI, chart,
  plugin, docs, telemetry, and tool revisions.
- Build and test the ecosystem from that manifest.

Effort: M

## P2: Supporting Projects

### SUP-1: Make the external test harness safe by default

The harness cleans resources by `test-*` prefix and can create a pool from all
devices reported unused. It also disables TLS verification, interpolates
arguments into a root shell string, enables broad guest/root shares, and does
not reliably restore protocol state.

References:

- `nasty-tests/test_cleanup.py:10-52`
- `nasty-tests/run_tests.py:119-135`
- `nasty-tests/run-tests.sh:158-161`

Proposal:

- Tag every resource with a run UUID and clean only exact ownership matches.
- Require an explicit device allowlist and confirmation for pool creation.
- Default to dry-run.
- Verify TLS by default and pass arguments as argv.
- Restore prior protocol state in a top-level `finally`.

Effort: M

### SUP-2: Harden nasty-top

`nasty-top` restores terminal state only after normal exit, runs storage
commands without deadlines, accepts invalid refresh intervals, and calculates
some rates using a fixed two-second divisor.

References:

- `nasty-top/src/main.rs:28-75`
- `nasty-top/src/main.rs:159-172`
- `nasty-top/src/sysfs.rs:190-212`
- `nasty-top/src/ui.rs:836-849`

Proposal:

- Add an RAII terminal guard and panic hook.
- Validate finite bounded intervals.
- Drive ticks from monotonic deadlines independent of input.
- Calculate rates from actual elapsed time.
- Run slow commands asynchronously with deadlines and last-good caching.

Effort: M

### SUP-3: Clarify and protect telemetry

Reports are unauthenticated and public counts are easily forged. Historical
aggregation is survivorship-biased, data is not pruned, numeric bounds are weak,
and privacy wording omits persistent IDs and IP processing.

References:

- `nasty-telemetry/worker/src/index.ts:79-269`
- `nasty-telemetry/worker/schema.sql`
- `nasty-telemetry/site/index.html:7`

Proposal:

- Label statistics as unverified until reports are authenticated.
- Enforce realistic finite integer bounds and `used <= total`.
- Materialize daily aggregates and prune raw history.
- Publish a precise privacy and retention policy.
- Self-host pinned frontend dependencies.
- Add tests, type checking, and versioned migrations before deploy.

Effort: M to L

### SUP-4: Correct generated documentation contracts

Documentation has used plaintext `ws://`, the wrong cookie name, obsolete
method examples, and schemas that do not match actual handler responses.

References:

- `nasty-docs/api.md`
- `engine/nasty-engine/src/registry/markdown.rs:22-33`
- `engine/nasty-engine/src/registry/methods.rs`

Proposal:

- Document `wss://`, REST, and `nasty_session` correctly.
- Generate examples from current method names.
- Contract-test schemas against runtime responses.

Effort: M

## Cross-Cutting Architecture

### ARCH-1: Managed resource identities

Normal APIs should exchange immutable resource IDs rather than raw paths,
device nodes, display names, or identifiers reconstructed from protocol names.
Only the owning subsystem should resolve an ID to a path or device.

### ARCH-2: Durable operation journal

Long-running and destructive operations should record:

- Operation ID and idempotency key.
- Stable resource identity.
- Preflight generation/version.
- Current phase.
- Completed external effects.
- Compensation or rollback state.
- Final result and audit actor.

This should cover updates, network changes, filesystems, apps, VMs, backups,
snapshots, restores, protocol targets, and CSI mutations.

### ARCH-3: Durable state writer

Provide one implementation that:

1. Creates a unique same-directory temporary file.
2. Sets final permissions before writing.
3. Writes and calls `sync_all`.
4. Renames atomically.
5. Syncs the parent directory.
6. Propagates failure to the API caller.

Use it for auth, settings, backup profiles, update state, Secure Boot state, and
other singleton JSON files.

### ARCH-4: Keyed operation admission

Provide process-local and cross-process admission keyed by filesystem,
subvolume, VM, app, backup repository, network configuration, system update,
CSI volume, and attachment identity.

### ARCH-5: Capability-safe filesystem operations

Replace canonicalize-then-use validation with descriptor-relative operations
using no-follow and beneath-only resolution. Keep validated parent descriptors
open throughout upload, copy, restore, cleanup, and migration operations.

### ARCH-6: Central subprocess supervisor

All privileged commands should use one supervisor with:

- Mandatory deadline.
- Bounded output.
- Process-group termination.
- `kill_on_drop` behavior.
- Secret redaction.
- Cancellation-safe cleanup.
- Structured exit diagnostics.

## Delivery Plan

### Phase 0: Stop-Ship Fixes

Target: Before the next stable release.

1. **Partial:** App, snapshot, clone, and device identity validators landed in
   [#678](https://github.com/nasty-project/nasty/pull/678); descriptor-safe
   cleanup remains under DATA-4 and ARCH-5.
2. **Completed:** Core and CSI volume shrinking is rejected by
   [core #675](https://github.com/nasty-project/nasty/pull/675) and
   [nasty-csi #4](https://github.com/nasty-project/nasty-csi/pull/4).
3. **Completed:** CSI format ownership and NFS/SMB failure preservation landed
   in [core #676](https://github.com/nasty-project/nasty/pull/676) and
   [nasty-csi #5](https://github.com/nasty-project/nasty-csi/pull/5).
4. **Partial:** [#663](https://github.com/nasty-project/nasty/pull/663) closed
   direct HTTP and specialized WebSocket authorization gaps; known bootstrap
   credentials and policy consolidation remain under AUTH-1 and AUTH-3.
5. **Completed:** Firewall replacement became transactional in
   [#664](https://github.com/nasty-project/nasty/pull/664).
6. **Completed:** Wrapper and lock updates became transactional in
   [#666](https://github.com/nasty-project/nasty/pull/666).
7. **Completed:** The effective bcachefs pin was wired and tested in
   [#668](https://github.com/nasty-project/nasty/pull/668).
8. **Partial:** [#680](https://github.com/nasty-project/nasty/pull/680) hides
   prereleases from Mild clients; release qualification ordering and the
   default channel remain under SYS-5.
9. **Completed:** Managed block exports restore by immutable filesystem and
   subvolume identity, with fail-closed legacy and partial-state handling, after
   [#698](https://github.com/nasty-project/nasty/pull/698).
10. **Completed:** Filesystem creation now finishes immutable request, topology,
    signature, and exact free-partition planning before disk writes and
    revalidates identities at each destructive boundary after
    [#699](https://github.com/nasty-project/nasty/pull/699).

Exit criteria:

- Regression tests exist for every item.
- Critical destructive tests run in an isolated VM or disposable pool.
- No stable release becomes visible before candidate qualification.

### Phase 1: Durable Control Plane

Target: One to two release cycles.

1. **Partial:** Managed block-share identities landed in
   [#698](https://github.com/nasty-project/nasty/pull/698); normal VM and
   expert-device APIs still need managed IDs.
2. **Partial:** Auth and firewall have durable writers, but the shared writer
   remains open under ARCH-3.
3. **Partial:** Local keyed admission exists for selected operations; durable
   idempotency and cross-process admission remain open.
4. **Partial:** Protocol `systemctl` execution is now bounded, but the central
   subprocess supervisor remains open.
5. **Open:** Add durable CSI attachment leases and fencing.
6. **Partial:** Guest download and archive boundaries are descriptor-safe;
   normal file mutations, restore, cleanup, and migration remain open.

Exit criteria:

- Ambiguous reconnects cannot duplicate mutations.
- Startup timeouts terminate child processes before continuing.
- Persisted state after a crash is exactly old or new, never partial.

### Phase 2: Product State And Destructive UX

Target: After durable operation primitives exist.

1. **Partial:** Session reset and rollback reconnect recovery landed in
   [#696](https://github.com/nasty-project/nasty/pull/696); global query
   invalidation remains open.
2. **Partial:** Backup jobs can rehydrate, but durable operations are not
   available consistently across subsystems.
3. **Partial:** Uploads are atomic after
   [#696](https://github.com/nasty-project/nasty/pull/696); destructive
   workflows still lack equivalent prepare/commit handling.
4. **Open:** Use engine-issued impact plans and confirmation tokens.
5. **Partial:** Accessible primitives exist, but raw overlays, touch actions,
   and mobile navigation remain.
6. **Partial:** Some polling and lazy loading improved, but route bundle and
   RPC-count budgets remain open.

### Phase 3: Installer, Upgrade, And Release Qualification

1. **Open:** Add non-destructive installer preflight.
2. **Open:** Add exact-commit development ISO behavior.
3. **Open:** Add N-1 upgrade and rollback tests with populated state.
4. **Open:** Add Secure Boot OVMF update/reboot/rollback tests.
5. **Partial:** Native ARM package and ISO foundations exist; complete
   appliance closure and smoke parity remain open.
6. **Open:** Publish checksums, SBOMs, and provenance.
7. **Partial:** Mild excludes prereleases, but qualification-before-tagging and
   protected stable approval remain open.

### Phase 4: Ecosystem Coherence

1. **Partial:** Core documentation schemas are generated; clients, response
   contracts, and shared ecosystem constants remain open.
2. **Open:** Publish a signed cross-repository release manifest.
3. **Open:** Add Helm schema and install/upgrade matrices.
4. **Open:** Make external tests run-owned and device-allowlisted.
5. **Partial:** Basic telemetry validation and rate limits exist; tests,
   migrations, retention, authentication, and privacy controls remain open.
6. **Open:** Add regular release qualification drills.

## Recommended First PR Series

The original first series has largely landed. Preserve these links as the
implementation record and continue the partial items as focused follow-ups:

1. **Completed:** `storage: reject block-volume shrink and fix quota resize units`
   ([#675](https://github.com/nasty-project/nasty/pull/675),
   [#677](https://github.com/nasty-project/nasty/pull/677)).
2. **Completed:** `csi: fail closed on device probes and never delete reused subvolumes`
   ([nasty-csi #5](https://github.com/nasty-project/nasty-csi/pull/5),
   [core #676](https://github.com/nasty-project/nasty/pull/676)).
3. **Partial:** `apps/storage: validate every path-derived identifier`
   ([#678](https://github.com/nasty-project/nasty/pull/678)); continue with
   descriptor-relative cleanup.
4. **Partial:** `auth: enforce roles and password-change state on non-RPC endpoints`
   ([#663](https://github.com/nasty-project/nasty/pull/663)); continue with a
   unified generated endpoint policy.
5. **Completed:** `firewall: validate and replace rules in one transaction`
   ([#664](https://github.com/nasty-project/nasty/pull/664)).
6. **Completed:** `update: stage and restore matching wrapper/lock pairs`
   ([#666](https://github.com/nasty-project/nasty/pull/666)).
7. **Completed:** `flake: make the wrapper bcachefs pin effective`
   ([#668](https://github.com/nasty-project/nasty/pull/668)).
8. **Partial:** `release: hide prereleases from the Mild channel`
   ([#680](https://github.com/nasty-project/nasty/pull/680)); continue with
   qualification-before-tagging and the default-channel decision.
9. **Completed:** `sharing: restore block exports by immutable identity`
   ([#698](https://github.com/nasty-project/nasty/pull/698)).
10. **Completed:** `storage: make filesystem preflight non-destructive`
    ([#699](https://github.com/nasty-project/nasty/pull/699)).

## Required Regression Test Matrix

- Every endpoint with Admin, Operator, ReadOnly, expired, revoked, scoped, and
  forced-password-change sessions.
- **Covered by [#699](https://github.com/nasty-project/nasty/pull/699):**
  filesystem creation request validation, root/boot system-disk rejection,
  unused sibling-data acceptance, swap, LVM/MD holder, read-only and foreign
  signature rejection, exact partition geometry, 4Kn normalization, and
  preflight-to-format identity changes.
- **Covered by [#698](https://github.com/nasty-project/nasty/pull/698):**
  duplicate block names across pools, multi-LUN and multi-namespace restoration,
  corrupt or partial state, and failed NVMe namespace rewrites.
- **Covered by [#700](https://github.com/nasty-project/nasty/pull/700):**
  the evaluated appliance configuration keeps changed `target.service` units
  under engine lifecycle ownership instead of racing generation activation.
- Failure injection after every persistence and external-effect boundary.
- Firewall parse, spawn, apply, and persistence failures preserving live rules.
- CSI shrink, format probe, clone, expansion, reconnect, stale session, and
  attachment fencing scenarios.
- Installer tests for offline operation, legacy BIOS, invalid network fields,
  missing release, small disks, and interrupted installation.
- N-1 upgrade, reboot, rollback, and garbage-collection behavior.
- Browser tests for reconnect, network rollback, failed mutation forms,
  destructive dialogs, touch actions, and mobile layouts.
- Cache-only x86_64 and aarch64 appliance builds.

## Existing Foundations To Keep

- Exact revisions and hashes in `flake.lock`.
- Fixed-output hashes for npm and fetched packages.
- Wrapper content-hash and lock-idempotence checks.
- Generation-owned wrapper snapshots.
- Generated API registry and bidirectional role consistency tests.
- Rust formatting, lint, and unit-test workflows.
- NixOS appliance and bcachefs smoke-test foundations.

Update the reconciliation ledger whenever a finding is reproduced, fixed,
rejected, or superseded. A status may move to **Completed** only when the full
proposal is supported by code and regression evidence; otherwise retain it as
**Partial** and record the remaining gap.
