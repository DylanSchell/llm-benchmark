//! Advent of Code 2015 answer validator.
//!
//! Serves `POST /api/aoc/validate`. It regenerates the exact input the agent was
//! given (deterministic from `(user, year, day)`), solves it with the vendored
//! solver, and reports whether the submitted answer matches part 1 or part 2.
//!
//! The agent never sees this module or the solver — it only knows the endpoint
//! URL from its prompt / `AOC_VALIDATOR_URL` env var.

use serde::{Deserialize, Serialize};

/// Request body for `POST /api/aoc/validate`.
#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    /// The seed that generated the input.
    pub user: String,
    /// The puzzle year (always 2015).
    pub year: u32,
    /// The puzzle day (1..=25).
    pub day: u32,
    /// The submitted answer.
    pub answer: String,
    /// Which part the agent believes it is answering (1 or 2). Optional; used
    /// only to bias the response, not to gate correctness.
    #[serde(default)]
    pub part: Option<u32>,
}

/// Response body.
#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    /// True if the answer matched part 1 or part 2.
    pub correct: bool,
    /// Which part the answer matched: `Some(1)`, `Some(2)`, or `None`.
    pub matched_part: Option<u32>,
    /// Human-readable detail (not the correct answer).
    pub message: String,
}

impl ValidateResponse {
    fn correct(part: u32) -> Self {
        Self {
            correct: true,
            matched_part: Some(part),
            message: format!("correct (part {part})"),
        }
    }

    fn incorrect() -> Self {
        Self {
            correct: false,
            matched_part: None,
            message: "incorrect — the answer matches neither part".to_string(),
        }
    }
}

/// Validate a submitted answer against the vendored solver.
///
/// Returns `None` when the request is invalid (out-of-range day, etc.).
pub fn validate(req: &ValidateRequest) -> Option<ValidateResponse> {
    if req.year != benchmark_types::aoc::AOC_YEAR {
        return None;
    }
    if !(1..=25).contains(&req.day) {
        return None;
    }

    let (part1, part2) = benchmark_aoc2015::solve(&req.user, req.year, req.day);
    if req.answer == part1 {
        Some(ValidateResponse::correct(1))
    } else if req.answer == part2 && !part2.is_empty() {
        Some(ValidateResponse::correct(2))
    } else {
        Some(ValidateResponse::incorrect())
    }
}

/// The base URL of the validator as the agent inside the container sees it.
///
/// The container reaches the host at `host.docker.internal`; `web_port` is the
/// web server's `server.port`. Returns the full validator URL.
pub fn validator_url(web_port: u16) -> String {
    format!("http://host.docker.internal:{web_port}/api/aoc/validate")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(user: &str, year: u32, day: u32, answer: &str, part: Option<u32>) -> ValidateRequest {
        ValidateRequest {
            user: user.to_string(),
            year,
            day,
            answer: answer.to_string(),
            part,
        }
    }

    #[test]
    fn accepts_correct_part1() {
        let (e1, _) = benchmark_aoc2015::solve("benchmark", 2015, 1);
        let resp = validate(&req("benchmark", 2015, 1, &e1, Some(1))).unwrap();
        assert!(resp.correct);
        assert_eq!(resp.matched_part, Some(1));
    }

    #[test]
    fn accepts_correct_part2() {
        let (_, e2) = benchmark_aoc2015::solve("benchmark", 2015, 1);
        let resp = validate(&req("benchmark", 2015, 1, &e2, Some(2))).unwrap();
        assert!(resp.correct);
        assert_eq!(resp.matched_part, Some(2));
    }

    #[test]
    fn rejects_wrong_answer() {
        let resp = validate(&req("benchmark", 2015, 1, "wrong", Some(1))).unwrap();
        assert!(!resp.correct);
        assert_eq!(resp.matched_part, None);
    }

    #[test]
    fn rejects_out_of_range_day() {
        assert!(validate(&req("benchmark", 2015, 0, "x", None)).is_none());
        assert!(validate(&req("benchmark", 2015, 26, "x", None)).is_none());
    }

    #[test]
    fn rejects_non_2015_year() {
        assert!(validate(&req("benchmark", 2016, 1, "x", None)).is_none());
    }

    #[test]
    fn day25_part2_matches_part1() {
        let (e1, _) = benchmark_aoc2015::solve("benchmark", 2015, 25);
        let resp = validate(&req("benchmark", 2015, 25, &e1, Some(1))).unwrap();
        assert!(resp.correct);
        assert_eq!(resp.matched_part, Some(1));
    }

    #[test]
    fn validator_url_uses_host_docker_internal() {
        assert_eq!(
            validator_url(8081),
            "http://host.docker.internal:8081/api/aoc/validate"
        );
    }
}
