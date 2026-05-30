//! Reasoning level configuration for LLM models.
//!
//! Different models and backends use different mechanisms to control reasoning depth:
//! - Anthropic (Sonnet/Opus/Haiku): native `--thinking` support in pi
//! - OpenAI o-series (o1/o3-mini/o4): `reasoning_effort` API kwarg (`low`/`medium`/`high`)
//! - DeepSeek-r1: reasoning is baked into the model, no config needed
//! - llama.cpp hosted models: may need custom parameters per model
//!
//! This module provides a unified abstraction: a pi-level `ThinkingLevel` that gets
//! translated into backend-specific configuration via a `ReasoningConfig`.
//!
//! # Registration API
//!
//! Use `ReasoningRegistry::register()` to add model-specific reasoning configs at
//! startup. Lookups support exact model IDs and glob patterns (e.g., `qwen3-*`).
//! More specific patterns take priority over broader ones.
//!
//! ```ignore
//! // Register a config for a specific model
//! ReasoningRegistry::register(
//!     "deepseek-r1-distill-llama-70b",
//!     ReasoningConfig {
//!         mechanism: ReasoningMechanism::NativeReasoning,
//!         level_mapping: None,
//!     },
//! );
//!
//! // Register a config for a family of models via glob pattern
//! ReasoningRegistry::register(
//!     "qwen3-*",
//!     ReasoningConfig {
//!         mechanism: ReasoningMechanism::AnthropicThinking,
//!         level_mapping: Some(ThinkingLevelMapping {
//!             off: Some("off".to_string()),
//!             minimal: None,
//!             low: Some("low".to_string()),
//!             medium: Some("medium".to_string()),
//!             high: Some("high".to_string()),
//!             xhigh: Some("xhigh".to_string()),
//!         }),
//!     },
//! );
//! ```

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use once_cell::sync::Lazy;

/// Unified thinking level from the benchmark UI.
/// These are pi-native levels that get translated to backend-specific parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    Xhigh,
}

impl ThinkingLevel {
    /// Parse a thinking level from a string. Returns None for unrecognized values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "minimal" | "min" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" | "hi" => Some(Self::High),
            "xhigh" | "maximum" | "max" => Some(Self::Xhigh),
            _ => None,
        }
    }

    /// Check if this level enables any reasoning at all.
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

impl std::fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Minimal => write!(f, "minimal"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Xhigh => write!(f, "xhigh"),
        }
    }
}

/// How pi should encode thinking levels for a model.
/// Maps directly to pi's `thinkingFormat` model config option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingFormat {
    /// Default: uses `reasoning_effort` (OpenAI o-series style)
    Openai,
    /// Uses `reasoning: { effort }` (OpenRouter style)
    Openrouter,
    /// Uses `thinking: { type }` plus `reasoning_effort` (DeepSeek style)
    Deepseek,
    /// Uses `reasoning: { enabled }` plus `reasoning_effort` (Together AI style)
    Together,
    /// Uses `enable_thinking` (Zai style)
    Zai,
    /// Uses `enable_thinking` (Qwen style)
    Qwen,
    /// Uses `chat_template_kwargs.enable_thinking` (Qwen chat template style)
    #[serde(rename = "qwen-chat-template")]
    QwenChatTemplate,
}

impl std::fmt::Display for ThinkingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Openai => write!(f, "openai"),
            Self::Openrouter => write!(f, "openrouter"),
            Self::Deepseek => write!(f, "deepseek"),
            Self::Together => write!(f, "together"),
            Self::Zai => write!(f, "zai"),
            Self::Qwen => write!(f, "qwen"),
            Self::QwenChatTemplate => write!(f, "qwen-chat-template"),
        }
    }
}

impl Default for ThinkingFormat {
    fn default() -> Self {
        Self::Openai
    }
}

/// Whether the provider supports reasoning_effort parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatConfig {
    /// Whether to use `system` role instead of `developer` for system prompts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_developer_role: Option<bool>,
    /// Whether the provider supports reasoning_effort parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
}

impl Default for CompatConfig {
    fn default() -> Self {
        Self {
            supports_developer_role: None,
            supports_reasoning_effort: Some(true),
        }
    }
}

/// Which reasoning mechanism a model family uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningMechanism {
    /// Anthropic: native thinking level via pi's --thinking flag or model config.
    AnthropicThinking,
    /// OpenAI o-series: reasoning_effort API parameter (low/medium/high).
    OpenAIReasoningEffort,
    /// DeepSeek/r1: reasoning is baked into the model architecture. No config needed.
    NativeReasoning,
    /// llama.cpp / other OpenAI-compatible: uses thinkingFormat for encoding.
    Custom,
}

