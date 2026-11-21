//! GCF generic-profile encoder (`encode_generic`).
//! Port of `open-sse/services/compression/engines/headroom/gcf/generic.ts`
//! (vendored from gcf-typescript — generic profile; MIT, GCF spec v3.2 context).

use crate::scalar::{format_key, format_scalar, Scalar};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

/// Options for controlling generic encoding behavior.
#[derive(Debug, Clone, Default)]
pub struct GenericOptions {
    /// When true, disables promotion of fixed-shape nested objects to path
    /// columns (e.g. `customer>name`). Nested objects use attachment syntax
    /// instead.
    pub no_flatten: bool,
}

pub fn encode_generic(data: &Value, opts: Option<&GenericOptions>) -> String {
    let mut out = String::from("GCF profile=generic\n");
    out.push_str(&encode_root_value(data, opts));
    out
}

fn encode_root_value(v: &Value, opts: Option<&GenericOptions>) -> String {
    match v {
        Value::Null => "=-\n".to_string(),
        Value::Array(arr) => encode_root_array(arr, opts),
        Value::Object(obj) => encode_object(obj, 0, opts),
        _ => format!("={}\n", format_scalar(&scalar_from_value(v), 0)),
    }
}

fn encode_object(
    obj: &serde_json::Map<String, Value>,
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    let prefix = indent(depth);
    let mut out = String::new();
    for (key, value) in obj.iter() {
        let fk = format_key(key);
        match value {
            Value::Array(arr) => {
                out.push_str(&encode_named_array(&fk, arr, depth, opts));
            }
            Value::Object(inner) => {
                out.push_str(&format!("{}## {}\n", prefix, fk));
                out.push_str(&encode_object(inner, depth + 1, opts));
            }
            _ => {
                out.push_str(&format!(
                    "{}{}={}\n",
                    prefix,
                    fk,
                    format_scalar(&scalar_from_value(value), 0)
                ));
            }
        }
    }
    out
}

fn encode_root_array(arr: &[Value], opts: Option<&GenericOptions>) -> String {
    if arr.is_empty() {
        return "## [0]\n".to_string();
    }
    if all_primitives(arr) {
        let vals: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(&scalar_from_value(v), 0x2c))
            .collect();
        return format!("## [{}]: {}\n", arr.len(), vals.join(","));
    }
    if let Some(fields) = tabular_fields(arr) {
        return encode_tabular("## ", arr, &fields, 0, opts);
    }
    encode_expanded("## ", arr, 0, opts)
}

fn encode_named_array(
    name: &str,
    arr: &[Value],
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    let prefix = indent(depth);
    if arr.is_empty() {
        return format!("{}## {} [0]\n", prefix, name);
    }
    if all_primitives(arr) {
        let vals: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(&scalar_from_value(v), 0x2c))
            .collect();
        return format!("{}{}[{}]: {}\n", prefix, name, arr.len(), vals.join(","));
    }
    if let Some(fields) = tabular_fields(arr) {
        return encode_tabular(
            &format!("{}## {} ", prefix, name),
            arr,
            &fields,
            depth,
            opts,
        );
    }
    encode_expanded(&format!("{}## {} ", prefix, name), arr, depth, opts)
}

fn all_primitives(arr: &[Value]) -> bool {
    arr.iter()
        .all(|v| !matches!(v, Value::Object(_) | Value::Array(_)))
}

