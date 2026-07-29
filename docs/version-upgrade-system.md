# Version Upgrade System

## Overview

NASty now treats the installed `/etc/nixos` flake as the source of truth for system versioning.

The Version page is designed around three managed flake inputs:

- `nixpkgs`
- `bcachefs-tools`
- `nasty`

The system reads:

- input URLs from `/etc/nixos/flake.nix`
- locked revisions from `/etc/nixos/flake.lock`

and all switching/upgrading behavior is built on top of that.

## Core model

The update model is:

1. `flake.nix` defines which upstreams are tracked.
2. `flake.lock` defines the exact pinned revisions.
3. A Version-page action rewrites input URLs in `/etc/nixos/flake.nix` when needed.
4. Selected inputs are refreshed in `flake.lock` with `nix flake update <input>`.
5. A rebuild is only triggered if `flake.lock` actually changed.

This means the UI is not pretending there is a hidden abstract channel system anymore. It is directly editing the live system flake.

## Version page behavior

### Top tagged-release panel

The top panel always shows.

On page load it starts in:

- `Fetching newest version...`

Then it checks the latest official tagged upstream release from:

- `github:nasty-project/nasty`
- tags matching `vX.Y.Z`

Possible states:

- `Already at newest tagged release`
  - shown only when the current `nasty.url` is exactly the newest official shorthand URL
  - example: `github:nasty-project/nasty/v0.0.2`
- `The newest tagged release is vX.Y.Z, click to switch`
  - shown when the current `nasty.url` is still an official tagged upstream URL, but not the newest one
- `You are using a custom build of nasty. Click to switch back to upstream release.`
  - shown when the current `nasty.url` is not in the standard upstream tagged form
  - this includes:
  - `main`
  - commit refs
  - forks
  - other custom GitHub URLs
- `Network failure, unable to fetch newest tagged release`
  - shown on timeout or fetch failure

While a version switch is already in progress and the page reloads, the panel shows:

- `Switching to another Version...`

instead of re-running release detection immediately.

### Installed version display

The panel also shows:

- `Installed NASty`

This value comes from the installed system version state, not from the latest upstream tag check.

## Two upgrade paths

## 1. Tagged-release Upgrade button

This is the opinionated, standard-release-path upgrade.

When clicked:

1. The page greys out the Upstream panel.
2. The backend checks the newest official tagged release.
3. It fetches that release's upstream wrapper template:
   - `nixos/system-flake/flake.nix.template`
4. It decides whether the local wrapper flake structure should be re-bootstrapped.
5. If re-bootstrap is needed, it rewrites `/etc/nixos/flake.nix` from the upstream template.
6. Then it updates:
   - `nixpkgs`
   - `bcachefs-tools`
   - `nasty`
7. Then it runs `nixos-rebuild switch`.

Important design point:

- This path is intentionally for returning to the official upstream release track.
- If the user is on a custom build, this path is allowed to normalize them back to the upstream release template and input shape.

## 2. Manual switch through the Upstream panel

This is the explicit advanced path.

The user can edit the live URLs for:

- `nixpkgs`
- `bcachefs-tools`
- `nasty`

and choose which inputs to update.

Behavior:

- if a URL changed, that input is always forced to update in `flake.lock`
- if no URL changed and no input was selected for update, the action is rejected
- if `flake.lock` does not change after the requested updates, rebuild is skipped

This path is meant for direct flake input editing, not for a hidden channel abstraction.

## Wrapper flake bootstrap and migration

## Bootstrapped wrapper flake

Installed systems use a local wrapper flake under:

- `/etc/nixos/flake.nix`
- `/etc/nixos/flake.lock`

That wrapper is generated from:

- `nixos/system-flake/flake.nix.template`

The bootstrap logic lives in Rust now, not in shell templating.

The template uses placeholders for:

- the local system architecture
- the tagged NASty release it should point at

## Wrapper schema version

The wrapper template now includes:

- `wrapperFlakeVersion = "v0.1";`

This is not the NASty product version.
It is the wrapper-flake schema version.

