//! Caveman rule pack validation, metadata, and pure selection helpers.
//! Ported (bounded) from `open-sse/services/compression/ruleLoader.ts` and
//! `cavemanRules.ts`. FS loading and JS-regex rule execution are out of scope.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CavemanIntensity {
    Lite,
    Full,
    Ultra,
}

impl CavemanIntensity {
    pub fn as_str(&self) -> &'static str {
        match self {
            CavemanIntensity::Lite => "lite",
            CavemanIntensity::Full => "full",
            CavemanIntensity::Ultra => "ultra",
        }
    }
}

pub fn intensity_rank(intensity: CavemanIntensity) -> u8 {
    match intensity {
        CavemanIntensity::Lite => 0,
        CavemanIntensity::Full => 1,
        CavemanIntensity::Ultra => 2,
    }
}

fn parse_intensity(s: &str) -> Option<CavemanIntensity> {
    match s {
        "lite" => Some(CavemanIntensity::Lite),
        "full" => Some(CavemanIntensity::Full),
        "ultra" => Some(CavemanIntensity::Ultra),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CavemanRuleContext {
    All,
    User,
    System,
    Assistant,
}

impl CavemanRuleContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            CavemanRuleContext::All => "all",
            CavemanRuleContext::User => "user",
            CavemanRuleContext::System => "system",
            CavemanRuleContext::Assistant => "assistant",
        }
    }
}

fn parse_context(s: &str) -> Option<CavemanRuleContext> {
    match s {
        "all" => Some(CavemanRuleContext::All),
        "user" => Some(CavemanRuleContext::User),
        "system" => Some(CavemanRuleContext::System),
        "assistant" => Some(CavemanRuleContext::Assistant),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CavemanRuleCategory {
    Filler,
    Context,
    Structural,
    Dedup,
    Terse,
    Ultra,
}

impl CavemanRuleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            CavemanRuleCategory::Filler => "filler",
            CavemanRuleCategory::Context => "context",
            CavemanRuleCategory::Structural => "structural",
            CavemanRuleCategory::Dedup => "dedup",
            CavemanRuleCategory::Terse => "terse",
            CavemanRuleCategory::Ultra => "ultra",
        }
    }
}

fn parse_category(s: &str) -> Option<CavemanRuleCategory> {
    match s {
        "filler" => Some(CavemanRuleCategory::Filler),
        "context" => Some(CavemanRuleCategory::Context),
        "structural" => Some(CavemanRuleCategory::Structural),
        "dedup" => Some(CavemanRuleCategory::Dedup),
        "terse" => Some(CavemanRuleCategory::Terse),
        "ultra" => Some(CavemanRuleCategory::Ultra),
        _ => None,
    }
}

/// Rule as represented in a file-based pack (JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRule {
    pub name: String,
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_map: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_intensity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Rule pack as represented in JSON files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RulePack {
    pub language: String,
    pub category: String,
    pub rules: Vec<FileRule>,
}

/// Minimal metadata for rule selection/filtering and projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CavemanRuleMeta {
    pub name: String,
    pub context: CavemanRuleContext,
    pub category: CavemanRuleCategory,
    pub min_intensity: CavemanIntensity,
    pub description: Option<String>,
}

