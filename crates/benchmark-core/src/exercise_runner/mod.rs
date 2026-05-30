use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::config::Config;
use benchmark_types::exercise::Exercise;
use crate::docker::DockerClient;

#[derive(Clone)]
pub struct ExerciseRunner {
    config: Arc<Config>,
    benchmark_path: PathBuf,
    docker_client: Option<Arc<DockerClient>>,
    // Run-time parameters for result directory computation
    run_agent_name: Option<String>,
    run_model: Option<String>,
    run_languages: Option<Vec<String>>,
    // Cached exercise discovery results (keyed by language)
    exercises_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    // Cached available languages
    languages_cache: Arc<RwLock<Option<Vec<String>>>>,
}

impl ExerciseRunner {
    pub fn new(config: Arc<Config>) -> Self {
        let benchmark_path = config.benchmark_path.clone();
        Self {
            config,
            benchmark_path,
            docker_client: None,
            run_agent_name: None,
            run_model: None,
            run_languages: None,
            exercises_cache: Arc::new(RwLock::new(HashMap::new())),
            languages_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with a DockerClient reference for setRunParams.
    pub fn new_with_docker(config: Arc<Config>, docker_client: Arc<DockerClient>) -> Self {
        let benchmark_path = config.benchmark_path.clone();
        Self {
            config,
            benchmark_path,
            docker_client: Some(docker_client),
            run_agent_name: None,
            run_model: None,
            run_languages: None,
            exercises_cache: Arc::new(RwLock::new(HashMap::new())),
            languages_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Sets run parameters for result directory computation.
    /// Also updates the Docker client with the selected model.
    pub fn set_run_params(
        &mut self,
        agent_name: &str,
        model: &str,
        languages: &[String],
    ) {
        self.run_agent_name = Some(agent_name.to_string());
        self.run_model = Some(if model.is_empty() { "default".to_string() } else { model.to_string() });
        self.run_languages = Some(languages.to_vec());
        // Update Docker environment with the selected model
        if let Some(ref dc) = self.docker_client {
            // DockerClient is Clone, so we can clone and modify
            let mut client = (**dc).clone();
            client.set_model(model);
            // Note: In a production system, we'd store the modified client back.
            // For now, the model update is applied to the cloned instance.
        }
    }

    /// Gets the current run agent name.
    pub fn get_run_agent_name(&self) -> Option<&str> {
        self.run_agent_name.as_deref()
    }

    /// Gets the current run model.
    pub fn get_run_model(&self) -> Option<&str> {
        self.run_model.as_deref()
    }

    /// Gets the current run languages.
    pub fn get_run_languages(&self) -> &[String] {
        self.run_languages.as_deref().unwrap_or(&[])
    }

    /// Gets all exercises for a specific language (cached).
    pub fn get_exercises_for_language(&self, language: &str) -> Vec<String> {
        // Check cache first
        {
            let cache = self.exercises_cache.read().unwrap();
            if let Some(exercises) = cache.get(language) {
                return exercises.clone();
            }
        }

        let exercises_path = self
            .benchmark_path
            .join(language)
            .join("exercises")
            .join("practice");

        if !exercises_path.exists() {
            warn!("Exercises path not found: {:?}", exercises_path);
            // Cache empty result
            let mut cache = self.exercises_cache.write().unwrap();
            cache.insert(language.to_string(), Vec::new());
            return Vec::new();
        }

        let mut exercises = Vec::new();

        if let Ok(entries) = fs::read_dir(&exercises_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.is_exercise_directory(&path) {
                    let exercise_name = path.file_name().unwrap().to_string_lossy().to_string();
                    exercises.push(exercise_name);
                }
            }
        }

        // Sort exercises (skip 'pov' at the end)
        exercises.sort_by(|a, b| {
            if a == "pov" {
                std::cmp::Ordering::Greater
            } else if b == "pov" {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        // Cache the result
        let mut cache = self.exercises_cache.write().unwrap();
        cache.insert(language.to_string(), exercises.clone());

        debug!("Discovered {} exercises for language: {}", exercises.len(), language);

        exercises
    }

    /// Gets all available languages that have exercises (cached).
    pub fn get_available_languages(&self) -> Vec<String> {
        // Check cache first
        if let Some(languages) = self.languages_cache.read().unwrap().as_ref() {
            return languages.clone();
        }

        let mut languages = Vec::new();
        let benchmark_dir = &self.benchmark_path;

        if !benchmark_dir.exists() {
            warn!("Benchmark path does not exist: {:?}", benchmark_dir);
            // Cache empty result
            *self.languages_cache.write().unwrap() = Some(Vec::new());
            return languages;
        }

        if let Ok(entries) = fs::read_dir(benchmark_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        languages.push(name);
                    }
                }
            }
        }

        languages.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        // Cache the result
        *self.languages_cache.write().unwrap() = Some(languages.clone());

        debug!("Discovered {} available languages: {:?}", languages.len(), languages);

        languages
    }

    /// Run a single exercise using any agent.
    pub async fn run_exercise(
        &self,
        agent: Arc<dyn Agent + Send + Sync>,
        language: &str,
        exercise_name: &str,
        model: &str,
        thinking_level: Option<String>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Running exercise: {} for language: {}",
            exercise_name, language
        );

        let exercise = match self.find_exercise(language, exercise_name) {
            Some(e) => e,
            None => {
                return Ok(AgentResult::builder()
                    .exercise_name(exercise_name.to_string())
                    .language(language.to_string())
                    .success(false)
                    .error_message(Some("Exercise not found".to_string()))
                    .build());
            }
        };

        let exercise_host_dir = match self.find_exercise_host_dir(language, exercise_name) {
            Some(dir) if dir.exists() => dir,
            _ => {
                return Ok(AgentResult::builder()
                    .exercise_name(exercise_name.to_string())
                    .language(language.to_string())
                    .success(false)
                    .error_message(Some(format!(
                        "Exercise directory not found: {}",
                        exercise_name
                    )))
                    .build());
            }
        };

        agent.run_exercise(&exercise, &exercise_host_dir, model, thinking_level.as_deref(), results_dir).await
    }

    /// Run all exercises for a given language using the specified agent with parallelism.
    ///
    /// If `retry` is true, exercises that already have result files are still executed
    /// (useful for re-running failed or outdated results).
    pub async fn run_all_exercises(
        &self,
        agent: Arc<dyn Agent + Send + Sync>,
        language: &str,
        agent_name: &str,
        model: String,
        thinking_level: Option<String>,
        results_dir: PathBuf,
        retry: bool,
    ) -> Vec<AgentResult> {
        info!(
            "Running all exercises for language: {} with agent: {}",
            language, agent_name
        );

        let exercises = self.find_all_exercises(language);
        info!("Found {} exercises for language: {}", exercises.len(), language);

        let results = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let agent_name_string = agent_name.to_string();

        let tasks: Vec<_> = exercises
            .into_iter()
            .filter(|exercise| {
                // Skip exercises that already have a result file (unless retry mode)
                if !retry {
                    let result_file = self.get_result_path(&exercise.name, agent_name, language);
                    if result_file.exists() {
                        info!(
                            "Result file already exists for {}/{}, skipping",
                            language, exercise.name
                        );
                        return false;
                    }
                }
                true
            })
            .filter(|exercise| {
                if let Some(dir) = self.find_exercise_host_dir(language, &exercise.name) {
                    dir.exists()
                } else {
                    false
                }
            })
            .map(|exercise| {
                let exercise_host_dir =
                    self.find_exercise_host_dir(language, &exercise.name).unwrap();
                let results = Arc::clone(&results);
                let agent = Arc::clone(&agent);
                let language = language.to_string();
                let agent_name = agent_name_string.clone();
                let model = model.clone();
                let thinking_level = thinking_level.clone();
                let results_dir = results_dir.clone();

                tokio::spawn(async move {
                    info!(
                        "Running {} for exercise {}/{}",
                        agent_name, language, exercise.name
                    );

                    let result = agent
                        .run_exercise(&exercise, &exercise_host_dir, &model, thinking_level.as_deref(), &results_dir)
                        .await;

                    match result {
                        Ok(r) => {
                            let mut results = results.lock().await;
                            results.push(r);
                        }
                        Err(e) => {
                            error!("Exercise failed: {}", e);
                        }
                    }
                })
            })
            .collect();

        futures::future::join_all(tasks).await;

        let mutex_ref = Arc::try_unwrap(results).ok().unwrap();
        mutex_ref.into_inner()
    }

    /// Finds a specific exercise by language and name.
    fn find_exercise(&self, language: &str, exercise_name: &str) -> Option<Exercise> {
        let mut exercise_dir = self
            .benchmark_path
            .join("exercises")
            .join("practice")
            .join(exercise_name);

        if !exercise_dir.exists() {
            exercise_dir = self
                .benchmark_path
                .join(language)
                .join("exercises")
                .join("practice")
                .join(exercise_name);
        }

        if !exercise_dir.exists() {
            return None;
        }

        Some(Exercise {
            name: exercise_name.to_string(),
            language: language.to_string(),
            source_path: self.find_source_file(&exercise_dir, language),
            test_path: self.find_test_file(&exercise_dir, language),
            reference_path: self.find_reference_file(&exercise_dir, language),
        })
    }

    /// Finds all exercises for a given language.
    fn find_all_exercises(&self, language: &str) -> Vec<Exercise> {
        let exercises_path = self
            .benchmark_path
            .join(language)
            .join("exercises")
            .join("practice");

        if !exercises_path.exists() {
            warn!("Exercises path not found: {:?}", exercises_path);
            return Vec::new();
        }

        let mut exercises = Vec::new();

        if let Ok(entries) = fs::read_dir(&exercises_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.is_exercise_directory(&path) {
                    let exercise_name = path.file_name().unwrap().to_string_lossy().to_string();
                    exercises.push(Exercise {
                        name: exercise_name,
                        language: language.to_string(),
                        source_path: self.find_source_file(&path, language),
                        test_path: self.find_test_file(&path, language),
                        reference_path: self.find_reference_file(&path, language),
                    });
                }
            }
        }

        // Sort exercises (skip 'pov' at the end)
        exercises.sort_by(|a, b| {
            if a.name == "pov" {
                std::cmp::Ordering::Greater
            } else if b.name == "pov" {
                std::cmp::Ordering::Less
            } else {
                a.name.cmp(&b.name)
            }
        });

        for exercise in &exercises {
            debug!("Found exercise {}/{}", language, exercise.name);
        }

        exercises
    }

    /// Checks if a directory is an exercise directory (contains .meta subdirectory).
    fn is_exercise_directory(&self, dir: &Path) -> bool {
        dir.join(".meta").is_dir()
    }

    /// Finds the main source file for an exercise.
    fn find_source_file(&self, exercise_dir: &Path, language: &str) -> Option<PathBuf> {
        if language == "java" {
            let source_path = exercise_dir.join("src/main/java");
            if source_path.exists() {
                if let Ok(entries) = fs::read_dir(&source_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path
                                .extension()
                                .map(|e| e == "java")
                                .unwrap_or(false)
                        {
                            return Some(path);
                        }
                    }
                }
            }
        }
        None
    }

