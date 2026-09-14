use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::config::Config;
use benchmark_types::exercise::{Exercise, ExerciseMetadata};
use benchmark_types::ExerciseSource;
use benchmark_exercises::EmbeddedSource;
use crate::docker::DockerClient;
use benchmark_types::util::recover_poisoned;

/// Exercises that are deprecated/removed upstream and should not be discovered or run.
/// E.g. Exercism deprecated the Go `counter` exercise (problem-specifications#80) because
/// it is not compatible with a standard solve-the-exercise prompt.
const DEPRECATED_EXERCISES: &[&str] = &["counter"];

#[derive(Clone)]
pub struct ExerciseRunner {
    config: Arc<Config>,
    /// The exercise suite. Exercises are read from here — no host checkout involved.
    source: Arc<dyn ExerciseSource>,
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
    /// Creates a runner backed by the compiled-in exercise bundle.
    pub fn new(config: Arc<Config>) -> Self {
        Self::with_source(config, Arc::new(EmbeddedSource::new()))
    }

    /// Create with a DockerClient reference for setRunParams.
    pub fn new_with_docker(config: Arc<Config>, docker_client: Arc<DockerClient>) -> Self {
        let mut runner = Self::with_source(config, Arc::new(EmbeddedSource::new()));
        runner.docker_client = Some(docker_client);
        runner
    }

