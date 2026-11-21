use omniroute_gcf::{decode_generic, encode_generic};
use serde_json::json;

#[test]
fn gcf_round_trips_inline_array_pattern_losslessly_b_gcf_quote() {
    let data = json!([
        { "id": 1, "status": "x", "code": 1, "meta": { "note": "ERR[404]: Not Found" } },
        { "id": 2, "status": "y", "code": 2, "meta": { "note": "[Speaker 1]: Hello world" } },
        { "id": 3, "status": "z", "code": 3, "meta": { "note": "plain note" } }
    ]);
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}

#[test]
fn gcf_round_trips_homogeneous_simple() {
    let mut arr = Vec::new();
    for i in 1..=50 {
        arr.push(json!({
            "id": i,
            "name": format!("item-{i}"),
            "value": i * 10,
            "active": i % 2 == 0
        }));
    }
    let data = serde_json::Value::Array(arr);
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}

#[test]
fn gcf_round_trips_root_primitives_and_arrays() {
    for data in [
        json!(null),
        json!(true),
        json!(42),
        json!("hello"),
        json!([]),
        json!([1, 2, 3]),
        json!(["a", "b", "c"]),
    ] {
        let encoded = encode_generic(&data, None);
        let decoded = decode_generic(&encoded).expect("decode");
        assert_eq!(decoded, data, "failed for {}", data);
    }
}

#[test]
fn gcf_round_trips_simple_object() {
    let data = json!({
        "name": "test",
        "count": 3,
        "ok": true,
        "nested": { "a": 1, "b": "x" }
    });
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}

#[test]
fn gcf_round_trips_heterogeneous_keys() {
    let mut arr = Vec::new();
    for i in 0..10 {
        arr.push(json!({ "id": i, "name": format!("user-{i}"), "role": "admin" }));
    }
    for i in 0..10 {
        arr.push(json!({ "id": i + 10, "email": format!("u{i}@test.com"), "verified": true }));
    }
    let data = serde_json::Value::Array(arr);
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}

#[test]
fn gcf_round_trips_mixed_type_columns() {
    let mut arr = Vec::new();
    for i in 0..25 {
        let score = if i % 3 == 0 {
            json!(null)
        } else {
            json!((i + 1) * 7)
        };
        let label = if i % 4 == 0 {
            json!(null)
        } else {
            json!(format!("label-{i}"))
        };
        let value = if i % 2 == 0 {
            json!(i * 10)
        } else {
            json!(format!("str-{i}"))
        };
        arr.push(json!({ "id": i + 1, "score": score, "label": label, "value": value }));
    }
    let data = serde_json::Value::Array(arr);
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}

#[test]
fn gcf_round_trips_nested_objects_tabular() {
    let mut arr = Vec::new();
    for i in 0..20 {
        arr.push(json!({
            "id": i,
            "user": {
                "name": format!("user-{i}"),
                "email": format!("user{i}@example.com"),
                "tier": if i % 3 == 0 { "premium" } else { "free" }
            },
            "amount": i * 100
        }));
    }
    let data = serde_json::Value::Array(arr);
    let encoded = encode_generic(&data, None);
    let decoded = decode_generic(&encoded).expect("decode");
    assert_eq!(decoded, data);
}
