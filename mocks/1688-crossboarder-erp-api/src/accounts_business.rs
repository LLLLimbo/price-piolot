use crate::{business::*, state::Store};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn open_uid(app_key: &str, owner: &str) -> String {
    use sha1::{Digest, Sha1};
    if app_key == "test-app" {
        return format!("mock-open-{owner}");
    }
    let mut hash = Sha1::new();
    hash.update(app_key.as_bytes());
    hash.update([0]);
    hash.update(owner.as_bytes());
    format!("mock-open-{:x}", hash.finalize())
}

pub fn suppliers(s: &Store, p: &Value, owner: &str) -> Result<Value> {
    let login = text(p, &["supplierLoginId", "sellerLoginId"]);
    let since = date_millis(p, "relationTime")?;
    let mut rows = vec![];
    for relation in s.rows("supplier_relations", Some(owner)) {
        let seller = text(&relation, &["memberId", "sellerMemberId"]);
        let Some(a) = s.accounts.get(&seller) else {
            continue;
        };
        let created = number(&relation, &["consignCreateTime", "created"]).unwrap_or(0);
        if (!login.is_empty() && a.login != login) || since.is_some_and(|t| created < t) {
            continue;
        }
        let orders: Vec<_> = s
            .orders
            .values()
            .filter(|o| {
                o.buyer == owner
                    && o.seller == seller
                    && o.status == "success"
                    && o.created >= s.now() - 180 * 86400000
            })
            .collect();
        let amount = orders
            .iter()
            .map(|o| o.amount - o.refunded)
            .sum::<Decimal>()
            * Decimal::from(100);
        let distributed = s
            .rows("distribution", Some(owner))
            .iter()
            .filter(|r| {
                number(r, &["offerId", "productIdInSource"])
                    .and_then(|id| s.products.get(&id))
                    .is_some_and(|p| p.seller == seller)
            })
            .count();
        let mut row = relation.clone();
        for (k,v) in json!({"memberId":seller,"loginId":a.login,"userId":a.user_id,"sellerId":a.user_id,"companyName":a.login,"supplierLoginId":a.login,"supplierCompany":a.login,"consignStatus":relation.get("consignStatus").cloned().unwrap_or(json!("cooperating")),"consignCreateTime":created,"lastOrder":orders.len(),"lastAmount":amount,"distributionNum":distributed,"relationDesc":[{"description":"分销合作","code":"distribution","related":true}],"domainInPlatforms":[format!("{}/contact/{}",s.base_url,seller)]}).as_object().unwrap(){row[k]=v.clone();}
        rows.push(row);
    }
    page(rows, p)
}
pub fn app_orders(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    if s.records
        .get("app_configs")
        .and_then(|r| r.get(&s.current_app))
        .is_some_and(|c| c["product_active"] == false)
    {
        return fail("11003", "application product not found or off shelf");
    }
    let member = text(p, &["memberId"]);
    let login = text(p, &["loginId"]);
    let ali = number(p, &["aliId"]);
    let date = date_millis(
        p,
        if name == "app.expire.get" {
            "gmtServiceEnd"
        } else {
            "gmtCreate"
        },
    )?;
    let statuses = array(p, &["bizStatusList"]);
    if statuses.is_some_and(|v| {
        v.iter()
            .any(|v| !["B", "S", "P", "E", "C"].contains(&v.as_str().unwrap_or("")))
    }) {
        return fail("PARAM_ILLEGAL", "invalid bizStatusList");
    }
    let rows = s
        .rows("app_orders", Some(owner))
        .into_iter()
        .filter(|r| {
            let account = s.accounts.get(&text(r, &["memberId"]));
            r.get("appKey")
                .and_then(Value::as_str)
                .unwrap_or("test-app")
                == s.current_app
                && (member.is_empty() || r["memberId"] == member)
                && (login.is_empty() || r["loginId"] == login)
                && ali.is_none_or(|id| account.is_some_and(|a| a.user_id == id))
                && statuses.is_none_or(|v| v.is_empty() || v.contains(&r["bizStatus"]))
                && date.is_none_or(|d| {
                    crate::dates::internal_millis(
                        &r[if name == "app.expire.get" {
                            "gmtServiceEnd"
                        } else {
                            "gmtCreate"
                        }],
                    )
                    .is_some_and(|t| {
                        if name == "app.expire.get" {
                            t >= d && t < d + 86400000
                        } else {
                            t >= d - 30 * 86400000 && t <= d
                        }
                    })
                })
        })
        .collect();
    let mut paging = p.clone();
    if let Some(page) = number(p, &["startIndex"]) {
        if page < 0 {
            return fail("PARAM_ILLEGAL", "negative page");
        }
        paging.as_object_mut().unwrap().remove("startIndex");
        paging["page"] = json!(page.max(1));
    }
    let mut result = page(rows, &paging)?;
    result["successed"] = json!(true);
    Ok(result)
}