    /// Creates a runner over an explicit exercise source (used by tests).
    pub fn with_source(config: Arc<Config>, source: Arc<dyn ExerciseSource>) -> Self {
        Self {
            config,
            source,
            docker_client: None,
            run_agent_name: None,
            run_model: None,
            run_languages: None,
            exercises_cache: Arc::new(RwLock::new(HashMap::new())),
            languages_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// The exercise source backing this runner.
    pub fn source(&self) -> &Arc<dyn ExerciseSource> {
        &self.source
    }

    /// Sets run parameters for result directory computation.
    pub fn set_run_params(
        &mut self,
        agent_name: &str,
        model: &str,
        languages: &[String],
    ) {
        self.run_agent_name = Some(agent_name.to_string());
        self.run_model = Some(if model.is_empty() { "default".to_string() } else { model.to_string() });
        self.run_languages = Some(languages.to_vec());
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
            let cache = recover_poisoned(self.exercises_cache.read());
            if let Some(exercises) = cache.get(language) {
                return exercises.clone();
            }
        }

        let mut exercises: Vec<String> = self
            .source
            .exercise_names(language)
            .into_iter()
            .filter(|name| !DEPRECATED_EXERCISES.contains(&name.as_str()))
            .collect();

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
        let mut cache = recover_poisoned(self.exercises_cache.write());
        cache.insert(language.to_string(), exercises.clone());

        debug!("Discovered {} exercises for language: {}", exercises.len(), language);

        exercises
    }

    /// Gets all available languages that have exercises (cached).
    pub fn get_available_languages(&self) -> Vec<String> {
        // Check cache first
        if let Some(languages) = recover_poisoned(self.languages_cache.read()).as_ref() {
            return languages.clone();
        }

        let mut languages = self.source.languages();
        languages.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        // Cache the result
        *recover_poisoned(self.languages_cache.write()) = Some(languages.clone());

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
        self.run_exercise_with_timeout(agent, language, exercise_name, model, thinking_level, results_dir, None).await
    }

    /// Run a single exercise using any agent, with an optional Docker container
    /// timeout override (in seconds). When `Some(secs)`, the container timeout is
    /// capped at that value; when `None`, the default from config is used.
    pub async fn run_exercise_with_timeout(
        &self,
        agent: Arc<dyn Agent + Send + Sync>,
        language: &str,
        exercise_name: &str,
        model: &str,
        thinking_level: Option<String>,
        results_dir: &Path,
        timeout_override_secs: Option<u64>,
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

        agent
            .run_exercise_with_timeout(
                &exercise,
                self.source.as_ref(),
                model,
                thinking_level.as_deref(),
                results_dir,
                timeout_override_secs,
            )
            .await
    }

    /// Run all exercises for a given language using the specified agent with parallelism.
    ///
    /// Uses `config.parallelism` to cap concurrent exercise execution. Each exercise
    /// runs as a spawned task, but tasks are spawned in buffered batches so at most
    /// `parallelism` tasks are active at once.
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
        use futures::stream::{self, StreamExt};

        info!(
            "Running all exercises for language: {} with agent: {}",
            language, agent_name
        );

        let mut exercises = self.find_all_exercises(language);
        let parallelism = self.config.parallelism.max(1) as usize;
        info!(
            "Found {} exercises for language: {} (parallelism={})",
            exercises.len(),
            language,
            parallelism
        );

        let agent_name_string = agent_name.to_string();

        // In retry mode, sort by previous duration descending (slowest first)
        // so the longest-running exercises start earliest, maximizing pipeline utilization.
        if retry && !exercises.is_empty() {
            let agent = &agent_name_string;
            let lang = language;
            let mdl = &model;
            let dir = &results_dir;
            // Pre-load all durations to avoid repeated file I/O per comparison
            let durations: std::collections::HashMap<String, u64> = exercises
                .iter()
                .map(|ex| {
                    let dur = Self::load_duration_ms(dir, agent, mdl, lang, &ex.name);
                    (ex.name.clone(), dur)
                })
                .collect();
            exercises.sort_by(|a, b| {
                let a_dur = durations.get(&a.name).copied().unwrap_or(0);
                let b_dur = durations.get(&b.name).copied().unwrap_or(0);
                b_dur.cmp(&a_dur)
            });
            info!("Retry mode: sorted {} exercises by previous duration (slowest first)", exercises.len());
        }

        // Build an async stream of exercise futures, buffered to `parallelism` concurrency
        let futures_stream = stream::iter(exercises)
            .filter(|exercise| {
                // Skip exercises that already have a successful result (unless retry mode)
                let keep = if !retry {
                    let result_file = self.get_result_path(&exercise.name, agent_name, language, &model);
                    if result_file.exists() {
                        // Only skip if the existing result was successful
                        let is_success = std::fs::read_to_string(&result_file)
                            .ok()
                            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
                            .and_then(|v| v.get("success")?.as_bool())
                            .unwrap_or(false);
                        if is_success {
                            info!(
                                "Successful result already exists for {}/{}, skipping",
                                language, exercise.name
                            );
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                };
                async move { keep }
            })
            .map(|exercise| {
                let source = Arc::clone(&self.source);
                let agent = Arc::clone(&agent);
                let language = language.to_string();
                let agent_name = agent_name_string.clone();
                let model = model.clone();
                let thinking_level = thinking_level.clone();
                let results_dir = results_dir.clone();

                async move {
                    info!(
                        "Running {} for exercise {}/{}",
                        agent_name, language, exercise.name
                    );

                    let result = agent
                        .run_exercise(
                            &exercise,
                            source.as_ref(),
                            &model,
                            thinking_level.as_deref(),
                            &results_dir,
                        )
                        .await;

                    match result {
                        Ok(r) => {
                            info!(
                                "Completed {}/{} (success={})",
                                language, exercise.name, r.success
                            );
                            Some(r)
                        }
                        Err(e) => {
                            error!("Exercise failed: {}", e);
                            None
                        }
                    }
                }
            })
            .buffer_unordered(parallelism);

        let results: Vec<AgentResult> = futures_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect();

        info!(
            "All exercises completed for language {}: {} results",
            language,
            results.len()
        );

        results
    }

    /// Builds an Exercise from the bundled files, parsing .meta/config.json for metadata.
    fn build_exercise(&self, name: &str, language: &str) -> Exercise {
        let files = self.source.list_files(language, name);
        let metadata = self.parse_metadata(language, name);
        let (solution_files, example_files, test_files) = Self::resolve_metadata_paths(&metadata);

        Exercise {
            name: name.to_string(),
            language: language.to_string(),
            source_file: Self::find_source_file(&files, language),
            test_file: Self::find_test_file(&files, language),
            reference_dir: Self::find_reference_dir(&files, language),
            metadata,
            example_files,
            solution_files,
            test_files,
        }
    }

    /// Extracts the relative file lists from metadata (config.json → files.*).
    fn resolve_metadata_paths(metadata: &Option<ExerciseMetadata>) -> (Vec<String>, Vec<String>, Vec<String>) {
        if let Some(meta) = metadata {
            if let Some(ref files) = meta.files {
                return (
                    files.solution.clone().unwrap_or_default(),
                    files.example.clone().unwrap_or_default(),
                    files.test.clone().unwrap_or_default(),
                );
            }
        }
        (Vec::new(), Vec::new(), Vec::new())
    }

    /// Finds a specific exercise by language and name.
    fn find_exercise(&self, language: &str, exercise_name: &str) -> Option<Exercise> {
        if DEPRECATED_EXERCISES.contains(&exercise_name) {
            debug!("Skipping deprecated exercise: {}/{}", language, exercise_name);
            return None;
        }
        if !self.source.has_exercise(language, exercise_name) {
            return None;
        }

        Some(self.build_exercise(exercise_name, language))
    }

    /// Finds all exercises for a given language.
    fn find_all_exercises(&self, language: &str) -> Vec<Exercise> {
        let mut exercises: Vec<Exercise> = self
            .source
            .exercise_names(language)
            .into_iter()
            .filter(|name| !DEPRECATED_EXERCISES.contains(&name.as_str()))
            .map(|name| {
                debug!("Found exercise {}/{}", language, name);
                self.build_exercise(&name, language)
            })
            .collect();

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

        exercises
    }

    /// Finds the Java main source file, relative to the exercise root.
    fn find_source_file(files: &[String], language: &str) -> Option<String> {
        if language != "java" {
            return None;
        }
        files
            .iter()
            .find(|f| {
                f.strip_prefix("src/main/java/")
                    .map(|rest| !rest.contains('/') && rest.ends_with(".java"))
                    .unwrap_or(false)
            })
            .cloned()
    }

    /// Finds the Java test file, relative to the exercise root.
    fn find_test_file(files: &[String], language: &str) -> Option<String> {
        if language != "java" {
            return None;
        }
        files
            .iter()
            .find(|f| {
                f.strip_prefix("src/test/java/")
                    .map(|rest| !rest.contains('/') && rest.ends_with("Test.java"))
                    .unwrap_or(false)
            })
            .cloned()
    }

    /// Finds the reference implementation directory, relative to the exercise root.
    /// Java keeps reference sources under `.meta/src/reference/java`; all other
    /// languages keep them directly in `.meta/`.
    fn find_reference_dir(files: &[String], language: &str) -> Option<String> {
        let dir = if language == "java" {
            ".meta/src/reference/java"
        } else {
            ".meta"
        };
        let prefix = format!("{dir}/");
        files
            .iter()
            .any(|f| f.starts_with(&prefix))
            .then(|| dir.to_string())
    }

    /// Gets the result file path for an exercise.
    fn get_result_path(&self, exercise_name: &str, agent_name: &str, language: &str, model: &str) -> PathBuf {
        let results_dir = &self.config.output.results_dir;
        let subdir = format!("{}-{}", agent_name, model);
        results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ))
    }

