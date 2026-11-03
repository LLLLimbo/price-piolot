use crate::{
    business::*,
    state::{Order, Store},
};
use chrono::{Datelike, TimeZone};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn validate(name: &str, p: &Value) -> Result<()> {
    if [
        "alibaba.cps.genClickUrl",
        "alibaba.cps.listActivityPageQuery",
        "alibaba.cps.listOfferPageQuery",
        "alibaba.cps.listShopPageQuery",
        "alibaba.cps.listCPSSettleInfoDetail",
    ]
    .contains(&name)
    {
        if number(p, &["pageNo"]).is_some_and(|n| n < 1) {
            return fail("88807", "pageNo must be positive");
        }
        if number(p, &["pageSize"]).is_some_and(|n| !(1..=200).contains(&n)) {
            return fail("88804", "pageSize must be 1..200");
        }
    }
    if name == "alibaba.cps.listOfferPageQuery"
        && number(p, &["categoryId"]).is_none()
        && text(p, &["feedInfo"]).is_empty()
    {
        return fail("99993", "categoryId or feedInfo required");
    }
    if [
        "alibaba.cps.tradeBillList",
        "alibaba.cps.webUnionTradeBillList",
    ]
    .contains(&name)
        && (number(p, &["pageSize"]).is_some_and(|n| !(1..=200).contains(&n))
            || number(p, &["pageNo"]).is_some_and(|n| n < 1))
    {
        return fail("150005", "invalid pagination");
    }
    if name == "alibaba.cps.genClickUrl" && array(p, &["objectValueList"]).is_none_or(Vec::is_empty)
    {
        return fail("99993", "objectValueList required");
    }
    if name == "alibaba.cps.genClickUrl"
        && number(p, &["type"]).is_some_and(|n| ![0, 1, 2, 9].contains(&n))
    {
        return fail("150008", "invalid promotion type");
    }
    if name == "alibaba.cps.listMediaInfo" && number(p, &["mediaId"]).is_some_and(|n| n != 1) {
        return fail("150009", "media not found");
    }
    Ok(())
}
fn order_state(o: &Order) -> i64 {
    match o.status.as_str() {
        "waitbuyerpay" => 10,
        "success" => 50,
        "cancel" => 80,
        _ => 20,
    }
}
fn paid(o: &Order) -> bool {
    o.metadata.paid.is_some()
        || ["waitsellersend", "waitbuyerreceive", "success"].contains(&o.status.as_str())
}
fn millis(p: &Value, key: &str) -> Result<i64> {
    crate::dates::Options::new(Some("yyyy-MM-dd"), Some("Asia/Shanghai"))
        .and_then(|options| options.parse(&text(p, &[key])))
        .map_err(|e| Error {
            code: "150005".into(),
            message: format!("{key}: {e}"),
        })
}
pub fn bills(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    if name == "alibaba.cps.webUnionTradeBillList"
        && s.records
            .get("app_configs")
            .and_then(|r| r.get(&s.current_app))
            .and_then(|r| r["webUnionKeys"].as_array())
            .is_none_or(|keys| !keys.contains(&json!(text(p, &["webUnionKey"]))))
    {
        return fail("160001", "invalid web union key");
    }

    validate(name, p)?;
    let kind = text(p, &["queryOrderType"]);
    let (start, end) = if ["orderMain", "orderSub"].contains(&kind.as_str()) {
        (None, None)
    } else {
        if !["orderAll", "orderSettle", "orderPay"].contains(&kind.as_str()) {
            return fail("150005", "invalid queryOrderType");
        }
        let (start, end) = (millis(p, "queryStartTime")?, millis(p, "queryEndTime")?);
        if start > end {
            return fail("150005", "time range is reversed");
        }
        (Some(start), Some(end + 86400000))
    };
    if kind == "orderMain" && number(p, &["bizId"]).is_none() {
        return fail("150005", "bizId required");
    }
    if kind == "orderSub" && number(p, &["bizSubId"]).is_none() {
        return fail("150005", "bizSubId required");
    }
    let time_kind = text(p, &["queryTimeType"]);
    if start.is_some()
        && ![
            "gmtCreateTime",
            "confirmTime",
            "settleTime",
            "rightsStartTime",
            "rightsEndTime",
        ]
        .contains(&time_kind.as_str())
    {
        return fail("150005", "queryTimeType required or invalid");
    }
    for (key, allowed) in [
        ("orderState", &[10, 20, 50, 80][..]),
        ("settleState", &[1, 2][..]),
        ("rightsState", &[10, 30, 40][..]),
    ] {
        if number(p, &[key]).is_some_and(|v| !allowed.contains(&v)) {
            return fail("150005", "invalid state filter");
        }
    }
    let mut rows = vec![];
    for o in s.orders.values().filter(|o| o.buyer == owner && !o.deleted) {
        if kind == "orderMain" && number(p, &["bizId"]) != Some(o.id) {
            continue;
        }
        if kind == "orderSettle" && o.status != "success" {
            continue;
        }
        if kind == "orderPay" && !paid(o) {
            continue;
        }
        let refund = s
            .refunds
            .values()
            .filter(|r| r.order_id == o.id)
            .next_back();
        let rights = refund
            .map(|r| match r.status.as_str() {
                "success" => 30,
                "cancelled" | "rejected" => 40,
                _ => 10,
            })
            .unwrap_or(0);
        let state = order_state(o);
        let settle = if o.status == "success" { 1 } else { 0 };
        if number(p, &["orderState"]).is_some_and(|n| n != state)
            || number(p, &["settleState"]).is_some_and(|n| n != settle)
            || number(p, &["rightsState"]).is_some_and(|n| n != rights)
        {
            continue;
        }
        let time = match time_kind.as_str() {
            "confirmTime" | "settleTime" => o.metadata.completed,
            "rightsStartTime" => refund
                .and_then(|r| r.history.first())
                .and_then(|h| h["time"].as_i64()),
            "rightsEndTime" => refund
                .filter(|r| ["success", "cancelled", "rejected"].contains(&r.status.as_str()))
                .and_then(|r| r.history.last())
                .and_then(|h| h["time"].as_i64()),
            _ => Some(o.created),
        };
        if start.is_some_and(|v| time.is_none_or(|t| t < v))
            || end.is_some_and(|v| time.is_none_or(|t| t >= v))
        {
            continue;
        }
        for (index, line) in o.lines.iter().enumerate() {
            let sub = o.id * 100 + index as i64;
            if kind == "orderSub" && number(p, &["bizSubId"]) != Some(sub) {
                continue;
            }
            let promotion = o
                .metadata
                .cps
                .get(&line.offer_id)
                .cloned()
                .unwrap_or(json!({}));
            let media = number(&promotion, &["mediaId"]).unwrap_or(1);
            let zone = number(&promotion, &["mediaZoneId"]).unwrap_or(1);
            if number(p, &["mediaId"]).is_some_and(|v| v != media)
                || number(p, &["mediaZoneId"]).is_some_and(|v| v != zone)
            {
                continue;
            }
            let amount = line_payment(o, index);
            let ratio = s.products[&line.offer_id].merchandise.ratio;
            rows.push(json!({"orderId":o.id,"bizId":o.id,"bizSubId":sub,"feedId":line.offer_id,"name":if line.title.is_empty(){&s.products[&line.offer_id].title}else{&line.title},"tradeAmount":amount,"tradeNumber":line.quantity,"createTime":o.created,"confirmTime":o.metadata.completed,"orderState":state,"settleTime":o.metadata.completed,"settleState":settle,"ratio":ratio,"commission":amount*ratio,"subCommission":0,"type":0,"mediaId":media,"mediaZoneId":zone,"tkId":s.accounts[owner].user_id,"subTkId":0,"tkRatio":1,"subTkRatio":0,"platformRatio":0,"buyerId":s.accounts[&o.buyer].user_id,"sellerId":s.accounts[&o.seller].user_id,"tradeType":order_trade_type(o),"subsidizationRatio":0,"subsidizationAmount":0,"subSubsidizationAmount":0,"ext":promotion.get("ext").cloned().unwrap_or(json!("{}")),"rightsState":rights,"totalPayment":amount}));
        }
    }
    page(rows, p)
}
fn commission(s: &Store, o: &Order) -> (Decimal, Decimal) {
    let mut gross = Decimal::ZERO;
    let mut returned = Decimal::ZERO;
    for (index, line) in o.lines.iter().enumerate() {
        let ratio = s.products[&line.offer_id].merchandise.ratio;
        gross += line_payment(o, index) * ratio;
        returned += crate::refunds::settled(s, o.id, o.id * 100 + index as i64).0 * ratio;
    }
    (gross, (gross - returned).max(Decimal::ZERO))
}

