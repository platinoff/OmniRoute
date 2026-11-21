//! GCF generic-profile decoder (`decode_generic`).
//! Port of `open-sse/services/compression/engines/headroom/gcf/decode_generic.ts`
//! (vendored from gcf-typescript — generic profile; MIT, GCF spec v3.2 context).

use crate::scalar::{
    parse_quoted_string, parse_scalar, split_field_decl, split_respecting_quotes, Scalar,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

pub fn decode_generic(input: &str) -> Result<Value, String> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err("missing_header: empty input".to_string());
    }

    let lines: Vec<String> = input.split('\n').map(|l| l.to_string()).collect();
    let header = lines[0].trim_end_matches('\r').to_string();
    if !header.starts_with("GCF ") {
        return Err("missing_header: first line does not begin with GCF".to_string());
    }

    let profile = parse_header_profile(&header)?;
    if profile == "graph" {
        return Err(
            "graph_profile_unsupported: this vendored build supports the generic profile only"
                .to_string(),
        );
    }
    if profile != "generic" {
        return Err(format!("unknown_profile: {}", profile));
    }

    let mut content_lines: Vec<String> = Vec::new();
    let mut summary_line = String::new();
    let mut deferred_section_count = 0usize;
    for (idx, raw) in lines.iter().enumerate().skip(1) {
        let _ = idx;
        let l = raw.trim_end_matches('\r').to_string();
        if l.is_empty() {
            continue;
        }
        // Tab check in leading whitespace.
        for ch in l.chars() {
            if ch == '\t' {
                return Err("tab_indentation: tabs in leading whitespace".to_string());
            }
            if ch != ' ' {
                break;
            }
        }
        let trimmed = l.trim_start().to_string();
        if trimmed.starts_with("# ") {
            continue;
        }
        if trimmed.starts_with("##! ") {
            summary_line = trimmed.clone();
            continue;
        }
        if trimmed.starts_with("## ") && trimmed.contains("[?]") {
            deferred_section_count += 1;
        }
        content_lines.push(l);
    }

    if !summary_line.is_empty() && deferred_section_count > 0 {
        validate_summary_counts(&summary_line, deferred_section_count, &content_lines)?;
    }

    if content_lines.is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    let first = content_lines[0].trim_start().to_string();

    // Root scalar.
    if let Some(rest) = first.strip_prefix('=') {
        if content_lines.len() > 1 {
            return Err("trailing_characters: extra lines after root scalar".to_string());
        }
        let s = parse_scalar(rest, false).map_err(|e| e.0)?;
        return Ok(scalar_to_value(s));
    }

    // Root array.
    if let Some(rest) = first.strip_prefix("## ") {
        if rest.starts_with('[') {
            let (arr, _) = parse_array_from_header(&content_lines, 0, 0, rest)?;
            return Ok(Value::Array(arr));
        }
    }

    // Root object.
    let mut result = Map::new();
    parse_object_body(&content_lines, 0, 0, &mut result)?;
    Ok(Value::Object(result))
}

fn parse_header_profile(header: &str) -> Result<String, String> {
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("missing_profile".to_string());
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut profile = String::new();
    for part in parts.iter().skip(1) {
        if let Some(eq) = part.find('=') {
            let key = &part[..eq];
            if !seen.insert(key.to_string()) {
                return Err(format!("duplicate_header_field: {}", key));
            }
            if key == "profile" {
                profile = part[eq + 1..].to_string();
            }
        } else {
            return Err(format!("malformed_header_field: {}", part));
        }
    }
    if profile.is_empty() {
        return Err("missing_profile".to_string());
    }
    Ok(profile)
}

