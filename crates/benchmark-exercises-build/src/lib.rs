//! Build-time assembler for the embedded exercise bundle.
//!
//! Third-party exercise content is never committed to this repository. Instead the
//! build fetches pinned Exercism tracks, prunes them according to a committed
//! manifest, and stages the result for embedding.
//!
//! The two entry points are:
//! * [`assemble_from`] — prune + lay out an already-fetched source tree.
//! * [`fetch_language`] — sparse/shallow clone of one pinned track into the cache.
//!
//! Keeping these separate makes the pruning rules unit-testable without network
//! access (point `assemble_from` at a local tree) and lets the build reuse a cache.

use anyhow::{bail, Context, Result};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Marker written into each fetched track checkout recording the resolved commit.
const REF_MARKER: &str = ".assembled-ref";

// =============================================================================
// Manifest model (mirrors exercises.manifest.yaml)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u32,
    pub sources: BTreeMap<String, Source>,
    pub layout: Layout,
    pub exercises: BTreeMap<String, ExerciseSelection>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Source {
    pub repo: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Where a selected exercise lives upstream and where it lands in the bundle.
/// Templates understand `{language}` and `{exercise}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layout {
    pub source: String,
    pub dest: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExerciseSelection {
    /// Curated subset. `exclude` wins over `include`.
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl ExerciseSelection {
    /// The exercises that survive include/exclude, in manifest order.
    pub fn effective(&self) -> Vec<String> {
        let excluded: HashSet<&String> = self.exclude.iter().collect();
        self.include
            .iter()
            .filter(|name| !excluded.contains(name))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        let manifest: Manifest = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing manifest {}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("unsupported manifest version: {}", self.version);
        }
        if self.sources.is_empty() {
            bail!("manifest defines no sources");
        }
        for language in self.sources.keys() {
            if !self.exercises.contains_key(language) {
                bail!("manifest source `{language}` has no `exercises` selection");
            }
        }
        for (language, selection) in &self.exercises {
            if !self.sources.contains_key(language) {
                bail!("manifest selects exercises for unknown source `{language}`");
            }
            if selection.include.is_empty() {
                bail!("`exercises.{language}.include` is empty (use explicit names)");
            }
        }
        Ok(())
    }

    fn selection(&self, language: &str) -> Result<&ExerciseSelection> {
        self.exercises
            .get(language)
            .with_context(|| format!("no exercise selection for `{language}`"))
    }
}

// =============================================================================
// File-level rules
// =============================================================================

/// Ordered include/exclude matcher over exercise-relative paths (forward slashes).
///
/// Rules are applied in order and the **last** match wins. Paths with no match are
/// included by default.
pub struct Rules {
    matchers: Vec<(GlobMatcher, bool)>,
}

impl Rules {
    pub fn compile(rules: &[Rule]) -> Result<Self> {
        let mut matchers = Vec::with_capacity(rules.len());
        for (idx, rule) in rules.iter().enumerate() {
            let (pattern, include) = match (&rule.include, &rule.exclude) {
                (None, Some(pattern)) => (pattern, false),
                (Some(pattern), None) => (pattern, true),
                _ => bail!("rule #{idx} must set exactly one of `include` or `exclude`"),
            };
            let matcher = Glob::new(pattern)
                .with_context(|| format!("invalid glob in rule #{idx}: {pattern}"))?
                .compile_matcher();
            matchers.push((matcher, include));
        }
        Ok(Self { matchers })
    }

    pub fn includes(&self, relative: &str) -> bool {
        let mut included = true;
        for (matcher, is_include) in &self.matchers {
            if matcher.is_match(relative) {
                included = *is_include;
            }
        }
        included
    }
}

// =============================================================================
// Assembly
// =============================================================================

#[derive(Debug, Default, Clone)]
pub struct LanguageReport {
    pub exercises: usize,
    pub files: usize,
    pub excluded_files: usize,
}

#[derive(Debug, Default, Clone)]
pub struct AssembleReport {
    pub exercises: usize,
    pub files: usize,
    pub excluded_files: usize,
    pub per_language: BTreeMap<String, LanguageReport>,
}

/// Prune `source_root` into `staging_root` according to `manifest`.
///
/// Expects `source_root` to contain one directory per language holding that track's
/// checkout (i.e. `source_root/<language>/<layout.source>`). This is the same shape
/// a per-track git checkout produces, which also makes it a convenient fixture for
/// assembling from a local tree.
pub fn assemble_from(
    source_root: &Path,
    staging_root: &Path,
    manifest: &Manifest,
) -> Result<AssembleReport> {
    let rules = Rules::compile(&manifest.rules)?;

    if staging_root.exists() {
        fs::remove_dir_all(staging_root)
            .with_context(|| format!("clearing staging dir {}", staging_root.display()))?;
    }

    let mut report = AssembleReport::default();

    for language in manifest.sources.keys() {
        let selection = manifest.selection(language)?;
        let exercises = selection.effective();
        let mut lang_report = LanguageReport::default();

        for exercise in &exercises {
            let source = source_root.join(language).join(render(
                &manifest.layout.source,
                language,
                exercise,
            ));
            if !source.is_dir() {
                bail!(
                    "exercise source not found: {} (language `{}`, exercise `{}`)",
                    source.display(),
                    language,
                    exercise
                );
            }

            let dest = staging_root
                .join(render(&manifest.layout.dest, language, exercise));

            let (files, excluded) = copy_pruned(&source, &dest, &rules)
                .with_context(|| format!("copying {language}/{exercise}"))?;

            lang_report.exercises += 1;
            lang_report.files += files;
            lang_report.excluded_files += excluded;
        }

        report.exercises += lang_report.exercises;
        report.files += lang_report.files;
        report.excluded_files += lang_report.excluded_files;
        report.per_language.insert(language.clone(), lang_report);
    }

    Ok(report)
}

/// Copy every rule-included file from `source` to `dest`, preserving relative paths.
fn copy_pruned(source: &Path, dest: &Path, rules: &Rules) -> Result<(usize, usize)> {
    let mut copied = 0usize;
    let mut excluded = 0usize;

    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("walkdir entry is under source");
        let relative_str = relative.to_string_lossy().replace('\\', "/");

        if !rules.includes(&relative_str) {
            excluded += 1;
            continue;
        }

        let target = dest.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(entry.path(), &target)
            .with_context(|| format!("copying {}", entry.path().display()))?;
        copied += 1;
    }

    Ok((copied, excluded))
}

