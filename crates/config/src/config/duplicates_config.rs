use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

const fn default_min_tokens() -> usize {
    50
}

const fn default_min_lines() -> usize {
    5
}

/// Configuration for code duplication detection.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesConfig {
    /// Whether duplication detection is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Detection mode: strict, mild, weak, or semantic.
    #[serde(default)]
    pub mode: DetectionMode,

    /// Minimum number of tokens for a clone.
    #[serde(default = "default_min_tokens")]
    pub min_tokens: usize,

    /// Minimum number of lines for a clone.
    #[serde(default = "default_min_lines")]
    pub min_lines: usize,

    /// Maximum allowed duplication percentage (0 = no limit).
    #[serde(default)]
    pub threshold: f64,

    /// Additional ignore patterns for duplication analysis.
    #[serde(default)]
    pub ignore: Vec<String>,

    /// Only report cross-directory duplicates.
    #[serde(default)]
    pub skip_local: bool,

    /// Enable cross-language clone detection by stripping type annotations.
    ///
    /// When enabled, TypeScript type annotations (parameter types, return types,
    /// generics, interfaces, type aliases) are stripped from the token stream,
    /// allowing detection of clones between `.ts` and `.js` files.
    #[serde(default)]
    pub cross_language: bool,

    /// Fine-grained normalization overrides on top of the detection mode.
    #[serde(default)]
    pub normalization: NormalizationConfig,
}

impl Default for DuplicatesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: DetectionMode::default(),
            min_tokens: default_min_tokens(),
            min_lines: default_min_lines(),
            threshold: 0.0,
            ignore: vec![],
            skip_local: false,
            cross_language: false,
            normalization: NormalizationConfig::default(),
        }
    }
}

/// Fine-grained normalization overrides.
///
/// Each option, when set to `Some(true)`, forces that normalization regardless of
/// the detection mode. When set to `Some(false)`, it forces preservation. When
/// `None`, the detection mode's default behavior applies.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationConfig {
    /// Blind all identifiers (variable names, function names, etc.) to the same hash.
    /// Default in `semantic` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_identifiers: Option<bool>,

    /// Blind string literal values to the same hash.
    /// Default in `weak` and `semantic` modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_string_values: Option<bool>,

    /// Blind numeric literal values to the same hash.
    /// Default in `semantic` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_numeric_values: Option<bool>,
}

/// Resolved normalization flags: mode defaults merged with user overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedNormalization {
    pub ignore_identifiers: bool,
    pub ignore_string_values: bool,
    pub ignore_numeric_values: bool,
}

impl ResolvedNormalization {
    /// Resolve normalization from a detection mode and optional overrides.
    pub fn resolve(mode: DetectionMode, overrides: &NormalizationConfig) -> Self {
        let (default_ids, default_strings, default_numbers) = match mode {
            DetectionMode::Strict | DetectionMode::Mild => (false, false, false),
            DetectionMode::Weak => (false, true, false),
            DetectionMode::Semantic => (true, true, true),
        };

        Self {
            ignore_identifiers: overrides.ignore_identifiers.unwrap_or(default_ids),
            ignore_string_values: overrides.ignore_string_values.unwrap_or(default_strings),
            ignore_numeric_values: overrides.ignore_numeric_values.unwrap_or(default_numbers),
        }
    }
}

/// Detection mode controlling how aggressively tokens are normalized.
///
/// Since fallow uses AST-based tokenization (not lexer-based), whitespace and
/// comments are inherently absent from the token stream. The `Strict` and `Mild`
/// modes are currently equivalent. `Weak` mode additionally blinds string
/// literals. `Semantic` mode blinds all identifiers and literal values for
/// Type-2 (renamed variable) clone detection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMode {
    /// All tokens preserved including identifier names and literal values (Type-1 only).
    Strict,
    /// Default mode -- equivalent to strict for AST-based tokenization.
    #[default]
    Mild,
    /// Blind string literal values (structure-preserving).
    Weak,
    /// Blind all identifiers and literal values for structural (Type-2) detection.
    Semantic,
}

impl std::fmt::Display for DetectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Strict => write!(f, "strict"),
            Self::Mild => write!(f, "mild"),
            Self::Weak => write!(f, "weak"),
            Self::Semantic => write!(f, "semantic"),
        }
    }
}

impl std::str::FromStr for DetectionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(Self::Strict),
            "mild" => Ok(Self::Mild),
            "weak" => Ok(Self::Weak),
            "semantic" => Ok(Self::Semantic),
            other => Err(format!("unknown detection mode: '{other}'")),
        }
    }
}