impl ReasoningMechanism {
    /// Determine the mechanism for a given model name.
    pub fn detect(model_name: &str) -> Self {
        let lower = model_name.to_lowercase();

        // Anthropic models
        if lower.contains("claude") || lower.contains("sonnet") || lower.contains("opus") || lower.contains("haiku") {
            return Self::AnthropicThinking;
        }

        // OpenAI o-series reasoning models (o1, o3-mini, o4, etc.)
        if lower.starts_with("o1") || lower.starts_with("o3") || lower.starts_with("o4") {
            return Self::OpenAIReasoningEffort;
        }

        // DeepSeek reasoning models
        if lower.contains("deepseek") && (lower.contains("r1") || lower.contains("chat")) {
            return Self::NativeReasoning;
        }

        // Default: everything else (qwen, llama, custom OpenAI-compatible endpoints)
        Self::Custom
    }
}

/// Configuration that maps pi thinking levels to backend-specific parameters.
/// This is written into models.json so pi knows how to translate a user-selected
/// thinking level into the actual API call parameters for the target model.
///
/// The config is serialized directly into pi's model config format, which uses:
/// - `thinkingFormat`: how to encode thinking (e.g., "qwen" → enable_thinking)
/// - `thinkingLevelMap`: per-level overrides for models with limited support
/// - `compat`: provider compatibility flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Which mechanism this model family uses.
    pub mechanism: ReasoningMechanism,

    /// How pi should encode thinking levels for this model.
    /// Maps to pi's `thinkingFormat` option.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<ThinkingFormat>,

    /// Per-level overrides for models that don't support all levels.
    /// Keys are pi thinking levels; values are what gets sent to the provider.
    /// Use `null` to mark a level as unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<ThinkingLevelMap>,

    /// Provider compatibility flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<CompatConfig>,
}

/// Per-level overrides for a model's thinking support.
/// Maps pi thinking levels to provider-specific values (string or null for unsupported).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingLevelMap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhigh: Option<serde_json::Value>,
}

// =============================================================================
// Default reasoning configs per model family
// =============================================================================