impl CavemanRuleMeta {
    pub fn from_file_rule(rule: &FileRule) -> Self {
        let context = rule
            .context
            .as_deref()
            .and_then(parse_context)
            .unwrap_or(CavemanRuleContext::All);
        let category = rule
            .category
            .as_deref()
            .and_then(parse_category)
            .unwrap_or(CavemanRuleCategory::Filler);
        let min_intensity = rule
            .min_intensity
            .as_deref()
            .and_then(parse_intensity)
            .unwrap_or(CavemanIntensity::Lite);
        CavemanRuleMeta {
            name: rule.name.clone(),
            context,
            category,
            min_intensity,
            description: rule.description.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePackValidation {
    pub valid: bool,
    pub errors: Vec<String>,
}

fn is_non_empty_trimmed_string(v: &Value) -> bool {
    matches!(v, Value::String(s) if !s.trim().is_empty())
}

fn try_compile_pattern(pattern: &str, flags: &str) -> Result<(), String> {
    // JS 'g' is behavioral (global replace), not a regex compilation flag in Rust engines.
    // 'u'/'y' have JS-specific semantics; we ignore 'g' and pass through common flags best-effort.
    // Many real packs include lookarounds; fancy-regex supports lookahead/lookbehind.
    let mut inline = String::new();
    let mut has_i = false;
    let mut has_m = false;
    for ch in flags.chars() {
        match ch {
            'g' => {}
            'i' => has_i = true,
            'm' => has_m = true,
            // 's','u','y' and others: not mapped directly; ignore for best-effort compile.
            _ => {}
        }
    }
    if has_i {
        inline.push('i');
    }
    if has_m {
        inline.push('m');
    }
    let pat = if inline.is_empty() {
        pattern.to_string()
    } else {
        format!("(?{}){}", inline, pattern)
    };
    fancy_regex::Regex::new(&pat)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Validate a caveman rule pack JSON value (structural checks + best-effort pattern compilation).
pub fn validate_rule_pack(pack: &Value) -> RulePackValidation {
    let mut errors: Vec<String> = Vec::new();
    let Some(obj) = pack.as_object() else {
        return RulePackValidation {
            valid: false,
            errors: vec!["Rule pack must be an object".to_string()],
        };
    };

    match obj.get("language") {
        Some(v) if is_non_empty_trimmed_string(v) => {}
        _ => errors.push("language must be a non-empty string".to_string()),
    }
    match obj.get("category") {
        Some(v) if is_non_empty_trimmed_string(v) => {}
        _ => errors.push("category must be a non-empty string".to_string()),
    }

    let Some(rules_val) = obj.get("rules") else {
        errors.push("rules must be an array".to_string());
        return RulePackValidation {
            valid: errors.is_empty(),
            errors,
        };
    };
    let Some(rules_arr) = rules_val.as_array() else {
        errors.push("rules must be an array".to_string());
        return RulePackValidation {
            valid: errors.is_empty(),
            errors,
        };
    };

    let valid_contexts = ["all", "user", "system", "assistant"];
    let valid_categories = ["filler", "context", "structural", "dedup", "terse", "ultra"];
    let valid_intensities = ["lite", "full", "ultra"];

    for (index, rule_val) in rules_arr.iter().enumerate() {
        let Some(entry) = rule_val.as_object() else {
            errors.push(format!("rules[{}] must be an object", index));
            continue;
        };

        let flags = match entry.get("flags") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => {
                errors.push(format!("rules[{}].flags must be a string", index));
                "gi".to_string()
            }
            None => "gi".to_string(),
        };

        // name
        match entry.get("name") {
            Some(v) if is_non_empty_trimmed_string(v) => {}
            _ => errors.push(format!("rules[{}].name must be a non-empty string", index)),
        }

        // pattern
        match entry.get("pattern") {
            Some(Value::String(p)) if !p.trim().is_empty() => {
                if let Err(msg) = try_compile_pattern(p, &flags) {
                    errors.push(format!("rules[{}].pattern is invalid: {}", index, msg));
                }
            }
            Some(Value::String(_)) => {
                errors.push(format!(
                    "rules[{}].pattern must be a non-empty string",
                    index
                ));
            }
            _ => errors.push(format!(
                "rules[{}].pattern must be a non-empty string",
                index
            )),
        }

        // replacement
        let has_replacement_string = match entry.get("replacement") {
            Some(Value::String(_)) => true,
            Some(_) => {
                errors.push(format!("rules[{}].replacement must be a string", index));
                false
            }
            None => false,
        };

        // replacementMap
        let mut has_replacement_map = false;
        match entry.get("replacementMap") {
            None => {}
            Some(Value::Object(map)) => {
                has_replacement_map = true;
                for (key, replacement) in map {
                    if key.trim().is_empty() {
                        errors.push(format!(
                            "rules[{}].replacementMap contains an empty key",
                            index
                        ));
                    }
                    if !matches!(replacement, Value::String(_)) {
                        errors.push(format!(
                            "rules[{}].replacementMap.{} must be a string",
                            index, key
                        ));
                    }
                }
            }
            Some(_) => {
                errors.push(format!("rules[{}].replacementMap must be an object", index));
            }
        }

        if !has_replacement_string && !has_replacement_map {
            errors.push(format!(
                "rules[{}] must define replacement or replacementMap",
                index
            ));
        }

        if let Some(Value::String(ctx)) = entry.get("context") {
            if !valid_contexts.contains(&ctx.as_str()) {
                errors.push(format!("rules[{}].context is invalid", index));
            }
        } else if entry.contains_key("context")
            && !matches!(entry.get("context"), Some(Value::String(_)))
        {
            errors.push(format!("rules[{}].context is invalid", index));
        }

        if let Some(Value::String(cat)) = entry.get("category") {
            if !valid_categories.contains(&cat.as_str()) {
                errors.push(format!("rules[{}].category is invalid", index));
            }
        } else if entry.contains_key("category")
            && !matches!(entry.get("category"), Some(Value::String(_)))
        {
            errors.push(format!("rules[{}].category is invalid", index));
        }

        if let Some(Value::String(mi)) = entry.get("minIntensity") {
            if !valid_intensities.contains(&mi.as_str()) {
                errors.push(format!("rules[{}].minIntensity is invalid", index));
            }
        } else if entry.contains_key("minIntensity")
            && !matches!(entry.get("minIntensity"), Some(Value::String(_)))
        {
            errors.push(format!("rules[{}].minIntensity is invalid", index));
        }
    }

    RulePackValidation {
        valid: errors.is_empty(),
        errors,
    }
}

/// Pure selector: filter rules by context and intensity rank (inclusive threshold).
pub fn filter_rules_for_context(
    rules: &[CavemanRuleMeta],
    context: CavemanRuleContext,
    intensity: CavemanIntensity,
) -> Vec<&CavemanRuleMeta> {
    let rank = intensity_rank(intensity);
    rules
        .iter()
        .filter(|rule| {
            let ctx_ok = rule.context == CavemanRuleContext::All || rule.context == context;
            let intensity_ok = intensity_rank(rule.min_intensity) <= rank;
            ctx_ok && intensity_ok
        })
        .collect()
}

/// Project file rules into metadata entries.
pub fn file_rules_to_meta(rules: &[FileRule]) -> Vec<CavemanRuleMeta> {
    rules.iter().map(CavemanRuleMeta::from_file_rule).collect()
}

/// Metadata projection similar to `getCavemanRuleMetadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CavemanRuleMetadataView {
    pub name: String,
    pub context: String,
    pub category: String,
    pub min_intensity: String,
    pub intensities: Vec<String>,
    pub description: String,
}

pub fn get_caveman_rule_metadata(rules: &[CavemanRuleMeta]) -> Vec<CavemanRuleMetadataView> {
    let all = [
        CavemanIntensity::Lite,
        CavemanIntensity::Full,
        CavemanIntensity::Ultra,
    ];
    rules
        .iter()
        .map(|rule| {
            let min_rank = intensity_rank(rule.min_intensity);
            let intensities = all
                .iter()
                .filter(|i| intensity_rank(**i) >= min_rank)
                .map(|i| i.as_str().to_string())
                .collect();
            let description = rule
                .description
                .clone()
                .unwrap_or_else(|| rule.name.replace('_', " "));
            CavemanRuleMetadataView {
                name: rule.name.clone(),
                context: rule.context.as_str().to_string(),
                category: rule.category.as_str().to_string(),
                min_intensity: rule.min_intensity.as_str().to_string(),
                intensities,
                description,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_rule_pack_structure_basic() {
        let valid = json!({
            "language": "xx",
            "category": "filler",
            "rules": [{ "name": "x", "pattern": "\\btest\\b", "replacement": "" }]
        });
        assert!(validate_rule_pack(&valid).valid);

        let invalid = json!({ "language": "xx", "category": "filler", "rules": [{}] });
        assert!(!validate_rule_pack(&invalid).valid);
    }

    #[test]
    fn validates_flags_and_replacement_maps() {
        let valid = json!({
            "language": "xx",
            "category": "context",
            "rules": [
                {
                    "name": "mapped",
                    "pattern": "\\b(?:one|two)\\b",
                    "flags": "g",
                    "replacementMap": { "one": "1", "two": "2" }
                }
            ]
        });
        assert!(validate_rule_pack(&valid).valid);

        let invalid = json!({
            "language": "xx",
            "category": "context",
            "rules": [
                {
                    "name": "bad_map",
                    "pattern": "\\b(?:one|two)\\b",
                    "replacementMap": { "one": 1 }
                }
            ]
        });
        let res = validate_rule_pack(&invalid);
        assert!(!res.valid);
        assert!(res
            .errors
            .iter()
            .any(|e| e.contains("replacementMap.one must be a string")));
    }

    #[test]
    fn validates_real_lookaround_pack_pattern() {
        let pack = json!({
            "language": "en",
            "category": "filler",
            "rules": [
                {
                    "name": "filler_adverbs",
                    "pattern": "(?<![a-z])\\b(?:basically|essentially)\\b\\s*",
                    "replacement": "",
                    "context": "all",
                    "category": "filler",
                    "minIntensity": "lite"
                }
            ]
        });
        assert!(
            validate_rule_pack(&pack).valid,
            "lookaround patterns should validate (fancy-regex)"
        );
    }

    #[test]
    fn intensity_filtering_and_metadata() {
        let rules = vec![
            CavemanRuleMeta {
                name: "lite_rule".into(),
                context: CavemanRuleContext::User,
                category: CavemanRuleCategory::Filler,
                min_intensity: CavemanIntensity::Lite,
                description: None,
            },
            CavemanRuleMeta {
                name: "ultra_rule".into(),
                context: CavemanRuleContext::All,
                category: CavemanRuleCategory::Ultra,
                min_intensity: CavemanIntensity::Ultra,
                description: Some("ultra desc".into()),
            },
            CavemanRuleMeta {
                name: "system_only".into(),
                context: CavemanRuleContext::System,
                category: CavemanRuleCategory::Context,
                min_intensity: CavemanIntensity::Lite,
                description: None,
            },
        ];
        let full_user =
            filter_rules_for_context(&rules, CavemanRuleContext::User, CavemanIntensity::Full);
        assert_eq!(full_user.len(), 1);
        assert_eq!(full_user[0].name, "lite_rule");

        let ultra_user =
            filter_rules_for_context(&rules, CavemanRuleContext::User, CavemanIntensity::Ultra);
        assert_eq!(ultra_user.len(), 2);

        let meta = get_caveman_rule_metadata(&rules);
        assert_eq!(meta[0].description, "lite rule");
        assert_eq!(meta[1].intensities, vec!["ultra"]);
    }

    #[test]
    fn file_rule_to_meta_defaults() {
        let r = FileRule {
            name: "x".into(),
            pattern: "x".into(),
            replacement: Some("".into()),
            replacement_map: None,
            flags: None,
            context: None,
            category: None,
            min_intensity: None,
            description: None,
        };
        let m = CavemanRuleMeta::from_file_rule(&r);
        assert_eq!(m.context, CavemanRuleContext::All);
        assert_eq!(m.category, CavemanRuleCategory::Filler);
        assert_eq!(m.min_intensity, CavemanIntensity::Lite);
    }
}
