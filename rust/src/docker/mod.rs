use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub image: String,
    pub memory: Option<String>,
    pub timeout: Option<u64>,
    pub work_dir: Option<String>,
    pub environment: Option<HashMap<String, String>>,
}

impl DockerConfig {
    pub fn image(&self) -> &str {
        &self.image
    }

    pub fn memory(&self) -> Option<&str> {
        self.memory.as_deref()
    }

    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    pub fn work_dir(&self) -> Option<&str> {
        self.work_dir.as_deref()
    }

    pub fn environment(&self) -> Option<&HashMap<String, String>> {
        self.environment.as_ref()
    }
}

#[derive(Clone)]
pub struct DockerClient {
    config: DockerConfig,
}

pub type OutputCallback = dyn Fn(&str) + Send + Sync + 'static;

impl DockerClient {
    pub fn new(config: DockerConfig) -> Self {
        Self { config }
    }

    /// Check if Docker is available and running
    pub fn is_available(&self) -> bool {
        match Command::new("docker")
            .args(&["version", "--format", "{{.Server.Version}}"])
            .output()
        {
            Ok(output) => output.status.success(),
            Err(e) => {
                error!("Docker is not available: {}", e);
                false
            }
        }
    }

    /// Run a command in a Docker container with resource limits
    pub fn run_command_with_limits_and_volume(
        &self,
        container_image: Option<&str>,
        work_dir: Option<&str>,
        command: &[&str],
        timeout_seconds: Option<u64>,
        memory_limit: Option<&str>,
        volume_host_dir: Option<&str>,
    ) -> Result<ProcessResult, std::io::Error> {
        self.run_command_with_limits_and_volume_with_callback(
            container_image,
            work_dir,
            command,
            timeout_seconds,
            memory_limit,
            volume_host_dir,
            None,
        )
    }

    /// Run a command in a Docker container with resource limits and optional output callback
    pub fn run_command_with_limits_and_volume_with_callback(
        &self,
        container_image: Option<&str>,
        work_dir: Option<&str>,
        command: &[&str],
        timeout_seconds: Option<u64>,
        memory_limit: Option<&str>,
        volume_host_dir: Option<&str>,
        output_callback: Option<Arc<OutputCallback>>,
    ) -> Result<ProcessResult, std::io::Error> {
        let image = container_image.unwrap_or(&self.config.image);
        let work = work_dir
            .or(self.config.work_dir.as_deref())
            .unwrap_or("/workspace");
        let timeout = timeout_seconds.or(self.config.timeout).unwrap_or(300);
        let memory = memory_limit.or(self.config.memory.as_deref());
        let host_dir = volume_host_dir.map(|s| s.to_string()).unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string()
        });

        // Generate deterministic container name
        let container_name = format!(
            "bench-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .replace('-', "")
                .chars()
                .take(12)
                .collect::<String>()
        );

        // Build docker command
        let mut full_command = Vec::with_capacity(32);
        full_command.push("docker");
        full_command.push("run");
        full_command.push("--name");
        full_command.push(&container_name);
        full_command.push("--rm");
        full_command.push("-w");
        full_command.push(work);
        if let Some(mem) = memory {
            full_command.push("-m");
            full_command.push(mem);
        }

        // Add environment variables
        let mut env_vars_strings = Vec::new();
        if let Some(env_vars) = &self.config.environment {
            for (key, value) in env_vars.iter() {
                let env_var = format!("{key}={value}");
                env_vars_strings.push(env_var);
            }

            for env_var in &env_vars_strings {
                full_command.push("-e");
                full_command.push(env_var.as_str());
            }
        }

        // Volume mounts (clone to avoid borrow issues)
        let host_dir_clone = host_dir.clone();
        let workspace_volume = format!("{}:/workspace", host_dir_clone);
        let host_dir_clone2 = host_dir.clone();
        let claude_volume = format!("{}/.claude:/home/runner/.claude", host_dir_clone2);
        full_command.push("-v");
        full_command.push(&workspace_volume);
        full_command.push("-v");
        full_command.push(&claude_volume);

        // Image and command
        full_command.push(image);
        for arg in command {
            full_command.push(arg);
        }

        debug!(
            "Executing with memory limit {:?} and volume {}:/workspace: {}",
            memory,
            host_dir,
            full_command.join(" ")
        );

        // Execute command
        let mut process = Command::new(&full_command[0])
            .args(&full_command[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let (tx, rx) = mpsc::channel();
        let tx_for_stderr = tx.clone();
        let stdout = process.stdout.take().unwrap();
        let stderr = process.stderr.take().unwrap();

        let callback_for_stdout = output_callback.clone();
        let _handle: std::thread::JoinHandle<()> = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if let Some(ref cb) = callback_for_stdout {
                            cb(&line);
                        }
                        let _ = tx.send(Ok(line));
                    }
                    Err(_) => {
                        let _ = tx.send(Err(()));
                        break;
                    }
                }
            }
        });

        let _stderr_handle: std::thread::JoinHandle<()> = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = tx_for_stderr.send(Ok(line));
                    }
                    Err(_) => {
                        let _ = tx_for_stderr.send(Err(()));
                        break;
                    }
                }
            }
        });

        // Wait for process with timeout
        let completed = wait_with_timeout(&mut process, timeout)?;

        let exit_code = if completed {
            process.wait()?.code().unwrap_or(-1)
        } else {
            let _ = process.kill();
            process.wait()?.code().unwrap_or(-1)
        };

        // Collect output
        let mut output = String::new();
        for result in rx.iter() {
            match result {
                Ok(line) => {
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(_) => break,
            }
        }

        Ok(ProcessResult {
            exit_code,
            output,
            completed,
            container_id: container_name,
        })
    }

    // /// Execute a simple docker command and return output
    // fn execute_command(&self, args: &[&str]) -> Result<Option<String>, std::io::Error> {
    //     let output = Command::new("docker").args(args).output()?;
    //     if output.status.success() {
    //         Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
    //     } else {
    //         Ok(None)
    //     }
    // }
}

/// Wait for a process with a timeout
fn wait_with_timeout(process: &mut Child, timeout_secs: u64) -> Result<bool, std::io::Error> {
    let (tx, rx) = mpsc::channel();

    std::thread::scope(|s| {
        s.spawn(|| {
            let result = process.wait();
            let _ = tx.send(result);
        });
    });

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(false),
    }
}

/// Result of a process execution
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub exit_code: i32,
    pub output: String,
    pub completed: bool,
    pub container_id: String,
}

impl ProcessResult {
    pub fn is_success(&self) -> bool {
        self.completed && self.exit_code == 0
    }
}
