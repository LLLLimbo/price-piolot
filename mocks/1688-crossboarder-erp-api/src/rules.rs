use serde_json::Value;
use std::{collections::BTreeMap, sync::LazyLock};

static RULES: LazyLock<BTreeMap<String, Vec<Value>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../contracts/validation-rules.json"))
        .expect("validation rules")
});

pub fn validate(name: &str, input: &Value) -> Result<(), String> {
    fn values<'a>(input: &'a Value, path: &[&str]) -> Vec<&'a Value> {
        if path.is_empty() {
            return vec![input];
        }
        if path[0] == "*" {
            input
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|v| values(v, &path[1..]))
                .collect()
        } else {
            input
                .get(path[0])
                .map(|v| values(v, &path[1..]))
                .unwrap_or_default()
        }
    }
    for rule in RULES.get(name).into_iter().flatten() {
        let path = rule["path"].as_str().unwrap();
        for value in values(input, &path.split('.').collect::<Vec<_>>()) {
            if value.is_null()
                || (name == "alibaba.product.push.syncPushProductResult"
                    && path == "platformDefinition.definitionId"
                    && value == "")
            {
                continue;
            }
            if rule["enum"]
                .as_array()
                .is_some_and(|options| !options.contains(value))
            {
                return Err(format!("{path}: unsupported enum value"));
            }
            for (key, minimum) in [("minimum", true), ("maximum", false)] {
                if let Some(bound) = rule[key].as_i64() {
                    let n = value
                        .as_i64()
                        .or_else(|| value.as_str().and_then(|v| v.parse::<i64>().ok()))
                        .ok_or_else(|| format!("{path}: integer required"))?;
                    if if minimum { n < bound } else { n > bound } {
                        return Err(format!("{path}: {key} is {bound}"));
                    }
                }
            }
            let length = value.as_str().map(|v| v.chars().count() as u64);
            let items = value.as_array().map(|v| v.len() as u64);
            for (key, actual, minimum) in [
                ("minLength", length, true),
                ("maxLength", length, false),
                ("minItems", items, true),
                ("maxItems", items, false),
            ] {
                if let Some((bound, actual)) = rule[key].as_u64().zip(actual) {
                    if (minimum && actual < bound) || (!minimum && actual > bound) {
                        return Err(format!("{path}: {key} is {bound}"));
                    }
                }
            }
        }
    }
    let get = |key| crate::business::text(input, &[key]);
    if name == "account.user.register" {
        let site = get("site");
        if !site.is_empty()
            && (!site.is_ascii() || site.chars().any(|c| c.is_whitespace() || c.is_uppercase()))
        {
            return Err("site must be lowercase without spaces".into());
        }
        let mobile = get("mobile");
        if !mobile.is_empty()
            && (mobile.chars().filter(char::is_ascii_digit).count() < 5
                || mobile
                    .chars()
                    .any(|c| !c.is_ascii_digit() && !['+', '-', ' '].contains(&c)))
        {
            return Err("invalid mobile".into());
        }
        let email = get("email");
        if !email.is_empty()
            && (!email.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty()
                    && domain.contains('.')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            }) || email.chars().any(char::is_whitespace))
        {
            return Err("invalid email".into());
        }
    }
    if name.starts_with("category.translation.")
        && get("outMemberId")
            .chars()
            .any(|c| !c.is_ascii_alphanumeric())
    {
        return Err("outMemberId must be alphanumeric".into());
    }
    if name.starts_with("alibaba.cps.getCpsRecommend") {
        let ext = get("ext");
        if !ext.is_empty() && serde_json::from_str::<Value>(&ext).is_err() {
            return Err("ext must contain JSON".into());
        }
    }
    if name == "trade.cross.orderSync" {
        if let Some(order) = crate::business::find(input, &["orderParam"]) {
            let children = crate::business::array(order, &["subOrderParamList"]);
            let required = ["productId", "productName", "skuId", "skuName"];
            if children.is_none_or(Vec::is_empty)
                && required
                    .iter()
                    .any(|key| crate::business::text(order, &[key]).is_empty())
            {
                return Err("single order requires product and SKU identity".into());
            }
            if children.is_some_and(|rows| {
                rows.iter().any(|row| {
                    required
                        .iter()
                        .any(|key| crate::business::text(row, &[key]).is_empty())
                })
            }) {
                return Err("child order requires product and SKU identity".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numeric_bounds_apply_to_documented_string_numbers() {
        for value in ["0", "51", "1.5", "invalid", "9223372036854775808"] {
            assert!(validate(
                "product.search.imageQueryBasic",
                &json!({"searchParam":{"pageSize":value}})
            )
            .is_err());
        }
        for value in ["1", "50"] {
            assert!(validate(
                "product.search.imageQueryBasic",
                &json!({"searchParam":{"pageSize":value}})
            )
            .is_ok());
        }
    }
}