impl ReasoningConfig {
    /// Get the default reasoning config for a given mechanism.
    pub fn default_for_mechanism(mechanism: &ReasoningMechanism) -> Self {
        match mechanism {
            ReasoningMechanism::AnthropicThinking => {
                // Anthropic: pi handles via --thinking flag, no extra config needed
                Self {
                    mechanism: mechanism.clone(),
                    thinking_format: None,
                    thinking_level_map: None,
                    compat: None,
                }
            }
            ReasoningMechanism::OpenAIReasoningEffort => {
                // OpenAI o-series: reasoning_effort via thinkingFormat=openai (default)
                Self {
                    mechanism: mechanism.clone(),
                    thinking_format: Some(ThinkingFormat::Openai),
                    thinking_level_map: Some(ThinkingLevelMap {
                        off: Some(serde_json::json!("disabled")),
                        minimal: Some(serde_json::json!("low")),
                        low: Some(serde_json::json!("low")),
                        medium: Some(serde_json::json!("medium")),
                        high: Some(serde_json::json!("high")),
                        xhigh: Some(serde_json::json!("high")),
                    }),
                    compat: None,
                }
            }
            ReasoningMechanism::NativeReasoning => {
                // DeepSeek-r1 etc. have baked-in reasoning — no config needed
                Self {
                    mechanism: mechanism.clone(),
                    thinking_format: None,
                    thinking_level_map: None,
                    compat: None,
                }
            }
            ReasoningMechanism::Custom => {
                // llama.cpp / qwen / generic OpenAI-compatible — no defaults
                // Caller should register via ReasoningRegistry
                Self {
                    mechanism: mechanism.clone(),
                    thinking_format: None,
                    thinking_level_map: None,
                    compat: None,
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_level_from_str() {
        assert_eq!(ThinkingLevel::from_str("off"), Some(ThinkingLevel::Off));
        assert_eq!(ThinkingLevel::from_str("minimal"), Some(ThinkingLevel::Minimal));
        assert_eq!(ThinkingLevel::from_str("low"), Some(ThinkingLevel::Low));
        assert_eq!(ThinkingLevel::from_str("medium"), Some(ThinkingLevel::Medium));
        assert_eq!(ThinkingLevel::from_str("high"), Some(ThinkingLevel::High));
        assert_eq!(ThinkingLevel::from_str("xhigh"), Some(ThinkingLevel::Xhigh));
        // Case insensitive
        assert_eq!(ThinkingLevel::from_str("HIGH"), Some(ThinkingLevel::High));
        assert_eq!(ThinkingLevel::from_str("XHIGH"), Some(ThinkingLevel::Xhigh));
        // Aliases
        assert_eq!(ThinkingLevel::from_str("min"), Some(ThinkingLevel::Minimal));
        assert_eq!(ThinkingLevel::from_str("max"), Some(ThinkingLevel::Xhigh));
    }

    #[test]
    fn test_thinking_level_display() {
        assert_eq!(ThinkingLevel::High.to_string(), "high");
        assert_eq!(ThinkingLevel::Xhigh.to_string(), "xhigh");
    }

    #[test]
    fn test_reasoning_mechanism_detection() {
        assert!(matches!(
            ReasoningMechanism::detect("claude-sonnet-4-20250514"),
            ReasoningMechanism::AnthropicThinking
        ));
        assert!(matches!(
            ReasoningMechanism::detect("o3-mini"),
            ReasoningMechanism::OpenAIReasoningEffort
        ));
        assert!(matches!(
            ReasoningMechanism::detect("o1"),
            ReasoningMechanism::OpenAIReasoningEffort
        ));
        assert!(matches!(
            ReasoningMechanism::detect("deepseek-r1"),
            ReasoningMechanism::NativeReasoning
        ));
        assert!(matches!(
            ReasoningMechanism::detect("gpt-4o"),
            ReasoningMechanism::Custom
        ));
        assert!(matches!(
            ReasoningMechanism::detect("llama-3.1-70b"),
            ReasoningMechanism::Custom
        ));
        assert!(matches!(
            ReasoningMechanism::detect("qwen3-235b-a22b"),
            ReasoningMechanism::Custom
        ));
    }

    #[test]
    fn test_openai_reasoning_effort_config() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::OpenAIReasoningEffort);
        assert!(matches!(config.thinking_format, Some(ThinkingFormat::Openai)));
        assert!(config.thinking_level_map.is_some());
    }

    #[test]
    fn test_anthropic_no_extra_config() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::AnthropicThinking);
        assert!(config.thinking_format.is_none());
        assert!(config.thinking_level_map.is_none());
    }

    #[test]
    fn test_native_reasoning_no_config() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::NativeReasoning);
        assert!(config.thinking_format.is_none());
        assert!(config.thinking_level_map.is_none());
    }

    #[test]
    fn test_custom_no_defaults() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::Custom);
        assert!(config.thinking_format.is_none());
        assert!(config.thinking_level_map.is_none());
    }

    #[test]
    fn test_qwen_config_serialization() {
        // Example: qwen3-* with enable_thinking
        let config = ReasoningConfig {
            mechanism: ReasoningMechanism::Custom,
            thinking_format: Some(ThinkingFormat::Qwen),
            thinking_level_map: Some(ThinkingLevelMap {
                off:     Some(serde_json::json!(false)),
                minimal: Some(serde_json::json!(true)),
                low:     Some(serde_json::json!(true)),
                medium:  Some(serde_json::json!(true)),
                high:    Some(serde_json::json!(true)),
                xhigh:   Some(serde_json::json!(true)),
            }),
            compat: None,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        println!("Qwen config:\n{}", json);
        assert!(json.contains("\"thinking_format\": \"qwen\""));
        assert!(json.contains("\"off\": false"));
        assert!(json.contains("\"minimal\": true"));
    }

    #[test]
    fn test_qwen_chat_template_config_serialization() {
        // Example: qwen with chat_template_kwargs.enable_thinking
        let config = ReasoningConfig {
            mechanism: ReasoningMechanism::Custom,
            thinking_format: Some(ThinkingFormat::QwenChatTemplate),
            thinking_level_map: Some(ThinkingLevelMap {
                off:     Some(serde_json::json!(false)),
                minimal: Some(serde_json::json!(true)),
                low:     Some(serde_json::json!(true)),
                medium:  Some(serde_json::json!(true)),
                high:    Some(serde_json::json!(true)),
                xhigh:   Some(serde_json::json!(true)),
            }),
            compat: None,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"thinking_format\": \"qwen-chat-template\""));
    }
}

// =============================================================================
// Reasoning Registry — per-model overrides
// =============================================================================