fn scalar_from_value(v: &Value) -> Scalar {
    match v {
        Value::Null => Scalar::Null,
        Value::Bool(b) => Scalar::Bool(*b),
        Value::Number(n) => Scalar::Number(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => Scalar::String(s.clone()),
        _ => Scalar::String(v.to_string()),
    }
}

fn tabular_fields(arr: &[Value]) -> Option<Vec<String>> {
    if arr.is_empty() {
        return None;
    }
    let mut field_order: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for item in arr {
        let obj = match item {
            Value::Object(m) => m,
            _ => return None,
        };
        for k in obj.keys() {
            if seen.insert(k.clone()) {
                field_order.push(k.clone());
            }
        }
    }
    if field_order.is_empty() {
        None
    } else {
        Some(field_order)
    }
}

fn is_primitive_value(v: &Value) -> bool {
    !matches!(v, Value::Object(_) | Value::Array(_))
}

fn inline_schema_fields(arr: &[Value], field_name: &str) -> Option<Vec<String>> {
    let first = arr.first()?.as_object()?;
    let first_val = first.get(field_name)?;
    if !first_val.is_object() {
        return None;
    }
    let mut canonical_keys: Option<Vec<String>> = None;
    for item in arr {
        let obj = item.as_object()?;
        if let Some(v) = obj.get(field_name) {
            if v.is_null() {
                continue;
            }
            let inner = v.as_object()?;
            let keys: Vec<String> = inner.keys().cloned().collect();
            for k in &keys {
                let val = inner.get(k)?;
                if !is_primitive_value(val) {
                    return None;
                }
            }
            match &canonical_keys {
                None => canonical_keys = Some(keys),
                Some(c) => {
                    if &keys != c {
                        return None;
                    }
                }
            }
        }
    }
    let c = canonical_keys?;
    if c.len() < 3 {
        None
    } else {
        Some(c)
    }
}

fn shared_array_schema(arr: &[Value], field_name: &str) -> Option<Vec<String>> {
    let first = arr.first()?.as_object()?;
    let first_val = first.get(field_name)?;
    if !first_val.is_array() {
        return None;
    }
    let mut canonical_fields: Option<Vec<String>> = None;
    for item in arr {
        let obj = item.as_object()?;
        if let Some(v) = obj.get(field_name) {
            if v.is_null() {
                continue;
            }
            let a = v.as_array()?;
            let fields = tabular_fields(a)?;
            for arr_item in a {
                let o = arr_item.as_object()?;
                for val in o.values() {
                    if !is_primitive_value(val) {
                        return None;
                    }
                }
            }
            match &canonical_fields {
                None => canonical_fields = Some(fields),
                Some(c) => {
                    if &fields != c {
                        return None;
                    }
                }
            }
        }
    }
    canonical_fields
}

fn is_unsafe_key(k: &str) -> bool {
    k == "__proto__" || k == "constructor" || k == "prototype"
}

#[derive(Clone)]
struct FlatLeaf {
    path: String,
    keys: Vec<String>,
}

fn analyze_flattenable(
    arr: &[Value],
    field_name: &str,
    parent_path: &str,
) -> Option<Vec<FlatLeaf>> {
    if field_name.contains('>') {
        return None;
    }
    let mut canonical_shape: Option<Vec<(String, &'static str)>> = None; // preserves order: (key, "scalar"/"nested")

    for item in arr {
        let obj = item.as_object()?;
        if !obj.contains_key(field_name) {
            continue;
        }
        let v = obj.get(field_name)?;
        if v.is_null() {
            if !parent_path.is_empty() {
                return None;
            }
            continue;
        }
        let inner = match v {
            Value::Object(m) => m,
            _ => return None,
        };
        let keys: Vec<String> = inner.keys().cloned().collect();
        match &canonical_shape {
            None => {
                let mut shape = Vec::new();
                for k in &keys {
                    if k.contains('>') || is_unsafe_key(k) {
                        return None;
                    }
                    let val = inner.get(k)?;
                    if let Value::Array(_) = val {
                        return None;
                    } else if let Value::Object(_) = val {
                        if val.is_null() {
                            shape.push((k.clone(), "scalar"));
                        } else {
                            shape.push((k.clone(), "nested"));
                        }
                    } else {
                        shape.push((k.clone(), "scalar"));
                    }
                }
                canonical_shape = Some(shape);
            }
            Some(shape) => {
                if keys.len() != shape.len() {
                    return None;
                }
                let shape_map: HashMap<&str, &str> =
                    shape.iter().map(|(k, t)| (k.as_str(), *t)).collect();
                for k in &keys {
                    let expected = *shape_map.get(k.as_str())?;
                    let val = inner.get(k)?;
                    if expected == "scalar" {
                        if matches!(val, Value::Object(_) | Value::Array(_)) {
                            return None;
                        }
                    } else {
                        // nested
                        if !val.is_null() && !matches!(val, Value::Object(_)) {
                            return None;
                        }
                        if matches!(val, Value::Array(_)) {
                            return None;
                        }
                    }
                }
            }
        }
    }

    let canonical_shape = canonical_shape?;
    let current_path = if parent_path.is_empty() {
        field_name.to_string()
    } else {
        format!("{}>{}", parent_path, field_name)
    };
    let parent_keys: Vec<String> = if parent_path.is_empty() {
        vec![field_name.to_string()]
    } else {
        let mut p: Vec<String> = parent_path.split('>').map(|s| s.to_string()).collect();
        p.push(field_name.to_string());
        p
    };

    let mut leaves: Vec<FlatLeaf> = Vec::new();
    for (k, kind) in &canonical_shape {
        if *kind == "scalar" {
            let mut keys = parent_keys.clone();
            keys.push(k.clone());
            leaves.push(FlatLeaf {
                path: format!("{}>{}", current_path, k),
                keys,
            });
        } else {
            let sub_arr: Vec<Value> = arr
                .iter()
                .map(|item| {
                    let obj = item.as_object();
                    if let Some(obj) = obj {
                        if let Some(v) = obj.get(field_name) {
                            if v.is_null() {
                                return Value::Object(serde_json::Map::new());
                            }
                            return v.clone();
                        }
                    }
                    Value::Object(serde_json::Map::new())
                })
                .collect();
            let sub_leaves = analyze_flattenable(&sub_arr, k, &current_path)?;
            if sub_leaves.is_empty() {
                return None;
            }
            leaves.extend(sub_leaves);
        }
    }

    if !leaves.is_empty() {
        for item in arr {
            let obj = match item.as_object() {
                Some(o) => o,
                None => continue,
            };
            if !obj.contains_key(field_name) {
                continue;
            }
            let v = obj.get(field_name)?;
            if v.is_null() {
                continue;
            }
            let all_null = leaves.iter().all(|leaf| {
                let resolved = resolve_key_chain(item, &leaf.keys);
                resolved.exists && resolved.value.is_null()
            });
            if all_null {
                return None;
            }
        }
    }

    Some(leaves)
}

#[derive(Clone)]
struct Resolved {
    value: Value,
    exists: bool,
}

fn resolve_key_chain(item: &Value, keys: &[String]) -> Resolved {
    if keys.is_empty() {
        return Resolved {
            value: Value::Null,
            exists: false,
        };
    }
    let obj = match item.as_object() {
        Some(o) => o,
        None => {
            return Resolved {
                value: Value::Null,
                exists: false,
            }
        }
    };
    if !obj.contains_key(&keys[0]) {
        return Resolved {
            value: Value::Null,
            exists: false,
        };
    }
    let mut current = obj.get(&keys[0]).cloned().unwrap_or(Value::Null);
    if current.is_null() {
        return Resolved {
            value: Value::Null,
            exists: true,
        };
    }
    for k in keys.iter().skip(1) {
        let c = match current.as_object() {
            Some(o) => o,
            None => {
                return Resolved {
                    value: Value::Null,
                    exists: false,
                }
            }
        };
        if !c.contains_key(k) {
            return Resolved {
                value: Value::Null,
                exists: false,
            };
        }
        current = c.get(k).cloned().unwrap_or(Value::Null);
    }
    Resolved {
        value: current,
        exists: true,
    }
}

#[derive(Clone)]
struct FlatColumn {
    header_name: String,
    col_type: &'static str, // "flat" or "original"
    field: String,
    keys: Vec<String>,
}

fn encode_tabular(
    header_prefix: &str,
    arr: &[Value],
    fields: &[String],
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    let prefix = indent(depth);

    // Phase 0: Analyze fields for flattening.
    let mut flatten_map: HashMap<String, Vec<FlatLeaf>> = HashMap::new();
    let no_flatten = opts.map(|o| o.no_flatten).unwrap_or(false);
    if !no_flatten {
        for f in fields {
            if let Some(leaves) = analyze_flattenable(arr, f, "") {
                if !leaves.is_empty() {
                    flatten_map.insert(f.clone(), leaves);
                }
            }
        }
    }

    let mut gt_fields: HashSet<String> = HashSet::new();
    for f in fields {
        if !flatten_map.contains_key(f) && f.contains('>') {
            gt_fields.insert(f.clone());
        }
    }

    let mut columns: Vec<FlatColumn> = Vec::new();
    for f in fields {
        if gt_fields.contains(f) {
            continue;
        }
        if let Some(leaves) = flatten_map.get(f) {
            for leaf in leaves {
                columns.push(FlatColumn {
                    header_name: format_key(&leaf.path),
                    col_type: "flat",
                    field: f.clone(),
                    keys: leaf.keys.clone(),
                });
            }
        } else {
            columns.push(FlatColumn {
                header_name: format_key(f),
                col_type: "original",
                field: f.clone(),
                keys: Vec::new(),
            });
        }
    }

    if columns.is_empty() {
        return encode_expanded(header_prefix, arr, depth, opts);
    }

    let mut inline_schemas: HashMap<String, Vec<String>> = HashMap::new();
    let mut shared_arr_schemas: HashMap<String, Vec<String>> = HashMap::new();
    for f in fields {
        if flatten_map.contains_key(f) {
            continue;
        }
        if let Some(ifs) = inline_schema_fields(arr, f) {
            inline_schemas.insert(f.clone(), ifs);
        }
        if let Some(sas) = shared_array_schema(arr, f) {
            shared_arr_schemas.insert(f.clone(), sas);
        }
    }

    let header_fields: Vec<String> = columns.iter().map(|c| c.header_name.clone()).collect();
    let mut out = format!(
        "{}[{}]{{{}}}\n",
        header_prefix,
        arr.len(),
        header_fields.join(",")
    );

    for (i, item) in arr.iter().enumerate() {
        let obj = item.as_object().cloned().unwrap_or_default();
        let mut cells: Vec<String> = Vec::new();
        let mut attachments: Vec<AttachmentEmit> = Vec::new();
        let mut row_has_attachment = false;

        for col in &columns {
            if col.col_type == "flat" {
                if !obj.contains_key(&col.keys[0]) {
                    cells.push("~".to_string());
                } else {
                    let top_val = obj.get(&col.keys[0]).cloned().unwrap_or(Value::Null);
                    if top_val.is_null() {
                        cells.push("-".to_string());
                    } else {
                        let resolved = resolve_key_chain(item, &col.keys);
                        if !resolved.exists {
                            cells.push("~".to_string());
                        } else if resolved.value.is_null() {
                            cells.push("-".to_string());
                        } else {
                            cells.push(format_scalar(&scalar_from_value(&resolved.value), 0x7c));
                        }
                    }
                }
                continue;
            }

            let f = &col.field;
            if !obj.contains_key(f) {
                cells.push("~".to_string());
                continue;
            }
            let v = obj.get(f).cloned().unwrap_or(Value::Null);
            if v.is_null() {
                cells.push("-".to_string());
                continue;
            }
            if v.is_object() || v.is_array() {
                if let Some(ifs) = inline_schemas.get(f) {
                    if v.is_object() {
                        if i == 0 {
                            let fmt_if: Vec<String> = ifs.iter().map(|k| format_key(k)).collect();
                            cells.push(format!("^{{{}}}", fmt_if.join(",")));
                        } else {
                            cells.push("^".to_string());
                        }
                        attachments.push(AttachmentEmit {
                            name: f.clone(),
                            value: v.clone(),
                            inline: true,
                            inline_fields: Some(ifs.clone()),
                        });
                    } else {
                        cells.push("^".to_string());
                        attachments.push(AttachmentEmit {
                            name: f.clone(),
                            value: v.clone(),
                            inline: false,
                            inline_fields: None,
                        });
                    }
                } else {
                    cells.push("^".to_string());
                    attachments.push(AttachmentEmit {
                        name: f.clone(),
                        value: v.clone(),
                        inline: false,
                        inline_fields: None,
                    });
                }
                row_has_attachment = true;
            } else {
                cells.push(format_scalar(&scalar_from_value(&v), 0x7c));
            }
        }

        for f in fields {
            if !gt_fields.contains(f) {
                continue;
            }
            if !obj.contains_key(f) {
                continue;
            }
            row_has_attachment = true;
            attachments.push(AttachmentEmit {
                name: f.clone(),
                value: obj.get(f).cloned().unwrap_or(Value::Null),
                inline: false,
                inline_fields: None,
            });
        }

        let row = cells.join("|");
        if row_has_attachment {
            out.push_str(&format!("{}@{} {}\n", prefix, i, row));
        } else {
            out.push_str(&format!("{}{}\n", prefix, row));
        }

        for att in &attachments {
            let fk = format_key(&att.name);
            if att.inline {
                if let Some(ifs) = &att.inline_fields {
                    if let Some(vobj) = att.value.as_object() {
                        let vals: Vec<String> = ifs
                            .iter()
                            .map(|inf| {
                                if !vobj.contains_key(inf) {
                                    "~".to_string()
                                } else {
                                    format_scalar(
                                        &scalar_from_value(vobj.get(inf).unwrap_or(&Value::Null)),
                                        0x7c,
                                    )
                                }
                            })
                            .collect();
                        out.push_str(&format!("{}{}\n", prefix, vals.join("|")));
                    }
                }
            } else if let Some(arr_att) = att.value.as_array() {
                if let Some(sas) = shared_arr_schemas.get(&att.name) {
                    if i > 0 {
                        out.push_str(&encode_attachment_array_shared(
                            &prefix,
                            &fk,
                            arr_att,
                            depth + 2,
                            sas,
                            opts,
                        ));
                    } else {
                        out.push_str(&encode_attachment_array(
                            &prefix,
                            &fk,
                            arr_att,
                            depth + 2,
                            opts,
                        ));
                    }
                } else {
                    out.push_str(&encode_attachment_array(
                        &prefix,
                        &fk,
                        arr_att,
                        depth + 2,
                        opts,
                    ));
                }
            } else if let Some(inner_obj) = att.value.as_object() {
                out.push_str(&format!("{}.{} {{}}\n", prefix, fk));
                out.push_str(&encode_object(inner_obj, depth + 2, opts));
            } else {
                // Scalar attachment
                if att.value.is_null() {
                    out.push_str(&format!("{}.{} =-\n", prefix, fk));
                } else {
                    out.push_str(&format!(
                        "{}.{} ={}\n",
                        prefix,
                        fk,
                        format_scalar(&scalar_from_value(&att.value), 0)
                    ));
                }
            }
        }
    }
    out
}

struct AttachmentEmit {
    name: String,
    value: Value,
    inline: bool,
    inline_fields: Option<Vec<String>>,
}

fn encode_attachment_array(
    att_prefix: &str,
    fk: &str,
    arr: &[Value],
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    if arr.is_empty() {
        return format!("{}.{} [0]\n", att_prefix, fk);
    }
    if all_primitives(arr) {
        let vals: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(&scalar_from_value(v), 0x2c))
            .collect();
        return format!(
            "{}.{} [{}]: {}\n",
            att_prefix,
            fk,
            arr.len(),
            vals.join(",")
        );
    }
    if let Some(fields) = tabular_fields(arr) {
        return encode_tabular(
            &format!("{}.{} ", att_prefix, fk),
            arr,
            &fields,
            depth,
            opts,
        );
    }
    encode_expanded(&format!("{}.{} ", att_prefix, fk), arr, depth, opts)
}

fn encode_attachment_array_shared(
    att_prefix: &str,
    fk: &str,
    arr: &[Value],
    depth: usize,
    shared_fields: &[String],
    opts: Option<&GenericOptions>,
) -> String {
    if arr.is_empty() {
        return format!("{}.{} [0]\n", att_prefix, fk);
    }
    if all_primitives(arr) {
        let vals: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(&scalar_from_value(v), 0x2c))
            .collect();
        return format!(
            "{}.{} [{}]: {}\n",
            att_prefix,
            fk,
            arr.len(),
            vals.join(",")
        );
    }
    if let Some(fields) = tabular_fields(arr) {
        if fields == shared_fields {
            let prefix = indent(depth);
            let mut out = format!("{}.{} [{}]\n", att_prefix, fk, arr.len());
            for item in arr {
                let obj = item.as_object().cloned().unwrap_or_default();
                let cells: Vec<String> = shared_fields
                    .iter()
                    .map(|f| {
                        if !obj.contains_key(f) {
                            "~".to_string()
                        } else {
                            let v = obj.get(f).unwrap_or(&Value::Null);
                            if v.is_null() {
                                "-".to_string()
                            } else {
                                format_scalar(&scalar_from_value(v), 0x7c)
                            }
                        }
                    })
                    .collect();
                out.push_str(&format!("{}{}\n", prefix, cells.join("|")));
            }
            return out;
        }
    }
    encode_attachment_array(att_prefix, fk, arr, depth, opts)
}