fn f64_to_json_number(n: f64) -> Value {
    // Preserve -0.0 as float.
    if n == 0.0 && n.is_sign_negative() {
        return serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null);
    }
    if n.is_finite() && n.fract() == 0.0 {
        if (i64::MIN as f64..=i64::MAX as f64).contains(&n) {
            let i = n as i64;
            if i as f64 == n {
                return Value::Number(serde_json::Number::from(i));
            }
        }
        if (0.0..=(u64::MAX as f64)).contains(&n) {
            let u = n as u64;
            if u as f64 == n {
                return Value::Number(serde_json::Number::from(u));
            }
        }
    }
    serde_json::Number::from_f64(n)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn scalar_to_value(s: Scalar) -> Value {
    match s {
        Scalar::Null => Value::Null,
        Scalar::Bool(b) => Value::Bool(b),
        Scalar::Number(n) => f64_to_json_number(n),
        Scalar::String(st) => Value::String(st),
        Scalar::Missing => Value::Null, // should not surface at root
        Scalar::Attachment => Value::Null,
        Scalar::InlineSchema(st) => Value::String(st),
    }
}

fn parse_object_body(
    lines: &[String],
    start: usize,
    depth: usize,
    out: &mut Map<String, Value>,
) -> Result<usize, String> {
    let ind = "  ".repeat(depth);
    let mut i = start;

    while i < lines.len() {
        let line = &lines[i];
        if depth > 0 && !line.starts_with(&ind) {
            break;
        }
        let content = if depth > 0 {
            &line[ind.len()..]
        } else {
            line.as_str()
        };
        if !content.is_empty() && content.as_bytes()[0] == b' ' {
            return Err("invalid_indent: indentation increases by more than one level".to_string());
        }

        // Array section.
        if let Some(hdr) = content.strip_prefix("## ") {
            if let Some(bracket_pos) = hdr.find(" [") {
                let name = parse_key_from_header(&hdr[..bracket_pos])?;
                check_dup(out, &name)?;
                let (arr, consumed) =
                    parse_array_from_header(lines, i, depth, &hdr[bracket_pos + 1..])?;
                out.insert(name, Value::Array(arr));
                i += consumed;
                continue;
            }
            let name = parse_key_from_header(hdr)?;
            check_dup(out, &name)?;
            i += 1;
            let mut nested = Map::new();
            let consumed = parse_object_body(lines, i, depth + 1, &mut nested)?;
            out.insert(name, Value::Object(nested));
            i += consumed;
            continue;
        }

        // Key=value. Check before inline array detection.
        let eq_idx = find_key_value_split(content);
        if eq_idx > 0 {
            let eq = eq_idx as usize;
            let name = parse_key_from_header(&content[..eq])?;
            check_dup(out, &name)?;
            let s = parse_scalar(&content[eq + 1..], false).map_err(|e| e.0)?;
            out.insert(name, scalar_to_value(s));
            i += 1;
            continue;
        }

        // Inline array (e.g. items[3]: a,b,c). Only if no = found.
        if !content.starts_with('@') && !content.starts_with("##") {
            if let Some(bracket_idx) = content.find('[') {
                if bracket_idx > 0 {
                    let rest = &content[bracket_idx..];
                    if let Some(close_idx) = rest.find(']') {
                        let after = &rest[close_idx + 1..];
                        if after.starts_with(": ") || after == ":" {
                            let name = parse_key_from_header(&content[..bracket_idx])?;
                            check_dup(out, &name)?;
                            let (arr, _) = parse_array_from_header(lines, i, depth, rest)?;
                            out.insert(name, Value::Array(arr));
                            i += 1;
                            continue;
                        }
                    }
                }
            }
        }

        i += 1;
    }
    Ok(i.saturating_sub(start))
}

fn find_key_value_split(s: &str) -> isize {
    if s.is_empty() {
        return -1;
    }
    let bytes = s.as_bytes();
    // Quoted key: "key"=value
    if bytes[0] == b'"' {
        let mut j = 1usize;
        while j < bytes.len() {
            if bytes[j] == b'\\' {
                j += 2;
                continue;
            }
            if bytes[j] == b'"' {
                return if j + 1 < bytes.len() && bytes[j + 1] == b'=' {
                    j as isize + 1
                } else {
                    -1
                };
            }
            j += 1;
        }
        return -1;
    }
    // Bare key
    let eq_idx = s.find('=');
    let bracket_idx = s.find('[');
    match (eq_idx, bracket_idx) {
        (None, _) => -1,
        (Some(e), Some(b)) if b < e => -1,
        (Some(e), _) => e as isize,
    }
}

fn parse_key_from_header(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') {
        parse_quoted_string(s).map_err(|e| e.0)
    } else {
        Ok(s.to_string())
    }
}

