//! gVisor (runsc) sandbox backend.
//!
//! Runs commands inside a Docker container launched with the `runsc` OCI
//! runtime. gVisor implements a user-space kernel (the "Sentry") that
//! intercepts and filters the guest's syscalls, providing defense in depth on
//! top of normal container isolation: the untrusted code never touches the
//! host kernel directly.
//!
//! Requires the `runsc` binary installed and registered as a Docker runtime.
//! See <https://gvisor.dev/docs/user_guide/install/>.
//!
//! **Status:** Implemented -- requires `runsc` + Docker.

use super::SandboxBackend;
use crate::error::{Error, Result};
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Default memory limit for a gVisor sandbox (512 MB).
const DEFAULT_MEMORY_LIMIT: u64 = 512 * 1024 * 1024;

/// Default CPU quota for a gVisor sandbox (1.0 CPUs).
const DEFAULT_CPU_QUOTA: f64 = 1.0;

/// Default execution timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Polling interval used while waiting for a sandboxed process.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// gVisor (runsc) sandbox backend.
///
/// The guest command runs in an Alpine container under the `runsc` runtime
/// with a hardened security profile: all capabilities dropped, the root
/// filesystem read-only, network disabled by default, and resource limits
/// applied. The caller's working directory is bind-mounted as the only
/// writable path.
pub struct GvisorBackend {
    /// Sandbox configuration (tier, network, mounts).
    config: SandboxConfig,
    /// Docker runtime name (default: `runsc`).
    runtime: String,
    /// Memory limit in bytes (`None` = unlimited).
    memory_limit: Option<u64>,
    /// CPU quota in CPUs (`None` = unlimited).
    cpu_quota: Option<f64>,
    /// Maximum execution time before the container is killed.
    timeout: Duration,
}

impl GvisorBackend {
    /// Create a new gVisor backend with default resource limits.
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            runtime: "runsc".to_string(),
            memory_limit: Some(DEFAULT_MEMORY_LIMIT),
            cpu_quota: Some(DEFAULT_CPU_QUOTA),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Set the Docker runtime name (e.g. a custom `runsc` install).
    #[must_use]
    pub fn with_runtime(mut self, runtime: impl Into<String>) -> Self {
        self.runtime = runtime.into();
        self
    }

    /// Set the memory limit in bytes.
    #[must_use]
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_limit = Some(bytes);
        self
    }

    /// Set the CPU quota (number of CPUs).
    #[must_use]
    pub fn with_cpu_quota(mut self, cpus: f64) -> Self {
        self.cpu_quota = Some(cpus);
        self
    }

    /// Set the maximum execution timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if the `runsc` binary itself is installed.
    #[must_use]
    pub fn runsc_available() -> bool {
        Command::new("runsc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if Docker has the `runsc` runtime registered.
    #[must_use]
    pub fn check_docker_runtime() -> bool {
        Command::new("docker")
            .args(["info", "--format", "{{json .Runtimes}}"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("runsc"))
            .unwrap_or(false)
    }

    /// Check if gVisor is usable: the `runsc` binary is present **and**
    /// registered as a Docker runtime.
    #[must_use]
    pub fn is_available() -> bool {
        Self::runsc_available() && Self::check_docker_runtime()
    }

    /// Build the `docker run` argument list for the guest command.
    ///
    /// The caller's working directory is bind-mounted to `/workspace` and used
    /// as the container working directory, matching the other container-based
    /// backends.
    fn build_run_command(&self, command: &str, args: &[&str], cwd: &Path) -> Vec<String> {
        let cwd_str = cwd.to_string_lossy();
        let tmpfs_size = self.memory_limit.unwrap_or(DEFAULT_MEMORY_LIMIT);

        let mut cmd: Vec<String> = Vec::new();

        // Base run
        cmd.push("run".into());
        cmd.push("--rm".into());
        cmd.push("--runtime".into());
        cmd.push(self.runtime.clone());
        cmd.push("--name".into());
        cmd.push(format!("clawdius-gvisor-{}", std::process::id()));

        // Network isolation
        if !self.config.network {
            cmd.push("--network".into());
            cmd.push("none".into());
        }

        // Resource limits
        if let Some(mem) = self.memory_limit {
            cmd.push("--memory".into());
            cmd.push(format!("{mem}b"));
        }
        if let Some(cpus) = self.cpu_quota {
            cmd.push("--cpus".into());
            cmd.push(format!("{cpus}"));
        }

        // Security hardening
        cmd.push("--security-opt".into());
        cmd.push("no-new-privileges".into());
        cmd.push("--cap-drop".into());
        cmd.push("ALL".into());
        cmd.push("--read-only".into());

        // Writable scratch space (/tmp) and the bind-mounted workspace.
        cmd.push("--tmpfs".into());
        cmd.push(format!("/tmp:rw,size={tmpfs_size}"));
        cmd.push("-v".into());
        cmd.push(format!("{cwd_str}:/workspace"));
        cmd.push("-w".into());
        cmd.push("/workspace".into());

        // User-configured mounts
        for mount in &self.config.mounts {
            let mode = if mount.read_only { ":ro" } else { "" };
            cmd.push("-v".into());
            cmd.push(format!("{}:{}{}", mount.source, mount.destination, mode));
        }

        // Image + command
        cmd.push("alpine:latest".into());
        cmd.push(command.into());
        for arg in args {
            cmd.push((*arg).into());
        }

        cmd
    }
}

impl SandboxBackend for GvisorBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        if !Self::is_available() {
            return Err(Error::Sandbox(format!(
                "gVisor runtime '{}' not found or not registered with Docker. \
                 Install runsc from: https://gvisor.dev/docs/user_guide/install/",
                self.runtime
            )));
        }

        let docker_args = self.build_run_command(command, args, cwd);

        let mut docker_cmd = Command::new("docker");
        docker_cmd.args(&docker_args).current_dir(cwd);

        run_timed(docker_cmd, self.timeout)
    }

    fn name(&self) -> &'static str {
        "gvisor"
    }
}

