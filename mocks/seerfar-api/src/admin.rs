use crate::{
    catalog,
    query::identity,
    state::{Account, Key, Policy, Store, MAX_TIME},
    tasks, App,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn output(result: Result<Value, String>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn state(State(app): State<App>) -> Json<Value> {
    let mut s = app.store.lock().unwrap();
    s.tick();
    Json(serde_json::to_value(&*s).unwrap())
}
pub async fn reset(State(app): State<App>, Json(input): Json<Value>) -> Response {
    output((|| {
        let mut s = app.store.lock().unwrap();
        let mut next = if let Some(v) = input.get("state") {
            serde_json::from_value::<Store>(v.clone()).map_err(|e| e.to_string())?
        } else {
            let fixed = match input.get("time") {
                None => s.fixed_time,
                Some(Value::Null) => None,
                Some(v) => Some(
                    v.as_i64()
                        .filter(|v| (0..=MAX_TIME - 30 * 86_400_000).contains(v))
                        .ok_or("invalid time")?,
                ),
            };
            Store::seed(s.base_url.clone(), fixed)
        };
        next.validate()?;
        next.generation = s.generation.checked_add(1).ok_or("generation overflow")?;
        *s = next;
        Ok(json!({"result":true}))
    })())
}
pub async fn advance(State(app): State<App>, Json(input): Json<Value>) -> Response {
    output((|| {
        let ms = input["milliseconds"]
            .as_i64()
            .filter(|v| *v >= 0)
            .ok_or("milliseconds must be nonnegative")?;
        let mut s = app.store.lock().unwrap();
        let now = s
            .now()
            .checked_add(ms)
            .filter(|v| *v <= MAX_TIME - 30 * 86_400_000)
            .ok_or("time overflow")?;
        s.fixed_time = Some(now);
        s.tick();
        Ok(json!({"time":now}))
    })())
}
pub fn validate_faults(rules: &[Value]) -> Result<(), String> {
    for r in rules {
        if !r["path"]
            .as_str()
            .is_some_and(|v| crate::contract::endpoints().iter().any(|e| e["path"] == v))
            || r["remaining"].as_u64().is_none()
            || r.get("status")
                .is_some_and(|v| v.as_u64().is_none_or(|n| !(200..=599).contains(&n)))
            || r.get("delay_ms")
                .is_some_and(|v| v.as_u64().is_none_or(|n| n > 60_000))
            || r.get("method")
                .is_some_and(|v| !matches!(v.as_str(), Some("GET" | "POST")))
            || ["account", "api_key"]
                .iter()
                .any(|k| r.get(k).is_some_and(|v| !v.is_string()))
        {
            return Err("invalid fault rule".into());
        }
    }
    Ok(())
}
pub async fn faults(State(app): State<App>, Json(input): Json<Value>) -> Response {
    output((|| {
        let rules = input.as_array().ok_or("fault rules must be an array")?;
        validate_faults(rules)?;
        app.store.lock().unwrap().faults = rules.clone();
        Ok(json!({"result":true}))
    })())
}
fn patch<T: serde::Serialize + DeserializeOwned>(
    old: T,
    input: &Value,
    fields: &[&str],
) -> Result<T, String> {
    let mut next = serde_json::to_value(old).unwrap();
    for field in fields {
        if let Some(v) = input.get(field) {
            next[*field] = v.clone();
        }
    }
    serde_json::from_value(next).map_err(|e| e.to_string())
}
fn text<'a>(input: &'a Value, name: &str) -> Result<&'a str, String> {
    input[name]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}