fn check_dup(obj: &Map<String, Value>, key: &str) -> Result<(), String> {
    if obj.contains_key(key) {
        return Err(format!("duplicate_key: {}", key));
    }
    Ok(())
}

fn parse_array_from_header(
    lines: &[String],
    header_line: usize,
    depth: usize,
    bracket_part: &str,
) -> Result<(Vec<Value>, usize), String> {
    let bp = bracket_part.trim_start();
    if !bp.starts_with('[') {
        return Err("invalid_count".to_string());
    }
    let close_idx = bp.find(']').ok_or_else(|| "invalid_count".to_string())?;
    let count_str = &bp[1..close_idx];
    let after_bracket = &bp[close_idx + 1..];
    let mut count: i64 = -1;
    if count_str != "?" {
        count = parse_count(count_str)? as i64;
    }

    if count == 0 && !after_bracket.starts_with('{') && !after_bracket.starts_with(':') {
        return Ok((Vec::new(), 1));
    }

    // Inline.
    if after_bracket.starts_with(": ") || after_bracket == ":" {
        let vals_str = after_bracket.strip_prefix(": ").unwrap_or("");
        if vals_str.is_empty() {
            if count > 0 {
                return Err(format!("count_mismatch: declared {}, got 0", count));
            }
            return Ok((Vec::new(), 1));
        }
        let vals = split_respecting_quotes(vals_str, ',');
        if count >= 0 && vals.len() as i64 != count {
            return Err(format!(
                "count_mismatch: declared {}, got {}",
                count,
                vals.len()
            ));
        }
        let mut out = Vec::new();
        for v in vals {
            let s = parse_scalar(v.trim(), false).map_err(|e| e.0)?;
            out.push(scalar_to_value(s));
        }
        return Ok((out, 1));
    }

    // Tabular.
    if after_bracket.starts_with('{') {
        let brace_end = find_closing_brace_str(after_bracket)
            .ok_or_else(|| "invalid field declaration".to_string())?;
        let fields = split_field_decl(&after_bracket[..=brace_end]).map_err(|e| e.0)?;
        let (rows, consumed) = parse_tabular_body(lines, header_line + 1, depth, &fields, count)?;
        if count >= 0 && rows.len() as i64 != count {
            return Err(format!(
                "count_mismatch: declared {}, got {}",
                count,
                rows.len()
            ));
        }
        return Ok((rows, consumed + 1));
    }

    // Expanded.
    let (items, consumed) = parse_expanded_body(lines, header_line + 1, depth)?;
    if count >= 0 && items.len() as i64 != count {
        return Err(format!(
            "count_mismatch: declared {}, got {}",
            count,
            items.len()
        ));
    }
    Ok((items, consumed + 1))
}

fn find_closing_brace_str(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut in_quote = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if b == b'\\' && in_quote {
            escaped = true;
            continue;
        }
        if b == b'"' {
            in_quote = !in_quote;
            continue;
        }
        if b == b'}' && !in_quote {
            return Some(i);
        }
    }
    None
}

fn parse_count(s: &str) -> Result<usize, String> {
    if s == "0" {
        return Ok(0);
    }
    if s.is_empty() || s.starts_with('0') {
        return Err(format!("invalid_count: {}", s));
    }
    let n: usize = s.parse().map_err(|_| format!("invalid_count: {}", s))?;
    if n.to_string() != s {
        return Err(format!("invalid_count: {}", s));
    }
    Ok(n)
}

fn is_unsafe_path_key(k: &str) -> bool {
    k == "__proto__" || k == "constructor" || k == "prototype"
}