pub fn settlement(s: &Store, p: &Value, owner: &str) -> Result<Value> {
    let mut months: std::collections::BTreeMap<(i32, u32), (Decimal, Decimal, Decimal)> =
        std::collections::BTreeMap::new();
    for order in s
        .orders
        .values()
        .filter(|o| o.buyer == owner && !o.deleted && paid(o))
    {
        let date = chrono_tz::Asia::Shanghai
            .timestamp_millis_opt(order.metadata.paid.unwrap_or(order.created))
            .single()
            .unwrap();
        let (gross, net) = commission(s, order);
        let entry = months.entry((date.year(), date.month())).or_default();
        entry.0 += gross;
        entry.1 += gross - net;
        if order.status == "success" {
            entry.2 += net;
        }
    }
    let rows=months.into_iter().rev().map(|((year,month),(gross,returned,settled))| {
        let date=chrono_tz::Asia::Shanghai.with_ymd_and_hms(year,month,1,0,0,0).single().unwrap().timestamp_millis();
        json!({"settleMonth":date,"estimateIncome":gross,"refundSuccAmount":returned,"refundAmount":returned,"freezeAmount":0,"redeductAmount":0,"realAmount":settled,"taxAmount":0,"serviceAmount":0})
    }).collect();
    page(rows, p)
}

