//! Advent of Code 2015 runner.
//!
//! An AoC run is different from an Exercism run: there is no test suite, no
//! `.meta` reference implementation, and no fixed language. The runner
//! materializes the puzzle description and a generated input into the container,
//! tells the agent how to call a validator endpoint, then reads the agent's
//! answers and validates them against the vendored solver.
//!
//! The agent never sees the generator or the solver — only the description, the
//! input, and the validator URL.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::aoc::AocDay;
use benchmark_types::Category;

/// Name of the answer file the agent writes into `/workspace` (one line per part).
pub const ANSWER_FILE: &str = "answer.txt";

/// A runner for Advent of Code 2015 puzzle days.
#[derive(Clone)]
pub struct AocRunner {
    /// The seed user for all days run by this runner (drives input generation).
    pub user: String,
    /// Base results directory.
    pub results_dir: PathBuf,
}

impl AocRunner {
    /// Create a runner for the given seed user.
    pub fn new(user: impl Into<String>, results_dir: impl Into<PathBuf>) -> Self {
        Self {
            user: user.into(),
            results_dir: results_dir.into(),
        }
    }

    /// All puzzle days, `1..=25`.
    pub fn days(&self) -> Vec<u32> {
        benchmark_aoc2015::days()
    }

    /// Materialize the puzzle description and a generated input into `dest`.
    ///
    /// Writes `problem.md` (both parts) and `input.txt`. Returns the generated
    /// input for reference.
    pub fn materialize_day(
        &self,
        day: &AocDay,
        dest: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let description = benchmark_aoc2015::description(day.day).ok_or_else(|| {
            format!("no description for day {}", day.day)
        })?;
        let input = benchmark_aoc2015::generate(&day.user, day.year, day.day);

        fs::create_dir_all(dest)?;
        fs::write(dest.join("problem.md"), description)?;
        fs::write(dest.join("input.txt"), &input)?;
        info!(
            "Materialized AoC day {} ({} input bytes) into {}",
            day.day,
            input.len(),
            dest.display()
        );
        Ok(input)
    }

    /// Build the prompt given to the agent for one day.
    pub fn build_prompt(&self, day: &AocDay, validator_url: &str) -> String {
        let mut prompt = String::new();
        prompt.push_str("You are solving an Advent of Code 2015 puzzle.\n\n");
        prompt.push_str("The puzzle description is in /workspace/problem.md (it contains part 1 and part 2).\n");
        prompt.push_str("The input for the puzzle is in /workspace/input.txt.\n\n");
        prompt.push_str("Read the description fully before writing any code.\n");
        prompt.push_str("Write a program (in any language available in this container) that reads /workspace/input.txt and computes the answer for each part.\n");
        prompt.push_str("Run it, and get the answer for part 1, then part 2.\n\n");
        prompt.push_str("To validate your answer, call the validator endpoint with HTTP POST:\n");
        prompt.push_str(&format!(
            "  curl -sS -X POST {validator_url} -H 'Content-Type: application/json' -d '{{\"user\": \"{}\", \"year\": {}, \"day\": {}, \"part\": 1, \"answer\": \"<your part-1 answer>\"}}'\n",
            day.user, day.year, day.day
        ));
        prompt.push_str("The response tells you whether the answer is correct and which part it matches.\n");
        prompt.push_str("Validate part 1 first, then part 2. Iterate until both are correct.\n\n");
        prompt.push_str("When you are satisfied, write your final answers to /workspace/answer.txt, one answer per line:\n");
        prompt.push_str("  line 1 = part 1 answer\n");
        prompt.push_str("  line 2 = part 2 answer\n");
        prompt.push_str("(For a day with only one part, write just the part 1 answer on line 1.)\n\n");
        prompt.push_str("Do not modify input.txt. Do not touch any file other than your solution and answer.txt.\n");
        prompt
    }

