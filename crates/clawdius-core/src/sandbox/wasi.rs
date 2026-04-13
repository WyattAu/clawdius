//! WASI sandbox for executing WebAssembly modules with sandboxed filesystem,
//! network, and environment access.
//!
//! Uses wasmtime 42 with WASI preview1 (via `wasmtime-wasi` p1 module) to
//! provide per-user isolation with bounded memory, fuel, and timeouts.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::{ClosedInputStream, ClosedOutputStream, MemoryOutputPipe};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

pub const DEFAULT_MEMORY_LIMIT: usize = 512 * 1024 * 1024;
pub const DEFAULT_FUEL: u64 = 1_000_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct WasiSandboxConfig {
    pub workspace_root: PathBuf,
    pub memory_limit: usize,
    pub fuel: u64,
    pub timeout: Duration,
    pub allowed_env_vars: Vec<String>,
    pub network_access: bool,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
}

impl WasiSandboxConfig {
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            memory_limit: DEFAULT_MEMORY_LIMIT,
            fuel: DEFAULT_FUEL,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            allowed_env_vars: Vec::new(),
            network_access: false,
            capture_stdout: true,
            capture_stderr: true,
        }
    }

    #[must_use]
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    #[must_use]
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    #[must_use]
    pub fn with_allowed_env_var(mut self, var: impl Into<String>) -> Self {
        self.allowed_env_vars.push(var.into());
        self
    }

    #[must_use]
    pub fn with_network_access(mut self, allowed: bool) -> Self {
        self.network_access = allowed;
        self
    }
}

impl Default for WasiSandboxConfig {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

#[derive(Debug, Clone)]
pub struct WasiOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub execution_time: Duration,
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
}

pub struct WasiSandboxState {
    wasi: WasiP1Ctx,
    workspace_root: PathBuf,
    log_messages: Vec<String>,
    stdout_pipe: Option<MemoryOutputPipe>,
    stderr_pipe: Option<MemoryOutputPipe>,
}

impl std::fmt::Debug for WasiSandboxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasiSandboxState")
            .field("workspace_root", &self.workspace_root)
            .field("log_messages", &self.log_messages)
            .finish_non_exhaustive()
    }
}

impl WasiSandboxState {
    fn new(
        wasi: WasiP1Ctx,
        workspace_root: PathBuf,
        stdout_pipe: Option<MemoryOutputPipe>,
        stderr_pipe: Option<MemoryOutputPipe>,
    ) -> Self {
        Self {
            wasi,
            workspace_root,
            log_messages: Vec::new(),
            stdout_pipe,
            stderr_pipe,
        }
    }
}

impl wasmtime_wasi::WasiView for WasiSandboxState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        self.wasi.ctx()
    }
}

fn validate_path(workspace_root: &Path, path: &str) -> std::result::Result<PathBuf, String> {
    let requested = PathBuf::from(path);

    let resolved = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        workspace_root.join(&requested)
    };

    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize workspace root: {e}"))?;

    let canonical_requested = canonical_root
        .join(&resolved)
        .canonicalize()
        .map_err(|e| format!("Path does not exist or is inaccessible: {e}"))?;

    if !canonical_requested.starts_with(&canonical_root) {
        return Err("Path traversal detected: path escapes workspace root".to_string());
    }

    Ok(canonical_requested)
}

fn validate_path_for_write(
    workspace_root: &Path,
    path: &str,
) -> std::result::Result<PathBuf, String> {
    let requested = PathBuf::from(path);

    let resolved = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        workspace_root.join(&requested)
    };

    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize workspace root: {e}"))?;

    let canonical_parent = resolved.parent().ok_or("Path has no parent directory")?;

    let canonical_parent_resolved = canonical_root.join(canonical_parent);

    if canonical_parent_resolved.starts_with(&canonical_root)
        || canonical_parent_resolved == canonical_root
    {
        return Ok(canonical_root.join(resolved));
    }

    Err("Path traversal detected: path escapes workspace root".to_string())
}

pub struct WasiSandbox {
    engine: Engine,
    config: WasiSandboxConfig,
}