fn unflatten_paths_ordered(
    fields: &[String],
    path_columns: &HashMap<String, Vec<String>>,
    flat_values: &HashMap<String, Value>,
    flat_absent: &HashSet<String>,
) -> Vec<(String, Value)> {
    // Determine group order and per-group leaf order by scanning fields in order.
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for f in fields {
        if let Some(paths) = path_columns.get(f) {
            if paths.is_empty() {
                continue;
            }
            if paths.iter().any(|p| is_unsafe_path_key(p)) {
                continue;
            }
            let top = paths[0].clone();
            if !groups.contains_key(&top) {
                groups.insert(top.clone(), Vec::new());
                group_order.push(top.clone());
            }
            groups.get_mut(&top).unwrap().push(f.clone());
        }
    }

    let mut result: Vec<(String, Value)> = Vec::new();
    for top in group_order {
        let field_names = groups.get(&top).cloned().unwrap_or_default();
        let all_absent = field_names.iter().all(|f| flat_absent.contains(f));
        let all_null = field_names.iter().all(|f| {
            if flat_absent.contains(f) {
                return false;
            }
            flat_values.get(f).map(|v| v.is_null()).unwrap_or(false)
        });
        if all_absent {
            continue;
        }
        if all_null {
            result.push((top.clone(), Value::Null));
            continue;
        }
        let mut obj = Map::new();
        for field_name in field_names {
            if flat_absent.contains(&field_name) {
                continue;
            }
            let paths = match path_columns.get(&field_name) {
                Some(p) => p,
                None => continue,
            };
            let val = flat_values.get(&field_name).cloned().unwrap_or(Value::Null);
            ensure_path(&mut obj, paths, val);
        }
        // obj is rooted at top-level group key(s); extract the top group value.
        if let Some(v) = obj.get(&top).cloned() {
            result.push((top, v));
        } else {
            // Fallback: insert assembled structure under top.
            // ensure_path built from top down, so obj should contain top.
            result.push((top, Value::Object(obj)));
        }
    }
    result
}

fn ensure_path(root: &mut Map<String, Value>, paths: &[String], val: Value) {
    if paths.is_empty() {
        return;
    }
    if paths.len() == 1 {
        root.insert(paths[0].clone(), val);
        return;
    }
    let first = &paths[0];
    let needs_new = match root.get(first) {
        None => true,
        Some(Value::Object(_)) => false,
        _ => true,
    };
    if needs_new {
        root.insert(first.clone(), Value::Object(Map::new()));
    }
    if let Some(Value::Object(child)) = root.get_mut(first) {
        ensure_path(child, &paths[1..], val);
    }
}

