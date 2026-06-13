//! Firecracker microVM sandbox backend.
//!
//! Runs each command inside a dedicated AWS Firecracker microVM for
//! hardware-level (KVM) isolation. Every execution gets its own kernel,
//! memory, and CPU, so a compromise of the guest cannot reach the host
//! kernel.
//!
//! Requires the `firecracker` binary and a root filesystem image. See
//! <https://github.com/firecracker-microvm/firecracker>.
//!
//! **Status:** Implemented -- requires `firecracker` + a rootfs/kernel image.

use super::SandboxBackend;
use crate::error::{Error, Result};
use crate::sandbox::tiers::SandboxConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Default path to the firecracker binary.
const DEFAULT_FIRECRACKER_BIN: &str = "/usr/bin/firecracker";

/// Default path to the jailer binary.
const DEFAULT_JAILER_BIN: &str = "/usr/bin/jailer";

/// Default root filesystem for sandboxed execution.
const DEFAULT_ROOTFS: &str = "/opt/clawdius/rootfs.ext4";

/// Default kernel image for the microVM.
const DEFAULT_KERNEL: &str = "/opt/clawdius/vmlinux";

/// Default VM memory size in MB.
const DEFAULT_MEM_SIZE_MB: u64 = 512;

/// Default vCPU count.
const DEFAULT_VCPUS: u8 = 1;

/// Default execution timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Polling interval used while waiting for the microVM.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Firecracker microVM sandbox backend.
///
/// On [`execute`](FirecrackerBackend::execute) the backend:
/// 1. Generates a Firecracker configuration (boot source, drives, machine
///    config) with the guest command encoded into the kernel boot arguments.
/// 2. Writes the configuration to a temp file.
/// 3. Launches `firecracker` with an API socket and the config file.
/// 4. Captures `stdout`/`stderr` and enforces a timeout.
/// 5. Cleans up the config and socket.
///
/// The root filesystem image is expected to ship a minimal `init` that reads
/// the `clawdius_cmd=` boot argument and runs it (this is the standard pattern
/// for command-driven Firecracker sandboxes).
pub struct FirecrackerBackend {
    /// Sandbox configuration (tier, network, mounts). Unused: the microVM
    /// image governs its own isolation, but the field is kept for interface
    /// symmetry with the other backends.
    _config: SandboxConfig,
    /// Path to the firecracker binary.
    firecracker_bin: PathBuf,
    /// Path to the jailer binary.
    jailer_bin: PathBuf,
    /// VM ID (unique identifier for this microVM).
    vm_id: String,
    /// Root filesystem image path.
    rootfs: PathBuf,
    /// Kernel image path.
    kernel_image: PathBuf,
    /// Memory for the VM (MB).
    mem_size_mb: u64,
    /// Number of vCPUs.
    vcpus: u8,
    /// Timeout for VM execution.
    timeout: Duration,
    /// Socket path for the Firecracker API.
    api_socket: PathBuf,
}

impl FirecrackerBackend {
    /// Create a new Firecracker backend with default binary/kernel/rootfs
    /// paths.
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        let vm_id = format!("clawdius-{}", std::process::id());
        let api_socket = std::env::temp_dir().join(format!("firecracker-{vm_id}.sock"));
        Self {
            _config: config,
            firecracker_bin: PathBuf::from(DEFAULT_FIRECRACKER_BIN),
            jailer_bin: PathBuf::from(DEFAULT_JAILER_BIN),
            vm_id,
            rootfs: PathBuf::from(DEFAULT_ROOTFS),
            kernel_image: PathBuf::from(DEFAULT_KERNEL),
            mem_size_mb: DEFAULT_MEM_SIZE_MB,
            vcpus: DEFAULT_VCPUS,
            timeout: DEFAULT_TIMEOUT,
            api_socket,
        }
    }

    /// Create with custom kernel and rootfs paths.
    #[must_use]
    pub fn with_paths(
        mut self,
        firecracker_bin: impl Into<PathBuf>,
        jailer_bin: impl Into<PathBuf>,
        kernel: impl Into<PathBuf>,
        rootfs: impl Into<PathBuf>,
    ) -> Self {
        self.firecracker_bin = firecracker_bin.into();
        self.jailer_bin = jailer_bin.into();
        self.kernel_image = kernel.into();
        self.rootfs = rootfs.into();
        self
    }

    /// Set VM memory size in MB.
    #[must_use]
    pub fn with_memory(mut self, mb: u64) -> Self {
        self.mem_size_mb = mb;
        self
    }

    /// Set number of vCPUs.
    #[must_use]
    pub fn with_vcpus(mut self, vcpus: u8) -> Self {
        self.vcpus = vcpus;
        self
    }

    /// Set execution timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the VM ID (also updates the API socket path).
    #[must_use]
    pub fn with_vm_id(mut self, id: impl Into<String>) -> Self {
        self.vm_id = id.into();
        let vm_id = self.vm_id.clone();
        self.api_socket = std::env::temp_dir().join(format!("firecracker-{vm_id}.sock"));
        self
    }

    /// Check if the firecracker binary is available and runs.
    #[must_use]
    pub fn is_available() -> bool {
        Command::new(DEFAULT_FIRECRACKER_BIN)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            && Path::new(DEFAULT_KERNEL).exists()
    }

    /// Check if the jailer binary is present.
    #[must_use]
    pub fn check_jailer(&self) -> bool {
        self.jailer_bin.exists()
    }

    /// Generate the Firecracker configuration (boot source, drives, machine
    /// config).
    ///
    /// The guest command and working directory are passed through kernel boot
    /// arguments (`clawdius_cmd` / `clawdius_workdir`); the root filesystem's
    /// init is expected to read and execute them.
    fn generate_config(&self, command: &str, args: &[&str], cwd: &Path) -> serde_json::Value {
        let cmd_str = if args.is_empty() {
            command.to_string()
        } else {
            format!("{} {}", command, args.join(" "))
        };

        let boot_args = format!(
            "console=ttyS0 reboot=k panic=1 pci=off nomodules \
             root=/dev/vda ro quiet \
             clawdius_cmd={} clawdius_workdir={}",
            shell_quote(&cmd_str),
            shell_quote(&cwd.to_string_lossy()),
        );

        serde_json::json!({
            "boot-source": {
                "kernel_image_path": self.kernel_image.to_string_lossy(),
                "boot_args": boot_args
            },
            "drives": [{
                "drive_id": "rootfs",
                "path_on_host": self.rootfs.to_string_lossy(),
                "is_root_device": true,
                "is_read_only": false
            }],
            "machine-config": {
                "vcpu_count": self.vcpus,
                "mem_size_mib": self.mem_size_mb
            }
        })
    }

    /// Execute a command inside a Firecracker microVM.
    fn run_vm(&self, config_path: &Path) -> Result<Output> {
        let mut fc_cmd = Command::new(&self.firecracker_bin);
        fc_cmd
            .args(["--api-sock"])
            .arg(&self.api_socket)
            .args(["--config-file"])
            .arg(config_path);

        run_timed(fc_cmd, self.timeout)
    }

    /// Remove temporary artifacts for this VM.
    fn cleanup(&self, config_path: &Path) {
        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_file(&self.api_socket);
    }
}

