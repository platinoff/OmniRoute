//! Compression pure utilities ported from OmniRoute `open-sse/services/compression`.
//!
//! Wave 3–4 (CNV-1): validators/helpers, exclusions, fenced-block preservation,
//! compression validation, and caveman rule pack validation/metadata/selectors.

pub mod caveman_rules;
pub mod exclusions;
pub mod preservation;
pub mod validation;

pub use caveman_rules::{
    file_rules_to_meta, filter_rules_for_context, get_caveman_rule_metadata, intensity_rank,
    validate_rule_pack, CavemanIntensity, CavemanRuleCategory, CavemanRuleContext, CavemanRuleMeta,
    CavemanRuleMetadataView, FileRule, RulePack, RulePackValidation,
};
pub use exclusions::{
    is_compression_excluded, normalize_compression_exclusions,
    normalize_compression_exclusions_strings, CompressionExclusions,
};
pub use preservation::{extract_fenced_code_blocks, find_fenced_code_blocks};
pub use validation::{validate_compression, ValidationResult};
