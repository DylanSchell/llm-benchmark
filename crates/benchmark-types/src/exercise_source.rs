//! Read-only access to the exercise suite.
//!
//! Implementations must **not** expose absolute host paths. Callers work purely in
//! relative-path space:
//!
//! ```text
//! <language>/exercises/practice/<exercise>/<relative>
//! ```
//!
//! The production implementation is the embedded bundle ([`EmbeddedSource`] in the
//! `benchmark-exercises` crate). Keeping this a trait lets the domain logic stay free
//! of filesystem and build concerns.
//!
//! [`EmbeddedSource`]: https://docs.rs/benchmark-exercises

/// Access to exercises by language and name, with no host paths involved.
pub trait ExerciseSource: Send + Sync {
    /// Languages that contain at least one exercise, sorted and de-duplicated.
    fn languages(&self) -> Vec<String>;

    /// Exercise directory names for a language, sorted and de-duplicated.
    ///
    /// Includes every exercise in the bundle; callers apply their own deprecation
    /// filtering (e.g. `DEPRECATED_EXERCISES`).
    fn exercise_names(&self, language: &str) -> Vec<String>;

    /// All files for an exercise as exercise-relative paths (includes `.meta/`),
    /// sorted.
    fn list_files(&self, language: &str, exercise: &str) -> Vec<String>;

    /// Raw bytes for an exercise-relative path, or `None` if it does not exist.
    fn read(&self, language: &str, exercise: &str, relative: &str) -> Option<Vec<u8>>;

    /// Whether an exercise exists. Defaults to probing its files.
    fn has_exercise(&self, language: &str, exercise: &str) -> bool {
        !self.list_files(language, exercise).is_empty()
    }
}
