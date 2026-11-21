//! Compression pure utilities ported from OmniRoute `open-sse/services/compression`.
//!
//! Wave 3–4 (CNV-1): validators/helpers, exclusions, fenced-block preservation,
//! compression validation, and caveman rule pack validation/metadata/selectors.
//! Wave 17+: portable core helpers (message content, language detection, ultra heuristics).
//! Wave 19+: output-mode primitives and lite text helpers.
//! Wave 21+: lite body-level transforms.
//! Wave 23+: lightweight compression stats/token-estimation helpers.

pub mod caveman_rules;
pub mod exclusions;
pub mod language_detector;
pub mod lite_body;
pub mod lite_text;
pub mod message_content;
pub mod output_mode;
pub mod preservation;
pub mod stats;
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
pub use lite_body::{
    collapse_whitespace, compress_tool_results, dedup_system_prompt, remove_redundant_content,
    replace_image_urls, BodyTransformResult, LiteCompressionOptions,
};
pub use lite_text::{
    back_off_to_word_boundary, collapse_newline_runs, normalize_message_whitespace,
    trim_trailing_horizontal_whitespace, truncate_tool_content, TOOL_TRUNCATION_LOOKBACK,
};
pub use message_content::{
    extract_text_content, is_text_block, map_text_content, replace_text_content,
};
pub use output_mode::{
    apply_caveman_output_mode, build_caveman_output_instruction, should_bypass_caveman_output_mode,
    CavemanOutputIntensity, CavemanOutputModeConfig, CavemanOutputModeResult,
    CAVEMAN_OUTPUT_MARKER, SHARED_BOUNDARIES,
};
pub use preservation::{extract_fenced_code_blocks, find_fenced_code_blocks};
pub use stats::{
    compression_ratio, compression_savings, estimate_token_savings, estimate_tokens,
    CompressionSavings,
};
pub use ultra_heuristic::{prune_by_score, score_token};
pub use validation::{validate_compression, ValidationResult};
