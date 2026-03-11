//! Firecracker microVM sandbox backend
//!
//! Firecracker provides lightweight VM isolation using KVM.
//! This backend requires firecracker and jailer to be installed.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::SandboxBackend;

/// Firecracker VM configuration
#[derive(Debug, Clone)]
pub struct FirecrackerConfig {
    /// VM ID prefix
    pub vm_id_prefix: String,
    /// Memory in MB
    pub memory_mb: u64,
    /// Number of vCPUs
    pub vcpu_count: u8,
    /// Enable HT (hyperthreading)
    pub ht_enabled: bool,
    /// Root filesystem path
    pub rootfs_path: PathBuf,
    /// Kernel image path
    pub kernel_path: PathBuf,
    /// Kernel command line
    pub kernel_cmdline: String,
    /// Network enabled
    pub network: bool,
    /// Tap device name
    pub tap_device: Option<String>,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Jailer configuration
    pub jailer: JailerConfig,
}

impl Default for FirecrackerConfig {
    fn default() -> Self {
        Self {
            vm_id_prefix: "clawdius".to_string(),
            memory_mb: 128,
            vcpu_count: 1,
            ht_enabled: false,
            rootfs_path: PathBuf::from("/var/lib/clawdius/rootfs.ext4"),
            kernel_path: PathBuf::from("/var/lib/clawdius/vmlinux"),
            kernel_cmdline: "console=ttyS0 reboot=k panic=1 pci=off".to_string(),
            network: false,
            tap_device: None,
            timeout_secs: 300,
            jailer: JailerConfig::default(),
        }
    }
}

/// Jailer configuration for Firecracker
#[derive(Debug, Clone)]
pub struct JailerConfig {
    /// Enable jailer (recommended for production)
    pub enabled: bool,
    /// Jail directory
    pub jail_dir: PathBuf,
    /// Numeric UID to switch to
    pub uid: u32,
    /// Numeric GID to switch to
    pub gid: u32,
    /// Chroot base
    pub chroot_base: PathBuf,
    /// Net namespace
    pub netns: Option<String>,
    /// Daemonize
    pub daemonize: bool,
}

impl Default for JailerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jail_dir: PathBuf::from("/srv/jailer"),
            uid: 1000,
            gid: 1000,
            chroot_base: PathBuf::from("/var/lib/clawdius/firecracker"),
            netns: None,
            daemonize: true,
        }
    }
}