fn parse_tabular_body(
    lines: &[String],
    start: usize,
    depth: usize,
    fields: &[String],
    expected_count: i64,
) -> Result<(Vec<Value>, usize), String> {
    let ind = "  ".repeat(depth);
    let mut rows: Vec<Value> = Vec::new();
    let mut i = start;

    let mut path_column_map: HashMap<String, Vec<String>> = HashMap::new();
    for f in fields {
        if f.contains('>') {
            let parts: Vec<String> = f.split('>').map(|s| s.to_string()).collect();
            if parts.iter().all(|p| !p.is_empty()) {
                path_column_map.insert(f.clone(), parts);
            }
        }
    }

    let mut inline_schemas: HashMap<String, Vec<String>> = HashMap::new();
    let mut shared_array_schemas: HashMap<String, Vec<String>> = HashMap::new();

    while i < lines.len() {
        let line = &lines[i];
        let content_opt = if depth > 0 {
            if line.starts_with(&ind) {
                Some(line[ind.len()..].to_string())
            } else {
                None
            }
        } else {
            Some(line.clone())
        };
        let content = match content_opt {
            Some(c) => c,
            None => break,
        };
        if content.starts_with("## ") || content.starts_with("##!") {
            break;
        }
        if !content.is_empty() && content.as_bytes()[0] == b' ' {
            let trimmed = content.trim_start();
            if trimmed.starts_with('.') {
                break;
            }
            break;
        }

        // Strip @N prefix
        let mut row_data = content.clone();
        let mut row_has_id = false;
        if row_data.starts_with('@') {
            if let Some(sp) = row_data.find(' ') {
                if sp > 0 {
                    let id_str = &row_data[1..sp];
                    if !id_str.is_empty() && id_str.chars().all(|c| c.is_ascii_digit()) {
                        row_data = row_data[sp + 1..].to_string();
                        row_has_id = true;
                    }
                }
            }
        }

        let vals = split_respecting_quotes(&row_data, '|');
        if vals.len() != fields.len() {
            return Err(format!(
                "row_width_mismatch: expected {}, got {}",
                fields.len(),
                vals.len()
            ));
        }

        let mut cell_values: HashMap<String, Value> = HashMap::new();
        let mut traditional_att_fields: Vec<String> = Vec::new();
        let mut inline_att_fields: Vec<String> = Vec::new();
        let mut inline_att_order: Vec<String> = Vec::new();
        let mut missing_fields: HashSet<String> = HashSet::new();
        let mut flat_values: HashMap<String, Value> = HashMap::new();
        let mut flat_absent: HashSet<String> = HashSet::new();

        for (j, field) in fields.iter().enumerate() {
            let cell_val = &vals[j];
            if path_column_map.contains_key(field) {
                let parsed = parse_scalar(cell_val, true).map_err(|e| e.0)?;
                if matches!(parsed, Scalar::Missing) {
                    flat_absent.insert(field.clone());
                } else {
                    flat_values.insert(field.clone(), scalar_to_value(parsed));
                }
                continue;
            }
            // ^{fields} inline schema declaration
            if cell_val.starts_with("^{") && cell_val.ends_with('}') {
                let schema_str = &cell_val[1..];
                let ifs = split_field_decl(schema_str).map_err(|e| e.0)?;
                inline_schemas.insert(field.clone(), ifs);
                inline_att_fields.push(field.clone());
                inline_att_order.push(field.clone());
                continue;
            }
            let parsed = parse_scalar(cell_val, true).map_err(|e| e.0)?;
            match parsed {
                Scalar::Missing => {
                    missing_fields.insert(field.clone());
                }
                Scalar::Attachment => {
                    if inline_schemas.contains_key(field) {
                        inline_att_fields.push(field.clone());
                        inline_att_order.push(field.clone());
                    } else {
                        traditional_att_fields.push(field.clone());
                    }
                }
                Scalar::InlineSchema(schema_str) => {
                    let ifs = split_field_decl(&schema_str).map_err(|e| e.0)?;
                    inline_schemas.insert(field.clone(), ifs);
                    inline_att_fields.push(field.clone());
                    inline_att_order.push(field.clone());
                }
                other => {
                    cell_values.insert(field.clone(), scalar_to_value(other));
                }
            }
        }
        i += 1;

        let mut all_att_fields: Vec<String> = traditional_att_fields.clone();
        all_att_fields.extend(inline_att_fields.clone());
        let mut attachment_values: HashMap<String, Value> = HashMap::new();

        if row_has_id {
            let mut inline_idx = 0usize;
            while i < lines.len() {
                let a_line = &lines[i];
                let a_content = if depth == 0 || a_line.starts_with(&ind) {
                    if depth > 0 {
                        a_line[ind.len()..].to_string()
                    } else {
                        a_line.clone()
                    }
                } else {
                    break;
                };

                let mut att_content = a_content.clone();
                if let Some(stripped) = att_content.strip_prefix("  .") {
                    att_content = stripped.to_string();
                }
                if let Some(rest) = att_content.strip_prefix('.') {
                    let (att_name, after_name) = parse_attachment_name(rest)?;

                    // Inline schema field with pipe-delimited data?
                    if let Some(ifs) = inline_schemas.get(&att_name).cloned() {
                        let trimmed_after = after_name.trim_start();
                        if !trimmed_after.starts_with("{}") && !trimmed_after.starts_with('[') {
                            let data = trimmed_after.to_string();
                            let inline_vals = split_respecting_quotes(&data, '|');
                            if inline_vals.len() != ifs.len() {
                                return Err(format!(
                                    "inline_width_mismatch: {} expected {}, got {}",
                                    att_name,
                                    ifs.len(),
                                    inline_vals.len()
                                ));
                            }
                            let mut obj = Map::new();
                            for (k, iv) in ifs.iter().zip(inline_vals.iter()) {
                                let p = parse_scalar(iv, true).map_err(|e| e.0)?;
                                if !matches!(p, Scalar::Missing) {
                                    obj.insert(k.clone(), scalar_to_value(p));
                                }
                            }
                            if attachment_values.contains_key(&att_name) {
                                return Err(format!("duplicate_attachment: {}", att_name));
                            }
                            attachment_values.insert(att_name, Value::Object(obj));
                            i += 1;
                            continue;
                        }
                    }

                    // Traditional attachment
                    let (name, val, consumed, parsed_fields) =
                        parse_attachment(lines, i, rest, depth + 2, &shared_array_schemas)?;
                    if attachment_values.contains_key(&name) {
                        return Err(format!("duplicate_attachment: {}", name));
                    }
                    if rows.is_empty() {
                        if let Some(pf) = parsed_fields {
                            shared_array_schemas.insert(name.clone(), pf);
                        }
                    }
                    attachment_values.insert(name, val);
                    i += consumed;
                    continue;
                }

                // No-prefix line: positional inline data.
                let mut found_inline = false;
                let mut next_inline_field = String::new();
                while inline_idx < inline_att_order.len() {
                    let candidate = &inline_att_order[inline_idx];
                    if !attachment_values.contains_key(candidate) {
                        next_inline_field = candidate.clone();
                        found_inline = true;
                        break;
                    }
                    inline_idx += 1;
                }
                if !found_inline {
                    break;
                }
                let ifs = inline_schemas
                    .get(&next_inline_field)
                    .cloned()
                    .ok_or_else(|| "missing inline schema".to_string())?;
                let inline_vals = split_respecting_quotes(&a_content, '|');
                if inline_vals.len() != ifs.len() {
                    return Err(format!(
                        "inline_width_mismatch: {} expected {}, got {}",
                        next_inline_field,
                        ifs.len(),
                        inline_vals.len()
                    ));
                }
                let mut obj = Map::new();
                for (k, iv) in ifs.iter().zip(inline_vals.iter()) {
                    let p = parse_scalar(iv, true).map_err(|e| e.0)?;
                    if !matches!(p, Scalar::Missing) {
                        obj.insert(k.clone(), scalar_to_value(p));
                    }
                }
                attachment_values.insert(next_inline_field, Value::Object(obj));
                inline_idx += 1;
                i += 1;
            }

            for f in &all_att_fields {
                if !attachment_values.contains_key(f) {
                    return Err(format!("missing_attachment: {}", f));
                }
            }

            // Check duplicate attachment peek
            if i < lines.len() {
                let peek_content = if depth == 0 || lines[i].starts_with(&ind) {
                    if depth > 0 {
                        Some(lines[i][ind.len()..].to_string())
                    } else {
                        Some(lines[i].clone())
                    }
                } else {
                    None
                };
                if let Some(pc) = peek_content {
                    let mut peek_att = pc.clone();
                    if let Some(stripped) = peek_att.strip_prefix("  .") {
                        peek_att = stripped.to_string();
                    }
                    if let Some(peek_rest) = peek_att.strip_prefix('.') {
                        let (peek_name, _) = parse_attachment_name(peek_rest)?;
                        if attachment_values.contains_key(&peek_name) {
                            return Err(format!("duplicate_attachment: {}", peek_name));
                        }
                    }
                }
            }
        }

        // Build row in original field order.
        let nested_groups = if path_column_map.is_empty() {
            Vec::new()
        } else {
            unflatten_paths_ordered(fields, &path_column_map, &flat_values, &flat_absent)
        };
        let nested_group_keys: HashSet<String> =
            nested_groups.iter().map(|(k, _)| k.clone()).collect();

        let mut row = Map::new();
        let mut inserted_nested: HashSet<String> = HashSet::new();
        for f in fields {
            if let Some(paths) = path_column_map.get(f) {
                if paths.is_empty() {
                    continue;
                }
                let top = &paths[0];
                if inserted_nested.contains(top) {
                    continue;
                }
                inserted_nested.insert(top.clone());
                if let Some((_, v)) = nested_groups.iter().find(|(k, _)| k == top) {
                    // all_absent groups are omitted by unflatten_paths_ordered
                    if nested_group_keys.contains(top) {
                        row.insert(top.clone(), v.clone());
                    }
                }
                continue;
            }
            if missing_fields.contains(f) {
                continue;
            }
            if let Some(v) = cell_values.get(f) {
                row.insert(f.clone(), v.clone());
                continue;
            }
            if let Some(v) = attachment_values.get(f) {
                row.insert(f.clone(), v.clone());
                continue;
            }
        }
        // orphan attachments (e.g. ">" fields excluded from columns)
        for (k, v) in &attachment_values {
            if !row.contains_key(k) {
                row.insert(k.clone(), v.clone());
            }
        }

        rows.push(Value::Object(row));
        if expected_count >= 0 && rows.len() as i64 >= expected_count {
            break;
        }
    }

    Ok((rows, i.saturating_sub(start)))
}

