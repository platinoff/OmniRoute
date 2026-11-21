//! Compression pure utilities ported from OmniRoute `open-sse/services/compression`.
//!
//! Wave 3 (CNV-1): validators/helpers — exclusions, fenced-block preservation
//! helpers, and compression validation.

pub mod exclusions;
pub mod preservation;
pub mod validation;

pub use exclusions::{
    is_compression_excluded, normalize_compression_exclusions, CompressionExclusions,
};
pub use preservation::{extract_fenced_code_blocks, find_fenced_code_blocks};
pub use validation::{validate_compression, ValidationResult};