It is used to decide whether a newer upstream template should replace the local wrapper structure.

### Re-bootstrap rules

When switching to an official tagged upstream release:

- if the local wrapper has no `wrapperFlakeVersion`, it is treated as old and re-bootstrapped
- if the upstream wrapper version is newer than the local one, it is re-bootstrapped
- if the upstream wrapper version is missing or cannot be parsed, re-bootstrap is silently skipped
- if versions are equal or local is newer, no re-bootstrap is done

So wrapper migration is best-effort and non-fatal.

## Generation-owned flake recovery

The wrapper flake is embedded into each system generation closure.

This allows the active generation to expose the exact wrapper flake that built it.

At boot and activation, a service restores only:

- `/etc/nixos/flake.nix`
- `/etc/nixos/flake.lock`

from the active generation snapshot.

It does not touch other files in `/etc/nixos`, such as:

- `hardware-configuration.nix`
- `networking.nix`

This is specifically meant to recover from interrupted flake mutation, such as power loss after `flake.nix` or `flake.lock` changed but before activation completed.

## Migration from older installs

Older installed systems may have wrapper flakes that do not provide the newer `nastySystemFlakeSnapshot` special arg.

To avoid breaking those systems:

- the NASty NixOS module now tolerates that argument being absent
- old wrappers can still evaluate and upgrade
- they simply do not get generation-flake recovery until their wrapper is regenerated through the newer path

So migration is intended to be non-panicking and forward-compatible.

## Installer behavior

The installer now bootstraps `/mnt/etc/nixos/flake.nix` using the Rust bootstrap path instead of shell substitution.

The installer derives the default NASty tagged release internally from the built engine version.

That means the installer is expected to be built from a tagged release and to produce a wrapper flake pointing at that tagged upstream release.

## Standard release path

The intended supported path is:

1. install from a tagged-release ISO
2. keep `nasty.url` on the standard form
   - `github:nasty-project/nasty/vX.Y.Z`
3. use the tagged-release panel and Upgrade button for normal release-to-release upgrades
4. let the shipped wrapper template define the expected flake structure

Custom URLs are still allowed and visible, but they are not the primary UX target.

## Failure behavior

## Release detection

- latest-tag detection has a timeout
- template fetches also have a timeout
- failure shows a warning in the banner
- failure does not break the rest of the Version page

## Wrapper version parsing

- if upstream wrapper version parsing fails, re-bootstrap is skipped
- this avoids turning template parse problems into hard upgrade failures

## Recovery service

- if the generation snapshot is missing, recovery is skipped
- boot is not meant to be blocked by that absence
- only `flake.nix` and `flake.lock` are restored

## Legacy block-export migration

Releases before v0.0.15 persisted managed iSCSI LUNs and NVMe-oF namespaces by their current `/dev/loopN` path. Loop numbers are not stable across reboots, so newer engines persist the filesystem UUID and bcachefs subvolume ID instead.

During an in-place upgrade, the engine records the exact managed backing identity of loops that were already attached before it allocates any new loops. Legacy exports are migrated only when their saved path matches that boot-scoped evidence. The evidence survives engine-process restarts in `/run`, but is cleared by reboot. A loop path freshly assigned during cold boot is never accepted as migration evidence.

If an identity cannot be proven safely, the protocol remains quiesced and the Sharing page marks the affected LUN or namespace as requiring repair. An unscoped Admin can reconnect only that entry to its original managed block subvolume. The target or subsystem name, portals, ACLs, CHAP credentials, allowed hosts, and other access settings are preserved. Cross-protocol conflicts are rejected, and a failed live activation rolls the identity change back.

## Practical summary

In plain terms, the system now works like this:

- the Version page edits the real live flake
- official tagged upgrades can re-bootstrap the wrapper when the wrapper schema changes
- older systems can migrate forward without crashing on missing new module args
- each generation can restore the exact wrapper flake that built it
- the top banner tells the user whether they are on:
  - the newest official release
  - an older official release
  - or a custom build outside the standard release path

If you want, I can next turn this into a shorter maintainer-facing version and a separate user-facing version.