fn parse_attachment_name(rest: &str) -> Result<(String, String), String> {
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return Ok(("".to_string(), rest.to_string()));
    }
    if bytes[0] == b'"' {
        let mut j = 1usize;
        while j < bytes.len() {
            if bytes[j] == b'\\' {
                j += 2;
                continue;
            }
            if bytes[j] == b'"' {
                let name = parse_quoted_string(&rest[..j + 1]).map_err(|e| e.0)?;
                return Ok((name, rest[j + 1..].to_string()));
            }
            j += 1;
        }
        return Ok(("".to_string(), rest.to_string()));
    }
    if let Some(sp) = rest.find(' ') {
        Ok((rest[..sp].to_string(), rest[sp..].to_string()))
    } else {
        Ok((rest.to_string(), "".to_string()))
    }
}

#[allow(clippy::type_complexity)]
fn parse_attachment(
    lines: &[String],
    line_idx: usize,
    rest: &str,
    depth: usize,
    shared_schemas: &HashMap<String, Vec<String>>,
) -> Result<(String, Value, usize, Option<Vec<String>>), String> {
    let (name, after_name_raw) = parse_attachment_name(rest)?;
    let after_name = after_name_raw.trim_start().to_string();

    if after_name.starts_with("{}") {
        let mut nested = Map::new();
        let consumed = parse_object_body(lines, line_idx + 1, depth, &mut nested)?;
        return Ok((name, Value::Object(nested), consumed + 1, None));
    }

    if after_name.starts_with('[') {
        let close_bracket = after_name
            .find(']')
            .ok_or_else(|| "invalid_count: missing ]".to_string())?;
        let after_close = &after_name[close_bracket + 1..];

        // [N]{fields}
        if after_close.starts_with('{') {
            let end_brace = find_closing_brace_str(after_close);
            let mut parsed_fields: Option<Vec<String>> = None;
            if let Some(eb) = end_brace {
                if let Ok(f) = split_field_decl(&after_close[..=eb]) {
                    parsed_fields = Some(f);
                }
            }
            let (arr, consumed) = parse_array_from_header(lines, line_idx, depth, &after_name)?;
            return Ok((name, Value::Array(arr), consumed, parsed_fields));
        }

        // inline primitive array
        if after_close.starts_with(": ") || after_close == ":" {
            let (arr, consumed) = parse_array_from_header(lines, line_idx, depth, &after_name)?;
            return Ok((name, Value::Array(arr), consumed, None));
        }

        // [N] without {fields}: check shared schema
        if let Some(sf) = shared_schemas.get(&name) {
            let count_str = &after_name[1..close_bracket];
            let mut count: i64 = -1;
            if count_str != "?" {
                if let Ok(c) = parse_count(count_str) {
                    count = c as i64;
                }
            }
            if count == 0 {
                return Ok((name, Value::Array(Vec::new()), 1, None));
            }
            let next_idx = line_idx + 1;
            let ind = "  ".repeat(depth);
            let mut use_shared = true;
            if next_idx < lines.len() {
                let mut next_content = lines[next_idx].clone();
                if depth > 0 && next_content.starts_with(&ind) {
                    next_content = next_content[ind.len()..].to_string();
                }
                if next_content.trim_start().starts_with('@') {
                    use_shared = false;
                }
            }
            if use_shared {
                let (rows, consumed) = parse_tabular_body(lines, line_idx + 1, depth, sf, count)?;
                if count >= 0 && rows.len() as i64 != count {
                    return Err(format!(
                        "count_mismatch: declared {}, got {}",
                        count,
                        rows.len()
                    ));
                }
                return Ok((name, Value::Array(rows), consumed + 1, None));
            }
        }

        let (arr, consumed) = parse_array_from_header(lines, line_idx, depth, &after_name)?;
        return Ok((name, Value::Array(arr), consumed, None));
    }

    // Scalar: =value
    if let Some(val_str) = after_name.strip_prefix('=') {
        let parsed = parse_scalar(val_str, true).map_err(|e| e.0)?;
        if matches!(parsed, Scalar::Missing) {
            return Ok((name, Value::Null, 1, None));
        }
        return Ok((name, scalar_to_value(parsed), 1, None));
    }

    Err(format!("invalid attachment form: {}", after_name))
}