    /// Parses the metadata from .meta/config.json for an exercise.
    pub fn parse_metadata(&self, language: &str, exercise: &str) -> Option<ExerciseMetadata> {
        let bytes = self.source.read(language, exercise, ".meta/config.json")?;
        match serde_json::from_slice::<ExerciseMetadata>(&bytes) {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                warn!("Failed to parse metadata for {}/{}: {}", language, exercise, e);
                None
            }
        }
    }

    /// Load the duration (in milliseconds) from a previous result file.
    /// Returns 0 if no previous result exists or the file cannot be parsed.
    fn load_duration_ms(
        results_dir: &Path,
        agent_name: &str,
        model: &str,
        language: &str,
        exercise: &str,
    ) -> u64 {
        let subdir = format!("{}-{}", agent_name, model);
        let path = results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise
        ));
        if !path.exists() {
            return 0;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(dur) = value.get("duration").and_then(|v| v.as_f64()) {
                        (dur * 1000.0) as u64
                    } else if let Some(dur) = value.get("duration_ms").and_then(|v| v.as_u64()) {
                        dur
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            Err(_) => 0,
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
        // `None` means no endpoint was configured *and* the startup probe found nothing
        // listening, so there is nothing to ask. The built-in list keeps the dashboard
        // usable rather than empty.
        let Some(endpoint) = self.config.inference_endpoint.as_deref() else {
            warn!(
                "No inference endpoint configured and none detected on the well-known local \
                 ports; set `inference_endpoint` in config.yaml to list the models your server \
                 offers"
            );
            return Ok(builtin_model_list());
        };
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
            return Ok(builtin_model_list());
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
                    Ok(builtin_model_list())
                } else {
                    Ok(model_ids)
                }
            }
            _ => {
                warn!("'data' field not found or not an array in models response from {}", endpoint);
                Ok(builtin_model_list())
            }
        }
    }
}