    /// Read the agent's answers from `/workspace/answer.txt`.
    pub fn read_answers(&self, work_dir: &Path) -> Result<(String, String), String> {
        let path = work_dir.join(ANSWER_FILE);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("agent did not write {ANSWER_FILE}: {e}"))?;
        let mut lines = content.lines().filter(|l| !l.trim().is_empty());
        let part1 = lines
            .next()
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        let part2 = lines
            .next()
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        Ok((part1, part2))
    }

    /// Validate an agent's answers against the vendored solver.
    ///
    /// Returns `(part1_ok, part2_ok)`. For day 25 (no part 2) `part2_ok` is `true`
    /// when part 1 is correct.
    pub fn validate_answers(&self, day: &AocDay, part1: &str, part2: &str) -> (bool, bool) {
        let (expected1, expected2) = benchmark_aoc2015::solve(&day.user, day.year, day.day);
        let p1_ok = part1 == expected1;
        let p2_ok = if expected2.is_empty() {
            p1_ok
        } else {
            part2 == expected2
        };
        (p1_ok, p2_ok)
    }

    /// Run one puzzle day with the given agent and persist the result.
    ///
    /// `validator_url` is the base URL of the validator endpoint (e.g.
    /// `http://host.docker.internal:8081/api/aoc/validate`), injected into the
    /// container env and the prompt so the agent knows how to call it.
    pub async fn run_day(
        &self,
        agent: Arc<dyn Agent + Send + Sync>,
        day: &AocDay,
        validator_url: &str,
        model: &str,
        thinking_level: Option<&str>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Running AoC day {} for agent {} (model={})",
            day.day,
            agent.get_name(),
            model
        );

        let work_dir = Self::create_temp_work_dir(day)?;
        info!("AoC temp work dir: {:?}", work_dir.display());

        // Materialize the description + generated input.
        self.materialize_day(day, &work_dir)?;

        // Run the agent (it writes answer.txt into the container /workspace).
        let result = agent
            .run_aoc(day, &work_dir, validator_url, model, thinking_level, &self.results_dir)
            .await?;

        // Read + validate the agent's answers.
        let (part1, part2) = match self.read_answers(&work_dir) {
            Ok(a) => a,
            Err(e) => {
                error!("{e}");
                (String::new(), String::new())
            }
        };
        let (p1_ok, p2_ok) = self.validate_answers(day, &part1, &part2);

        let success = p1_ok && p2_ok;
        let output = format!(
            "part1: {} -> {}\npart2: {} -> {}\n",
            if part1.is_empty() { "<empty>" } else { &part1 },
            if p1_ok { "correct" } else { "incorrect" },
            if part2.is_empty() { "<empty>" } else { &part2 },
            if p2_ok { "correct" } else { "incorrect" }
        );

        let mut final_result = result;
        final_result.category = Category::Aoc2015;
        final_result.success = success;
        final_result.output = if final_result.output.is_empty() {
            output
        } else {
            format!("{}{}", final_result.output, output)
        };
        final_result.error_message = if success {
            None
        } else {
            Some(format!("AoC day {}: one or both answers incorrect", day.day))
        };

        // Cleanup.
        let _ = fs::remove_dir_all(&work_dir);

        info!(
            "AoC day {} complete: success={} (part1={}, part2={})",
            day.day, success, p1_ok, p2_ok
        );
        Ok(final_result)
    }

    /// Create a temporary work directory for one AoC day under `.benchmark-temp/`.
    pub fn create_temp_work_dir(day: &AocDay) -> Result<PathBuf, std::io::Error> {
        let base_dir = std::env::current_dir()?;
        let base_temp_dir = base_dir.join(".benchmark-temp");
        fs::create_dir_all(&base_temp_dir)?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let day_temp_dir = base_temp_dir
            .join(format!("aoc-{}", day.exercise_name()))
            .join(ts.to_string());
        fs::create_dir_all(&day_temp_dir)?;
        debug!("Created AoC temp work dir: {:?}", day_temp_dir);
        Ok(day_temp_dir)
    }

    /// Run all days sequentially, one by one.
    pub async fn run_all_days(
        &self,
        agent: Arc<dyn Agent + Send + Sync>,
        validator_url: &str,
        model: &str,
        thinking_level: Option<&str>,
        days: Option<&[u32]>,
    ) -> Vec<AgentResult> {
        let mut results = Vec::new();
        let days: Vec<u32> = days
            .map(|d| d.to_vec())
            .unwrap_or_else(|| self.days());
        for day_num in days {
            let day = AocDay::new(day_num, self.user.clone());
            match self.run_day(agent.clone(), &day, validator_url, model, thinking_level).await {
                Ok(r) => results.push(r),
                Err(e) => {
                    error!("AoC day {} failed: {}", day_num, e);
                    results.push(
                        AgentResult::builder()
                            .category(Category::Aoc2015)
                            .exercise_name(day.exercise_name())
                            .language(day.language().to_string())
                            .success(false)
                            .error_message(Some(format!("AoC day {day_num} failed: {e}")))
                            .build(),
                    );
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: u32) -> AocDay {
        AocDay::new(n, "benchmark")
    }

    #[test]
    fn materialize_writes_description_and_input() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let dir = tempfile::tempdir().unwrap();
        let input = runner.materialize_day(&day(1), dir.path()).unwrap();

        assert!(!input.is_empty());
        let desc = fs::read_to_string(dir.path().join("problem.md")).unwrap();
        assert!(desc.contains("Part Two"), "day 1 description missing part 2");
        let written = fs::read_to_string(dir.path().join("input.txt")).unwrap();
        assert_eq!(written, input);
    }

    #[test]
    fn build_prompt_includes_validator_and_answer_file() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let prompt = runner.build_prompt(&day(1), "http://host.docker.internal:8081/api/aoc/validate");
        assert!(prompt.contains("problem.md"));
        assert!(prompt.contains("input.txt"));
        assert!(prompt.contains("host.docker.internal:8081/api/aoc/validate"));
        assert!(prompt.contains("answer.txt"));
    }

    #[test]
    fn validate_accepts_correct_part1() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let (e1, _e2) = benchmark_aoc2015::solve("benchmark", 2015, 1);
        let (p1, p2) = runner.validate_answers(&day(1), &e1, "");
        assert!(p1);
        assert!(!p2, "empty part 2 must not validate for a two-part day");
    }

    #[test]
    fn validate_rejects_wrong_answer() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let (p1, _p2) = runner.validate_answers(&day(1), "definitely-wrong", "");
        assert!(!p1);
    }

    #[test]
    fn validate_day25_treats_part2_as_part1() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let (e1, _) = benchmark_aoc2015::solve("benchmark", 2015, 25);
        let (p1, p2) = runner.validate_answers(&day(25), &e1, "");
        assert!(p1);
        assert!(p2, "day 25 part 2 should be satisfied by a correct part 1");
    }

    #[test]
    fn read_answers_parses_two_lines() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(ANSWER_FILE), "280\n5\n").unwrap();
        let (p1, p2) = runner.read_answers(dir.path()).unwrap();
        assert_eq!(p1, "280");
        assert_eq!(p2, "5");
    }

    #[test]
    fn read_answers_missing_file_is_error() {
        let runner = AocRunner::new("benchmark", std::env::temp_dir());
        let dir = tempfile::tempdir().unwrap();
        assert!(runner.read_answers(dir.path()).is_err());
    }

    /// End-to-end Docker integration: the reference agent solves day 1 through
    /// `run_day`. Network-gated — the reference agent ignores the validator and
    /// writes the ground-truth answers, so no model server is required, only the
    /// runner image. Run with `cargo test -p benchmark-core -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn reference_agent_completes_day1_in_docker() {
        use crate::agent::ReferenceAgent;
        use crate::docker::{DockerClient, DockerConfig};

        let config = benchmark_types::config::Config::default();
        let docker = DockerClient::new(DockerConfig::from(&config.docker));
        let agent = Arc::new(ReferenceAgent::new(docker));
        let runner = AocRunner::new("benchmark", std::env::temp_dir());

        let day = AocDay::new(1, "benchmark");
        let result = runner
            .run_day(agent, &day, "http://host.docker.internal:8081/api/aoc/validate", "reference", None)
            .await
            .expect("reference agent should complete day 1 in Docker");

        assert_eq!(result.category, Category::Aoc2015);
        assert!(result.success, "reference agent should solve day 1: {:?}", result.error_message);
        assert!(result.output.contains("part1"), "output should report part1/part2");
        assert!(result.output.contains("correct"), "both parts should validate correct");
    }
}