fn parse_expanded_body(
    lines: &[String],
    start: usize,
    depth: usize,
) -> Result<(Vec<Value>, usize), String> {
    let ind = "  ".repeat(depth);
    let mut items: Vec<Value> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = &lines[i];
        let content = if depth > 0 {
            if line.starts_with(&ind) {
                line[ind.len()..].to_string()
            } else {
                break;
            }
        } else {
            line.clone()
        };
        if content.starts_with("## ") || content.starts_with("##!") {
            break;
        }
        if !content.starts_with('@') {
            break;
        }
        let sp = match content.find(' ') {
            Some(s) => s,
            None => break,
        };
        let id_str = &content[1..sp];
        if let Ok(id) = id_str.parse::<usize>() {
            if id != items.len() {
                return Err(format!(
                    "invalid_item_id: expected @{}, got @{}",
                    items.len(),
                    id_str
                ));
            }
        }
        let marker = &content[sp + 1..];

        if let Some(rest_marker) = marker.strip_prefix('=') {
            let s = parse_scalar(rest_marker, false).map_err(|e| e.0)?;
            items.push(scalar_to_value(s));
            i += 1;
            continue;
        }
        if marker.starts_with("{}") {
            let mut nested = Map::new();
            i += 1;
            let consumed = parse_object_body(lines, i, depth + 1, &mut nested)?;
            items.push(Value::Object(nested));
            i += consumed;
            continue;
        }
        if marker.starts_with('[') {
            let (arr, consumed) = parse_array_from_header(lines, i, depth + 1, marker)?;
            items.push(Value::Array(arr));
            i += consumed;
            continue;
        }
        break;
    }
    Ok((items, i.saturating_sub(start)))
}