pub fn links(s: &mut Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    let ids = array(p, &["objectValueList", "offerIds"])
        .cloned()
        .unwrap_or_else(|| vec![json!(text(p, &["keyword"]))]);
    let rows=ids.into_iter().map(|id| {
        let key=id.as_str().map(str::to_owned).unwrap_or_else(||id.to_string());
        let encoded: String=key.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        let url=format!("{}/promotion/{encoded}",s.base_url);
        if let Some(offer)=id.as_i64().or_else(||id.as_str().and_then(|v|v.parse::<i64>().ok())) {
            if s.products.contains_key(&offer) {
                s.put("promotion_links", &format!("{owner}:{offer}"), json!({"mediaId":number(p,&["mediaId"]).unwrap_or(1),"mediaZoneId":number(p,&["mediaZoneId"]).unwrap_or(1),"ext":p.get("ext").cloned().unwrap_or(json!("{}"))}));
            }
        }
        let link=json!({"objectId":id,"longClickUrl":url,"shortClickUrl":url,"clickUrl":url});
        if name=="alibaba.cps.generateIntroduceUrlByParam" {json!({"offerId":id,"clickUrlVO":link})}
        else {json!({"offerId":id,"objectValue":id,"objectId":id,"longClickUrl":url,"shortClickUrl":url,"clickUrl":url,"url":url})}
    }).collect();
    if name == "alibaba.cps.genSearchPjjxIntroduceUrlByKeyword" {
        let rows: Vec<Value> = rows;
        let url = rows
            .first()
            .map(|r| r["longClickUrl"].clone())
            .unwrap_or(Value::Null);
        return Ok(
            json!({"result":{"success":true,"result":{"pcPjjxCpsUrl":url,"wapPjjxCpsUrl":url}}}),
        );
    }
    page(rows, p)
}