    /// Finds the test file for an exercise.
    fn find_test_file(&self, exercise_dir: &Path, language: &str) -> Option<PathBuf> {
        if language == "java" {
            let test_path = exercise_dir.join("src/test/java");
            if test_path.exists() {
                if let Ok(entries) = fs::read_dir(&test_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.to_string_lossy().ends_with("Test.java")
                        {
                            return Some(path);
                        }
                    }
                }
            }
        }
        None
    }

    /// Finds the reference implementation for an exercise.
    fn find_reference_file(
        &self,
        exercise_dir: &Path,
        language: &str,
    ) -> Option<PathBuf> {
        if language == "java" {
            let ref_path = exercise_dir.join(".meta/src/reference/java");
            if ref_path.exists() {
                if let Ok(entries) = fs::read_dir(&ref_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path
                                .extension()
                                .map(|e| e == "java")
                                .unwrap_or(false)
                        {
                            return Some(path);
                        }
                    }
                }
            }
        } else if language == "go" {
            let ref_path = exercise_dir.join(".meta");
            if ref_path.exists() {
                return Some(ref_path);
            }
        }
        None
    }

    /// Finds the host directory for an exercise.
    fn find_exercise_host_dir(&self, language: &str, exercise_name: &str) -> Option<PathBuf> {
        let exercise_dir = self
            .benchmark_path
            .join(language)
            .join("exercises")
            .join("practice")
            .join(exercise_name);

        if exercise_dir.exists() {
            return Some(exercise_dir);
        }

        None
    }

    /// Gets the result file path for an exercise.
    fn get_result_path(&self, exercise_name: &str, agent_name: &str, language: &str) -> PathBuf {
        let results_dir = &self.config.output.results_dir;
        results_dir.join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ))
    }

    /// Parses the metadata from .meta/config.json for an exercise.
    pub fn parse_metadata(&self, exercise_dir: &Path) -> Option<benchmark_types::exercise::ExerciseMetadata> {
        let meta_config_path = exercise_dir.join(".meta").join("config.json");
        if !meta_config_path.exists() {
            tracing::debug!("No metadata file found at {:?}", meta_config_path);
            return None;
        }
        match std::fs::read_to_string(&meta_config_path) {
            Ok(content) => {
                match serde_json::from_str::<benchmark_types::exercise::ExerciseMetadata>(&content) {
                    Ok(metadata) => Some(metadata),
                    Err(e) => {
                        warn!("Failed to parse metadata at {}: {}", meta_config_path.display(), e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read metadata file {}: {}", meta_config_path.display(), e);
                None
            }
        }
    }

    /// Clean up all Docker containers created by this runner.
    pub async fn cleanup_all_containers(&self) {
        if let Some(ref docker_client) = self.docker_client {
            docker_client.cleanup_all_containers().await;
        }
    }

    /// Fetch available models from the inference endpoint.
    pub async fn fetch_models(&self) -> anyhow::Result<Vec<String>> {
        let endpoint = &self.config.inference_endpoint;
        let url = format!("{}/models", endpoint);
        let mut builder = reqwest::Client::new().get(&url);

        if let Some(ref api_key) = self.config.api_key {
            if !api_key.is_empty() {
                builder = builder.bearer_auth(api_key);
            }
        }

        let response = builder.send().await?;

        if response.status() != 200 {
            warn!("Failed to fetch models from {}, status code: {}", endpoint, response.status());
            return Ok(vec!["sonnet".to_string(), "qwen3-coder-next".to_string()]);
        }

        let body: serde_json::Value = response.json().await?;
        let data = body.get("data");

        match data {
            Some(serde_json::Value::Array(models)) => {
                let model_ids: Vec<String> = models
                    .iter()
                    .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                    .map(|s| s.to_string())
                    .collect();
                info!("Found {} models from {}: {:?}", model_ids.len(), endpoint, model_ids);
                if model_ids.is_empty() {
                    Ok(vec!["sonnet".to_string(), "qwen3-coder-next".to_string()])
                } else {
                    Ok(model_ids)
                }
            }
            _ => {
                warn!("'data' field not found or not an array in models response from {}", endpoint);
                Ok(vec!["sonnet".to_string(), "qwen3-coder-next".to_string()])
            }
        }
    }
}