/// VM configuration for Firecracker API
#[derive(Debug, Clone, serde::Serialize)]
struct VmConfig {
    #[serde(rename = "boot-source")]
    boot_source: BootSource,
    #[serde(rename = "machine-config")]
    machine_config: MachineConfig,
    drives: Vec<Drive>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct BootSource {
    #[serde(rename = "kernel-image-path")]
    kernel_image_path: String,
    #[serde(rename = "boot-args")]
    boot_args: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct MachineConfig {
    #[serde(rename = "vcpu-count")]
    vcpu_count: u8,
    #[serde(rename = "mem-size-mib")]
    mem_size_mib: u64,
    #[serde(rename = "ht-enabled")]
    ht_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct Drive {
    #[serde(rename = "drive-id")]
    drive_id: String,
    #[serde(rename = "path-on-host")]
    path_on_host: String,
    #[serde(rename = "is-root-device")]
    is_root_device: bool,
    #[serde(rename = "is-read-only")]
    is_read_only: bool,
}

/// Firecracker microVM backend
pub struct FirecrackerBackend {
    config: FirecrackerConfig,
    vm_counter: Arc<RwLock<u64>>,
    running_vms: Arc<RwLock<Vec<RunningVm>>>,
}

/// Running VM tracking
#[derive(Debug, Clone)]
struct RunningVm {
    vm_id: String,
    pid: Option<u32>,
    socket_path: PathBuf,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl FirecrackerBackend {
    /// Create a new Firecracker backend
    #[must_use]
    pub fn new(config: FirecrackerConfig) -> Self {
        Self {
            config,
            vm_counter: Arc::new(RwLock::new(0)),
            running_vms: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(FirecrackerConfig::default())
    }

    /// Check if Firecracker is available
    #[must_use]
    pub fn is_available() -> bool {
        std::process::Command::new("firecracker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if KVM is available
    #[must_use]
    pub fn is_kvm_available() -> bool {
        Path::new("/dev/kvm").exists()
    }

    /// Generate a unique VM ID
    async fn generate_vm_id(&self) -> String {
        let mut counter = self.vm_counter.write().await;
        *counter += 1;
        format!("{}-{}", self.config.vm_id_prefix, *counter)
    }

    /// Start a microVM
    pub async fn start_vm(&self, vm_id: &str) -> Result<PathBuf> {
        let socket_path = self
            .config
            .jailer
            .chroot_base
            .join(vm_id)
            .join("root")
            .join("run")
            .join("firecracker.socket");

        // Ensure directories exist
        tokio::fs::create_dir_all(socket_path.parent().unwrap()).await?;

        if self.config.jailer.enabled {
            self.start_with_jailer(vm_id, &socket_path).await?;
        } else {
            self.start_without_jailer(vm_id, &socket_path).await?;
        }

        // Track VM
        self.running_vms.write().await.push(RunningVm {
            vm_id: vm_id.to_string(),
            pid: None,
            socket_path: socket_path.clone(),
            created_at: chrono::Utc::now(),
        });

        Ok(socket_path)
    }

    async fn start_with_jailer(&self, vm_id: &str, _socket_path: &Path) -> Result<()> {
        let mut cmd = tokio::process::Command::new("jailer");

        cmd.arg("--id")
            .arg(vm_id)
            .arg("--uid")
            .arg(self.config.jailer.uid.to_string())
            .arg("--gid")
            .arg(self.config.jailer.gid.to_string())
            .arg("--exec-file")
            .arg(&self.config.kernel_path)
            .arg("--chroot-base-dir")
            .arg(&self.config.jailer.chroot_base)
            .arg("--daemonize");

        if let Some(ref netns) = self.config.jailer.netns {
            cmd.arg("--netns").arg(netns);
        }

        cmd.arg("--")
            .arg("--config-file")
            .arg(self.generate_config_path(vm_id));

        let status = cmd
            .status()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to start jailer: {e}")))?;

        if !status.success() {
            return Err(Error::Sandbox("Jailer failed to start".to_string()));
        }

        Ok(())
    }

    async fn start_without_jailer(&self, vm_id: &str, socket_path: &Path) -> Result<()> {
        // Start firecracker directly (less secure, for development)
        let mut cmd = tokio::process::Command::new("firecracker");

        cmd.arg("--api-sock")
            .arg(socket_path)
            .arg("--config-file")
            .arg(self.generate_config_path(vm_id));

        let _child = cmd
            .spawn()
            .map_err(|e| Error::Sandbox(format!("Failed to start firecracker: {e}")))?;

        // Wait for socket to be available
        for _ in 0..50 {
            if socket_path.exists() {
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Err(Error::Sandbox(
            "Firecracker socket not available".to_string(),
        ))
    }

    fn generate_config_path(&self, vm_id: &str) -> PathBuf {
        self.config
            .jailer
            .chroot_base
            .join(vm_id)
            .join("config.json")
    }

    /// Generate VM configuration
    fn generate_vm_config(&self) -> VmConfig {
        VmConfig {
            boot_source: BootSource {
                kernel_image_path: self.config.kernel_path.to_string_lossy().to_string(),
                boot_args: self.config.kernel_cmdline.clone(),
            },
            machine_config: MachineConfig {
                vcpu_count: self.config.vcpu_count,
                mem_size_mib: self.config.memory_mb,
                ht_enabled: self.config.ht_enabled,
            },
            drives: vec![Drive {
                drive_id: "rootfs".to_string(),
                path_on_host: self.config.rootfs_path.to_string_lossy().to_string(),
                is_root_device: true,
                is_read_only: false,
            }],
        }
    }

    /// Stop a microVM
    pub async fn stop_vm(&self, vm_id: &str) -> Result<()> {
        let vms = self.running_vms.read().await;
        if let Some(vm) = vms.iter().find(|v| v.vm_id == vm_id) {
            // Send shutdown via API
            let client = reqwest::Client::new();
            let url = format!("http://unix:{}/actions", vm.socket_path.to_string_lossy());

            let _ = client
                .put(&url)
                .json(&serde_json::json!({"action_type": "SendCtrlAltDel"}))
                .send()
                .await;
        }

        // Remove from tracking
        self.running_vms.write().await.retain(|v| v.vm_id != vm_id);

        Ok(())
    }

    /// List running VMs
    pub async fn list_vms(&self) -> Vec<String> {
        self.running_vms
            .read()
            .await
            .iter()
            .map(|v| v.vm_id.clone())
            .collect()
    }

    /// Cleanup all VMs
    pub async fn cleanup(&self) -> Result<()> {
        let vm_ids: Vec<String> = self.list_vms().await;

        for vm_id in vm_ids {
            self.stop_vm(&vm_id).await.ok();
        }

        Ok(())
    }
}

impl SandboxBackend for FirecrackerBackend {
    fn execute(&self, _command: &str, _args: &[&str], _cwd: &Path) -> Result<Output> {
        // Firecracker requires async API interaction
        // This synchronous implementation would block
        Err(Error::Sandbox(
            "Firecracker requires async execution. Use execute_async instead.".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "firecracker"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firecracker_config_defaults() {
        let config = FirecrackerConfig::default();
        assert_eq!(config.memory_mb, 128);
        assert_eq!(config.vcpu_count, 1);
    }

    #[test]
    fn test_vm_config_generation() {
        let backend = FirecrackerBackend::with_defaults();
        let vm_config = backend.generate_vm_config();

        assert_eq!(vm_config.machine_config.vcpu_count, 1);
        assert_eq!(vm_config.machine_config.mem_size_mib, 128);
    }

    #[test]
    fn test_firecracker_backend_creation() {
        let backend = FirecrackerBackend::with_defaults();
        // Just ensure it doesn't panic
        assert!(true);
    }
}