impl SandboxBackend for FirecrackerBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        if !Self::is_available() {
            return Err(Error::Sandbox(format!(
                "Firecracker binary not found at '{}' or kernel image missing. \
                 Install from: \
                 https://github.com/firecracker-microvm/firecracker/releases",
                DEFAULT_FIRECRACKER_BIN
            )));
        }

        let config = self.generate_config(command, args, cwd);
        let config_path = std::env::temp_dir().join(format!("fc-config-{}.json", self.vm_id));

        let config_str =
            serde_json::to_string_pretty(&config).map_err(Error::Serialization)?;
        if let Err(e) = std::fs::write(&config_path, config_str) {
            return Err(Error::Io(e));
        }

        let result = self.run_vm(&config_path);

        // Always clean up, even on failure.
        self.cleanup(&config_path);

        result
    }

    fn name(&self) -> &'static str {
        "firecracker"
    }
}

/// Quote a string for safe inclusion in a kernel boot argument value.
///
/// Wraps the value in single quotes and escapes any embedded single quotes,
/// mirroring the standard POSIX shell-escaping used by minimal VM init scripts.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Spawn a command, capture `stdout`/`stderr` without pipe-buffer deadlock, and
/// enforce a timeout.
///
/// Output streams are drained on background threads so a guest producing more
/// than the pipe buffer (~64 KiB) does not stall forever. If the deadline
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
    fn test_firecracker_available() {
        let _ = FirecrackerBackend::is_available();
    }

    #[test]
    fn test_config_contains_machine_and_boot_source() {
        let config = SandboxConfig {
            tier: SandboxTier::Hardened,
            network: false,
            mounts: vec![],
        };
        let backend = FirecrackerBackend::new(config);
        let json = backend.generate_config("echo", &["hi"], Path::new("/workspace"));

        let boot = json.get("boot-source").expect("boot-source present");
        assert_eq!(
            boot["kernel_image_path"].as_str().unwrap(),
            DEFAULT_KERNEL
        );
        let boot_args = boot["boot_args"].as_str().unwrap();
        assert!(boot_args.contains("clawdius_cmd="));
        assert!(boot_args.contains("echo hi"));

        let machine = json.get("machine-config").expect("machine-config present");
        assert_eq!(machine["vcpu_count"].as_u64().unwrap(), u64::from(DEFAULT_VCPUS));
        assert_eq!(
            machine["mem_size_mib"].as_u64().unwrap(),
            DEFAULT_MEM_SIZE_MB
        );

        let drives = json.get("drives").expect("drives present");
        assert!(drives[0]["is_root_device"].as_bool().unwrap());
    }

    #[test]
    fn test_shell_quote_escapes_quotes() {
        assert_eq!(shell_quote("hello"), "'hello'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn test_builder_methods() {
        let config = SandboxConfig {
            tier: SandboxTier::Hardened,
            network: false,
            mounts: vec![],
        };
        let backend = FirecrackerBackend::new(config)
            .with_memory(1024)
            .with_vcpus(2)
            .with_vm_id("test-vm");
        assert_eq!(backend.mem_size_mb, 1024);
        assert_eq!(backend.vcpus, 2);
        assert_eq!(backend.vm_id, "test-vm");
        assert!(backend
            .api_socket
            .to_string_lossy()
            .contains("firecracker-test-vm.sock"));
    }
}
