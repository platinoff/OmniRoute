//! GCF (Graph Compact Format) primitives ported from OmniRoute `open-sse`.
//!
//! Wave 1 (CNV-1): scalar grammar parity module (`scalar`), vendored-origin
//! logic based on gcf-typescript (MIT). See source headers in TS originals.

pub mod scalar;

pub use scalar::{
    format_key, format_number, format_scalar, is_bare_key, needs_quote, parse_quoted_string,
    parse_scalar, quote_string, split_field_decl, split_respecting_quotes, Scalar, ScalarError,
    ATTACHMENT, MISSING,
};
