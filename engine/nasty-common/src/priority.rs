//! Scheduling helpers that keep bulk work below the engine control plane.

use tokio::process::Command;

#[cfg(target_os = "linux")]
const BULK_NICE: i32 = 10;
const MAX_BULK_WORKERS: usize = 4;
static BULK_WORKERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_BULK_WORKERS);
#[cfg(target_os = "linux")]
const IOPRIO_WHO_PROCESS: libc::c_int = 1;
#[cfg(target_os = "linux")]
const IOPRIO_CLASS_SHIFT: libc::c_int = 13;
#[cfg(target_os = "linux")]
const IOPRIO_CLASS_BE: libc::c_int = 2;
#[cfg(target_os = "linux")]
const BULK_IO_PRIORITY: libc::c_int = 7;

/// Properties for bulk work launched as a transient systemd service.
pub const BULK_SYSTEMD_PROPERTIES: [&str; 3] = [
    "--property=Nice=10",
    "--property=IOSchedulingClass=best-effort",
    "--property=IOSchedulingPriority=7",
];

/// Build a subprocess command that runs at low CPU and best-effort-low IO
/// priority. On non-Linux development hosts this is a normal command.
pub fn bulk_command(program: &str) -> Command {
    let command = Command::new(program);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let mut command = command;

        // Runs in the child after fork and before exec. Only async-signal-safe
        // syscalls are used here.
        unsafe {
            command
                .as_std_mut()
                .pre_exec(set_current_thread_bulk_priority);
        }
        command
    }
    #[cfg(not(target_os = "linux"))]
    {
        command
    }
}

/// Run in-process bulk work on a dedicated OS thread. A dedicated thread is
/// intentional: lowering a Tokio blocking-pool thread would affect unrelated
/// control-plane work when that reusable thread handles its next task.
pub async fn spawn_bulk<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = BULK_WORKERS
        .acquire()
        .await
        .map_err(|_| "bulk worker queue closed".to_string())?;
    spawn_bulk_inner(work, Some(permit)).await
}

/// Run bulk work outside the shared queue when the caller already owns a
/// separate concurrency permit for the full lifetime of the work.
pub async fn spawn_bounded_bulk<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_bulk_inner(work, None).await
}

async fn spawn_bulk_inner<T, F>(
    work: F,
    permit: Option<tokio::sync::SemaphorePermit<'static>>,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("nasty-bulk-worker".into())
        .spawn(move || {
            let _permit = permit;
            #[cfg(target_os = "linux")]
            if let Err(error) = set_current_thread_bulk_priority() {
                let _ = tx.send(Err(format!("set bulk worker priority: {error}")));
                return;
            }
            let _ = tx.send(Ok(work()));
        })
        .map_err(|error| format!("spawn bulk worker: {error}"))?;
    rx.await
        .map_err(|_| "bulk worker exited without a result".to_string())?
}

#[cfg(target_os = "linux")]
fn set_current_thread_bulk_priority() -> std::io::Result<()> {
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, BULK_NICE) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let ioprio = (IOPRIO_CLASS_BE << IOPRIO_CLASS_SHIFT) | BULK_IO_PRIORITY;
    if unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, ioprio) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bulk_worker_returns_result() {
        assert_eq!(spawn_bulk(|| 42).await.unwrap(), 42);
    }

    #[tokio::test]
    async fn bulk_command_executes_child() {
        let status = bulk_command("true").status().await.unwrap();
        assert!(status.success());
    }

    #[test]
    fn bulk_worker_limit_is_bounded() {
        assert_eq!(MAX_BULK_WORKERS, 4);
    }

    #[test]
    fn transient_service_properties_match_bulk_process_priorities() {
        assert_eq!(
            BULK_SYSTEMD_PROPERTIES,
            [
                "--property=Nice=10",
                "--property=IOSchedulingClass=best-effort",
                "--property=IOSchedulingPriority=7",
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn bulk_command_has_expected_linux_nice_value() {
        let output = bulk_command("sh")
            .args(["-c", "ps -o ni= -p $$"])
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn bulk_worker_has_expected_linux_priorities() {
        let (nice, ioprio) = spawn_bulk(|| {
            let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) };
            let ioprio = unsafe { libc::syscall(libc::SYS_ioprio_get, IOPRIO_WHO_PROCESS, 0) };
            (nice, ioprio as libc::c_int)
        })
        .await
        .unwrap();
        assert_eq!(nice, BULK_NICE);
        assert_eq!(ioprio >> IOPRIO_CLASS_SHIFT, IOPRIO_CLASS_BE);
        assert_eq!(ioprio & ((1 << IOPRIO_CLASS_SHIFT) - 1), BULK_IO_PRIORITY);
    }
}