fn encode_expanded(
    header_prefix: &str,
    arr: &[Value],
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    let prefix = indent(depth);
    let mut out = format!("{}[{}]\n", header_prefix, arr.len());
    for (i, item) in arr.iter().enumerate() {
        match item {
            Value::Array(inner) => {
                out.push_str(&encode_expanded_array_item(&prefix, i, inner, depth, opts));
            }
            Value::Object(obj) => {
                out.push_str(&format!("{}@{} {{}}\n", prefix, i));
                out.push_str(&encode_object(obj, depth + 1, opts));
            }
            _ => {
                out.push_str(&format!(
                    "{}@{} ={}\n",
                    prefix,
                    i,
                    format_scalar(&scalar_from_value(item), 0)
                ));
            }
        }
    }
    out
}

fn encode_expanded_array_item(
    prefix: &str,
    idx: usize,
    arr: &[Value],
    depth: usize,
    opts: Option<&GenericOptions>,
) -> String {
    if arr.is_empty() {
        return format!("{}@{} [0]\n", prefix, idx);
    }
    if all_primitives(arr) {
        let vals: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(&scalar_from_value(v), 0x2c))
            .collect();
        return format!("{}@{} [{}]: {}\n", prefix, idx, arr.len(), vals.join(","));
    }
    if let Some(fields) = tabular_fields(arr) {
        return encode_tabular(
            &format!("{}@{} ", prefix, idx),
            arr,
            &fields,
            depth + 1,
            opts,
        );
    }
    encode_expanded(&format!("{}@{} ", prefix, idx), arr, depth + 1, opts)
}