fn render(template: &str, language: &str, exercise: &str) -> String {
    template
        .replace("{language}", language)
        .replace("{exercise}", exercise)
}

// =============================================================================
// Fetch (sparse + shallow + blobless clone of a pinned track)
// =============================================================================

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    /// Directory holding the track checkout.
    pub dir: PathBuf,
    /// Resolved commit SHA (from the manifest ref, or the cache marker if reused).
    pub resolved: String,
    /// True when an existing cache entry was reused (no network access).
    pub cached: bool,
}

/// Ensure a pinned track is available in `cache_root/<language>`.
///
/// Reuses an existing checkout whose marker matches the manifest ref. When
/// `offline` is set, a cache miss is an error rather than a fetch.
pub fn fetch_language(
    source: &Source,
    language: &str,
    exercises: &[String],
    layout: &Layout,
    cache_root: &Path,
    offline: bool,
) -> Result<FetchOutcome> {
    let dir = cache_root.join(language);
    let marker = dir.join(REF_MARKER);

    if let Ok(recorded) = fs::read_to_string(&marker) {
        if recorded.trim() == source.reference && dir.join("exercises").is_dir() {
            return Ok(FetchOutcome {
                dir,
                resolved: recorded.trim().to_string(),
                cached: true,
            });
        }
    }

    if offline {
        bail!(
            "exercise cache for `{language}` is missing or stale and offline mode is enabled \
             (expected ref {}). Run a build with network access first, or set \
             LLM_BENCHMARK_EXERCISES_DIR.",
            source.reference
        );
    }

    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("clearing stale cache {}", dir.display()))?;
    }
    fs::create_dir_all(&dir)?;

    let sparse_paths: Vec<String> = exercises
        .iter()
        .map(|exercise| render(&layout.source, language, exercise))
        .collect();

    git(&dir, &["init", "-q"])?;
    git(&dir, &["remote", "add", "origin", &source.repo])?;
    git(&dir, &["sparse-checkout", "init", "--cone"])?;
    git(
        &dir,
        &[
            "fetch",
            "-q",
            "--depth",
            "1",
            "--filter=blob:none",
            "origin",
            &source.reference,
        ],
    )?;
    let mut set_args: Vec<&str> = vec!["sparse-checkout", "set"];
    set_args.extend(sparse_paths.iter().map(String::as_str));
    git(&dir, &set_args)?;
    git(&dir, &["checkout", "-q", "FETCH_HEAD"])?;

    let resolved = git_capture(&dir, &["rev-parse", "HEAD"])?;
    let resolved = resolved.trim().to_string();
    fs::write(&marker, &resolved)?;

    Ok(FetchOutcome {
        dir,
        resolved,
        cached: false,
    })
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed in {}:\n{}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn git_capture(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed in {}:\n{}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// =============================================================================
// Hashing helpers (used by the lock file)
// =============================================================================

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("hashing {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

/// Deterministically hash every file under `root` (sorted by relative path).
pub fn hash_tree(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walkdir entry is under root")
            .to_string_lossy()
            .replace('\\', "/");
        out.insert(relative, sha256_file(entry.path())?);
    }
    Ok(out)
}

// =============================================================================
// Lock file
// =============================================================================

/// Resolved, reproducible description of the bundled exercise set.
///
/// This is derived metadata (commit SHAs + content hashes), not third-party content,
/// so it is safe to commit. It makes builds reproducible and drift detectable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    pub version: u32,
    pub sources: BTreeMap<String, LockedSource>,
    /// Bundle-relative path -> sha256 of file contents.
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedSource {
    pub repo: String,
    pub resolved: String,
}

impl Lock {
    /// Build a lock from a staged bundle plus the resolved per-language commits.
    pub fn generate(
        staging_root: &Path,
        manifest: &Manifest,
        resolved: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut sources = BTreeMap::new();
        for (language, source) in &manifest.sources {
            let sha = resolved.get(language).with_context(|| {
                format!("no resolved commit for `{language}` (was it fetched?)")
            })?;
            sources.insert(
                language.clone(),
                LockedSource {
                    repo: source.repo.clone(),
                    resolved: sha.clone(),
                },
            );
        }
        Ok(Self {
            version: 1,
            sources,
            files: hash_tree(staging_root)?,
        })
    }

    pub fn to_yaml(&self) -> Result<String> {
        let body = serde_yaml::to_string(self).context("serializing lock")?;
        let header = "# exercises.lock.yaml\n#\n# Generated by the build from exercises.manifest.yaml. Commit this file.\n# It records the resolved upstream commits and a sha256 per bundled file so builds\n# are reproducible and drift is detectable.\n\n";
        Ok(format!("{header}{body}"))
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        fs::write(path, self.to_yaml()?)
            .with_context(|| format!("writing lock {}", path.display()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading lock {}", path.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parsing lock {}", path.display()))
    }

    /// Compare this lock against a freshly staged bundle; returns differing paths.
    pub fn drift(&self, staging_root: &Path) -> Result<Vec<String>> {
        let actual = hash_tree(staging_root)?;
        let mut changed: Vec<String> = Vec::new();
        for (path, hash) in &actual {
            match self.files.get(path) {
                Some(locked) if locked == hash => {}
                Some(_) => changed.push(format!("modified: {path}")),
                None => changed.push(format!("added: {path}")),
            }
        }
        for path in self.files.keys() {
            if !actual.contains_key(path) {
                changed.push(format!("removed: {path}"));
            }
        }
        changed.sort();
        Ok(changed)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn rules(pairs: &[(&str, bool)]) -> Rules {
        // (pattern, is_include)
        let manifest_rules: Vec<Rule> = pairs
            .iter()
            .map(|(pattern, include)| Rule {
                include: include.then(|| pattern.to_string()),
                exclude: (!include).then(|| pattern.to_string()),
            })
            .collect();
        Rules::compile(&manifest_rules).unwrap()
    }

    #[test]
    fn rules_default_to_included() {
        let r = rules(&[(".meta/tests.toml", false)]);
        assert!(r.includes("Cargo.toml"));
        assert!(!r.includes(".meta/tests.toml"));
    }

    #[test]
    fn rules_last_match_wins() {
        // exclude everything, then re-include the prompt
        let r = rules(&[("**/*.md", false), (".docs/instructions.md", true)]);
        assert!(!r.includes(".docs/hints.md"));
        assert!(r.includes(".docs/instructions.md"));
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn test_manifest() -> Manifest {
        let mut sources = BTreeMap::new();
        sources.insert(
            "rust".to_string(),
            Source {
                repo: "https://example.invalid/rust".into(),
                reference: "deadbeef".into(),
            },
        );
        let mut exercises = BTreeMap::new();
        exercises.insert(
            "rust".to_string(),
            ExerciseSelection {
                include: vec!["series".into(), "accumulate".into()],
                exclude: vec!["accumulate".into()],
            },
        );
        Manifest {
            version: 1,
            sources,
            layout: Layout {
                source: "exercises/practice/{exercise}".into(),
                dest: "{language}/exercises/practice/{exercise}".into(),
            },
            exercises,
            rules: vec![
                Rule { include: None, exclude: Some(".approaches/**".into()) },
                Rule { include: None, exclude: Some(".meta/tests.toml".into()) },
            ],
        }
    }

    #[test]
    fn exercise_exclude_wins_over_include() {
        let m = test_manifest();
        assert_eq!(m.selection("rust").unwrap().effective(), vec!["series"]);
    }

    #[test]
    fn assembles_layout_and_prunes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src");
        let staging = tmp.path().join("staging");

        write(
            &source.join("rust/exercises/practice/series/Cargo.toml"),
            "[package]\nname = \"series\"\n",
        );
        write(
            &source.join("rust/exercises/practice/series/.meta/config.json"),
            "{}",
        );
        write(
            &source.join("rust/exercises/practice/series/.meta/tests.toml"),
            "excluded",
        );
        write(
            &source.join("rust/exercises/practice/series/.approaches/x.md"),
            "excluded",
        );

        let report = assemble_from(&source, &staging, &test_manifest()).unwrap();

        assert_eq!(report.exercises, 1, "accumulate should be excluded");
        assert_eq!(report.files, 2);
        assert_eq!(report.excluded_files, 2);

        let dest = staging.join("rust/exercises/practice/series");
        assert!(dest.join("Cargo.toml").is_file());
        assert!(dest.join(".meta/config.json").is_file());
        assert!(!dest.join(".meta/tests.toml").exists());
        assert!(!dest.join(".approaches").exists());
        assert!(!staging.join("rust/exercises/practice/accumulate").exists());
    }

    #[test]
    fn missing_exercise_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = assemble_from(
            &tmp.path().join("src"),
            &tmp.path().join("staging"),
            &test_manifest(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("exercise source not found"));
    }

    #[test]
    fn staging_is_cleared_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src");
        let staging = tmp.path().join("staging");
        write(
            &source.join("rust/exercises/practice/series/Cargo.toml"),
            "x",
        );
        write(&staging.join("stale/leftover.txt"), "stale");

        assemble_from(&source, &staging, &test_manifest()).unwrap();
        assert!(!staging.join("stale/leftover.txt").exists());
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn lock_round_trips_and_detects_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src");
        let staging = tmp.path().join("staging");
        write(&source.join("rust/exercises/practice/series/Cargo.toml"), "x");
        assemble_from(&source, &staging, &test_manifest()).unwrap();

        let mut resolved = BTreeMap::new();
        resolved.insert("rust".to_string(), "deadbeef".to_string());
        let lock = Lock::generate(&staging, &test_manifest(), &resolved).unwrap();
        assert_eq!(lock.files.len(), 1);
        assert!(lock.drift(&staging).unwrap().is_empty());

        let path = tmp.path().join("exercises.lock.yaml");
        lock.write(&path).unwrap();
        let reloaded = Lock::load(&path).unwrap();
        assert_eq!(reloaded.files, lock.files);
        assert_eq!(reloaded.sources["rust"].resolved, "deadbeef");

        write(&staging.join("rust/exercises/practice/series/Cargo.toml"), "changed");
        let drift = reloaded.drift(&staging).unwrap();
        assert_eq!(drift.len(), 1);
        assert!(drift[0].starts_with("modified:"));
    }

    /// Migration/fidelity check against a local polyglot-benchmark checkout.
    /// Requires `POLYGLOT_DIR` (or `../polyglot-benchmark` next to the repo).
    #[test]
    #[ignore = "requires a local polyglot-benchmark checkout (set POLYGLOT_DIR)"]
    fn assembles_current_snapshot() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = Manifest::load(&repo_root.join("exercises.manifest.yaml")).unwrap();
        let source_root = std::env::var("POLYGLOT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("../polyglot-benchmark"));
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("bundle");

        let report = assemble_from(&source_root, &staging, &manifest).unwrap();
        assert_eq!(report.exercises, 225, "all curated exercises assembled");
        assert!(report.excluded_files > 0, "pruning removed files");

        for entry in WalkDir::new(&staging).into_iter().filter_map(|e| e.ok()) {
            let relative = entry.path().strip_prefix(&staging).unwrap();
            let parts: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
            assert!(!parts.iter().any(|p| p == ".approaches" || p == ".articles"), "pruned dir leaked: {}", relative.display());
            let name = parts.last().cloned().unwrap_or_default();
            assert_ne!(name, "tests.toml", "pruned tests.toml leaked: {}", relative.display());
            assert!(!name.ends_with(".j2") && !name.ends_with(".tera"), "pruned template leaked: {}", relative.display());
        }

        // Files the application depends on must survive.
        for required in [
            "rust/exercises/practice/alphametics/Cargo.toml",
            "rust/exercises/practice/alphametics/.meta/Cargo-example.toml",
            "java/exercises/practice/series/gradle/wrapper/gradle-wrapper.jar",
            "python/exercises/practice/beer-song/.meta/example.py",
            "cpp/exercises/practice/allergies/test/catch.hpp",
        ] {
            assert!(staging.join(required).is_file(), "missing required file: {required}");
        }
    }

    /// Network check: sparse/shallow/blobless clone of a pinned Exercism track,
    /// including fetching by commit SHA (not a branch).
    #[test]
    #[ignore = "network: clones a pinned Exercism track"]
    fn fetches_pinned_track() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = Manifest::load(&repo_root.join("exercises.manifest.yaml")).unwrap();
        let source = &manifest.sources["rust"];
        let exercises = vec!["alphametics".to_string()];
        let tmp = tempfile::tempdir().unwrap();

        let outcome = fetch_language(source, "rust", &exercises, &manifest.layout, tmp.path(), false)
            .unwrap();
        assert!(!outcome.cached);
        assert_eq!(outcome.resolved, source.reference);

        let cargo = outcome.dir.join("exercises/practice/alphametics/Cargo.toml");
        assert!(cargo.is_file(), "sparse checkout did not materialize the exercise");
        let text = fs::read_to_string(&cargo).unwrap();
        assert!(text.contains("edition = \"2021\""), "pinned content not reproduced");

        // A second call must reuse the cache without network access.
        let again = fetch_language(source, "rust", &exercises, &manifest.layout, tmp.path(), true)
            .unwrap();
        assert!(again.cached);
        assert_eq!(again.resolved, source.reference);
    }

    /// Full-subset fidelity check: clean-clone every pinned track, assemble, and
    /// hash-compare against a local polyglot-benchmark checkout.
    #[test]
    #[ignore = "network: full-subset fidelity check against a local polyglot checkout"]
    fn full_subset_fidelity() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = Manifest::load(&repo_root.join("exercises.manifest.yaml")).unwrap();
        let polyglot = std::env::var("POLYGLOT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("../polyglot-benchmark"));
        let rules = Rules::compile(&manifest.rules).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        let staging = tmp.path().join("bundle");

        // 1. Clean clone of every pinned track (fresh temp cache).
        for (language, source) in &manifest.sources {
            let exercises = manifest.selection(language).unwrap().effective();
            let outcome =
                fetch_language(source, language, &exercises, &manifest.layout, &cache, false)
                    .unwrap_or_else(|e| panic!("fetch {language}: {e}"));
            println!(
                "fetched {language:10} @ {} ({} exercises, cached={})",
                &outcome.resolved[..10],
                exercises.len(),
                outcome.cached
            );
        }

        // 2. Assemble with the committed rules.
        let report = assemble_from(&cache, &staging, &manifest).unwrap();
        println!(
            "assembled: {} exercises, {} files, {} pruned\n",
            report.exercises, report.files, report.excluded_files
        );

        let staged = hash_tree(&staging).unwrap();

        // 3. Expected tree: polyglot subset, keeping only rule-included files.
        let mut expected: BTreeMap<String, String> = BTreeMap::new();
        for language in manifest.sources.keys() {
            for exercise in manifest.selection(language).unwrap().effective() {
                let dir = polyglot
                    .join(language)
                    .join(render(&manifest.layout.source, language, &exercise));
                if !dir.is_dir() {
                    println!("!! polyglot missing {language}/{exercise}");
                    continue;
                }
                for (rel, hash) in hash_tree(&dir).unwrap() {
                    if !rules.includes(&rel) {
                        continue;
                    }
                    expected.insert(
                        format!("{language}/exercises/practice/{exercise}/{rel}"),
                        hash,
                    );
                }
            }
        }

        let mut missing = Vec::new();
        let mut differing = Vec::new();
        for (path, hash) in &expected {
            match staged.get(path) {
                Some(h) if h == hash => {}
                Some(_) => differing.push(path.clone()),
                None => missing.push(path.clone()),
            }
        }
        let extra: Vec<String> = staged
            .keys()
            .filter(|p| !expected.contains_key(*p))
            .cloned()
            .collect();
        missing.sort();
        differing.sort();

        let sample = |label: &str, items: &[String]| {
            if items.is_empty() {
                return;
            }
            println!("\n{label} ({}):", items.len());
            for p in items.iter().take(40) {
                println!("  {p}");
            }
            if items.len() > 40 {
                println!("  ... and {} more", items.len() - 40);
            }
        };

        println!("=== FIDELITY REPORT (staged vs polyglot) ===");
        println!("expected (polyglot, post-rules): {}", expected.len());
        println!("staged:                          {}", staged.len());
        println!("matched:                         {}", expected.len() - missing.len() - differing.len());
        println!("missing (in polyglot, not staged): {}", missing.len());
        println!("extra   (in staged, not polyglot): {}", extra.len());
        println!("differing content:                 {}", differing.len());
        println!("\nper-language:");
        for language in manifest.sources.keys() {
            let prefix = format!("{language}/");
            let count = |v: &[String]| v.iter().filter(|p| p.starts_with(&prefix)).count();
            println!(
                "  {language:10} missing={} extra={} differing={}",
                count(&missing),
                count(&extra),
                count(&differing)
            );
        }
        sample("MISSING", &missing);
        sample("EXTRA", &extra);
        sample("DIFFERING", &differing);
    }
}