pub fn summary(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    if name == "alibaba.cps.getCPSSettleSummaryInfo" {
        let today = chrono_tz::Asia::Shanghai
            .timestamp_millis_opt(s.now())
            .single()
            .unwrap()
            .date_naive();
        let first = today.with_day(1).unwrap();
        let previous = first.pred_opt().unwrap();
        let mut yesterday = Decimal::ZERO;
        let mut current_month = Decimal::ZERO;
        let mut last_month = Decimal::ZERO;
        let mut unsettled = Decimal::ZERO;
        let mut settled = Decimal::ZERO;
        for order in s
            .orders
            .values()
            .filter(|o| o.buyer == owner && !o.deleted && paid(o))
        {
            let date = chrono_tz::Asia::Shanghai
                .timestamp_millis_opt(order.metadata.paid.unwrap_or(order.created))
                .single()
                .unwrap()
                .date_naive();
            let (_, net) = commission(s, order);
            if Some(date) == today.pred_opt() {
                yesterday += net;
            }
            if date.year() == today.year() && date.month() == today.month() {
                current_month += net;
            }
            if date.year() == previous.year() && date.month() == previous.month() {
                last_month += net;
            }
            if order.status == "success" {
                settled += net;
            } else {
                unsettled += net;
            }
        }
        return Ok(
            json!({"yesterdayEstimateAmount":yesterday,"thisMonthEstimateAmount":current_month,"lastMonthEstimateAmount":last_month,"unsettleEstimateAmount":unsettled,"settleAmount":settled}),
        );
    }

    let (start, end) = if name == "alibaba.cps.queryTradeInfoSum" {
        let start = number(p, &["startDate"]).unwrap_or(0);
        let end = number(p, &["endDate"]).unwrap_or(0);
        if !(1_000_000_000_000..10_000_000_000_000).contains(&start)
            || !(1_000_000_000_000..10_000_000_000_000).contains(&end)
            || end < start
            || end - start > 31 * 86400000
        {
            return fail(
                "PARAM_ILLEGAL",
                "13 digit time window must be at most 31 days",
            );
        }
        (Some(start), Some(end))
    } else {
        (None, None)
    };
    let matches_zone = |o: &Order, index: usize| {
        number(p, &["mediaZoneId"]).is_none_or(|zone| {
            o.metadata
                .cps
                .get(&o.lines[index].offer_id)
                .and_then(|r| number(r, &["mediaZoneId"]))
                .unwrap_or(1)
                == zone
        })
    };
    let selected_amount = |o: &Order| {
        o.lines
            .iter()
            .enumerate()
            .filter(|(index, _)| matches_zone(o, *index))
            .map(|(index, _)| line_payment(o, index))
            .sum::<Decimal>()
    };
    let orders: Vec<_> = s
        .orders
        .values()
        .filter(|o| {
            o.buyer == owner
                && !o.deleted
                && start.is_none_or(|t| o.created >= t)
                && end.is_none_or(|t| o.created <= t)
                && o.lines
                    .iter()
                    .enumerate()
                    .any(|(index, _)| matches_zone(o, index))
        })
        .collect();
    let total = orders.iter().map(|o| selected_amount(o)).sum::<Decimal>();
    let payment = orders
        .iter()
        .filter(|o| paid(o))
        .map(|o| selected_amount(o))
        .sum::<Decimal>();
    let payment_commission: Decimal = orders
        .iter()
        .filter(|o| paid(o))
        .map(|o| {
            o.lines
                .iter()
                .enumerate()
                .filter(|(i, _)| matches_zone(o, *i))
                .map(|(i, l)| {
                    (line_payment(o, i) - crate::refunds::settled(s, o.id, o.id * 100 + i as i64).0)
                        .max(Decimal::ZERO)
                        * s.products[&l.offer_id].merchandise.ratio
                })
                .sum::<Decimal>()
        })
        .sum();
    let count = orders.iter().filter(|o| paid(o)).count();
    Ok(
        json!({"orderCount":orders.len(),"gmv":(total*Decimal::from(100)).round(),"paymentGmv":(payment*Decimal::from(100)).round(),"paymentCommission":(payment_commission*Decimal::from(100)).round(),"buyerCount":usize::from(!orders.is_empty()),"paymentBuyerCount":usize::from(count>0),"cancelOrderCount":orders.iter().filter(|o|o.status=="cancel").count(),"paymentOrderCount":count,"initOrderCount":orders.iter().filter(|o|o.status=="waitbuyerpay").count()}),
    )
}

