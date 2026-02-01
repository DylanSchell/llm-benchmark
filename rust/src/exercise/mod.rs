use crate::agent::{Agent, AgentResult};
use crate::config::Config;
use crate::model::Exercise;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub struct ExerciseRunner {
    config: Arc<Config>,
    benchmark_path: PathBuf,
}

impl ExerciseRunner {
    pub fn new(config: Arc<Config>) -> Self {
        let benchmark_path = config.benchmark_path.clone();
        Self { config, benchmark_path }
    }

    /// Run a single exercise using any agent
    pub async fn run_exercise(
        &self,
        agent: Arc<Mutex<dyn Agent + Send + Sync>>,
        language: &str,
        exercise_name: &str,
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
                    .error_message(Some(format!("Exercise directory not found: {}", exercise_name)))
                    .build());
            }
        };

        let agent_guard = agent.lock().await;
        agent_guard.run_exercise(&exercise, &exercise_host_dir).await
    }

    /// Run all exercises for a given language using the specified agent with parallelism
    pub async fn run_all_exercises(
        &self,
        agent: Arc<Mutex<dyn Agent + Send + Sync>>,
        language: &str,
        agent_name: &str,
    ) -> Vec<AgentResult> {
        info!("Running all exercises for language: {} with agent: {}", language, agent_name);

        let exercises = self.find_all_exercises(language);
        info!("Found {} exercises for language: {}", exercises.len(), language);

        let results = Arc::new(Mutex::new(Vec::new()));
        let agent_name_string = agent_name.to_string();

        let tasks: Vec<_> = exercises
            .into_iter()
            .filter(|exercise| {
                let result_file = self.get_result_path(&exercise.name, agent_name, language);
                if result_file.exists() {
                    info!(
                        "Result file already exists for {}/{}, skipping",
                        language, exercise.name
                    );
                    return false;
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
                let exercise_host_dir = self.find_exercise_host_dir(language, &exercise.name).unwrap();
                let results = Arc::clone(&results);
                let agent = Arc::clone(&agent);
                let language = language.to_string();
                let agent_name = agent_name_string.clone();

                tokio::spawn(async move {
                    info!(
                        "Running {} for exercise {}/{}",
                        agent_name, language, exercise.name
                    );

                    let agent_guard = agent.lock().await;
                    let result = agent_guard.run_exercise(&exercise, &exercise_host_dir).await;
                    drop(agent_guard);

                    match result {
                        Ok(r) => {
                            let mut results = results.lock().await;
                            results.push(r);
                        }
                        Err(e) => {
                            tracing::error!("Exercise failed: {}", e);
                        }
                    }
                })
            })
            .collect();

        futures::future::join_all(tasks).await;

        let mutex_ref = Arc::try_unwrap(results).ok().unwrap();
        mutex_ref.into_inner()
    }

    /// Finds a specific exercise by language and name
    fn find_exercise(&self, language: &str, exercise_name: &str) -> Option<Exercise> {
        let mut exercise_dir = self.benchmark_path
            .join("exercises")
            .join("practice")
            .join(exercise_name);

        if !exercise_dir.exists() {
            exercise_dir = self.benchmark_path
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

    /// Finds all exercises for a given language
    fn find_all_exercises(&self, language: &str) -> Vec<Exercise> {
        let exercises_path = self.benchmark_path
            .join(language)
            .join("exercises")
            .join("practice");

        if !exercises_path.exists() {
            tracing::warn!("Exercises path not found: {:?}", exercises_path);
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
            info!("Found exercise {}/{}", language, exercise.name);
        }

        exercises
    }

    /// Checks if a directory is an exercise directory
    fn is_exercise_directory(&self, dir: &Path) -> bool {
        dir.join("build.gradle").exists()
            || dir.join("go.mod").exists()
            || dir.join("pom.xml").exists()
            || dir.join("package.json").exists()
            || dir.join("Cargo.toml").exists()
            || dir.join("pyproject.toml").exists()
            || dir.join("setup.py").exists()
            || dir.join("Gemfile").exists()
            || dir.join("CMakeLists.txt").exists()
    }

    /// Finds the main source file for an exercise
    fn find_source_file(&self, exercise_dir: &Path, language: &str) -> Option<PathBuf> {
        if language == "java" {
            let source_path = exercise_dir.join("src/main/java");
            if source_path.exists() {
                if let Ok(entries) = fs::read_dir(&source_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().map(|e| e == "java") == Some(true) {
                            return Some(path);
                        }
                    }
                }
            }
        }
        None
    }

    /// Finds the test file for an exercise
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

    /// Finds the reference implementation for an exercise
    fn find_reference_file(&self, exercise_dir: &Path, language: &str) -> Option<PathBuf> {
        if language == "java" {
            let ref_path = exercise_dir.join(".meta/src/reference/java");
            if ref_path.exists() {
                if let Ok(entries) = fs::read_dir(&ref_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().map(|e| e == "java") == Some(true)
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

    /// Finds the host directory for an exercise
    fn find_exercise_host_dir(&self, language: &str, exercise_name: &str) -> Option<PathBuf> {
        let exercise_dir = self.benchmark_path
            .join(language)
            .join("exercises")
            .join("practice")
            .join(exercise_name);

        if exercise_dir.exists() {
            return Some(exercise_dir);
        }

        None
    }

    /// Gets the result file path for an exercise
    fn get_result_path(&self, exercise_name: &str, agent_name: &str, language: &str) -> PathBuf {
        let results_dir = &self.config.output.results_dir;
        format!("{}-{}-{}.json", exercise_name, agent_name, language)
            .parse()
            .unwrap_or_else(|_| results_dir.join(format!("{}-{}-{}.json", exercise_name, agent_name, language)))
    }
}
