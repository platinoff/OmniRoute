//! Common scalar grammar for GCF (Graph Compact Format).
//! Port of `open-sse/services/compression/engines/headroom/gcf/scalar.ts`
//! (vendored from gcf-typescript — generic profile; MIT, GCF spec v3.2 context).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    /// `~` in tabular context.
    Missing,
    /// `^` in tabular context.
    Attachment,
    /// Inline schema marker `^{...}` in tabular context: carries raw schema string e.g. "{name,email,tier}".
    InlineSchema(String),
}

impl Scalar {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Scalar::String(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarError(pub String);

impl fmt::Display for ScalarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ScalarError {}

fn err(msg: impl Into<String>) -> ScalarError {
    ScalarError(msg.into())
}

/// Sentinel values exposed for parity with TS `MISSING` / `ATTACHMENT` symbols.
pub const MISSING: Scalar = Scalar::Missing;
pub const ATTACHMENT: Scalar = Scalar::Attachment;

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// JSON number: -?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?
fn is_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut i = 0;
    if b[i] == b'-' {
        i += 1;
        if i >= b.len() {
            return false;
        }
    }
    // int
    if b[i] == b'0' {
        i += 1;
    } else if b[i] >= b'1' && b[i] <= b'9' {
        i += 1;
        while i < b.len() && is_digit(b[i]) {
            i += 1;
        }
    } else {
        return false;
    }
    // frac
    if i < b.len() && b[i] == b'.' {
        i += 1;
        if i >= b.len() || !is_digit(b[i]) {
            return false;
        }
        while i < b.len() && is_digit(b[i]) {
            i += 1;
        }
    }
    // exp
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i >= b.len() {
            return false;
        }
        if b[i] == b'+' || b[i] == b'-' {
            i += 1;
        }
        if i >= b.len() || !is_digit(b[i]) {
            return false;
        }
        while i < b.len() && is_digit(b[i]) {
            i += 1;
        }
    }
    i == b.len()
}

/// NUMERIC_LIKE_RE = /^[+-]\.?\d|^\.\d|^0\d/
fn is_numeric_like(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() >= 2 {
        // ^0\d
        if b[0] == b'0' && is_digit(b[1]) {
            return true;
        }
    }
    if b.len() >= 2 {
        // ^\.\d
        if b[0] == b'.' && is_digit(b[1]) {
            return true;
        }
    }
    if !b.is_empty() {
        let mut i = 0;
        if b[0] == b'+' || b[0] == b'-' {
            i = 1;
        }
        if i < b.len() {
            if b[i] == b'.' {
                // ^[+-]\.\d
                if i + 1 < b.len() && is_digit(b[i + 1]) {
                    return true;
                }
            } else if is_digit(b[i]) {
                // ^[+-]?\d (covers ^[+-]\.?\d start with digit)
                return true;
            }
        }
    }
    false
}

