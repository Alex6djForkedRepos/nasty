# Changelog

## v0.0.7

> **This is the NetworkManager-migration release.** v0.0.7 runs both the legacy networking layer and NetworkManager in parallel so existing installs migrate transparently. **v0.0.8 will drop the compatibility shim** — once you're on 0.0.7 and your network reconciles cleanly, you'll be ready for 0.0.8. Boxes still on 0.0.6 or earlier should not jump straight to 0.0.8.

### Headline changes

- **Networking moved to NetworkManager**, with a confirm-or-rollback safety net. Network edits stage, apply, and revert automatically if you don't confirm in time — no more SSH-locking yourself out from a typo. The WebUI surfaces risk-classified change previews, an active-edit banner with countdown, and per-connection DNS. (PRs #75–#94, #103–#110, #120, #122, #123, #127, #128)
- **Encrypted filesystem lifecycle is now end-to-end.** Lock / unlock / mount-with-keyring-key all work, the dashboard shows a "locked" alert with one-click recovery, and the WebUI warns about every app, VM, share, and backup that would break before you lock — including a per-row "🔒 on tank" badge linking to the unlock dialog. (#112, #115, #121, #124, #125, #126)
- **Hardware passthrough has a real UI.** IOMMU groups, system / BIOS / DIMM summary, USB devices, and a passthrough toggle that survives reboots. VMs can be created or edited with USB passthrough, network bridge selection, and an inline disk-import wizard. (#117–#119, #128, #133, #150–#153, #155, #165)
- **Subvolumes overview is the new default landing view.** One table grouped by filesystem, with real disk-usage progress bars (proper ceiling per subvolume type), block-image actual-allocation reporting, and a self-healing reconcile on engine startup. (#169, #174, #176, #177, #179)
- **Update flow is dramatically more reliable.** The dev-build channel now refreshes all flake inputs (kernel finally bumps), wrapper-flake templates rebootstrap on drift, failed rebuilds dump the switch-to-configuration journal so you can see what went wrong, and `nasty-cleanup` is now a one-shot fix for `/boot` full. (#157, #160–#163, #175, #180, #182, #183)

### Apps

- Inline "Enable Apps" prompt when you click Install before the Docker service is running. (#116, #129)
- Volume permission and device checks aggregate into a single warning panel instead of toast spam. (#130, #131, #149)
- Volume / backup source / ingress port pickers replaced raw text inputs with browsable paths. (#132, #134, #136, #137)
- Ingress reverse-proxy panel formatting fixed; `<name>` literal no longer renders as HTML. (#166)
- Apps view rejects bind-mount paths that don't exist on any mounted FS. (#148)
- Live per-app resource usage (CPU %, memory, network I/O, disk I/O) on the Apps page. (#185)

### Sharing

- Per-protocol panels for NFS, SMB, iSCSI, NVMe-oF — one place to see and edit each protocol's exports. (#141–#144)
- Share-creation wizard now uses the same protocol-specific forms (no more "one form fits all"). (#145)
- SMB advertises via mDNS + wsdd for Windows / macOS discovery. (#114)

### Subvolumes

- Unified overview table with filesystem group headers — alignment matches across groups. (#174)
- Size cell shows a coloured progress bar (amber 75% / red 90%) against the correct ceiling: volsize for block, quota for filesystem-with-quota, FS total otherwise. (#176)
- Block-image rows report **actual on-disk allocation** (`st_blocks * 512`) instead of the logical-sparse size, so iSCSI / NVMe-oF images no longer show as 100% full. (#179)
- **Quota inflation bug fixed:** `setquota` was passed bytes where it expected 1 KiB blocks, so every NFS PVC got a quota 1024× the requested size (a 5 Gi PVC ended up with 5 TiB). Engine now divides correctly; startup reconcile auto-rewrites existing inflated quotas. (#181)
- Project IDs back-filled at startup for subvolumes created before always-assign landed. (#177)
- Wizard's advanced bcachefs knobs collapsed behind disclosures. (#167, #168)

### Files / backups

- Files page now supports rename, in-place edit, and sortable columns. (#135)
- Backup wizard has a proper source picker. (#137)

### Updates / system

- Weekly nixpkgs-bump bot landed, with curated package-version diff in the PR body. (#147, #172)
- Dev-build channel correctly refreshes `nixpkgs` + `bcachefs-tools` + `nasty` (kernel-not-bumping bug). (#175, #180)
- Wrapper-flake content hash drives rebootstrap-on-drift; the upstream template flowing onto existing installs no longer needs manual rebootstrap. (#157, #160, #161)
- `/boot` free-space alert with `nasty-cleanup` as the one-shot remedy. (#156, #182, #183, #186)
- bcachefs-tools bumped to 1.38.3. (#154)

### CI / infrastructure

- aarch64 engine, webui, and bcachefs-tools binaries now pushed to `nasty.cachix.org` — Pi / Odroid / Rockchip boxes get cache hits instead of compiling Rust + npm locally every upgrade. (#184)
- Cachix push folded into the integration workflow (one build, not two). (#139)

### Bug fixes

- Setquota 1024× quota inflation on filesystem subvolumes. (#181)
- Block subvolume size cell stuck at 100% because `metadata.len()` returned logical-sparse size. (#179)
- Dev-build upgrade button only refreshed the `nasty` input, never `nixpkgs` or `bcachefs-tools` — explained the "kernel won't update" reports. (#180)
- `<name>` literal rendered as HTML element in Apps page. (#166)
- VM-import auto-naming included image-format suffixes (`.qcow2`, `.img`). (#164)
- WebSocket reconnect didn't refresh sysInfo, so the layout footer showed stale data. (#163)
- `/run/booted-system/kernel` vs `/run/current-system/kernel` reboot-required check (multiple update-path fixes). (#162)
- Orphan network interfaces left behind after bond/bridge deletion now cleaned up. (#120)
- Filesystem mount uses the keyring key directly instead of re-prompting. (#121)

## v0.0.6 — 2026-05-08

### Highlights

- **OIDC / SSO login support.** SSO configuration moved into Access Control. (PRs from `auth-oidc-sso` and `webui-move-sso-config`)
- **Auth hardening.** Browser session is now an httpOnly cookie, the legacy `?token=` URL fallback is gone, per-IP rate limit + persisted lockouts with an Admin-only escape hatch, and constant-time comparisons / SMB-guest / OIDC-SSRF cleanups bundled in.
- **Security hardening across the surface.** Compose deploys sandboxed, engine systemd unit hardened, NFS exports tightened, WS endpoints gated with origin validation, `{@html}` XSS sinks removed, HTTP security headers added, audit-log rotation fixed.
- **Apps `allow_unsafe` escape hatch** surfaced in the deploy/edit form and the app list (badge), for cases where the strict sandbox is too tight.
- **Test infrastructure built out** — bcachefs smoke, appliance integration smoke, JSON-RPC framing tests, alerts evaluation, sharing config, storage parser, JSON-RPC appliance smoke, pinned Rust toolchain, CI test gate. (#22–#36)
- **Big dependency bumps**: rusqlite 0.34 → 0.39, sha2, rand, x509-parser, bollard, reqwest, openidconnect 4, vitest 4. (#44, #45, #47, #48, #49)

### Other changes

- Alerts evaluated by a background notifier instead of waiting on a browser-attached client.
- Network bridge support. (#39)
- MTU configurable on connections; input crash on Apply fixed. (#63, #64)
- Encrypted filesystem no longer shows as "locked" after a successful unlock. (#59)
- ISO releases marked as pre-release by default on GitHub. (#60)
- bcachefs-tools bumped to v1.38.2.

## v0.0.5 — 2026-05-02

### Highlights

- **Backup system polished** — friendlier create wizard, human-readable schedule + next-run on cards, Edit button on profiles, config-backup warning banner with one-click "Create backup" shortcuts, dismiss control. Daily ACME cert renewal check, configurable DNS-propagation timeout, TLS cert details parsed in Rust (no `openssl` shell-out).
- **Services page unified.** SSH config, UPS config, Docker enable/disable, Backup-server storage path, and per-service Configure panels all live in one place now.
- **Access Control rebuilt.** System users and groups shown together, click a user to manage group memberships, inline user creation in the share wizard, last-admin can no longer be deleted, share wizard uses a real user/group picker.
- **Installer fixes** — explicit `mount -t` for partitions, partprobe + udevadm settle + sync after format, ext4 reserved blocks at 1%, installer text matches actual partition size, TTY banner skips link-local addresses.
- **Sidebar search bar** for quick navigation.
- Filesystem label now equals the user-chosen name on `bcachefs format`.

### Cleanups

- Removed all backward-compatibility hacks accumulated through 0.0.x.
- Removed GitHub token auth path now that repos are public.
- Dashboard SMART section retired (already visible in Disks).

## v0.0.4 — 2026-04-21

### Highlights

- **Apps runtime replaced**: k3s + Helm → Docker + bollard. Much smaller closure, faster install, no k8s overhead for a single-node appliance.
- **Live deploy streaming** for app installs, compose deploys, and `docker pull`. Per-container Shell and Logs for compose apps.
- **Apps lifecycle**: stop/start, restart, pull, prune, container details, ports, compose ingress, port-conflict detection with auto-suggest, default host port = container port, auto-detect EXPOSE.
- **Compose YAML editor** (CodeMirror) with error-line marking.
- **File preview + download** in the Files browser.
- **bcachefs-tools 1.38.0** + nixpkgs bump.
- Per-subvolume bcachefs options exposed in the WebUI.
- BIOS warning during install when booted in legacy mode (must reinstall in UEFI).

### Fixes

- Filesystem destroy now wipes superblocks reliably; stale signatures no longer block re-use of devices.
- Mount/unmount and other long operations now give live feedback.
- `nasty-top` integrated into the appliance PATH.

## v0.0.3 — 2026-04-13

### Highlights

- **Tailscale VPN integration** — enabled by default on all NASty appliances, simple Connect / Disconnect UI.
- **NUT (Network UPS Tools)** support for local UPS monitoring, configured from Settings.
- **Apps** got auto-assigned NodePorts and nginx ingress, `/apps/{name}/` proxy links replacing port-forward, auto-detected EXPOSE ports, in-place editing via `helm upgrade`.
- **NAS tuning settings** (NFS threads, SMB, iSCSI, VM writeback) exposed in the UI.
- **Filesystem options**: `journal_flush_delay`, `io_scheduler`, `fs.reconcile.enable/disable`, checksum options in the edit panel, erasure-coding indicator (gated on disk count).
- **Audit log** records all mutations; new `audit.list` API.
- **Kernel error monitoring** with alert rules.
- WebUI Licenses page; GPL-3.0 LICENSE file + third-party inventory added.

## v0.0.2 — 2026-04-06

### Highlights

- **Flake-based system architecture.** Slim installed wrapper at `/etc/nixos`, upstream pulled in via flake inputs — system upgrades stop being a `git pull` and become a `nix flake update`.
- **Offline-capable ISO installer.** Bootstraps without network access.
- **`nasty.cachix.org` binary cache** added — fast appliance updates instead of building Rust + npm locally.
- **Disk Topology tab** with controller / port mapping, plus ATA port mapping in disk health.
- **Periodic auth check** detects expired sessions and bounces the user to login instead of leaving stale UI.
- **Performance**: merged xattr reads into a single `list+get` pass per subvolume, batched `du` / `losetup` queries.
- `croc` added to the appliance for debug-report transfers.
- INSTALL.md added with an alternative install-from-Linux-live-environment recipe.

## v0.0.1 — 2026-04-01

Initial public release. NixOS-based NAS appliance built on bcachefs.

### Foundations

- bcachefs storage with project-quota-aware subvolumes (nested allowed, `.nasty/*` for internals).
- WebUI with Apps, VMs, Subvolumes, Sharing, Backups, Files, Network, Update, and Help pages.
- Three release flavors: **Mild** (`v*` tags, stable), **Spicy** (`s*` tags, snapshots), **Nasty** (`main` branch, dev builds) — all picked from a single flake.
- Engine `--version` flag, in-WebUI engine version detection with auto-reload on change.
- ISO build workflow (GRUB EFI + systemd-boot variant for picky UEFI firmware).
- Periodic config backup from `/var/lib/nasty` to bcachefs.
- Backup system using rustic (deduplicating, encrypted).
- Quota / size support for filesystem subvolumes.
- Help menu with community links.
