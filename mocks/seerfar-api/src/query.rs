use crate::state::{date, Store};
use chrono::{Datelike, Months, NaiveDate};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::{cmp::Ordering, str::FromStr};

pub fn identity(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string())
}
pub fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}
pub fn sum(rows: &[Value], field: &str) -> Value {
    let sum: Decimal = rows
        .iter()
        .filter_map(|r| Decimal::from_str(&r[field].to_string()).ok())
        .sum();
    serde_json::from_str(&sum.normalize().to_string()).unwrap()
}
pub fn mean(rows: &[Value], field: &str) -> Value {
    let present: Vec<_> = rows
        .iter()
        .filter(|r| r[field].is_number())
        .cloned()
        .collect();
    if present.is_empty() {
        return Value::Null;
    }
    let n = Decimal::from_str(&sum(&present, field).to_string()).unwrap()
        / Decimal::from(present.len());
    serde_json::from_str(&n.round_dp(2).normalize().to_string()).unwrap()
}
pub fn alias(field: &str) -> &str {
    match field {
        "monthlySales" => "sales",
        "monthlyRevenue" => "revenue",
        "monthlySalesRate" => "salesRate",
        "monthlyRevenueRate" => "revenueRate",
        "searchChange30" => "count30GrowthRate",
        "productViews" => "itemsViews",
        _ => field,
    }
}
pub fn range_matches(value: &Value, range: &Value) -> bool {
    if !range.is_object() {
        return true;
    }
    if range["min"].is_null() && range["max"].is_null() {
        return true;
    }
    value.is_number()
        && (range["min"].is_null() || number(value) >= number(&range["min"]))
        && (range["max"].is_null() || number(value) <= number(&range["max"]))
}
pub fn selected(list: &Value, value: &Value) -> bool {
    list.as_array()
        .is_none_or(|a| a.is_empty() || a.iter().any(|v| identity(v) == identity(value)))
}
pub fn text_match(text: &str, keyword: &str, fuzzy: bool) -> bool {
    if fuzzy {
        text.to_lowercase().contains(&keyword.to_lowercase())
    } else {
        text.eq_ignore_ascii_case(keyword)
    }
}
pub fn filters(row: &Value, input: &Value, now: i64) -> bool {
    for (field, range) in input.as_object().into_iter().flatten() {
        if range.get("min").is_some() || range.get("max").is_some() {
            let target = if row.get(alias(field)).is_some() {
                &row[alias(field)]
            } else {
                &row["dimension"][field]
            };
            if !range_matches(target, range) {
                return false;
            }
        }
    }
    if !selected(&input["skus"], &row["sku"]) || !selected(&input["sellerName"], &row["sellerName"])
    {
        return false;
    }
    if let Some(a) = input["categoryIds"].as_array().filter(|a| !a.is_empty()) {
        let path = row["categoryInfo"]["fullCategoryId"].as_array();
        if !a
            .iter()
            .any(|id| path.is_some_and(|p| p.iter().any(|v| identity(v) == identity(id))))
        {
            return false;
        }
    }
    let fulfillment = &input["fulfillment"];
    let delivery = row["fulfillment"].as_array();
    if let Some(a) = fulfillment.as_array().filter(|a| !a.is_empty()) {
        if !a.iter().any(|v| delivery.is_some_and(|d| d.contains(v))) {
            return false;
        }
    } else if let Some(v) = fulfillment.as_str().filter(|s| !s.is_empty()) {
        if delivery.is_none_or(|d| !d.contains(&json!(v))) {
            return false;
        }
    }
    let brand = &input["brand"];
    let names = &brand["brandName"];
    let ids = &brand["brandId"];
    let name = row
        .get("brandName")
        .unwrap_or(&row["brandInfo"]["brandName"]);
    let id = row.get("brandId").unwrap_or(&row["brandInfo"]["brandId"]);
    let has_filter = [names, ids]
        .iter()
        .any(|v| v.as_array().is_some_and(|a| !a.is_empty()));
    let matches = selected(names, name) && selected(ids, id);
    match brand["type"].as_u64().unwrap_or(0) {
        0 if has_filter && !matches => return false,
        1 if has_filter && matches => return false,
        2 if !name.is_null() && name.as_str() != Some("") => return false,
        _ => (),
    }
    if let Some(labels) = input["labels"].as_array().filter(|a| !a.is_empty()) {
        if !labels
            .iter()
            .all(|l| row["labels"].as_array().is_some_and(|a| a.contains(l)))
        {
            return false;
        }
    }
    let text = [
        row["title"].as_str().unwrap_or(""),
        row["query"].as_str().unwrap_or(""),
    ]
    .join(" ");
    if input["keywords"].as_array().is_some_and(|a| {
        !a.is_empty()
            && !a
                .iter()
                .any(|v| text_match(text.trim(), v.as_str().unwrap_or(""), true))
    }) {
        return false;
    }
    if let Some(tag) = input["tag"].as_str().filter(|s| !s.is_empty()) {
        if !text.to_lowercase().contains(&tag.to_lowercase()) {
            return false;
        }
    }
    if input["filterRemoveProduct"] == true && row["removed"] == true {
        return false;
    }
    if let Some(months) = input["creationDate"].as_i64() {
        if row["upTime"]
            .as_i64()
            .is_none_or(|t| t < now - months * 30 * 86_400_000)
        {
            return false;
        }
    }
    true
}