/// A single registry entry: a model pattern and its reasoning config.
pub struct RegisteredEntry {
    /// The model pattern (exact ID or glob like `qwen3-*`).
    pub pattern: String,
    /// The reasoning config to apply.
    pub config: ReasoningConfig,
}

/// Global registry of per-model reasoning configurations.
///
/// Lookups are resolved by finding the most specific matching pattern:
/// 1. Exact match (e.g., `"deepseek-r1"`) takes priority
/// 2. Glob patterns (e.g., `"qwen3-*"`) match via simple wildcard
/// 3. If no pattern matches, falls through to `ReasoningMechanism::detect(model)`
///
/// This is initialized with built-in defaults but can be extended at runtime.
pub struct ReasoningRegistry {
    entries: Mutex<Vec<RegisteredEntry>>,
}

impl ReasoningRegistry {
    /// Register a reasoning config for a specific model ID or glob pattern.
    ///
    /// Patterns are matched in registration order. More specific patterns
    /// (exact matches) automatically take priority over broader ones during lookup.
    ///
    /// # Glob syntax
    /// - `*` matches any sequence of characters (including empty)
    /// - Example: `"qwen3-*"` matches `"qwen3-235b-a22b"`, `"qwen3-30b-a3b"`, etc.
    pub fn register(pattern: &str, config: ReasoningConfig) {
        let mut entries = REASONING_REGISTRY.entries.lock().unwrap();
        // Insert at the front so most-recent registrations take priority
        // (exact matches will still sort above globs during lookup)
        entries.insert(0, RegisteredEntry {
            pattern: pattern.to_string(),
            config,
        });
    }