/// The model list shown when the endpoint cannot be reached, so the dashboard still has
/// something to offer on a machine with no model server running.
fn builtin_model_list() -> Vec<String> {
    vec!["sonnet".to_string(), "qwen3-coder-next".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner() -> ExerciseRunner {
        ExerciseRunner::new(Arc::new(Config::default()))
    }

    #[test]
    fn languages_come_from_the_bundle() {
        assert_eq!(
            runner().get_available_languages(),
            vec!["cpp", "go", "java", "javascript", "python", "rust"]
        );
    }

    #[test]
    fn exercises_are_discovered_per_language() {
        let exercises = runner().get_exercises_for_language("rust");
        assert_eq!(exercises.len(), 30);
        assert!(exercises.contains(&"alphametics".to_string()));
    }

    #[test]
    fn java_exercise_carries_relative_paths_only() {
        let exercise = runner().find_exercise("java", "series").expect("series exists");

        assert_eq!(exercise.source_file.as_deref(), Some("src/main/java/Series.java"));
        assert_eq!(exercise.test_file.as_deref(), Some("src/test/java/SeriesTest.java"));
        assert_eq!(exercise.reference_dir.as_deref(), Some(".meta/src/reference/java"));
        assert_eq!(exercise.solution_files, vec!["src/main/java/Series.java"]);
        assert_eq!(exercise.test_files, vec!["src/test/java/SeriesTest.java"]);
        assert_eq!(exercise.example_files, vec![".meta/src/reference/java/Series.java"]);

        // The model must never contain absolute host paths.
        for path in exercise
            .example_files
            .iter()
            .chain(&exercise.solution_files)
            .chain(&exercise.test_files)
        {
            assert!(!path.starts_with('/'), "absolute path leaked into Exercise: {path}");
        }
    }

    #[test]
    fn rust_exercise_resolves_metadata_and_reference_dir() {
        let exercise = runner().find_exercise("rust", "alphametics").expect("alphametics exists");

        assert!(exercise.test_files.contains(&"tests/alphametics.rs".to_string()));
        assert!(exercise.example_files.contains(&".meta/example.rs".to_string()));
        assert!(exercise.solution_files.contains(&"src/lib.rs".to_string()));
        assert_eq!(exercise.reference_dir.as_deref(), Some(".meta"));
        // Java-only fields stay empty for other languages.
        assert!(exercise.source_file.is_none());
        assert!(exercise.test_file.is_none());
    }

    #[test]
    fn deprecated_and_unknown_exercises_are_not_found() {
        let runner = runner();
        assert!(runner.find_exercise("go", "counter").is_none());
        assert!(runner.find_exercise("rust", "not-a-real-exercise").is_none());
    }
}
