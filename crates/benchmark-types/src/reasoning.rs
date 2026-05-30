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

use serde::{Deserialize, Serialize};

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

/// Which reasoning mechanism a model family uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningMechanism {
    /// Anthropic: native thinking level via pi's --thinking flag or model config.
    AnthropicThinking,
    /// OpenAI o-series: reasoning_effort API parameter (low/medium/high).
    OpenAIReasoningEffort,
    /// DeepSeek/r1: reasoning is baked into the model architecture. No config needed.
    NativeReasoning,
    /// llama.cpp / other OpenAI-compatible: custom parameters per model.
    Custom { kwargs: Vec<(String, String)> },
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

        // OpenAI non-reasoning models (gpt-4o, gpt-4, etc.) — no reasoning config needed
        if lower.starts_with("gpt") || lower.starts_with("o1-mini") {
            return Self::Custom { kwargs: vec![] };
        }

        // DeepSeek reasoning models
        if lower.contains("deepseek") && (lower.contains("r1") || lower.contains("chat")) {
            return Self::NativeReasoning;
        }

        // Default: llama.cpp / other OpenAI-compatible endpoint
        // These may need custom kwargs — caller can override via ReasoningConfig
        Self::Custom { kwargs: vec![] }
    }
}

/// Configuration that maps pi thinking levels to backend-specific parameters.
/// This is written into models.json so pi knows how to translate a user-selected
/// thinking level into the actual API call parameters for the target model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Which mechanism this model family uses.
    pub mechanism: ReasoningMechanism,

    /// Mapping from pi thinking levels to backend-specific values.
    /// Only needs entries for levels that differ from the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_mapping: Option<ThinkingLevelMapping>,
}

/// Maps each pi thinking level to its backend-specific equivalent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingLevelMapping {
    /// What the backend receives for `off` thinking level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhigh: Option<String>,
}

// =============================================================================
// Default reasoning configs per model family
// =============================================================================

impl ReasoningConfig {
    /// Get the default reasoning config for a given mechanism.
    pub fn default_for_mechanism(mechanism: &ReasoningMechanism) -> Self {
        match mechanism {
            ReasoningMechanism::AnthropicThinking => {
                // Anthropic uses the same level names natively — no translation needed
                Self {
                    mechanism: mechanism.clone(),
                    level_mapping: None,
                }
            }
            ReasoningMechanism::OpenAIReasoningEffort => {
                // Map pi thinking levels to OpenAI reasoning_effort values
                Self {
                    mechanism: mechanism.clone(),
                    level_mapping: Some(ThinkingLevelMapping {
                        off: Some("disabled".to_string()),
                        minimal: Some("low".to_string()),
                        low: Some("low".to_string()),
                        medium: Some("medium".to_string()),
                        high: Some("high".to_string()),
                        xhigh: Some("high".to_string()),
                    }),
                }
            }
            ReasoningMechanism::NativeReasoning => {
                // DeepSeek-r1 etc. have baked-in reasoning — no config needed
                Self {
                    mechanism: mechanism.clone(),
                    level_mapping: None,
                }
            }
            ReasoningMechanism::Custom { .. } => {
                // llama.cpp / generic OpenAI-compatible — no default mapping
                Self {
                    mechanism: mechanism.clone(),
                    level_mapping: None,
                }
            }
        }
    }

    /// Get the backend-specific level for a given pi thinking level.
    pub fn resolve(&self, level: ThinkingLevel) -> Option<Vec<(String, serde_json::Value)>> {
        match &self.mechanism {
            ReasoningMechanism::AnthropicThinking => {
                // Pi handles this natively via --thinking flag
                Some(vec![("thinking_level".to_string(), serde_json::json!(level.to_string()))])
            }
            ReasoningMechanism::OpenAIReasoningEffort => {
                // Map to reasoning_effort API parameter
                let mapping = self.level_mapping.as_ref()?;
                let value = match level {
                    ThinkingLevel::Off => &mapping.off,
                    ThinkingLevel::Minimal => &mapping.minimal,
                    ThinkingLevel::Low => &mapping.low,
                    ThinkingLevel::Medium => &mapping.medium,
                    ThinkingLevel::High => &mapping.high,
                    ThinkingLevel::Xhigh => &mapping.xhigh,
                };
                value.as_ref().map(|s| {
                    vec![("reasoning_effort".to_string(), serde_json::json!(s))]
                })
            }
            ReasoningMechanism::NativeReasoning => {
                // No parameters to set — reasoning is always active
                None
            }
            ReasoningMechanism::Custom { kwargs } => {
                // Use model-specific kwargs (caller must configure these)
                if kwargs.is_empty() && !level.is_active() {
                    None
                } else {
                    let mut result: Vec<(String, serde_json::Value)> = kwargs
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    // Add thinking level as a hint for the backend
                    result.push(("thinking_level".to_string(), serde_json::Value::String(level.to_string())));
                    Some(result)
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
            ReasoningMechanism::Custom { .. }
        ));
        assert!(matches!(
            ReasoningMechanism::detect("llama-3.1-70b"),
            ReasoningMechanism::Custom { .. }
        ));
    }

    #[test]
    fn test_openai_reasoning_effort_mapping() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::OpenAIReasoningEffort);
        
        assert_eq!(
            config.resolve(ThinkingLevel::Off),
            Some(vec![("reasoning_effort".to_string(), serde_json::json!("disabled"))])
        );
        assert_eq!(
            config.resolve(ThinkingLevel::Low),
            Some(vec![("reasoning_effort".to_string(), serde_json::json!("low"))])
        );
        assert_eq!(
            config.resolve(ThinkingLevel::Medium),
            Some(vec![("reasoning_effort".to_string(), serde_json::json!("medium"))])
        );
        assert_eq!(
            config.resolve(ThinkingLevel::High),
            Some(vec![("reasoning_effort".to_string(), serde_json::json!("high"))])
        );
        assert_eq!(
            config.resolve(ThinkingLevel::Xhigh),
            Some(vec![("reasoning_effort".to_string(), serde_json::json!("high"))])
        );
    }

    #[test]
    fn test_anthropic_thinking_mapping() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::AnthropicThinking);
        
        assert_eq!(
            config.resolve(ThinkingLevel::High),
            Some(vec![("thinking_level".to_string(), serde_json::json!("high"))])
        );
    }

    #[test]
    fn test_native_reasoning_no_config() {
        let config = ReasoningConfig::default_for_mechanism(&ReasoningMechanism::NativeReasoning);
        assert!(config.resolve(ThinkingLevel::High).is_none());
    }

    #[test]
    fn test_custom_reasoning_with_kwargs() {
        let config = ReasoningConfig {
            mechanism: ReasoningMechanism::Custom {
                kwargs: vec![
                    ("temperature".to_string(), "0.7".to_string()),
                ],
            },
            level_mapping: None,
        };

        let result = config.resolve(ThinkingLevel::High).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "temperature");
        assert_eq!(result[1].0, "thinking_level");
    }
}
