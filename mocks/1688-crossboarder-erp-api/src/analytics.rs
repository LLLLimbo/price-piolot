use crate::{business::*, state::Store};
use chrono::{Duration, NaiveDate, TimeZone};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn trend(s: &Store, name: &str, p: &Value) -> Result<Value> {
    let id = number(p, &["offerId"]).unwrap_or(0);
    s.products.get(&id).ok_or_else(|| Error {
        code: "PRODUCT_NOT_EXISTS".into(),
        message: "product not found".into(),
    })?;
    let parse = |field: &str| {
        NaiveDate::parse_from_str(&text(p, &[field]), "%Y%m%d").map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: format!("{field} must be yyyyMMdd"),
        })
    };
    let start = parse("startDate")?;
    let end = parse("endDate")?;
    if end < start || (end - start).num_days() > 366 {
        return fail(
            "PARAM_ILLEGAL",
            "trend range must be ordered and at most 367 dates",
        );
    }
    let mut rows = vec![];
    for day in 0..=(end - start).num_days() {
        let date = start + Duration::days(day);
        let begin = chrono_tz::Asia::Shanghai
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap()
            .timestamp_millis();
        let mut quantities = Decimal::ZERO;
        let mut prices = vec![];
        let mut buyers = std::collections::BTreeMap::new();
        for order in s.orders.values().filter(|o| {
            o.metadata.paid.is_some()
                || ["waitsellersend", "waitbuyerreceive", "success"].contains(&o.status.as_str())
        }) {
            let time = order.metadata.paid.unwrap_or(order.created);
            for (i, line) in order
                .lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.offer_id == id)
            {
                if time >= begin && time < begin + 86400000 {
                    quantities += line.quantity;
                }
                if time >= begin - 29 * 86400000 && time < begin + 86400000 {
                    prices.push(line_payment(order, i) / line.quantity);
                    *buyers.entry(&order.buyer).or_insert(0_usize) += 1;
                }
            }
        }
        prices.sort();
        let value = if name.contains("MedianPrice") {
            if prices.is_empty() {
                Decimal::ZERO
            } else if prices.len() % 2 == 0 {
                (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / Decimal::from(2)
            } else {
                prices[prices.len() / 2]
            }
        } else if name.contains("Repurchase") {
            if buyers.is_empty() {
                Decimal::ZERO
            } else {
                Decimal::from(buyers.values().filter(|n| **n > 1).count() as u64)
                    / Decimal::from(buyers.len() as u64)
            }
        } else {
            quantities
        };
        rows.push(json!({"date":date.format("%Y%m%d").to_string(),"value":value.to_string()}));
    }
    Ok(json!({"items":rows,"retCode":"S000","retMsg":"成功"}))
}
pub fn rank(s: &Store, p: &Value) -> Result<Value> {
    let rank = text(p, &["rankId"]);
    let kind = text(p, &["rankType"]);
    if !["complex", "hot", "goodPrice"].contains(&kind.as_str()) {
        return fail("PARAM_ILLEGAL", "unknown rankType");
    }
    let limit = number(p, &["limit"]).unwrap_or(20);
    if !(1..=20).contains(&limit) {
        return fail("PARAM_ILLEGAL", "limit must be 1..20");
    }
    let language = crate::search::language(p)?;
    let category = rank.parse::<i64>().map_err(|_| Error {
        code: "CATEGORY_NOT_EXISTS".into(),
        message: "rankId must identify a fixture category".into(),
    })?;
    let mut products: Vec<_> = s
        .products
        .values()
        .filter(|p| p.active && (category == 1 || p.category == category))
        .collect();
    products.sort_by(|a, b| {
        let cmp = match kind.as_str() {
            "goodPrice" => a.price.cmp(&b.price),
            "hot" => b
                .merchandise
                .sales
                .get("sales30")
                .cmp(&a.merchandise.sales.get("sales30")),
            _ => b.merchandise.score.cmp(&a.merchandise.score),
        };
        cmp.then(a.id.cmp(&b.id))
    });
    let rows:Vec<_>=products.into_iter().take(limit as usize).enumerate().map(|(i,p)|json!({"itemId":p.id,"title":p.title,"translateTitle":p.merchandise.translations.get(&language).unwrap_or(&p.title),"imgUrl":format!("{}/assets/product.svg",s.base_url),"sort":i+1,"serviceList":match p.merchandise.shipping.as_str(){"shipIn24Hours"=>vec!["sendGoods24H"],"shipIn48Hours"=>vec!["sendGoods48H"],_=>vec![]},"buyerNum":p.merchandise.buyer_count,"soldOut":p.merchandise.sales.get("sales30").copied().unwrap_or(0),"goodsScore":p.merchandise.score})).collect();
    Ok(json!({"rankId":rank,"rankName":"模拟类目榜","rankType":kind,"rankProductModels":rows}))
}
