//! Compression pure utilities ported from OmniRoute `open-sse/services/compression`.
//!
//! Wave 3–4 (CNV-1): validators/helpers, exclusions, fenced-block preservation,
//! compression validation, and caveman rule pack validation/metadata/selectors.
//! Wave 17+: portable core helpers (message content, language detection, ultra heuristics).

pub mod caveman_rules;
pub mod exclusions;
pub mod language_detector;
pub mod message_content;
pub mod preservation;
pub mod ultra_heuristic;
pub mod validation;

pub use caveman_rules::{
    file_rules_to_meta, filter_file_rules_for_context, filter_rules_for_context,
    get_caveman_rule_metadata, intensity_rank, parse_rule_pack_str, parse_rule_pack_value,
    select_file_rule_metadata, summarize_language_packs, validate_rule_pack,
    validate_rule_pack_str, CavemanIntensity, CavemanRuleCategory, CavemanRuleContext,
    CavemanRuleMeta, CavemanRuleMetadataView, FileRule, RulePack, RulePackMetadata,
    RulePackValidation,
};
pub use exclusions::{
    is_compression_excluded, normalize_compression_exclusions,
    normalize_compression_exclusions_strings, CompressionExclusions,
};
pub use language_detector::{detect_compression_language, list_supported_compression_languages};
pub use message_content::{
    extract_text_content, is_text_block, map_text_content, replace_text_content,
};
pub use preservation::{extract_fenced_code_blocks, find_fenced_code_blocks};
pub use ultra_heuristic::{prune_by_score, score_token};
pub use validation::{validate_compression, ValidationResult};