    /// Look up the reasoning config for a model.
    ///
    /// Returns `Some(config)` if an exact or glob pattern match is found,
    /// otherwise returns `None` (caller should fall back to
    /// `ReasoningConfig::default_for_mechanism(&ReasoningMechanism::detect(model))`).
    pub fn lookup(model: &str) -> Option<ReasoningConfig> {
        let entries = REASONING_REGISTRY.entries.lock().unwrap();
        let lower = model.to_lowercase();

        // First pass: exact match (highest priority)
        for entry in entries.iter() {
            if entry.pattern == model || entry.pattern.to_lowercase() == lower {
                return Some(entry.config.clone());
            }
        }

        // Second pass: glob patterns — prefer more specific matches
        // (longer pattern = more specific)
        let mut best_match: Option<(&RegisteredEntry, usize)> = None;
        for entry in entries.iter() {
            if entry.pattern.contains('*') && Self::pattern_matches(&entry.pattern, &lower) {
                let specificity = entry.pattern.len();
                match &best_match {
                    Some((_, best_spec)) if specificity > *best_spec => {
                        best_match = Some((entry, specificity));
                    }
                    None => {
                        best_match = Some((entry, specificity));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(entry, _)| entry.config.clone())
    }

    /// Check if a model name matches a glob pattern.
    /// Supports `*` as a wildcard for any character sequence.
    fn pattern_matches(pattern: &str, model: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 1 {
            // No wildcard — exact match (already handled above)
            return pattern.to_lowercase() == model.to_lowercase();
        }

        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                // Prefix match
                if !model[pos..].starts_with(*part) {
                    return false;
                }
                pos += part.len();
            } else if i == parts.len() - 1 {
                // Suffix match
                if !model.ends_with(*part) {
                    return false;
                }
                break;
            } else {
                // Middle segment — find anywhere after current position
                match model[pos..].find(*part) {
                    Some(idx) => pos += idx + part.len(),
                    None => return false,
                }
            }
        }
        true
    }

    /// Get the reasoning config for a model, falling back to mechanism detection.
    ///
    /// This is the main entry point used by PiAgent:
    /// 1. Check registry for exact/glob match
    /// 2. Fall back to `ReasoningMechanism::detect(model)`
    pub fn get_for_model(model: &str) -> ReasoningConfig {
        if let Some(config) = Self::lookup(model) {
            return config;
        }
        let mechanism = ReasoningMechanism::detect(model);
        ReasoningConfig::default_for_mechanism(&mechanism)
    }

    /// Clear all registered entries. Useful for testing.
    #[cfg(test)]
    pub fn clear() {
        REASONING_REGISTRY.entries.lock().unwrap().clear();
    }

    /// Register built-in defaults. Call this once at application startup.
    pub fn register_defaults() {
        // Anthropic models — native thinking, no extra config needed
        Self::register("claude-*", ReasoningConfig {
            mechanism: ReasoningMechanism::AnthropicThinking,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });
        Self::register("sonnet", ReasoningConfig {
            mechanism: ReasoningMechanism::AnthropicThinking,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });
        Self::register("opus", ReasoningConfig {
            mechanism: ReasoningMechanism::AnthropicThinking,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });
        Self::register("haiku", ReasoningConfig {
            mechanism: ReasoningMechanism::AnthropicThinking,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });

        // OpenAI o-series — reasoning_effort (thinkingFormat=openai is default)
        Self::register("o3-*", ReasoningConfig {
            mechanism: ReasoningMechanism::OpenAIReasoningEffort,
            thinking_format: Some(ThinkingFormat::Openai),
            thinking_level_map: Some(ThinkingLevelMap {
                off: Some(serde_json::json!("disabled")),
                minimal: Some(serde_json::json!("low")),
                low: Some(serde_json::json!("low")),
                medium: Some(serde_json::json!("medium")),
                high: Some(serde_json::json!("high")),
                xhigh: Some(serde_json::json!("high")),
            }),
            compat: None,
        });
        Self::register("o1-*", ReasoningConfig {
            mechanism: ReasoningMechanism::OpenAIReasoningEffort,
            thinking_format: Some(ThinkingFormat::Openai),
            thinking_level_map: Some(ThinkingLevelMap {
                off: Some(serde_json::json!("disabled")),
                minimal: Some(serde_json::json!("low")),
                low: Some(serde_json::json!("low")),
                medium: Some(serde_json::json!("medium")),
                high: Some(serde_json::json!("high")),
                xhigh: Some(serde_json::json!("high")),
            }),
            compat: None,
        });

        // Qwen3 — enable_thinking kwarg (thinkingFormat=qwen)
        Self::register("qwen3-*", ReasoningConfig {
            mechanism: ReasoningMechanism::Custom,
            thinking_format: Some(ThinkingFormat::Qwen),
            thinking_level_map: Some(ThinkingLevelMap {
                off:     Some(serde_json::json!(false)),
                minimal: Some(serde_json::json!(true)),
                low:     Some(serde_json::json!(true)),
                medium:  Some(serde_json::json!(true)),
                high:    Some(serde_json::json!(true)),
                xhigh:   Some(serde_json::json!(true)),
            }),
            compat: None,
        });
    }
}

static REASONING_REGISTRY: Lazy<ReasoningRegistry> = Lazy::new(|| ReasoningRegistry {
    entries: Mutex::new(Vec::new()),
});

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn test_exact_match_priority() {
        // Register a broad pattern then a specific override
        ReasoningRegistry::register("qwen-*", ReasoningConfig {
            mechanism: ReasoningMechanism::Custom,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });
        ReasoningRegistry::register("qwen3-235b-a22b", ReasoningConfig {
            mechanism: ReasoningMechanism::AnthropicThinking,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });

        let specific = ReasoningRegistry::lookup("qwen3-235b-a22b").unwrap();
        assert!(matches!(specific.mechanism, ReasoningMechanism::AnthropicThinking));
    }

    #[test]
    fn test_glob_pattern_match() {
        ReasoningRegistry::register("llama-*", ReasoningConfig {
            mechanism: ReasoningMechanism::Custom,
            thinking_format: None,
            thinking_level_map: None,
            compat: None,
        });

        let config = ReasoningRegistry::lookup("llama-3.1-70b").unwrap();
        assert!(matches!(config.mechanism, ReasoningMechanism::Custom));
    }

    #[test]
    fn test_no_match_returns_none() {
        let result = ReasoningRegistry::lookup("unknown-model-xyz");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_for_model_fallback() {
        // No registrations — should fall back to detect
        let config = ReasoningRegistry::get_for_model("o3-mini");
        assert!(matches!(config.mechanism, ReasoningMechanism::OpenAIReasoningEffort));
    }

    #[test]
    fn test_qwen3_builtin_registration() {
        // Reset registry to avoid pollution from other tests
        ReasoningRegistry::clear();
        ReasoningRegistry::register_defaults();
        let config = ReasoningRegistry::get_for_model("qwen3-235b-a22b");
        assert_eq!(config.thinking_format, Some(ThinkingFormat::Qwen));
        let map = config.thinking_level_map.as_ref().expect("qwen3 should have level map");
        assert_eq!(map.off, Some(serde_json::json!(false)));
        assert_eq!(map.high, Some(serde_json::json!(true)));
    }
}
