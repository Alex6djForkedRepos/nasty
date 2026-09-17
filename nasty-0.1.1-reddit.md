This release turns the dashboard into a workspace you can shape around the way
you run your NAS, while putting a lot of work into backup reliability, recovery,
and security boundaries underneath it.

## Dashboards that fit the job

* Choose built-in layouts or create named custom views
* Place widgets freely on a responsive grid, resize them, or switch supported cards to a tiny presentation
* Add focused cards for storage, alerts, operations, CPU, memory, services, containers, VMs, Docker workloads, host time, MOTD, and upcoming backups
* Hide built-in views, remove and restore widgets without losing their setup, and keep keyboard controls when drag-and-drop is not practical
* See release availability in the sidebar, with background checks that do not block the main UI connection

## Backups that fail clearly and recover cleanly

* Failed runs now reach history and alerts even when validation, credentials, initialization, or repository access fails
* Abandoned workers no longer leave jobs stuck as active forever
* Profile and run state is committed atomically, and interrupted repository initialization is safe to retry
* Backup schedules use validated five-field POSIX cron with authoritative UTC previews and stable restart behavior

## Apps, storage, and maintenance

* Missing Apps storage is detected before Docker starts, with a guarded recovery path instead of an unsafe socket-activated startup
* Private container registry credentials are encrypted at rest and work with both Simple Apps and Compose without leaking into Compose files or logs
* bcachefs scrubs can be scheduled from the UI and now report corrected, uncorrected, and interrupted outcomes with much better diagnostics
* Slow filesystem recovery restores configured networking and SSH first while keeping storage-dependent services closed until the pool is safe
* An optional Linux watchdog can reboot on configured load, low-memory, or connectivity failures

## Security and platform updates

* A broad management API review tightened role and resource-scope checks across settings, apps, backups, firmware, logs, notifications, sharing, storage, networking, and audit data
* Secret-bearing responses are redacted, and ambiguous scoped inventories fail closed instead of exposing global data
* bcachefs-tools moves to 1.39.6, Linux to 6.18.52, and Tailscale to 1.102.4
* DiskWatch moves to 0.5.8, NetWatch to 0.31.4, SysWatch to 0.14.2, and nasty-top to 0.0.11

Fresh x86_64 and aarch64 ISOs are on the releases page.

**Proxmox users:** NASty requires UEFI. Switch the VM firmware from SeaBIOS to OVMF before installing, otherwise NASty will not boot after the first restart.