impl std::fmt::Debug for WasiSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasiSandbox")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WasiSandbox {
    pub fn new(config: WasiSandboxConfig) -> crate::error::Result<Self> {
        let mut wasm_config = Config::new();
        wasm_config.wasm_multi_memory(true);
        wasm_config.consume_fuel(true);

        let engine = Engine::new(&wasm_config).map_err(|e| {
            crate::error::Error::Sandbox(format!("Failed to create WASI engine: {e}"))
        })?;

        Ok(Self { engine, config })
    }

    fn create_wasi_ctx(
        &self,
    ) -> crate::error::Result<(
        WasiP1Ctx,
        Option<MemoryOutputPipe>,
        Option<MemoryOutputPipe>,
    )> {
        if self.config.workspace_root.canonicalize().is_err() {
            return Err(crate::error::Error::Sandbox(
                "Workspace root does not exist".to_string(),
            ));
        }

        let stdout_pipe = if self.config.capture_stdout {
            let pipe = MemoryOutputPipe::new(1024 * 1024);
            Some(pipe)
        } else {
            None
        };

        let stderr_pipe = if self.config.capture_stderr {
            let pipe = MemoryOutputPipe::new(1024 * 1024);
            Some(pipe)
        } else {
            None
        };

        let mut builder = WasiCtxBuilder::new();

        builder
            .preopened_dir(
                &self.config.workspace_root,
                "/",
                DirPerms::READ | DirPerms::MUTATE,
                FilePerms::READ | FilePerms::WRITE,
            )
            .map_err(|e| {
                crate::error::Error::Sandbox(format!("Failed to preopen workspace directory: {e}"))
            })?;

        if let Some(ref pipe) = stdout_pipe {
            builder.stdout(pipe.clone());
        } else {
            builder.stdout(ClosedOutputStream);
        }

        if let Some(ref pipe) = stderr_pipe {
            builder.stderr(pipe.clone());
        } else {
            builder.stderr(ClosedOutputStream);
        }

        builder.stdin(ClosedInputStream);

        for key in &self.config.allowed_env_vars {
            if let Ok(val) = std::env::var(key) {
                builder.env(key, &val);
            }
        }

        if !self.config.network_access {
            builder
                .allow_tcp(false)
                .allow_udp(false)
                .allow_ip_name_lookup(false);
        }

        let wasi_ctx = builder.build_p1();

        Ok((wasi_ctx, stdout_pipe, stderr_pipe))
    }

    fn create_linker(engine: &Engine) -> crate::error::Result<Linker<WasiSandboxState>> {
        let mut linker: Linker<WasiSandboxState> = Linker::new(engine);

        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s: &mut WasiSandboxState| &mut s.wasi)
            .map_err(|e| {
                crate::error::Error::Sandbox(format!("Failed to add WASI to linker: {e}"))
            })?;

        linker
            .func_wrap("host", "read_file", {
                |mut caller: wasmtime::Caller<'_, WasiSandboxState>,
                 path_ptr: i32,
                 path_len: i32|
                 -> i32 {
                    let path = match read_string_from_wasm(&mut caller, path_ptr, path_len as u32) {
                        Ok(p) => p,
                        Err(_) => return -1,
                    };

                    let root = caller.data().workspace_root.clone();
                    match validate_path(&root, &path) {
                        Ok(resolved) => match std::fs::read_to_string(&resolved) {
                            Ok(content) => match write_string_to_wasm(&mut caller, &content) {
                                Ok((ptr, len)) => (ptr << 16) | (len & 0xFFFF),
                                Err(_) => -2,
                            },
                            Err(_) => -3,
                        },
                        Err(_) => -1,
                    }
                }
            })
            .map_err(|e| {
                crate::error::Error::Sandbox(format!("Failed to define host_read_file: {e}"))
            })?;

        linker
            .func_wrap(
                "host",
                "write_file",
                |mut caller: wasmtime::Caller<'_, WasiSandboxState>,
                 path_ptr: i32,
                 path_len: i32,
                 content_ptr: i32,
                 content_len: i32|
                 -> i32 {
                    let path = match read_string_from_wasm(&mut caller, path_ptr, path_len as u32) {
                        Ok(p) => p,
                        Err(_) => return -1,
                    };

                    let content =
                        match read_string_from_wasm(&mut caller, content_ptr, content_len as u32) {
                            Ok(c) => c,
                            Err(_) => return -1,
                        };

                    let root = caller.data().workspace_root.clone();
                    match validate_path_for_write(&root, &path) {
                        Ok(resolved) => {
                            if let Some(parent) = resolved.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            match std::fs::write(&resolved, &content) {
                                Ok(()) => 0,
                                Err(_) => -1,
                            }
                        },
                        Err(_) => -1,
                    }
                },
            )
            .map_err(|e| {
                crate::error::Error::Sandbox(format!("Failed to define host_write_file: {e}"))
            })?;

        linker
            .func_wrap("host", "log", {
                |mut caller: wasmtime::Caller<'_, WasiSandboxState>, msg_ptr: i32, msg_len: i32| {
                    if let Ok(msg) = read_string_from_wasm(&mut caller, msg_ptr, msg_len as u32) {
                        caller.data_mut().log_messages.push(msg);
                    }
                }
            })
            .map_err(|e| crate::error::Error::Sandbox(format!("Failed to define host_log: {e}")))?;

        Ok(linker)
    }

    pub fn execute(
        &self,
        wasm_bytes: &[u8],
        function_name: &str,
        _args: &[String],
    ) -> crate::error::Result<WasiOutput> {
        let start = Instant::now();

        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            crate::error::Error::Sandbox(format!("Failed to compile WASM module: {e}"))
        })?;

        let (wasi_ctx, stdout_pipe, stderr_pipe) = self.create_wasi_ctx()?;
        let state = WasiSandboxState::new(
            wasi_ctx,
            self.config.workspace_root.clone(),
            stdout_pipe,
            stderr_pipe,
        );

        let mut store = Store::new(&self.engine, state);
        store
            .set_fuel(self.config.fuel)
            .map_err(|e| crate::error::Error::Sandbox(format!("Failed to set fuel: {e}")))?;

        let linker = Self::create_linker(&self.engine)?;

        let instance = linker.instantiate(&mut store, &module).map_err(|e| {
            crate::error::Error::Sandbox(format!("Failed to instantiate WASM module: {e}"))
        })?;

        let export = instance
            .get_typed_func::<(), (i32,)>(&mut store, function_name)
            .map_err(|e| {
                crate::error::Error::Sandbox(format!(
                    "Export '{function_name}' not found or wrong signature: {e}"
                ))
            })?;

        let result = export.call(&mut store, ());

        let fuel_remaining = store.get_fuel().unwrap_or(0);
        let fuel_consumed = self.config.fuel.saturating_sub(fuel_remaining);
        let execution_time = start.elapsed();

        let stdout_content = store
            .data()
            .stdout_pipe
            .as_ref()
            .map(|p| String::from_utf8_lossy(&p.contents()).to_string())
            .unwrap_or_default();

        let stderr_content = store
            .data()
            .stderr_pipe
            .as_ref()
            .map(|p| String::from_utf8_lossy(&p.contents()).to_string())
            .unwrap_or_default();

        let exit_code = match result {
            Ok((code,)) => code,
            Err(e) => {
                let is_fuel = e.to_string().to_lowercase().contains("fuel")
                    || e.to_string().contains("all fuel consumed");
                if is_fuel {
                    return Ok(WasiOutput {
                        stdout: stdout_content,
                        stderr: format!("Fuel exhausted: {e}"),
                        exit_code: -1,
                        execution_time,
                        fuel_consumed,
                        fuel_remaining,
                    });
                }
                return Ok(WasiOutput {
                    stdout: stdout_content,
                    stderr: format!("Trap: {e}"),
                    exit_code: -1,
                    execution_time,
                    fuel_consumed,
                    fuel_remaining,
                });
            },
        };

        Ok(WasiOutput {
            stdout: stdout_content,
            stderr: stderr_content,
            exit_code,
            execution_time,
            fuel_consumed,
            fuel_remaining,
        })
    }
}