/// Spawn a command, capture `stdout`/`stderr` without pipe-buffer deadlock, and
/// enforce a timeout.
///
/// Output streams are drained on background threads so that a guest producing
/// more than the pipe buffer (~64 KiB) does not stall forever. If the deadline
/// elapses the child is killed and [`Error::Timeout`] is returned.
fn run_timed(mut cmd: Command, timeout: Duration) -> Result<Output> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::Sandbox(format!("Failed to spawn process: {e}")))?;

    // Drain stdout/stderr off-thread to avoid blocking the writer.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_handle = std::thread::spawn(move || drain(stdout));
    let err_handle = std::thread::spawn(move || drain(stderr));

    // Wait with a deadline.
    let deadline = Instant::now() + timeout;
    let status_result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Best-effort cleanup of the runaway container.
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(Error::Timeout(timeout));
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(e) => break Err(Error::Sandbox(format!("Failed to wait on process: {e}"))),
        }
    };

    let stdout_buf = out_handle.join().unwrap_or_default();
    let stderr_buf = err_handle.join().unwrap_or_default();
    let status = status_result?;

    Ok(Output {
        status,
        stdout: stdout_buf,
        stderr: stderr_buf,
    })
}

/// Read an optional child stream fully into a buffer.
fn drain<R: std::io::Read>(stream: Option<R>) -> Vec<u8> {
    match stream {
        Some(mut s) => {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut s, &mut buf);
            buf
        }
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxTier;

    #[test]
    fn test_gvisor_available() {
        let _ = GvisorBackend::is_available();
    }

    #[test]
    fn test_build_run_command_no_network() {
        let config = SandboxConfig {
            tier: SandboxTier::Hardened,
            network: false,
            mounts: vec![],
        };
        let backend = GvisorBackend::new(config);
        let cwd = Path::new("/tmp");
        let cmd = backend.build_run_command("echo", &["hi"], cwd);

        assert!(cmd.contains(&"--runtime".into()));
        assert!(cmd.contains(&"runsc".into()));
        assert!(cmd.contains(&"--network".into()));
        assert!(cmd.contains(&"none".into()));
        assert!(cmd.contains(&"--read-only".into()));
        assert!(cmd.iter().any(|a| a == "echo"));
    }

    #[test]
    fn test_build_run_command_with_network() {
        let config = SandboxConfig {
            tier: SandboxTier::Hardened,
            network: true,
            mounts: vec![],
        };
        let backend = GvisorBackend::new(config);
        let cmd = backend.build_run_command("ls", &[], Path::new("/tmp"));
        assert!(!cmd.contains(&"none".into()));
    }
}
