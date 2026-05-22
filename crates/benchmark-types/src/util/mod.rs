use std::collections::HashSet;

/// All programming languages supported by the polyglot benchmark.
pub static SUPPORTED_LANGUAGES: &[&str] = &[
    "java", "go", "rust", "javascript", "typescript", "python", "ruby", "c",
    "cpp", "csharp", "kotlin", "scala", "swift", "php", "dart", "haskell",
    "elixir", "ocaml", "racket", "lua", "julia", "perl", "r", "zig",
];

/// Returns a HashSet of all supported language names.
pub fn supported_languages_set() -> HashSet<&'static str> {
    SUPPORTED_LANGUAGES.iter().copied().collect()
}

/// Checks if a language is supported by the benchmark.
pub fn is_supported_language(language: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language)
}

/// Joins a slice of strings with a separator.
pub fn join_strings(strings: &[&str], sep: &str) -> String {
    strings.join(sep)
}

/// Converts a value to non-null string, defaulting to "unknown".
pub fn to_non_null(value: Option<&str>) -> &str {
    value.unwrap_or("unknown")
}