/// INLINE_ARRAY_RE = /\[[^\]]*\]\s*:/
fn has_inline_array_pattern(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' {
            // find ]
            let mut j = i + 1;
            while j < b.len() && b[j] != b']' {
                j += 1;
            }
            if j < b.len() && b[j] == b']' {
                // followed by optional whitespace then ':'
                let mut k = j + 1;
                while k < b.len() && (b[k] == b' ' || b[k] == b'\t') {
                    k += 1;
                }
                if k < b.len() && b[k] == b':' {
                    return true;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    false
}

fn is_unicode_whitespace_special(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{FEFF}'
            | '\u{1680}'
            | '\u{2000}'
            | '\u{2001}'
            | '\u{2002}'
            | '\u{2003}'
            | '\u{2004}'
            | '\u{2005}'
            | '\u{2006}'
            | '\u{2007}'
            | '\u{2008}'
            | '\u{2009}'
            | '\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// Check if a string value must be quoted per Section 2.4.
pub fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if matches!(s, "-" | "~" | "^" | "true" | "false") {
        return true;
    }
    if is_json_number(s) {
        return true;
    }
    if is_numeric_like(s) {
        return true;
    }
    let bytes = s.as_bytes();
    if bytes[0] == b' ' || bytes[bytes.len() - 1] == b' ' {
        return true;
    }
    let first = bytes[0];
    if first == b'#' || first == b'@' || first == b'.' {
        return true;
    }
    if has_inline_array_pattern(s) {
        return true;
    }
    for c in s.chars() {
        let code = c as u32;
        if matches!(code, 0x22 | 0x5C | 0x0A | 0x0D | 0x7C | 0x2C) || code < 0x20 {
            return true; // " \ control \n \r | ,
        }
        if (0x80..=0x9F).contains(&code) {
            return true;
        }
        if code > 0x7F && is_unicode_whitespace_special(c) {
            return true;
        }
    }
    false
}

/// Produce a JSON-compatible quoted string.
pub fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_neg_zero(f: f64) -> bool {
    f == 0.0 && f.is_sign_negative()
}

/// Format a number per Section 2.3 canonical rules (best-effort parity with JS).
pub fn format_number(f: f64) -> String {
    if is_neg_zero(f) {
        return "-0".to_string();
    }
    if f == 0.0 {
        return "0".to_string();
    }
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .to_string();
    }
    let abs = f.abs();
    if (1e-6..1e21).contains(&abs) {
        // JS String(f) gives shortest round-trippable representation in this range in practice.
        // Rust's default formatting is close; normalize integer-valued floats if needed.
        let s = format!("{}", f);
        return s;
    }
    // Exponent notation: match JS toExponential normalization (lowercase e, sign in exponent, no leading zero padding beyond 1 digit min).
    // JS toExponential() produces e.g. "1e+21", "1.23e-7", "1e+0".
    let s = format!("{:e}", f);
    normalize_exponent(&s)
}

fn normalize_exponent(s: &str) -> String {
    // Rust {:e} gives e.g. "1e21" or "1e0"? ensure "+"/"-" and no leading zeros in exponent like JS normalization target.
    // Find 'e' or 'E'
    if let Some(pos) = s.find(['e', 'E']) {
        let (mant, exp_part) = s.split_at(pos);
        let exp_rest = &exp_part[1..]; // after e
        let (sign, digits) = if let Some(stripped) = exp_rest.strip_prefix('+') {
            ('+', stripped)
        } else if let Some(stripped) = exp_rest.strip_prefix('-') {
            ('-', stripped)
        } else {
            ('+', exp_rest)
        };
        // strip leading zeros but keep at least one digit
        let digits_trimmed = digits.trim_start_matches('0');
        let digits_final = if digits_trimmed.is_empty() {
            "0"
        } else {
            digits_trimmed
        };
        format!("{}e{}{}", mant, sign, digits_final)
    } else {
        s.to_string()
    }
}

/// Format a value as a GCF scalar. `delimiter` is '|' ',' or 0.
pub fn format_scalar(v: &Scalar, delimiter: u8) -> String {
    match v {
        Scalar::Null => "-".to_string(),
        Scalar::Bool(true) => "true".to_string(),
        Scalar::Bool(false) => "false".to_string(),
        Scalar::Number(n) => format_number(*n),
        Scalar::String(s) => {
            let needs_delim_quote = delimiter != 0 && s.as_bytes().contains(&delimiter);
            if needs_quote(s) || needs_delim_quote {
                quote_string(s)
            } else {
                s.clone()
            }
        }
        Scalar::Missing => "~".to_string(),
        Scalar::Attachment => "^".to_string(),
        Scalar::InlineSchema(schema) => format!("^{}", schema),
    }
}

/// Convenience for JS-like unknown values (subset used in tests/ports).
#[derive(Debug, Clone, PartialEq)]
pub enum JsLike {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

pub fn format_scalar_jslike(v: &JsLike, delimiter: u8) -> String {
    match v {
        JsLike::Null => "-".to_string(),
        JsLike::Bool(b) => format_scalar(&Scalar::Bool(*b), delimiter),
        JsLike::Number(n) => format_scalar(&Scalar::Number(*n), delimiter),
        JsLike::String(s) => format_scalar(&Scalar::String(s.clone()), delimiter),
    }
}

const fn is_alpha_or_underscore(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}
const fn is_alnum_or_underscore(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

/// Check if a key is a valid bare key.
pub fn is_bare_key(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || !is_alpha_or_underscore(b[0]) {
        return false;
    }
    b[1..].iter().all(|&c| is_alnum_or_underscore(c))
}

/// Format a key, quoting if necessary.
pub fn format_key(s: &str) -> String {
    if is_bare_key(s) {
        s.to_string()
    } else {
        quote_string(s)
    }
}

/// Parse a GCF scalar token per Section 2.1 precedence.
pub fn parse_scalar(s: &str, tabular_context: bool) -> Result<Scalar, ScalarError> {
    if s.is_empty() {
        return Ok(Scalar::String(String::new()));
    }
    // 1. Quoted string.
    if s.as_bytes()[0] == b'"' {
        return parse_quoted_string(s).map(Scalar::String);
    }
    // 2. Null.
    if s == "-" {
        return Ok(Scalar::Null);
    }
    // 3. Missing (tabular only).
    if s == "~" {
        if !tabular_context {
            return Err(err("invalid_missing: ~ outside tabular row cell"));
        }
        return Ok(Scalar::Missing);
    }
    // 4. Attachment (tabular only).
    if s == "^" || (s.starts_with("^{") && s.ends_with('}')) {
        if !tabular_context {
            return Err(err("invalid_attachment_marker: ^ outside tabular row cell"));
        }
        if s == "^" {
            return Ok(Scalar::Attachment);
        }
        return Ok(Scalar::InlineSchema(s[1..].to_string()));
    }
    // 5. Boolean.
    if s == "true" {
        return Ok(Scalar::Bool(true));
    }
    if s == "false" {
        return Ok(Scalar::Bool(false));
    }
    // 6. Number.
    if is_json_number(s) {
        if let Ok(f) = s.parse::<f64>() {
            if !f.is_nan() {
                return Ok(Scalar::Number(f));
            }
        }
    }
    // 7. Bare string.
    Ok(Scalar::String(s.to_string()))
}

/// Parse a JSON-compatible quoted string.
pub fn parse_quoted_string(s: &str) -> Result<String, ScalarError> {
    let b = s.as_bytes();
    if b.len() < 2 || b[0] != b'"' {
        return Err(err("unterminated_quote"));
    }
    let mut out = String::new();
    let mut i = 1usize;
    while i < b.len() {
        if b[i] == b'"' {
            if i + 1 != b.len() {
                return Err(err("trailing_characters: after closing quote"));
            }
            return Ok(out);
        }
        if b[i] == b'\\' {
            if i + 1 >= b.len() {
                return Err(err("unterminated_quote"));
            }
            i += 1;
            match b[i] {
                b'"' => {
                    out.push('"');
                    i += 1;
                    continue;
                }
                b'\\' => {
                    out.push('\\');
                    i += 1;
                    continue;
                }
                b'/' => {
                    out.push('/');
                    i += 1;
                    continue;
                }
                b'b' => {
                    out.push('\u{08}');
                    i += 1;
                    continue;
                }
                b'f' => {
                    out.push('\u{0C}');
                    i += 1;
                    continue;
                }
                b'n' => {
                    out.push('\n');
                    i += 1;
                    continue;
                }
                b'r' => {
                    out.push('\r');
                    i += 1;
                    continue;
                }
                b't' => {
                    out.push('\t');
                    i += 1;
                    continue;
                }
                b'u' => {
                    if i + 4 >= b.len() {
                        return Err(err("invalid_escape: incomplete unicode"));
                    }
                    let hex = &s[i + 1..i + 5];
                    let code = u16::from_str_radix(hex, 16)
                        .map_err(|_| err(format!("invalid_escape: invalid unicode \\u{}", hex)))?
                        as u32;
                    // Surrogate pair handling.
                    if (0xD800..=0xDBFF).contains(&code) {
                        if i + 10 >= b.len() || b[i + 5] != b'\\' || b[i + 6] != b'u' {
                            return Err(err("invalid_surrogate: isolated high surrogate"));
                        }
                        let hex2 = &s[i + 7..i + 11];
                        let low = u16::from_str_radix(hex2, 16)
                            .map_err(|_| err("invalid_surrogate: invalid low surrogate"))?
                            as u32;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(err("invalid_surrogate: invalid low surrogate"));
                        }
                        let cp = 0x10000 + (code - 0xD800) * 0x400 + (low - 0xDC00);
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        } else {
                            return Err(err("invalid_surrogate: invalid codepoint"));
                        }
                        i += 11;
                        continue;
                    }
                    if (0xDC00..=0xDFFF).contains(&code) {
                        return Err(err("invalid_surrogate: isolated low surrogate"));
                    }
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    } else {
                        return Err(err("invalid_escape: invalid unicode scalar"));
                    }
                    i += 5;
                    continue;
                }
                other => {
                    return Err(err(format!("invalid_escape: unknown \\{}", other as char)));
                }
            }
        }
        if b[i] < 0x20 {
            return Err(err(format!(
                "invalid_escape: unescaped control U+{:04X}",
                b[i]
            )));
        }
        // push UTF-8 char at i
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Err(err("unterminated_quote"))
}

/// Split a string on a delimiter, respecting quoted strings.
pub fn split_respecting_quotes(s: &str, delim: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if escaped {
            current.push(c);
            escaped = false;
            i += 1;
            continue;
        }
        if c == '\\' && in_quote {
            current.push(c);
            escaped = true;
            i += 1;
            continue;
        }
        if c == '"' {
            in_quote = !in_quote;
            current.push(c);
            i += 1;
            continue;
        }
        if c == delim && !in_quote {
            parts.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    parts.push(current);
    parts
}

fn find_closing_brace(s: &str) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    let mut in_quote = false;
    let mut escaped = false;
    for (i, &c) in chars.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && in_quote {
            escaped = true;
            continue;
        }
        if c == '"' {
            in_quote = !in_quote;
            continue;
        }
        if c == '}' && !in_quote {
            return Some(i);
        }
    }
    None
}

