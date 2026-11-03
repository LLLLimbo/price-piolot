use crate::{
    query::{self, identity},
    state::{Collection, Settlement, Store, Task},
    ApiResult, Failure,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn charge(s: &mut Store, key: &str, cost: u64) -> Result<(), Failure> {
    let k = s.keys.get_mut(key).unwrap();
    if cost > k.credit_limit - k.credit_used {
        return Err(Failure::new(403, 8000, "Api Key limit exhausted"));
    }
    k.credit_used += cost;
    Ok(())
}
fn refund(s: &mut Store, t: &mut Task) {
    if t.charged && !t.refunded {
        s.keys.get_mut(&t.key).unwrap().credit_used -= t.cost;
        t.refunded = true;
        if let Some(id) = &t.settlement_id {
            s.settlements.get_mut(id).unwrap().refunded += t.cost;
        }
    }
}
pub fn finish(
    s: &mut Store,
    id: &str,
    success: bool,
    reason: Option<i32>,
    fail_type: Option<&str>,
) -> Result<(), String> {
    let mut t = s.tasks.get(id).ok_or("task not found")?.clone();
    if ["success", "failed"].contains(&t.status.as_str()) {
        return Err("task is already settled".into());
    }
    let mut success = success;
    if success && !t.charged {
        if charge(s, &t.key, t.cost).is_err() {
            success = false;
            t.reason = Some(8000);
        } else {
            t.charged = true;
        }
    }
    t.status = if success { "success" } else { "failed" }.into();
    if !success {
        t.reason = reason.or(t.reason).or(Some(5059));
        refund(s, &mut t);
    }
    if let Some(id) = &t.collection_id {
        let data = s
            .products
            .get(&t.sku)
            .map(collection_data)
            .unwrap_or_else(|| json!({}));
        let c = s.collections.get_mut(id).unwrap();
        c.status = if success { 0 } else { 2 };
        c.fail_type = if success {
            None
        } else {
            Some(
                fail_type
                    .unwrap_or(if t.reason == Some(8000) {
                        "INSUFFICIENT_CREDIT"
                    } else {
                        "NO_PRODUCT_INFORMATION_OBTAINED"
                    })
                    .into(),
            )
        };
        if success {
            c.data = data;
        }
    }
    s.tasks.insert(id.into(), t);
    Ok(())
}
pub fn tick(s: &mut Store) {
    let now = s.now();
    loop {
        let id = s
            .tasks
            .values()
            .filter(|t| t.due <= now && !["success", "failed"].contains(&t.status.as_str()))
            .min_by_key(|t| t.due)
            .map(|t| t.id.clone());
        let Some(id) = id else {
            break;
        };
        let t = s.tasks[&id].clone();
        if t.kind == "review" && t.status == "getting_reviews" {
            let reason = if !s.products.contains_key(&t.sku) {
                Some(5001)
            } else if s.products[&t.sku]["reviewCount"].as_u64() == Some(0) {
                Some(5002)
            } else {
                None
            };
            if reason.is_some() {
                finish(s, &id, false, reason, None).unwrap();
            } else {
                let t = s.tasks.get_mut(&id).unwrap();
                t.status = "analyzing".into();
                t.due = t.due.saturating_add(s.task_delay_ms);
            }
        } else {
            let success = s.products.contains_key(&t.sku)
                && (t.kind == "collection" || s.review_results.contains_key(&t.sku));
            finish(s, &id, success, None, None).unwrap();
        }
    }
}
pub fn collect(s: &mut Store, key: &str, input: &Value) -> ApiResult {
    let account = s.keys[key].account.clone();
    let sku = identity(&input["sku"]);
    if s.tasks.values().any(|t| {
        t.account == account && t.sku == sku && t.kind == "collection" && t.status == "collecting"
    }) {
        return Ok(Value::Null);
    }
    let cost = s.policies["ozon-product-collect"].cost;
    charge(s, key, cost)?;
    let mut skus = vec![sku.clone()];
    if input["collectVariant"] == true {
        if let Some(ids) = s
            .products
            .get(&sku)
            .and_then(|p| p["variationIds"].as_array())
        {
            for id in ids {
                let v = identity(id);
                if !skus.contains(&v) {
                    skus.push(v);
                }
            }
        }
    }
    let creator = input["subAccountPhone"]
        .as_str()
        .and_then(|p| s.accounts[&account].sub_accounts.get(p))
        .cloned()
        .unwrap_or_else(|| account.clone());
    let request_id = s.next_id("request");
    let now = s.now();
    for item in skus {
        if item != sku
            && s.tasks.values().any(|t| {
                t.account == account
                    && t.sku == item
                    && t.kind == "collection"
                    && t.status == "collecting"
            })
        {
            continue;
        }
        let collection_id = s.next_id("collect");
        let task_id = s.next_id("task");
        s.collections.insert(
            collection_id.clone(),
            Collection {
                id: collection_id.clone(),
                account: account.clone(),
                creator: creator.clone(),
                sku: item.clone(),
                status: 1,
                fail_type: None,
                request_id: request_id.clone(),
                create_time: now,
                expire_at: now + 30 * 86_400_000,
                data: json!({}),
            },
        );
        s.tasks.insert(
            task_id.clone(),
            Task {
                id: task_id,
                account: account.clone(),
                key: key.into(),
                sku: item.clone(),
                kind: "collection".into(),
                status: "collecting".into(),
                due: now + s.task_delay_ms,
                cost,
                charged: item == sku,
                refunded: false,
                reason: None,
                collection_id: Some(collection_id),
                settlement_id: None,
            },
        );
    }
    Ok(Value::Null)
}
pub fn collection_data(p: &Value) -> Value {
    json!({
        "sku": p["sku"], "title": p["title"], "currency": "RUB", "price": p["price"],
        "photos": p.get("imageUrls").cloned().unwrap_or_else(|| json!([p["imageUrl"]])),
        "videos": [], "coverVideo": null, "sourceProductPrimaryImage": p["imageUrl"],
        "description": p["title"], "keyword": null,
        "backendCategoryId": p["categoryInfo"]["category"]["id"],
        "sellerType": p["sellerType"], "weight": p["weight"],
        "length": null, "width": null, "height": null,
        "remark": null, "characteristics": [],
    })
}
pub fn collection_page(s: &Store, account: &str, input: &Value) -> Value {
    let mut rows: Vec<_> = s
        .collections
        .values()
        .filter(|c| {
            c.account == account
                && c.expire_at > s.now()
                && query::selected(
                    &input["skus"],
                    &serde_json::from_str::<Value>(&c.sku).unwrap(),
                )
        })
        .map(|c| {
            let mut row = c.data.clone();
            for (k, v) in [
                ("id", json!(c.id)),
                ("sku", serde_json::from_str(&c.sku).unwrap()),
                ("status", json!(c.status)),
                ("failType", json!(c.fail_type)),
                ("openApiRequestId", json!(c.request_id)),
                ("createTime", json!(c.create_time)),
                ("expireAt", json!(c.expire_at)),
            ] {
                row[k] = v;
            }
            row
        })
        .collect();
    rows.sort_by(|a, b| {
        b["createTime"]
            .as_i64()
            .cmp(&a["createTime"].as_i64())
            .then_with(|| identity(&b["id"]).cmp(&identity(&a["id"])))
    });
    let orders =
        json!([{"field":"createTime","direction":"DESC"},{"field":"id","direction":"DESC"}]);
    let mut p = input.clone();
    p["orders"] = orders;
    let mut result = query::paginate(rows, &p, true, "createTime");
    result["orders"] =
        json!([{"field":"create_time","direction":"DESC"},{"field":"id","direction":"DESC"}]);
    result
}
pub fn review_add(s: &mut Store, key: &str, input: &Value) -> ApiResult {
    let raw = input["skus"]
        .as_array()
        .ok_or_else(|| Failure::new(400, 400, "Bad Request"))?;
    let mut skus = BTreeSet::new();
    for sku in raw {
        let v = sku
            .as_str()
            .ok_or_else(|| Failure::new(400, 400, "Bad Request"))?;
        if v.is_empty() || v.len() > 15 || !v.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Failure::new(200, 400, "sku format error"));
        }
        skus.insert(v.to_string());
    }
    if skus.is_empty() {
        return Err(Failure::new(200, 400, "skus cannot be empty"));
    }
    if skus.len() > 10 {
        return Err(Failure::new(200, 400, "skus cannot be greater than 10"));
    }
    let skus: Vec<_> = skus.into_iter().collect();
    let account = s.keys[key].account.clone();
    if let Some(c) = s
        .settlements
        .values()
        .find(|c| c.key == key && c.request_skus == skus)
    {
        return Ok(settlement_result(c, vec![], vec![]));
    }
    let request_skus = skus.clone();
    let mut fresh = vec![];
    let mut failed = vec![];
    for sku in skus {
        if s.tasks
            .values()
            .any(|t| t.kind == "review" && t.account == account && t.sku == sku)
        {
            failed.push(json!({"sku":sku,"code":5012,"msg":"Duplicate sku"}));
        } else {
            fresh.push(sku);
        }
    }
    if fresh.is_empty() {
        return Err(Failure::new(200, 5012, "Duplicate sku"));
    }
    let cost = s.policies["ozon-review-analyse-task-add"].cost;
    let total = cost
        .checked_mul(fresh.len() as u64)
        .ok_or_else(|| Failure::new(400, 400, "Bad Request"))?;
    charge(s, key, total)?;
    let id = s.next_id("settlement");
    let mut success = vec![];
    let due = s.now() + s.task_delay_ms;
    for sku in &fresh {
        let task_id = s.next_id("task");
        success.push(json!({"sku":sku,"taskId":task_id}));
        s.tasks.insert(
            task_id.clone(),
            Task {
                id: task_id,
                account: account.clone(),
                key: key.into(),
                sku: sku.clone(),
                kind: "review".into(),
                status: "getting_reviews".into(),
                due,
                cost,
                charged: true,
                refunded: false,
                reason: None,
                collection_id: None,
                settlement_id: Some(id.clone()),
            },
        );
    }
    let c = Settlement {
        id: id.clone(),
        account,
        key: key.into(),
        skus: fresh,
        request_skus,
        unit_cost: cost,
        total_cost: total,
        refunded: 0,
    };
    let result = settlement_result(&c, success, failed);
    s.settlements.insert(id, c);
    Ok(result)
}
fn settlement_result(c: &Settlement, success: Vec<Value>, failed: Vec<Value>) -> Value {
    let status = if c.refunded == 0 {
        "PRE_DEDUCTED"
    } else if c.refunded == c.total_cost {
        "REFUNDED"
    } else {
        "PARTIALLY_REFUNDED"
    };
    json!({"settlementId":c.id,"status":status,"skuCount":c.skus.len(),"unitCost":c.unit_cost,"totalCost":c.total_cost,"freeCost":0,"paidCost":c.total_cost,"reason":null,"successTasks":success,"failedTasks":failed})
}
pub fn review_result(s: &Store, account: &str, id: &str, input: &Value) -> ApiResult {
    let sku = identity(&input["sku"]);
    let t = s
        .tasks
        .values()
        .find(|t| t.kind == "review" && t.account == account && t.sku == sku)
        .ok_or_else(|| Failure::new(200, 403, "forbidden"))?;
    match t.status.as_str() {
        "getting_reviews" => {
            return Err(Failure::new(
                200,
                5057,
                "Review analysis task is getting reviews",
            ))
        }
        "analyzing" => return Err(Failure::new(200, 5058, "Review analysis task is analyzing")),
        "failed" => {
            let code = if [
                "ozon-review-analyse-review-statistic",
                "ozon-review-analyse-original-review",
            ]
            .contains(&id)
            {
                5059
            } else {
                t.reason.unwrap_or(5059)
            };
            let message = match code {
                5001 => "Review analysis task failed: product not exist",
                5002 => "Review analysis task failed: no review",
                _ => "Review analysis task failed",
            };
            return Err(Failure::new(200, code, message));
        }
        _ => (),
    }
    let data = s
        .review_results
        .get(&sku)
        .ok_or_else(|| Failure::new(200, 5059, "Review analysis task failed"))?;
    let language = input["language"].as_str().unwrap_or("CN");
    let mut result = match id {
        "ozon-review-analyse-report" => data[language]["report"].clone(),
        "ozon-review-analyse-word-frequency" => data[language]["word_frequency"].clone(),
        "ozon-review-analyse-review-statistic" => data["statistic"].clone(),
        _ => data["original"].clone(),
    };
    if result.is_null() {
        return Err(Failure::new(200, 5059, "Review analysis task failed"));
    }
    if [
        "ozon-review-analyse-report",
        "ozon-review-analyse-word-frequency",
    ]
    .contains(&id)
    {
        result["userId"] = serde_json::from_str(account).unwrap_or_else(|_| json!(account));
    }
    Ok(result)
}
