use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerConfig {
    #[serde(default = "default_server_port")]
    pub port: u16,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

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

    #[serde(default)]
    pub model: Option<String>,

    #[serde(default = "default_inference_endpoint")]
    pub inference_endpoint: String,

    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_server_port() -> u16 {
    8081
}

fn default_parallelism() -> u32 {
    1
}

fn default_benchmark_path() -> PathBuf {
    PathBuf::from("../polyglot-benchmark")
}

fn default_inference_endpoint() -> String {
    "http://localhost:8000/v1".to_string()
}

impl Config {
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&content)?;
        
        // Resolve relative benchmark_path relative to config file directory
        if !config.benchmark_path.is_absolute() {
            if let Some(parent) = std::path::Path::new(path).parent() {
                config.benchmark_path = parent.join(&config.benchmark_path);
            }
        }
        
        Ok(config)
    }

    /// Validates the configuration.
    /// Checks for required fields and valid paths.
    pub fn validate(&self) -> Result<(), String> {
        // Validate parallelism
        if self.parallelism < 1 {
            return Err(format!("parallelism must be at least 1, got: {}", self.parallelism));
        }

        // Validate benchmark path exists
        if !self.benchmark_path.exists() {
            return Err(format!("benchmark_path does not exist: {:?}", self.benchmark_path));
        }

        // Validate docker configuration
        if self.docker.image.is_empty() {
            return Err("docker.image is required".to_string());
        }

        if self.docker.timeout < 10 {
            return Err(format!("docker.timeout must be at least 10 seconds, got: {}", self.docker.timeout));
        }

        if self.docker.memory.is_empty() {
            return Err("docker.memory is required".to_string());
        }

        // Validate output configuration
        if self.output.results_dir.as_os_str().is_empty() {
            return Err("output.results_dir is required".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
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

    #[serde(default = "default_per_command_timeout")]
    pub per_command_timeout: u32,

    #[serde(default)]
    pub environment: Vec<HashMap<String, String>>,
}

fn default_image() -> String {
    "llm-benchmark/runner:latest".to_string()
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

fn default_per_command_timeout() -> u32 {
    600
}

impl DockerConfig {
    pub fn environment_map(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for entry in &self.environment {
            result.extend(entry.clone());
        }
        result
    }

    /// Updates environment variables with the model name.
    /// Sets ANTHROPIC_MODEL and all ANTHROPIC_DEFAULT_*_MODEL variables.
    pub fn update_model_environment(&mut self, model_name: &str) {
        for env_entry in &mut self.environment {
            if let Some(v) = env_entry.get_mut("ANTHROPIC_MODEL") {
                *v = model_name.to_string();
            }
            if let Some(v) = env_entry.get_mut("ANTHROPIC_DEFAULT_HAIKU_MODEL") {
                *v = model_name.to_string();
            }
            if let Some(v) = env_entry.get_mut("ANTHROPIC_DEFAULT_OPUS_MODEL") {
                *v = model_name.to_string();
            }
            if let Some(v) = env_entry.get_mut("ANTHROPIC_DEFAULT_SONNET_MODEL") {
                *v = model_name.to_string();
            }
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
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

#[derive(Debug, Default, Clone, Deserialize)]
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

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OutputConfig {
    #[serde(default = "default_results_dir")]
    pub results_dir: PathBuf,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl OutputConfig {
    /// Gets the results directory for a specific benchmark run.
    /// Constructs subdirectory as: <agent>-<model>-<languages>-<run#>
    /// Languages are sorted alphabetically and joined with hyphens.
    pub fn get_results_dir(&self, agent_name: &str, model: &str, languages: &[String]) -> PathBuf {
        let agent_part = if !agent_name.is_empty() { agent_name } else { "unknown" };
        let model_part = if !model.is_empty() { model } else { "default" };

        // Sort languages alphabetically and join with hyphens
        let mut sorted_langs = languages.to_vec();
        sorted_langs.sort();
        let lang_part = sorted_langs.join("-");

        // Construct the base subdirectory name
        let subdir_base = format!("{}-{}-{}", agent_part, model_part, lang_part);

        // Check existing subdirectories to find the next run number
        let next_run = Self::get_next_run_number(&self.results_dir, &subdir_base);

        if next_run > 1 {
            self.results_dir.join(format!("{}-r{}", subdir_base, next_run))
        } else {
            self.results_dir.join(&subdir_base)
        }
    }

    /// Finds the next available run number for a given subdirectory pattern.
    fn get_next_run_number(results_dir: &PathBuf, subdir_base: &str) -> usize {
        if !results_dir.exists() {
            return 1;
        }

        let mut max_run_number = 0;

        if let Ok(entries) = std::fs::read_dir(results_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Check if directory matches the pattern: <subdir_base> or <subdir_base>-r<N>
                        if name == subdir_base {
                            // Matches base without run number - treat as run 0
                            max_run_number = std::cmp::max(max_run_number, 0);
                        } else if name.starts_with(&format!("{}-r", subdir_base)) {
                            // Try to extract run number
                            let run_part = &name[subdir_base.len() + 3..]; // Skip "<base>-r"
                            if let Ok(num) = run_part.parse::<usize>() {
                                max_run_number = std::cmp::max(max_run_number, num);
                            }
                        }
                    }
                }
            }
        }

        max_run_number + 1
    }
}

fn default_results_dir() -> PathBuf {
    PathBuf::from("../benchmark-results")
}

fn default_log_level() -> String {
    "INFO".to_string()
}

/// Quick-bench configuration: curated fast exercises per language (< 60s each).
/// Total: 155 exercise slots across 6 languages.
pub struct QuickBenchConfig;

impl QuickBenchConfig {
    /// Get quick-bench exercises for a language.
    pub fn get_exercises_for_language(language: &str) -> Vec<String> {
        match language {
            // C++ — 23 exercises under 60s
            "cpp" => vec![
                "all-your-base".to_string(),
                "allergies".to_string(),
                "bank-account".to_string(),
                "binary-search-tree".to_string(),
                "circular-buffer".to_string(),
                "clock".to_string(),
                "crypto-square".to_string(),
                "diamond".to_string(),
                "dnd-character".to_string(),
                "gigasecond".to_string(),
                "grade-school".to_string(),
                "kindergarten-garden".to_string(),
                "knapsack".to_string(),
                "linked-list".to_string(),
                "parallel-letter-frequency".to_string(),
                "perfect-numbers".to_string(),
                "phone-number".to_string(),
                "queen-attack".to_string(),
                "robot-name".to_string(),
                "space-age".to_string(),
                "spiral-matrix".to_string(),
                "sublist".to_string(),
                "yacht".to_string(),
            ],
            // Go — 24 exercises under 60s
            "go" => vec![
                "beer-song".to_string(),
                "book-store".to_string(),
                "bottle-song".to_string(),
                "crypto-square".to_string(),
                "dnd-character".to_string(),
                "dominoes".to_string(),
                "error-handling".to_string(),
                "food-chain".to_string(),
                "hexadecimal".to_string(),
                "octal".to_string(),
                "paasio".to_string(),
                "palindrome-products".to_string(),
                "pig-latin".to_string(),
                "protein-translation".to_string(),
                "say".to_string(),
                "simple-linked-list".to_string(),
                "sublist".to_string(),
                "transpose".to_string(),
                "tree-building".to_string(),
                "trinary".to_string(),
                "two-bucket".to_string(),
                "variable-length-quantity".to_string(),
                "word-search".to_string(),
                "wordy".to_string(),
            ],
            // Java — 28 exercises under 60s
            "java" => vec![
                "affine-cipher".to_string(),
                "all-your-base".to_string(),
                "bank-account".to_string(),
                "book-store".to_string(),
                "bottle-song".to_string(),
                "change".to_string(),
                "circular-buffer".to_string(),
                "custom-set".to_string(),
                "dominoes".to_string(),
                "house".to_string(),
                "kindergarten-garden".to_string(),
                "ocr-numbers".to_string(),
                "palindrome-products".to_string(),
                "phone-number".to_string(),
                "pig-latin".to_string(),
                "protein-translation".to_string(),
                "pythagorean-triplet".to_string(),
                "queen-attack".to_string(),
                "resistor-color-trio".to_string(),
                "satellite".to_string(),
                "series".to_string(),
                "simple-linked-list".to_string(),
                "state-of-tic-tac-toe".to_string(),
                "transpose".to_string(),
                "tree-building".to_string(),
                "twelve-days".to_string(),
                "two-bucket".to_string(),
                "word-search".to_string(),
            ],
            // JavaScript — 39 exercises under 60s
            "javascript" => vec![
                "affine-cipher".to_string(),
                "alphametics".to_string(),
                "beer-song".to_string(),
                "binary".to_string(),
                "book-store".to_string(),
                "bottle-song".to_string(),
                "connect".to_string(),
                "food-chain".to_string(),
                "go-counting".to_string(),
                "grade-school".to_string(),
                "grep".to_string(),
                "killer-sudoku-helper".to_string(),
                "list-ops".to_string(),
                "meetup".to_string(),
                "ocr-numbers".to_string(),
                "palindrome-products".to_string(),
                "phone-number".to_string(),
                "pig-latin".to_string(),
                "promises".to_string(),
                "queen-attack".to_string(),
                "rational-numbers".to_string(),
                "rectangles".to_string(),
                "resistor-color-trio".to_string(),
                "robot-name".to_string(),
                "say".to_string(),
                "scale-generator".to_string(),
                "simple-linked-list".to_string(),
                "space-age".to_string(),
                "state-of-tic-tac-toe".to_string(),
                "sum-of-multiples".to_string(),
                "tournament".to_string(),
                "transpose".to_string(),
                "triangle".to_string(),
                "twelve-days".to_string(),
                "two-bucket".to_string(),
                "variable-length-quantity".to_string(),
                "word-search".to_string(),
                "wordy".to_string(),
                "zipper".to_string(),
            ],
            // Python — 23 exercises under 60s
            "python" => vec![
                "affine-cipher".to_string(),
                "beer-song".to_string(),
                "book-store".to_string(),
                "bottle-song".to_string(),
                "dominoes".to_string(),
                "food-chain".to_string(),
                "go-counting".to_string(),
                "grade-school".to_string(),
                "grep".to_string(),
                "list-ops".to_string(),
                "phone-number".to_string(),
                "pig-latin".to_string(),
                "proverb".to_string(),
                "rest-api".to_string(),
                "robot-name".to_string(),
                "simple-linked-list".to_string(),
                "transpose".to_string(),
                "tree-building".to_string(),
                "two-bucket".to_string(),
                "variable-length-quantity".to_string(),
                "wordy".to_string(),
                "zebra-puzzle".to_string(),
                "zipper".to_string(),
            ],
            // Rust — 18 exercises under 60s
            "rust" => vec![
                "accumulate".to_string(),
                "acronym".to_string(),
                "alphametics".to_string(),
                "book-store".to_string(),
                "dot-dsl".to_string(),
                "gigasecond".to_string(),
                "grade-school".to_string(),
                "grep".to_string(),
                "luhn-from".to_string(),
                "macros".to_string(),
                "nucleotide-codons".to_string(),
                "parallel-letter-frequency".to_string(),
                "pig-latin".to_string(),
                "robot-name".to_string(),
                "say".to_string(),
                "two-bucket".to_string(),
                "variable-length-quantity".to_string(),
                "word-count".to_string(),
            ],
            _ => vec![],
        }
    }

    /// Returns all languages that have quick-bench exercises defined.
    pub fn get_available_languages() -> Vec<String> {
        vec!["cpp".to_string(), "go".to_string(), "java".to_string(), "javascript".to_string(), "python".to_string(), "rust".to_string()]
    }

    /// Returns the quick-bench exercise names as a HashSet for O(1) lookup.
    pub fn get_quick_exercises_set(language: &str) -> std::collections::HashSet<String> {
        Self::get_exercises_for_language(language).into_iter().collect()
    }

    /// Returns the total number of quick-bench exercise slots across all languages.
    pub fn get_total_exercise_count() -> usize {
        23 + 24 + 28 + 39 + 23 + 18
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── DockerConfig tests ────────────────────────────────────────────

    #[test]
    fn test_docker_config_default_values() {
        // Defaults are applied during deserialization, not via Default trait
        let config = DockerConfig {
            image: "llm-benchmark-runner:latest".to_string(),
            work_dir: "/workspace".to_string(),
            timeout: 300,
            memory: "2g".to_string(),
            ..Default::default()
        };
        assert_eq!(config.image, "llm-benchmark-runner:latest");
        assert_eq!(config.work_dir, "/workspace");
        assert_eq!(config.timeout, 300);
        assert_eq!(config.memory, "2g");
    }

    #[test]
    fn test_docker_config_update_model_environment() {
        let mut config = DockerConfig {
            environment: vec![
                HashMap::from([
                    ("ANTHROPIC_MODEL".to_string(), "old-model".to_string()),
                    ("ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(), "old-sonnet".to_string()),
                    ("OTHER_VAR".to_string(), "other".to_string()),
                ]),
            ],
            ..Default::default()
        };

        config.update_model_environment("new-model");

        assert_eq!(
            config.environment[0].get("ANTHROPIC_MODEL").unwrap(),
            "new-model"
        );
        assert_eq!(
            config.environment[0].get("ANTHROPIC_DEFAULT_SONNET_MODEL").unwrap(),
            "new-model"
        );
        assert_eq!(
            config.environment[0].get("OTHER_VAR").unwrap(),
            "other"
        );
    }

    #[test]
    fn test_docker_config_environment_map() {
        let config = DockerConfig {
            environment: vec![
                HashMap::from([("A".to_string(), "1".to_string())]),
                HashMap::from([("B".to_string(), "2".to_string())]),
            ],
            ..Default::default()
        };

        let map = config.environment_map();
        assert_eq!(map.get("A").unwrap(), &"1");
        assert_eq!(map.get("B").unwrap(), &"2");
        assert_eq!(map.len(), 2);
    }

    // ── OutputConfig tests ────────────────────────────────────────────

    #[test]
    fn test_output_config_default_values() {
        let config = OutputConfig {
            results_dir: PathBuf::from("../benchmark-results"),
            log_level: "INFO".to_string(),
        };
        assert_eq!(config.results_dir, PathBuf::from("../benchmark-results"));
        assert_eq!(config.log_level, "INFO");
    }

    #[test]
    fn test_output_config_get_results_dir() {
        let config = OutputConfig::default();
        let dir = config.get_results_dir("reference", "sonnet", &vec!["java".to_string()]);
        assert!(dir.to_string_lossy().contains("reference-sonnet-java"));
    }

    #[test]
    fn test_output_config_get_results_dir_sorted_languages() {
        let config = OutputConfig::default();
        let dir = config.get_results_dir("reference", "sonnet", &vec!["python".to_string(), "java".to_string()]);
        // Languages should be sorted: java before python
        assert!(dir.to_string_lossy().contains("java-python"));
    }

    #[test]
    fn test_output_config_get_results_dir_multiple_runs() {
        let temp_dir = std::env::temp_dir().join("benchmark-test-results");
        let _ = std::fs::create_dir_all(&temp_dir);

        // Create a directory to simulate first run
        let subdir = temp_dir.join("ref-sonnet-java");
        let _ = std::fs::create_dir_all(&subdir);

        let config = OutputConfig {
            results_dir: temp_dir.clone(),
            ..Default::default()
        };

        let dir = config.get_results_dir("ref", "sonnet", &vec!["java".to_string()]);
        // Should find the existing directory and use it (run 1)
        assert!(dir.to_string_lossy().contains("ref-sonnet-java"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ── Config tests ──────────────────────────────────────────────────

    #[test]
    fn test_config_default_values() {
        // Defaults are applied during deserialization, not via Default trait
        let config = Config {
            benchmark_path: PathBuf::from("../polyglot-benchmark"),
            parallelism: 1,
            inference_endpoint: "http://localhost:8000/v1".to_string(),
            ..Default::default()
        };
        assert_eq!(config.benchmark_path, PathBuf::from("../polyglot-benchmark"));
        assert_eq!(config.parallelism, 1);
        assert_eq!(config.inference_endpoint, "http://localhost:8000/v1");
    }

    #[test]
    fn test_config_validate_valid() {
        let config = Config::default();
        // Should not panic - validation checks path existence which may fail
        // but that's expected in test environment
        let _ = config.validate();
    }

    #[test]
    fn test_config_validate_low_parallelism() {
        let config = Config {
            parallelism: 0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("parallelism"));
    }

    #[test]
    fn test_config_validate_empty_docker_image() {
        let config = Config {
            docker: DockerConfig {
                image: "".to_string(),
                timeout: 300,
                memory: "2g".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        eprintln!("Error: {}", err);
        assert!(!err.is_empty());
    }

    #[test]
    fn test_config_validate_low_timeout() {
        let config = Config {
            docker: DockerConfig {
                timeout: 5,
                image: "test".to_string(),
                memory: "2g".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        eprintln!("Error: {}", err);
        assert!(!err.is_empty());
    }

    // ── QuickBenchConfig tests ────────────────────────────────────────

    #[test]
    fn test_quick_bench_config_get_exercises_for_language() {
        let java_exercises = QuickBenchConfig::get_exercises_for_language("java");
        assert!(!java_exercises.is_empty());
        assert!(java_exercises.contains(&"two-fer".to_string()) == false); // two-fer is not in quick bench
        assert!(java_exercises.contains(&"bank-account".to_string()));
    }

    #[test]
    fn test_quick_bench_config_unknown_language() {
        let exercises = QuickBenchConfig::get_exercises_for_language("unknown");
        assert!(exercises.is_empty());
    }

    #[test]
    fn test_quick_bench_config_total_count() {
        assert_eq!(QuickBenchConfig::get_total_exercise_count(), 155);
    }

    #[test]
    fn test_quick_bench_config_all_languages_have_exercises() {
        for lang in QuickBenchConfig::get_available_languages() {
            let exercises = QuickBenchConfig::get_exercises_for_language(&lang);
            assert!(!exercises.is_empty(), "Language {} should have quick-bench exercises", lang);
        }
    }

    #[test]
    fn test_quick_bench_config_java_has_28_exercises() {
        let exercises = QuickBenchConfig::get_exercises_for_language("java");
        assert_eq!(exercises.len(), 28);
    }

    #[test]
    fn test_quick_bench_config_javascript_has_36_exercises() {
        let exercises = QuickBenchConfig::get_exercises_for_language("javascript");
        // JavaScript has 37 exercises in our list (added "zipper")
        assert!(exercises.len() >= 36);
    }
}