/// Split a field declaration like {id,"display name","a,b"}.
pub fn split_field_decl(s: &str) -> Result<Vec<String>, ScalarError> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 || chars[0] != '{' {
        return Err(err("invalid field declaration"));
    }
    let close_idx = find_closing_brace(s).ok_or_else(|| err("invalid field declaration"))?;
    let inner: String = chars[1..close_idx].iter().collect();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let raw = split_respecting_quotes(&inner, ',');
    let mut fields = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for f in raw {
        let trimmed = f.trim().to_string();
        let name = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            parse_quoted_string(&trimmed)?
        } else {
            if !is_bare_key(&trimmed) {
                return Err(err(format!("invalid field name: {}", trimmed)));
            }
            trimmed
        };
        if !seen.insert(name.clone()) {
            return Err(err(format!("duplicate_field_name: {}", name)));
        }
        fields.push(name);
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_quote_inline_array_pattern_spec_2_4() {
        assert!(needs_quote("ERR[404]: Not Found"));
        assert!(needs_quote("[Speaker 1]: Hello"));
        assert!(needs_quote("[1]: y"));
        assert!(!needs_quote("arr[0] index"));
        assert!(!needs_quote("plain value here"));
    }

    #[test]
    fn needs_quote_basics() {
        assert!(needs_quote(""));
        assert!(needs_quote("-"));
        assert!(needs_quote("~"));
        assert!(needs_quote("^"));
        assert!(needs_quote("true"));
        assert!(needs_quote("false"));
        assert!(needs_quote("123"));
        assert!(needs_quote("0"));
        assert!(needs_quote("-1.5"));
        assert!(needs_quote(" leading"));
        assert!(needs_quote("trailing "));
        assert!(needs_quote("#hash"));
        assert!(needs_quote("@at"));
        assert!(needs_quote(".dot"));
        assert!(needs_quote("a,b"));
        assert!(needs_quote("a|b"));
        assert!(!needs_quote("plain"));
        assert!(!needs_quote("hello_world"));
    }

    #[test]
    fn quote_and_parse_roundtrip_basic() {
        let cases = vec![
            "simple",
            "with space",
            "quote\"inside",
            "back\\slash",
            "line\nbreak",
            "tab\tchar",
            "car\rret",
            "ERR[404]: Not Found",
        ];
        for c in cases {
            let q = quote_string(c);
            let p = parse_quoted_string(&q).unwrap();
            assert_eq!(p, c, "roundtrip failed for {:?}", c);
        }
    }

    #[test]
    fn format_scalar_basics() {
        assert_eq!(format_scalar(&Scalar::Null, 0), "-");
        assert_eq!(format_scalar(&Scalar::Bool(true), 0), "true");
        assert_eq!(format_scalar(&Scalar::Bool(false), 0), "false");
        assert_eq!(format_scalar(&Scalar::Number(42.0), 0), "42");
        assert_eq!(format_scalar(&Scalar::String("plain".into()), 0), "plain");
        assert_eq!(
            format_scalar(&Scalar::String("ERR[404]: Not Found".into()), 0),
            "\"ERR[404]: Not Found\""
        );
    }

    #[test]
    fn format_number_specials() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "-0");
        assert_eq!(format_number(1e21), "1e+21");
        assert_eq!(format_number(1e-7), "1e-7");
    }

    #[test]
    fn parse_scalar_core() {
        assert_eq!(parse_scalar("", false).unwrap(), Scalar::String("".into()));
        assert_eq!(parse_scalar("-", false).unwrap(), Scalar::Null);
        assert_eq!(parse_scalar("true", false).unwrap(), Scalar::Bool(true));
        assert_eq!(parse_scalar("false", false).unwrap(), Scalar::Bool(false));
        assert_eq!(parse_scalar("42", false).unwrap(), Scalar::Number(42.0));
        assert_eq!(
            parse_scalar("\"hi\"", false).unwrap(),
            Scalar::String("hi".into())
        );
        assert_eq!(
            parse_scalar("bare", false).unwrap(),
            Scalar::String("bare".into())
        );
    }

    #[test]
    fn parse_scalar_tabular_markers() {
        assert_eq!(parse_scalar("~", true).unwrap(), Scalar::Missing);
        assert_eq!(parse_scalar("^", true).unwrap(), Scalar::Attachment);
        assert_eq!(
            parse_scalar("^{name,email}", true).unwrap(),
            Scalar::InlineSchema("{name,email}".into())
        );
        assert!(parse_scalar("~", false).is_err());
        assert!(parse_scalar("^", false).is_err());
    }

    #[test]
    fn keys_and_fields() {
        assert!(is_bare_key("abc"));
        assert!(is_bare_key("_a1"));
        assert!(!is_bare_key("1a"));
        assert!(!is_bare_key("a-b"));
        assert_eq!(format_key("plain"), "plain");
        assert_eq!(format_key("a b"), "\"a b\"");
        let fields = split_field_decl("{id,\"display name\",a_b}").unwrap();
        assert_eq!(fields, vec!["id", "display name", "a_b"]);
    }

    #[test]
    fn split_quotes_and_errors() {
        assert_eq!(
            split_respecting_quotes("a,b,\"c,d\"", ','),
            vec!["a", "b", "\"c,d\""]
        );
        assert!(parse_quoted_string("\"unterminated").is_err());
        assert!(parse_quoted_string("\"trailing\"x").is_err());
        assert!(parse_scalar("\"\\q\"", false).is_err());
    }
}
