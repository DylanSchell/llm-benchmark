use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    #[serde(default = "default_parallelism")]
    pub parallelism: u32,

    #[serde(default = "default_benchmark_path")]
    pub benchmark_path: PathBuf,

    #[serde(default)]
    pub docker: DockerConfig,

    #[serde(default)]
    pub exercise: ExerciseConfig,

    #[serde(default)]
    pub claude: ClaudeConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

fn default_parallelism() -> u32 {
    1
}

fn default_benchmark_path() -> PathBuf {
    PathBuf::from("../polyglot-benchmark")
}

impl Config {
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DockerConfig {
    #[serde(default = "default_image")]
    pub image: String,

    #[serde(default = "default_work_dir")]
    pub work_dir: String,

    #[serde(default = "default_timeout")]
    pub timeout: u32,

    #[serde(default = "default_memory")]
    pub memory: String,

    #[serde(default)]
    pub environment: Vec<EnvironmentEntry>,
}

fn default_image() -> String {
    "claude-benchmark-runner:latest".to_string()
}

fn default_work_dir() -> String {
    "/workspace".to_string()
}

fn default_timeout() -> u32 {
    300
}

fn default_memory() -> String {
    "2g".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct EnvironmentEntry(pub std::collections::HashMap<String, String>);

impl DockerConfig {
    pub fn environment_map(&self) -> std::collections::HashMap<String, String> {
        let mut result = std::collections::HashMap::new();
        for entry in &self.environment {
            result.extend(entry.0.clone());
        }
        result
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExerciseConfig {
    #[serde(default = "default_language")]
    pub language: String,

    pub name: Option<String>,
    pub path: Option<PathBuf>,
}

fn default_language() -> String {
    "java".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClaudeConfig {
    #[serde(default = "default_cli_path")]
    pub cli_path: String,

    #[serde(default = "default_model")]
    pub model: String,

    pub extra_args: Option<Vec<String>>,
}

fn default_cli_path() -> String {
    "/usr/local/bin/claude".to_string()
}

fn default_model() -> String {
    "sonnet".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OutputConfig {
    #[serde(default = "default_results_dir")]
    pub results_dir: PathBuf,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_results_dir() -> PathBuf {
    PathBuf::from("../benchmark-results")
}

fn default_log_level() -> String {
    "INFO".to_string()
}
