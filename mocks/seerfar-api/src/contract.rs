use serde_json::Value;
use std::sync::OnceLock;

pub fn endpoints() -> &'static Vec<Value> {
    static DOCS: OnceLock<Value> = OnceLock::new();
    DOCS.get_or_init(|| serde_json::from_str(include_str!("../contracts/api-docs.json")).unwrap())
        ["endpoints"]
        .as_array()
        .unwrap()
}

pub fn example(id: &str) -> Value {
    endpoints().iter().find(|e| e["id"] == id).unwrap()["responseExample"]["data"].clone()
}

/// The published form widget type is sometimes `text` for a numeric wire field.
/// docType and JSON examples define the wire contract, not the widget type.
pub fn validate(endpoint: &Value, body: &Value) -> Result<(), String> {
    if !body.is_object() {
        return Err("Bad Request".into());
    }
    for p in endpoint["bodyParams"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap();
        let parts: Vec<_> = name.split('.').collect();
        validate_path(body, &parts, p, true)?;
    }
    Ok(())
}

fn validate_path(value: &Value, parts: &[&str], p: &Value, root: bool) -> Result<(), String> {
    if let Some(array) = value.as_array() {
        for item in array {
            validate_path(item, parts, p, false)?;
        }
        return Ok(());
    }
    let child = &value[parts[0]];
    if child.is_null() {
        if p["required"] == true && (root || value.is_object()) {
            return Err("Bad Request".into());
        }
        return Ok(());
    }
    if parts.len() > 1 {
        return validate_path(child, &parts[1..], p, false);
    }
    let kind = p["docType"].as_str().unwrap_or("string");
    let valid = match kind {
        "integer" => child.as_i64().is_some() || child.as_u64().is_some(),
        "number" => child.is_number(),
        "boolean" => child.is_boolean(),
        "object" => child.is_object(),
        k if k.starts_with("array") => child.as_array().is_some_and(|a| {
            a.iter().all(|v| {
                if k.contains("string") {
                    v.is_string()
                } else if k.contains("integer") {
                    v.as_u64().is_some()
                } else {
                    true
                }
            })
        }),
        _ => child.is_string(),
    };
    if !valid {
        return Err("Bad Request".into());
    }
    if let Some(options) = p["options"].as_array() {
        if let Some(a) = child.as_array() {
            if a.iter().any(|v| !options.contains(v)) {
                return Err("Bad Request".into());
            }
        } else if !options.contains(child) {
            return Err("Bad Request".into());
        }
    }
    Ok(())
}
