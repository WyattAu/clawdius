//! gVisor (runsc) sandbox backend.
//!
//! Provides kernel-level isolation via gVisor's user-space kernel (Sentry).
//! Containers run with `docker run --runtime=runsc` for defense in depth.
//!
//! **Status:** Stub -- requires `runsc` runtime installed and Docker configured.
//! **Planned for:** v1.7.0
//! **Prerequisite:** gVisor installation (https://gvisor.dev/docs/user_guide/install/)

use super::SandboxBackend;
use crate::error::Result;
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};

/// gVisor sandbox backend using `docker --runtime=runsc`.
pub struct GvisorBackend {
    config: SandboxConfig,
    runtime: String,
}

impl GvisorBackend {
    /// Create a new gVisor backend with the default `runsc` runtime.
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            runtime: "runsc".to_string(),
        }
    }

    /// Create with a custom runtime name.
    #[allow(dead_code)]
    pub fn with_runtime(config: SandboxConfig, runtime: &str) -> Self {
        Self {
            config,
            runtime: runtime.to_string(),
        }
    }

    /// Check if gVisor runtime is available.
    pub fn is_available() -> bool {
        if Command::new("docker").arg("info").output().is_err() {
            return false;
        }
        Command::new("docker")
            .args(["info", "--format", "{{.Runtimes}}"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains("runsc")
            })
            .unwrap_or(false)
    }
}

impl SandboxBackend for GvisorBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let mut docker_args: Vec<String> = vec![
            "run".into(),
            "--rm".into(),
            format!("--runtime={}", self.runtime),
            "-v".into(),
            format!("{}:/workspace", cwd.display()),
            "-w".into(),
            "/workspace".into(),
        ];

        if !self.config.network {
            docker_args.push("--network=none".into());
        }

        for mount in &self.config.mounts {
            let flag = if mount.read_only { ":ro" } else { "" };
            docker_args.push("-v".into());
            docker_args.push(format!("{}:{}{}", mount.source, mount.destination, flag));
        }

        docker_args.push("alpine".into());
        docker_args.push(command.into());
        for arg in args {
            docker_args.push((*arg).into());
        }

        let output = Command::new("docker")
            .args(&docker_args)
            .output()?;

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "gvisor"
    }
}