pub fn validate(input: &Value, endpoint: &Value, now: i64) -> Result<(), String> {
    let bad = || "Bad Request".to_string();
    for (name, v) in input.as_object().into_iter().flatten() {
        if v.is_object() && (v.get("min").is_some() || v.get("max").is_some()) {
            for part in ["min", "max"] {
                if !v[part].is_null() && !v[part].is_number() {
                    return Err(bad());
                }
            }
            if !v["min"].is_null() && !v["max"].is_null() && number(&v["min"]) > number(&v["max"]) {
                return Err(bad());
            }
        }
        let max = match name.as_str() {
            "skuIds" => 20,
            "skus" if endpoint["id"] == "ozon-collect-box-page" => 100,
            "skus" => 10,
            _ => usize::MAX,
        };
        if v.as_array().is_some_and(|a| a.len() > max) {
            return Err(bad());
        }
    }
    for name in ["sku", "id"] {
        if let Some(v) = input.get(name) {
            if (v.is_number() && v.as_u64().is_none_or(|n| n == 0))
                || (v.is_string()
                    && (identity(v).is_empty() || !identity(v).bytes().all(|b| b.is_ascii_digit())))
            {
                return Err(bad());
            }
        }
    }
    if let Some(v) = input["creationDate"].as_u64() {
        if ![1, 3, 6, 12, 24].contains(&v) {
            return Err(bad());
        }
    }
    for (name, allowed) in [
        ("matchType", vec![0, 1]),
        ("hasVariant", vec![0, 1]),
        ("variationsMerge", vec![0, 1]),
    ] {
        if !input[name].is_null() && input[name].as_u64().is_none_or(|v| !allowed.contains(&v)) {
            return Err(bad());
        }
    }
    if !input["brand"]["type"].is_null() && input["brand"]["type"].as_u64().is_none_or(|v| v > 2) {
        return Err(bad());
    }
    if let Some(lang) = input["clientLanguage"].as_str() {
        if !["zh-CN", "en-US", "ru-RU"].contains(&lang) {
            return Err(bad());
        }
    }
    if let Some(lang) = input["language"].as_str() {
        if !["CN", "EN", "RU"].contains(&lang) {
            return Err(bad());
        }
    }
    if input.get("page").is_some() {
        page(input, false)?;
    }
    if let Some(orders) = input["page"]["orders"].as_array() {
        let endpoint_id = endpoint["id"].as_str().unwrap();
        let fields=match endpoint_id {
            "ozon-shop-brand-search"|"ozon-shop-category-search"=>"products avgPrice totalSales totalRevenues salesRate revenueRate",
            "ozon-market-keyword-search"=>"searchVolume avgPrice sales revenue count30GrowthRate productCount sellers competingProducts reviews ratings marketSpace itemsViews conversionSharing returnCancellationRate weight volume",
            "ozon-keyword-mining"=>"uniqQueriesWCa ca searchVolume count30GrowthRate relevancy titleDensity wordCount avgPrice competingProducts productCount sellers adRivalCount itemsViews marketSpace conversionSharing returnCancellationRate",
            "ozon-keyword-back-search"=>"exposure conversion uniqQueriesWCa ca searchVolume count30GrowthRate naturalRank adRank titleDensity wordCount competingProducts productCount sellers adRivalCount itemsViews marketSpace conversionSharing returnCancellationRate",
            "ozon-product-variations-search"=>"price upTime sales revenue reviewCount reviewRating",
            _=>"revenue price sales grossMargin views sessionCountSearch sessionCount convToCartPdp orderConversionRate drr returnCancellationRate missedRevenue variants reviewCount reviewRating upTime weight volume questionsAndAnswers",
        };
        for o in orders {
            let field = o["field"].as_str().ok_or_else(bad)?;
            if !(fields.split_whitespace().any(|v| v == field)
                || endpoint_id == "ozon-product-variations-search"
                    && input["page"]["isExpand"] == true)
            {
                return Err(bad());
            }
            if !matches!(o["direction"].as_str(), Some("ASC" | "DESC")) {
                return Err(bad());
            }
        }
    }
    if endpoint["id"] == "ozon-collect-box-page" {
        page(input, true)?;
    }
    for name in ["date", "searchDate", "historyDate"] {
        if let Some(v) = input[name].as_str().filter(|v| !v.is_empty()) {
            period(v, now, endpoint["id"].as_str().unwrap())?;
        }
    }
    if let Some(v) = input["dateRange"].as_str() {
        if ![
            "past_7_days",
            "past_30_days",
            "past_90_days",
            "past_180_days",
            "past_365_days",
        ]
        .contains(&v)
        {
            return Err(bad());
        }
    }
    Ok(())
}
pub fn period(value: &str, now: i64, id: &str) -> Result<(String, String), String> {
    let today = chrono::DateTime::from_timestamp_millis(now)
        .unwrap()
        .date_naive();
    if value.is_empty() {
        return Ok((date(now - 29 * 86_400_000), date(now)));
    }
    let month = if id == "ozon-category-search"
        || (id == "ozon-product-report-search" && value.len() == 7)
    {
        NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").ok()
    } else {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|d| d.day() == 1)
            .and_then(|d| d.checked_sub_months(Months::new(1)))
    }
    .ok_or("Bad Request")?;
    let first = today.with_day(1).unwrap();
    if month > first || month < first.checked_sub_months(Months::new(12)).unwrap() {
        return Err("Bad Request".into());
    }
    let end = month
        .checked_add_months(Months::new(1))
        .unwrap()
        .pred_opt()
        .unwrap();
    Ok((month.to_string(), end.to_string()))
}
pub fn dates(input: &Value, now: i64, id: &str) -> (String, String) {
    for key in ["date", "searchDate", "historyDate"] {
        if let Some(v) = input[key].as_str().filter(|v| !v.is_empty()) {
            return period(v, now, id).unwrap();
        }
    }
    let days = input["dateRange"]
        .as_str()
        .and_then(|v| {
            v.strip_prefix("past_")?
                .strip_suffix("_days")?
                .parse::<i64>()
                .ok()
        })
        .unwrap_or(30);
    (date(now - (days - 1) * 86_400_000), date(now))
}
pub fn product_rows(s: &Store, input: &Value, id: &str) -> Vec<Value> {
    let (start, end) = dates(input, s.now(), id);
    s.products
        .iter()
        .map(|(sku, p)| {
            let mut p = p.clone();
            let daily: Vec<_> = s
                .history
                .get(sku)
                .into_iter()
                .flatten()
                .filter(|r| {
                    r["date"]
                        .as_str()
                        .is_some_and(|d| d >= start.as_str() && d <= end.as_str())
                })
                .cloned()
                .collect();
            if !daily.is_empty()
                || input["date"].as_str().is_some_and(|v| !v.is_empty())
                || input["searchDate"].as_str().is_some_and(|v| !v.is_empty())
                || input.get("dateRange").is_some()
            {
                p["sales"] = sum(&daily, "sales");
                p["revenue"] = sum(&daily, "revenue");
            }
            p
        })
        .filter(|p| filters(p, input, s.now()))
        .collect()
}
pub fn page(input: &Value, flat: bool) -> Result<(usize, usize), String> {
    let p = if flat { input } else { &input["page"] };
    let n = if p["pageNumber"].is_null() {
        Some(1)
    } else {
        p["pageNumber"].as_u64()
    };
    let size = if p["pageSize"].is_null() {
        Some(20)
    } else {
        p["pageSize"].as_u64()
    };
    match (n, size) {
        (Some(n), Some(size)) if n > 0 && n <= 1_000_000_000 && (1..=100).contains(&size) => {
            Ok((n as usize, size as usize))
        }
        _ => Err("Bad Request".into()),
    }
}
pub fn sort(rows: &mut [Value], orders: &Value, fallback: &str) {
    let default = json!([{"field":fallback,"direction":"DESC"}]);
    let orders = orders
        .as_array()
        .filter(|a| !a.is_empty())
        .unwrap_or(default.as_array().unwrap());
    rows.sort_by(|a, b| {
        for o in orders {
            let field = o["field"].as_str().unwrap_or(fallback);
            let get = |r: &Value| -> Value {
                if let Some(v) = r.get(field) {
                    return v.clone();
                }
                if let Some(v) = r["dimension"].get(field) {
                    return v.clone();
                }
                r["characteristics"]
                    .as_array()
                    .and_then(|a| a.iter().find(|c| c["sourceName"] == field))
                    .map(|c| c["value"].clone())
                    .unwrap_or(Value::Null)
            };
            let (a, b) = (get(a), get(b));
            let cmp = if a.is_number() && b.is_number() {
                number(&a)
                    .partial_cmp(&number(&b))
                    .unwrap_or(Ordering::Equal)
            } else {
                identity(&a).cmp(&identity(&b))
            };
            if cmp != Ordering::Equal {
                return if o["direction"] == "ASC" {
                    cmp
                } else {
                    cmp.reverse()
                };
            }
        }
        identity(&a["sku"]).cmp(&identity(&b["sku"]))
    });
}
pub fn paginate(mut rows: Vec<Value>, input: &Value, flat: bool, fallback: &str) -> Value {
    let p = if flat { input } else { &input["page"] };
    sort(&mut rows, &p["orders"], fallback);
    let (n, size) = page(input, flat).unwrap();
    let total = rows.len();
    let records: Vec<_> = rows.into_iter().skip((n - 1) * size).take(size).collect();
    json!({"currentSize":records.len(),"records":records,"total":total,"size":size,"current":n,"totalPages":total.div_ceil(size),"orders":p.get("orders").cloned().unwrap_or_else(||json!([]))})
}