pub fn shops(s: &Store, p: &Value) -> Result<Value> {
    let mut rows = vec![];
    let low = find(p, &["filterRatioMin"])
        .map(|_| decimal(p, &["filterRatioMin"]))
        .transpose()?;
    let high = find(p, &["filterRatioMax"])
        .map(|_| decimal(p, &["filterRatioMax"]))
        .transpose()?;
    if low.zip(high).is_some_and(|(a, b)| a > b) {
        return fail("PARAM_ILLEGAL", "ratio range reversed");
    }
    let name = text(p, &["companyName"]);
    let tags = text(p, &["defineTags"]);
    let mut tag_rules = vec![];
    for tag in tags.split(',').filter(|v| !v.is_empty()) {
        let (key, val) = tag.split_once('=').ok_or_else(|| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "invalid defineTags".into(),
        })?;
        if !["slsj_flag", "sdrd_flag"].contains(&key) || !["true", "false"].contains(&val) {
            return fail("PARAM_ILLEGAL", "invalid defineTags");
        }
        tag_rules.push(val == "true");
    }
    for seller in s.accounts.values() {
        let products: Vec<_> = s
            .products
            .values()
            .filter(|p| p.seller == seller.id && p.active)
            .collect();
        if products.is_empty()
            || number(p, &["sellerId"]).is_some_and(|id| id != seller.user_id)
            || (!name.is_empty() && !seller.login.contains(&name))
        {
            continue;
        }
        if number(p, &["categoryId"])
            .is_some_and(|id| id != 0 && !products.iter().any(|p| p.category == id))
        {
            continue;
        }
        let ratio = products
            .iter()
            .map(|p| p.merchandise.ratio)
            .sum::<Decimal>()
            / Decimal::from(products.len() as u64);
        let certified = products.iter().any(|p| p.merchandise.certified);
        if low.is_some_and(|v| ratio < v)
            || high.is_some_and(|v| ratio > v)
            || tag_rules.iter().any(|v| *v != certified)
        {
            continue;
        }
        rows.push(json!({"sellerId":seller.user_id,"loginId":seller.login,"companyName":seller.login,"tradeGrade":5,"ratio":ratio,"productCnt":products.len(),"tkCnt":products.iter().map(|p|p.merchandise.promotion_count).sum::<i64>(),"linkUrl":format!("{}/contact/{}",s.base_url,seller.id)}));
    }
    let sort = text(p, &["sortField"]);
    let mut ordering = vec![];
    for part in sort.split(';').filter(|v| !v.is_empty()) {
        let (key, direction) = part.split_once('^').ok_or_else(|| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "sortField requires field^direction".into(),
        })?;
        if !["ratio", "productCnt", "tkCnt"].contains(&key) || !["asc", "desc"].contains(&direction)
        {
            return fail("PARAM_ILLEGAL", "unsupported sortField");
        }
        ordering.push((key, direction));
    }
    rows.sort_by(|a, b| {
        for (key, direction) in &ordering {
            let cmp = decimal(a, &[key])
                .unwrap()
                .cmp(&decimal(b, &[key]).unwrap());
            if !cmp.is_eq() {
                return if *direction == "desc" {
                    cmp.reverse()
                } else {
                    cmp
                };
            }
        }
        a["sellerId"].as_i64().cmp(&b["sellerId"].as_i64())
    });
    page(rows, p)
}