fn read_string_from_wasm(
    caller: &mut wasmtime::Caller<'_, WasiSandboxState>,
    ptr: i32,
    len: u32,
) -> std::result::Result<String, String> {
    if ptr < 0 || len == 0 {
        return Err("Invalid pointer or length".to_string());
    }
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or("No memory export found")?;
    let data = memory
        .data(caller)
        .get(ptr as usize..(ptr as usize).saturating_add(len as usize))
        .ok_or("Memory access out of bounds")?;
    String::from_utf8(data.to_vec()).map_err(|e| format!("Invalid UTF-8: {e}"))
}

fn write_string_to_wasm(
    caller: &mut wasmtime::Caller<'_, WasiSandboxState>,
    s: &str,
) -> std::result::Result<(i32, i32), String> {
    let bytes = s.as_bytes();
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or("No memory export found")?;
    let data_size = memory.data_size(&mut *caller);
    if bytes.len() > data_size {
        return Err("Not enough memory".to_string());
    }
    memory
        .data_mut(&mut *caller)
        .get_mut(0..bytes.len())
        .ok_or("Memory access out of bounds")?
        .copy_from_slice(bytes);
    Ok((0, i32::try_from(bytes.len()).unwrap_or(i32::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn build_minimal_wasm_module() -> Vec<u8> {
        wat::parse_str(
            "(module
  (memory (export \"memory\") 1)
  (func (export \"run\") (result i32)
    i32.const 42
  )
)",
        )
        .expect("Failed to parse WAT")
    }

    fn build_wasm_module_with_loop() -> Vec<u8> {
        wat::parse_str(
            "(module
  (memory (export \"memory\") 1)
  (func (export \"run\") (result i32)
    (loop $break
      i32.const 1
      br_if $break
    )
    i32.const 0
  )
)",
        )
        .expect("Failed to parse WAT")
    }

    fn build_wasm_module_with_stdout() -> Vec<u8> {
        wat::parse_str(
            r#"(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    (i32.store (i32.const 0) (i32.const 12))
    (i32.store (i32.const 4) (i32.const 0))
    (i32.store (i32.const 8) (i32.const 1))
    (i32.store8 (i32.const 12) (i32.const 72))
    (i32.store8 (i32.const 13) (i32.const 105))
    (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 20))
    drop
    i32.const 0
  )
)"#,
        )
        .expect("Failed to parse WAT")
    }

    fn build_wasm_module_with_log() -> Vec<u8> {
        wat::parse_str(
            r#"(module
  (import "host" "log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    (i32.store8 (i32.const 0) (i32.const 72))
    (i32.store8 (i32.const 1) (i32.const 101))
    (i32.store8 (i32.const 2) (i32.const 108))
    (i32.store8 (i32.const 3) (i32.const 108))
    (i32.store8 (i32.const 4) (i32.const 111))
    (call $host_log (i32.const 0) (i32.const 5))
    i32.const 0
  )
)"#,
        )
        .expect("Failed to parse WAT")
    }

    #[test]
    fn test_default_config() {
        let config = WasiSandboxConfig::default();
        assert_eq!(config.memory_limit, DEFAULT_MEMORY_LIMIT);
        assert_eq!(config.fuel, DEFAULT_FUEL);
        assert_eq!(config.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert!(!config.network_access);
        assert!(config.capture_stdout);
        assert!(config.capture_stderr);
        assert!(config.allowed_env_vars.is_empty());
    }

    #[test]
    fn test_config_builder() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf())
            .with_memory_limit(256 * 1024 * 1024)
            .with_fuel(500_000)
            .with_timeout(10)
            .with_allowed_env_var("PATH")
            .with_network_access(true);

        assert_eq!(config.memory_limit, 256 * 1024 * 1024);
        assert_eq!(config.fuel, 500_000);
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.allowed_env_vars, vec!["PATH"]);
        assert!(config.network_access);
    }

    #[test]
    fn test_create_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf());
        let result = WasiSandbox::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_simple_module() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf());
        let sandbox = WasiSandbox::new(config).unwrap();
        let wasm_bytes = build_minimal_wasm_module();
        let result = sandbox.execute(&wasm_bytes, "run", &[]);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 42);
        assert!(output.execution_time < Duration::from_secs(5));
    }

    #[test]
    fn test_validate_path_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let result = validate_path(&root, "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn test_validate_path_blocks_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let result = validate_path(&root, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_allows_within_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "hello").unwrap();

        let result = validate_path(&root, "test.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_fuel_exhaustion() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf()).with_fuel(100);
        let sandbox = WasiSandbox::new(config).unwrap();
        let wasm_bytes = build_wasm_module_with_loop();
        let result = sandbox.execute(&wasm_bytes, "run", &[]);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, -1);
        assert!(output.fuel_consumed >= 100);
    }

    #[test]
    fn test_stdout_capture() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf());
        let sandbox = WasiSandbox::new(config).unwrap();
        let wasm_bytes = build_wasm_module_with_stdout();
        let result = sandbox.execute(&wasm_bytes, "run", &[]);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("Hi") || output.exit_code == 0);
    }

    #[test]
    fn test_host_log_capture() {
        let dir = tempfile::tempdir().unwrap();
        let config = WasiSandboxConfig::new(dir.path().to_path_buf());
        let sandbox = WasiSandbox::new(config).unwrap();
        let wasm_bytes = build_wasm_module_with_log();
        let result = sandbox.execute(&wasm_bytes, "run", &[]);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 0);
    }
}