pub async fn actions(State(app): State<App>, Json(input): Json<Value>) -> Response {
    output((|| {
        let mut current = app.store.lock().unwrap();
        current.tick();
        let mut s = current.clone();
        let action = text(&input, "action")?;
        let account = input["account"].as_str().unwrap_or("123");
        if action != "account_upsert" && !s.accounts.contains_key(account) {
            return Err("account not found".into());
        }
        let result = apply(&mut s, account, action, &input)?;
        s.validate()?;
        *current = s;
        Ok(result)
    })())
}
fn apply(s: &mut Store, account: &str, action: &str, input: &Value) -> Result<Value, String> {
    match action {
        "account_upsert" => {
            let old = s.accounts.get(account).cloned().unwrap_or(Account {
                name: account.into(),
                active: true,
                terms_accepted: true,
                ozon_authorized: true,
                partner_type: 0,
                sub_accounts: BTreeMap::new(),
            });
            let account = patch(
                old,
                input,
                &[
                    "name",
                    "active",
                    "terms_accepted",
                    "ozon_authorized",
                    "partner_type",
                    "sub_accounts",
                ],
            )?;
            s.accounts
                .insert(input["account"].as_str().unwrap_or("123").into(), account);
        }
        "key_upsert" => {
            let token = text(input, "api_key")?;
            if token.chars().any(char::is_whitespace) {
                return Err("API Key cannot contain whitespace".into());
            }
            let old = s.keys.get(token).cloned().unwrap_or(Key {
                account: account.into(),
                active: true,
                credit_limit: 10000,
                credit_used: 0,
            });
            if old.account != account {
                return Err("key belongs to another account".into());
            }
            s.keys.insert(
                token.into(),
                patch(old, input, &["active", "credit_limit", "credit_used"])?,
            );
        }
        "product_upsert" => {
            let product = input["product"]
                .as_object()
                .ok_or("product must be an object")?;
            let sku = identity(&input["product"]["sku"]);
            if input["product"]["sku"].as_u64().is_none_or(|n| n == 0) {
                return Err("sku must be a positive integer".into());
            }
            let mut p = s.products.get(&sku).cloned().unwrap_or_else(|| {
                json!({
                    "sku": input["product"]["sku"], "title": "", "price": 0,
                    "sales": 0, "revenue": 0, "stock": 0, "reviewCount": 0,
                    "reviewRating": null, "fulfillment": [], "imageUrl": null,
                    "variationIds": [], "characteristics": [], "categoryInfo": null,
                })
            });
            for (k, v) in product {
                p[k] = v.clone();
            }
            if let Some(category) = input["category_id"].as_str() {
                let c = s.categories.get(category).ok_or("category not found")?;
                p["categoryInfo"] = json!({"category":c,"fullCategoryId":[category],"titlePath":c["title"],"cnTitlePath":c["cnTitle"],"enTitlePath":c["enTitle"]});
            }
            if let Some(shop) = s.shops.get(&identity(&p["sellerId"])) {
                p["sellerName"] = shop["name"].clone();
            }
            if let Some(images) = p["imageUrls"].as_array() {
                p["imageUrl"] = images.first().cloned().unwrap_or(Value::Null);
            }
            s.products.insert(sku.clone(), p);
            if let Some(rows) = input.get("history") {
                s.history.insert(
                    sku.clone(),
                    serde_json::from_value(rows.clone()).map_err(|e| e.to_string())?,
                );
            } else if product.contains_key("sales") || product.contains_key("revenue") {
                use rust_decimal::Decimal;
                use std::str::FromStr;
                let p = &s.products[&sku];
                let sales = p["sales"].as_u64().ok_or("sales must be nonnegative")?;
                let revenue = Decimal::from_str(&p["revenue"].to_string())
                    .map_err(|_| "revenue must be a number")?;
                let daily = (revenue / Decimal::from(30))
                    .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero);
                let now = s.now();
                let rows = (0..30)
                    .map(|i| {
                        let amount = if i == 29 {
                            revenue - daily * Decimal::from(29)
                        } else {
                            daily
                        };
                        json!({
                            "date": crate::state::date(now - (29-i) * 86_400_000),
                            "sales": sales/30 + u64::from((i as u64) < sales%30),
                            "revenue": serde_json::from_str::<Value>(&amount.to_string()).unwrap(),
                            "price": p["price"], "stock": p["stock"],
                            "reviewCount": p["reviewCount"], "reviewRating": p["reviewRating"],
                        })
                    })
                    .collect();
                s.history.insert(sku.clone(), rows);
            }
            if !s.review_results.contains_key(&sku) {
                s.review_results
                    .insert(sku.clone(), catalog::review_template(&sku));
            }
            return Ok(json!({"result":true,"sku":sku}));
        }
        "product_delete" => {
            let sku = identity(&input["sku"]);
            s.products.remove(&sku).ok_or("product not found")?;
            s.history.remove(&sku);
            s.monthly.remove(&sku);
            s.review_results.remove(&sku);
            for p in s.products.values_mut() {
                if let Some(ids) = p["variationIds"].as_array_mut() {
                    ids.retain(|id| identity(id) != sku);
                }
            }
        }
        "shop_upsert" => {
            let id = identity(&input["shop"]["id"]);
            if !input["shop"].is_object() || !input["shop"]["name"].is_string() {
                return Err("invalid shop".into());
            }
            let mut row = s.shops.get(&id).cloned().unwrap_or_else(|| json!({}));
            for (k, v) in input["shop"].as_object().unwrap() {
                row[k] = v.clone();
            }
            s.shops.insert(id.clone(), row.clone());
            for p in s
                .products
                .values_mut()
                .filter(|p| identity(&p["sellerId"]) == id)
            {
                p["sellerName"] = row["name"].clone();
            }
        }
        "category_upsert" => {
            let id = text(&input["category"], "id")?;
            let mut row = s.categories.get(id).cloned().unwrap_or_else(|| json!({}));
            for (k, v) in input["category"].as_object().ok_or("invalid category")? {
                row[k] = v.clone();
            }
            s.categories.insert(id.into(), row.clone());
            for p in s
                .products
                .values_mut()
                .filter(|p| p["categoryInfo"]["category"]["id"] == id)
            {
                p["categoryInfo"]["category"] = row.clone();
            }
        }
        "history_set" | "monthly_set" => {
            let sku = identity(&input["sku"]);
            if !s.products.contains_key(&sku) {
                return Err("product not found".into());
            }
            let rows = serde_json::from_value(input["rows"].clone()).map_err(|e| e.to_string())?;
            if action == "history_set" {
                s.history.insert(sku, rows);
            } else {
                s.monthly.insert(sku, rows);
            }
        }
        "market_upsert" => {
            let kind = text(input, "kind")?;
            let rows = s.market.get_mut(kind).ok_or("invalid market kind")?;
            let mut row = input["record"].clone();
            let id = text(&row, "id")?.to_string();
            text(&row, "query")?;
            if let Some(ids) = input.get("sku_ids") {
                if !ids.is_array() {
                    return Err("sku_ids must be an array".into());
                }
                row["_skuIds"] = ids.clone();
            }
            if let Some(old) = rows.iter_mut().find(|v| v["id"] == id) {
                for (k, v) in row.as_object().unwrap() {
                    old[k] = v.clone();
                }
            } else {
                rows.push(row);
            }
        }
        "review_result_set" => {
            let sku = identity(&input["sku"]);
            if !s.products.contains_key(&sku) {
                return Err("product not found".into());
            }
            let lang = input["language"].as_str().unwrap_or("CN");
            if !["CN", "EN", "RU"].contains(&lang) {
                return Err("invalid language".into());
            }
            let data = s
                .review_results
                .entry(sku.clone())
                .or_insert_with(|| catalog::review_template(&sku));
            for k in ["report", "word_frequency", "statistic", "original"] {
                if let Some(v) = input.get(k) {
                    if !v.is_object() {
                        return Err(format!("{k} must be an object"));
                    }
                    if ["report", "word_frequency"].contains(&k) {
                        data[lang][k] = v.clone();
                    } else {
                        data[k] = v.clone();
                    }
                }
            }
        }
        "task_transition" => {
            let id = text(input, "task_id")?;
            let t = s.tasks.get(id).ok_or("task not found")?;
            if t.account != account {
                return Err("task belongs to another account".into());
            }
            match text(input, "status")? {
                "analyzing" if t.kind == "review" && t.status == "getting_reviews" => {
                    let due = s.now() + s.task_delay_ms;
                    let t = s.tasks.get_mut(id).unwrap();
                    t.status = "analyzing".into();
                    t.due = due;
                }
                "success" => {
                    if !s.products.contains_key(&t.sku) {
                        return Err("product not found".into());
                    }
                    tasks::finish(s, id, true, None, None)?;
                }
                "failed" => {
                    let reason = input["reason"].as_i64().unwrap_or(5059) as i32;
                    if ![5001, 5002, 5059].contains(&reason) {
                        return Err("invalid failure reason".into());
                    }
                    tasks::finish(s, id, false, Some(reason), input["fail_type"].as_str())?;
                }
                _ => return Err("invalid task transition".into()),
            }
        }
        "collection_edit" => {
            let id = text(input, "collection_id")?;
            let c = s.collections.get_mut(id).ok_or("collection not found")?;
            if c.account != account {
                return Err("collection belongs to another account".into());
            }
            let data = input["data"].as_object().ok_or("data must be an object")?;
            for (k, v) in data {
                if [
                    "id",
                    "sku",
                    "status",
                    "failType",
                    "createTime",
                    "expireAt",
                    "openApiRequestId",
                ]
                .contains(&k.as_str())
                {
                    return Err("use task transition for collection state".into());
                }
                c.data[k] = v.clone();
            }
        }
        "policy_set" => {
            let id = text(input, "endpoint")?;
            let old = s.policies.get(id).ok_or("endpoint not found")?.clone();
            let p: Policy = patch(
                old,
                input,
                &[
                    "cost",
                    "limit",
                    "window_ms",
                    "account_limit",
                    "global_limit",
                ],
            )?;
            s.policies.insert(id.into(), p);
        }
        "simulation_policy" => {
            s.task_delay_ms = input["task_delay_ms"]
                .as_i64()
                .ok_or("task_delay_ms must be an integer")?;
        }
        "clock_real" => {
            s.fixed_time = None;
            s.rate_windows.clear();
        }
        _ => return Err("unknown action".into()),
    }
    Ok(json!({"result":true}))
}