fn validate_summary_counts(
    summary_line: &str,
    deferred_count: usize,
    content_lines: &[String],
) -> Result<(), String> {
    let parts: Vec<&str> = summary_line.split_whitespace().collect();
    let mut counts_str = "";
    for p in parts {
        if let Some(rest) = p.strip_prefix("counts=") {
            counts_str = rest;
            break;
        }
    }
    if counts_str.is_empty() {
        return Ok(());
    }
    let count_vals: Vec<&str> = counts_str.split(',').collect();
    if count_vals.len() != deferred_count {
        return Err(format!(
            "count_mismatch: summary has {} count entries but {} deferred sections",
            count_vals.len(),
            deferred_count
        ));
    }
    let mut actual_counts: Vec<usize> = Vec::new();
    let mut in_deferred = false;
    let mut current_count = 0usize;
    for l in content_lines {
        let trimmed = l.trim_start();
        if trimmed.starts_with("## ") && trimmed.contains("[?]") {
            if in_deferred {
                actual_counts.push(current_count);
            }
            in_deferred = true;
            current_count = 0;
            continue;
        }
        if trimmed.starts_with("## ") {
            if in_deferred {
                actual_counts.push(current_count);
                in_deferred = false;
            }
            continue;
        }
        if in_deferred && !trimmed.starts_with(' ') && !trimmed.starts_with('.') {
            current_count += 1;
        }
    }
    if in_deferred {
        actual_counts.push(current_count);
    }
    for (idx, cv) in count_vals.iter().enumerate() {
        let declared: usize = cv
            .parse()
            .map_err(|_| format!("count_mismatch: invalid count value \"{}\"", cv))?;
        if idx < actual_counts.len() && declared != actual_counts[idx] {
            return Err(format!(
                "count_mismatch: section {} declared {} in summary, actual {}",
                idx, declared, actual_counts[idx]
            ));
        }
    }
    Ok(())
}
