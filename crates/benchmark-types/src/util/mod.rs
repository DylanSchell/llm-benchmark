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

/// Unwrap a std lock result, recovering from poisoning.
///
/// A std `Mutex`/`RwLock` becomes poisoned when a thread panics while
/// holding it. `.unwrap()` on a poisoned lock then panics on EVERY
/// subsequent access — one panicking request thread takes down the whole
/// service (queue, session manager, caches). The guarded data is still
/// intact; poisoning is only a heuristic, so recovering keeps the app
/// functional at the cost of possibly stale state in the (rare) panic case.
pub fn recover_poisoned<T>(result: std::sync::LockResult<T>) -> T {
    result.unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recover_poisoned_returns_data_after_panic() {
        let mutex = std::sync::Arc::new(std::sync::Mutex::new(41));
        let poisoned = std::sync::Arc::clone(&mutex);
        let handle = std::thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("boom"); // panics while holding the lock -> poisons it
        });
        assert!(handle.join().is_err());

        // A plain unwrap would cascade the panic; recovery returns the data.
        assert_eq!(*recover_poisoned(mutex.lock()), 41);
    }

    #[test]
    fn recover_poisoned_works_on_clean_lock() {
        let mutex = std::sync::Mutex::new(7);
        assert_eq!(*recover_poisoned(mutex.lock()), 7);
    }

    #[test]
    fn recover_poisoned_works_for_rwlock_read() {
        let lock = std::sync::RwLock::new(3);
        assert_eq!(*recover_poisoned(lock.read()), 3);
    }

    #[test]
    fn recover_poisoned_works_for_rwlock_write() {
        let lock = std::sync::RwLock::new(3);
        *recover_poisoned(lock.write()) += 1;
        assert_eq!(*recover_poisoned(lock.read()), 4);
    }
}
