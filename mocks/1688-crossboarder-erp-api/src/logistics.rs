use crate::{business::*, state::Store};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn sent(s: &Store, oid: i64) -> BTreeMap<i64, Decimal> {
    let mut sent = BTreeMap::new();
    for package in s
        .records
        .get("packages")
        .into_iter()
        .flat_map(|p| p.values())
        .filter(|p| p["orderId"] == oid)
    {
        for entry in package["entries"].as_array().into_iter().flatten() {
            *sent
                .entry(number(entry, &["entryId"]).unwrap())
                .or_default() += decimal(entry, &["quantity"]).unwrap_or_default();
        }
    }
    sent
}
pub fn send(
    s: &mut Store,
    oid: i64,
    seller: &str,
    tracking: String,
    entries: Option<&Vec<Value>>,
    options: Option<&Value>,
) -> Result<Value> {
    let order = owned_order(s, oid, seller)?.clone();
    if order.seller != seller {
        return fail("ACCESS_DENIED", "seller authorization required");
    }
    if order.status != "waitsellersend" {
        return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting shipment");
    }
    if tracking.trim().is_empty() {
        return fail("PARAM_ILLEGAL", "tracking number required");
    }
    if s.records
        .get("packages")
        .into_iter()
        .flat_map(|p| p.values())
        .filter(|p| p["tracking"] == tracking)
        .count()
        >= 15
    {
        return fail(
            "INVALID_PARAM",
            "tracking number reuse exceeds 15 shipments",
        );
    }
    let already = sent(s, oid);
    let mut requested = BTreeMap::new();
    if let Some(entries) = entries {
        if entries.is_empty() {
            return fail("PARAM_ILLEGAL", "sendGoodEntries must not be empty");
        }
        for entry in entries {
            let id = number(entry, &["sourceEntryId", "orderEntryId"]).unwrap_or(0);
            let quantity = decimal(entry, &["amount", "quantity"])?;
            if quantity <= Decimal::ZERO || requested.insert(id, quantity).is_some() {
                return fail("PARAM_ILLEGAL", "positive unique entry quantities required");
            }
        }
    } else {
        for (i, line) in order.lines.iter().enumerate() {
            let id = oid * 100 + i as i64;
            let quantity = line.quantity
                - crate::refunds::settled(s, oid, id).1
                - already.get(&id).copied().unwrap_or_default();
            if quantity > Decimal::ZERO {
                requested.insert(id, quantity);
            }
        }
    }
    for (id, quantity) in &requested {
        let index = id
            .checked_sub(oid * 100)
            .and_then(|n| usize::try_from(n).ok());
        let Some(line) = index.and_then(|i| order.lines.get(i)) else {
            return fail("ORDER_ENTRY_NOT_EXISTS", "entry does not belong to order");
        };
        if *quantity
            > line.quantity
                - crate::refunds::settled(s, oid, *id).1
                - already.get(id).copied().unwrap_or_default()
        {
            return fail(
                "SHIPMENT_QUANTITY_EXCEEDED",
                "shipment exceeds remaining quantity",
            );
        }
    }
    for relation in s.rows("order.relation.write", Some(&order.buyer)) {
        if number(&relation, &["purchaseParentOrderId"]) != Some(oid)
            || !requested.contains_key(&number(&relation, &["purchaseOrderId"]).unwrap_or(0))
        {
            continue;
        }
        let external = text(&relation, &["orderId"]);
        let parent = text(&relation, &["parentOrderId"]);
        for synced in s.rows("trade.cross.orderSync", Some(&order.buyer)) {
            let row = synced.get("orderParam").unwrap_or(&synced);
            let main = text(row, &["orderId"]);
            let child = array(row, &["subOrderParamList"])
                .and_then(|a| a.iter().find(|r| text(r, &["orderId"]) == external));
            if main != external && (main != parent || child.is_none()) {
                continue;
            }
            if text(row, &["status"]) == "close" {
                return fail("5002", "downstream sales order is closed");
            }
            if number(child.unwrap_or(row), &["refundStatus"]) == Some(1) {
                return fail("5001", "downstream sales order is refunding");
            }
        }
    }
    let complete = order.lines.iter().enumerate().all(|(i, l)| {
        let id = oid * 100 + i as i64;
        already.get(&id).copied().unwrap_or_default()
            + requested.get(&id).copied().unwrap_or_default()
            == l.quantity - crate::refunds::settled(s, oid, id).1
    });
    let empty = json!({});
    let options = options.unwrap_or(&empty);
    let now = date_millis(options, "gmtSend")?.unwrap_or_else(|| s.now());
    if now < order.created || now > s.now() {
        return fail("PARAM_ILLEGAL", "gmtSend outside order lifetime");
    }
    let mut receiver = options
        .get("receiverInfo")
        .filter(|r| r.is_object() && r.as_object().is_some_and(|m| !m.is_empty()))
        .unwrap_or(&order.address)
        .clone();
    for (from, to) in [
        ("province", "provinceText"),
        ("city", "cityText"),
        ("area", "areaText"),
        ("post", "postCode"),
    ] {
        if receiver.get(to).is_none() {
            if let Some(value) = receiver.get(from).cloned() {
                receiver[to] = value;
            }
        }
    }
    let company_code = options["cpCode"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or("MOCK");
    let company_name = options["logisticsCpName"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or("模拟物流");
    let package_id = s.next_id();
    let entries: Vec<_> = requested
        .into_iter()
        .map(|(id, q)| json!({"entryId":id,"quantity":q}))
        .collect();
    s.put("packages",&format!("{}:{package_id}",order.buyer),json!({"logisticsId":package_id.to_string(),"orderId":oid,"tracking":tracking,"created":now,"entries":entries,"receiver":receiver,"cpCode":company_code,"logisticsCpName":company_name,"remarks":text(options,&["remarks"])}));
    let current = s.orders.get_mut(&oid).unwrap();
    current.tracking = Some(tracking.clone());
    current.metadata.modified = Some(now);
    if complete {
        current.status = "waitbuyerreceive".into();
        current.metadata.shipped = Some(now);
    }
    s.emit(
        if complete {
            "ORDER_BUYER_VIEW_ANNOUNCE_SENDGOODS"
        } else {
            "ORDER_BUYER_VIEW_PART_PART_SENDGOODS"
        },
        &order.buyer,
        json!({"orderId":oid.to_string()}),
    );
    s.emit("LOGISTICS_BUYER_VIEW_TRACE",&order.buyer,json!({"orderId":oid.to_string(),"logisticsBillNo":tracking,"logisticsId":package_id,"entries":entries,"cpCode":company_code}));
    Ok(order_value(&s.orders[&oid], s))
}
pub fn query(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    let oid = id(p)?;
    let o = buyer_order(s, oid, owner)?;
    if name.contains("TraceInfo") && o.created < s.now() - 365 * 86400000 {
        return fail(
            "order.createtime.history",
            "trace history is limited to one year",
        );
    }
    let requested = text(p, &["logisticsId"]);
    let fields = text(p, &["fields"]);
    if fields
        .split(',')
        .filter(|v| !v.is_empty())
        .any(|v| !["company.name", "sender", "receiver", "sendgood"].contains(&v))
    {
        return fail("PARAM_ILLEGAL", "unknown logistics field");
    }
    let mut packages: Vec<_> = s
        .rows("packages", Some(owner))
        .into_iter()
        .filter(|v| v["orderId"] == oid)
        .collect();
    if packages.is_empty() {
        if let Some(tracking) = &o.tracking {
            packages.push(json!({"logisticsId":oid.to_string(),"orderId":oid,"tracking":tracking,"created":o.metadata.shipped.unwrap_or(o.created),"receiver":o.address,"entries":o.lines.iter().enumerate().map(|(i,l)|json!({"entryId":oid*100+i as i64,"quantity":l.quantity})).collect::<Vec<_>>()}));
        }
    }
    if packages.is_empty() {
        return fail("500_2", "order has no logistics");
    }
    let mut rows = vec![];
    for package in packages {
        if !requested.is_empty() && package["logisticsId"] != requested {
            continue;
        }
        let pid = text(&package, &["logisticsId"]);
        let created = package["created"].as_i64().unwrap();
        let a = &package["receiver"];
        let mut goods = vec![];
        let mut ids = vec![];
        for entry in package["entries"].as_array().unwrap() {
            let eid = number(entry, &["entryId"]).unwrap();
            let line = &o.lines[(eid - oid * 100) as usize];
            ids.push(eid.to_string());
            let title = if line.title.is_empty() {
                &s.products[&line.offer_id].title
            } else {
                &line.title
            };
            goods.push(json!({"logisticsId":pid,"tradeOrderId":oid,"tradeOrderItemId":eid,"description":line.spec_id,"quantity":entry["quantity"],"unit":"件","productName":title,"goodName":title}));
        }
        let signed = o.status == "success"
            || ids.iter().all(|id| {
                id.parse::<i64>()
                    .ok()
                    .is_some_and(|id| o.metadata.received.contains_key(&id))
            });
        let mut steps =
            vec![json!({"remark":"包裹已发出","acceptTime":crate::dates::task_time(created)})];
        if signed {
            steps.push(json!({"remark":"包裹已签收","acceptTime":crate::dates::task_time(ids.iter().filter_map(|id|id.parse::<i64>().ok().and_then(|id|o.metadata.received.get(&id).copied())).max().or(o.metadata.completed).unwrap_or(created))}));
        }
        let row = json!({"id":pid,"logisticsId":pid,"orderId":oid,"logisticsBillNo":package["tracking"],"mailNo":package["tracking"],"orderEntryIds":ids.join(","),"status":if signed{"SIGN"}else{"TRANSPORT"},"logisticsCompanyId":"MOCK","logisticsCompanyName":package["logisticsCpName"].as_str().unwrap_or("模拟物流"),"logisticsCompanyNo":package["cpCode"].as_str().unwrap_or("MOCK"),"remarks":text(&package,&["remarks"]),"serviceFeature":"standard","gmtSystemSend":crate::dates::task_time(created),"sendGoods":goods,"logisticsOrderGoods":goods,"logisticsOrderSendGood":goods,"logisticsSteps":steps,
        "receiver":{"receiverName":text(a,&["fullName"]),"receiverPhone":text(a,&["phone","mobile"]),"receiverMobile":text(a,&["mobile"]),"receiverProvinceCode":text(a,&["provinceCode"]),"receiverCityCode":text(a,&["cityCode"]),"receiverCountyCode":text(a,&["areaCode"]),"receiverAddress":text(a,&["address"]),"receiverProvince":text(a,&["provinceText"]),"receiverCity":text(a,&["cityText"]),"receiverCounty":text(a,&["areaText"])},
        "sender":{"senderName":s.accounts[&o.seller].login,"senderPhone":"13800000001","senderMobile":"13800000001","senderProvinceCode":"330000","senderCityCode":"330100","senderCountyCode":"330108","senderAddress":"测试发货仓","senderProvince":"浙江省","senderCity":"杭州市","senderCounty":"滨江区"}});
        rows.push(row);
    }
    if name.contains("TraceInfo") && rows.is_empty() {
        return fail("500_2", "logisticsId does not belong to order");
    }
    page(rows, p)
}
