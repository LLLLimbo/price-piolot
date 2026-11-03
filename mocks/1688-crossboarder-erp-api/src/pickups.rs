use crate::{business::*, state::Store};
use chrono::{NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use serde_json::{json, Value};

fn addresses(s: &Store, p: &Value, order: &crate::state::Order) -> Result<(Value, Value)> {
    let seller=s.rows("addresses",Some(&order.seller)).into_iter().next().unwrap_or_else(||json!({"address":"测试发货仓","fullName":s.accounts[&order.seller].login,"mobile":"13800000001","provinceCode":"330000","cityCode":"330100","areaCode":"330108"}));
    let mut values = vec![];
    for (field, default) in [("senderInfo", &order.address), ("receiverInfo", &seller)] {
        let value = if let Some(v) = p
            .get(field)
            .filter(|v| v.is_object() && v.as_object().is_some_and(|m| !m.is_empty()))
        {
            v.clone()
        } else if let Some(text) = p[format!("{field}Str")].as_str().filter(|v| !v.is_empty()) {
            serde_json::from_str(text).map_err(|_| Error {
                code: "PARAM_ILLEGAL".into(),
                message: format!("{field}Str must be address JSON"),
            })?
        } else {
            default.clone()
        };
        if !value.is_object()
            || text(&value, &["address"]).is_empty()
            || text(&value, &["fullName"]).is_empty()
            || (text(&value, &["mobile"]).is_empty() && text(&value, &["phone"]).is_empty())
        {
            return fail("PARAM_ILLEGAL", "contact name, address and phone required");
        }
        values.push(value);
    }
    Ok((values.remove(0), values.remove(0)))
}
fn measures(p: &Value, quantity: Decimal) -> Result<(i64, Decimal, Decimal)> {
    let count = number(p, &["packageCount"]).unwrap_or(1);
    let weight = if p.get("totalWeight").is_some() {
        decimal(p, &["totalWeight"])?
    } else {
        quantity * Decimal::from(200)
    };
    let volume = if p.get("totalVolume").is_some() {
        decimal(p, &["totalVolume"])?
    } else {
        quantity * Decimal::from(1000)
    };
    if !(1..=1000).contains(&count)
        || weight <= Decimal::ZERO
        || volume <= Decimal::ZERO
        || weight > Decimal::from(1000000000_i64)
        || volume > Decimal::from(1000000000_i64)
    {
        return fail("PARAM_ILLEGAL", "invalid package count, weight or volume");
    }
    Ok((count, weight, volume))
}
fn appointment(p: &Value) -> Result<()> {
    let date =
        NaiveDate::parse_from_str(&text(p, &["dateStr"]), "%Y-%m-%d").map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "dateStr must be yyyy-MM-dd".into(),
        })?;
    let start =
        NaiveTime::parse_from_str(&text(p, &["gmtAcceptStart"]), "%H:%M").map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "invalid pickup start".into(),
        })?;
    let end =
        NaiveTime::parse_from_str(&text(p, &["gmtAcceptEnd"]), "%H:%M").map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "invalid pickup end".into(),
        })?;
    if end <= start
        || date < NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        || date > NaiveDate::from_ymd_opt(2100, 1, 1).unwrap()
    {
        return fail("PARAM_ILLEGAL", "invalid appointment range");
    }
    if ![
        "DAILY_NECESSITIES",
        "FOOD",
        "FURNITURE",
        "METALS",
        "COSMETICS",
        "DIGITAL",
        "GIFT",
        "OTHER",
    ]
    .contains(&text(p, &["goodsType"]).as_str())
    {
        return fail("PARAM_ILLEGAL", "unknown goodsType");
    }
    if text(p, &["solutionCode"]) != "pickup-standard" {
        return fail("SOLUTION_NOT_EXISTS", "unknown pickup solution");
    }
    Ok(())
}
pub fn handle(s: &mut Store, name: &str, input: &Value, owner: &str) -> Result<Value> {
    let p = input.get("param").unwrap_or(input);
    if name.ends_with("order.get")
        || name.ends_with("order.cancel")
        || name.ends_with("order.modify")
    {
        let pid = number(p, &["officialDeliveryOrderId"]).unwrap_or(0);
        let key = format!("{owner}:{pid}");
        let old = s
            .records
            .get("pickups")
            .and_then(|m| m.get(&key))
            .cloned()
            .ok_or_else(|| Error {
                code: "PICKUP_NOT_EXISTS".into(),
                message: "pickup order not found".into(),
            })?;
        if name.ends_with("get") {
            return Ok(old);
        }
        if old["status"] != "WAIT_PICKUP" {
            return fail("ILLEGAL_PICKUP_STATUS", "pickup cannot be changed");
        }
        if name.ends_with("cancel") {
            if ![
                "INFORMATION_INCORRECT",
                "WANT_TO_SEND_MYSELF",
                "PLAN_CHANGED",
                "WANT_TO_CHANGE_PICKUP_TIME",
                "PRICE_TOO_HIGH",
                "ATE",
                "LATE",
                "COURIER_NOT_COMING",
                "COURIER_BAD_ATTITUDE",
                "ITEM_CANNOT_BE_SHIPPED",
                "COURIER_TOO_BUSY",
                "OTHER",
            ]
            .contains(&text(p, &["cancelType"]).as_str())
            {
                return fail("PARAM_ILLEGAL", "unknown cancelType");
            }
            let mut row = old;
            row["status"] = json!("CANCELLED");
            row["logisticsStatus"] = json!("CANCELLED");
            row["gmtModified"] = json!(s.now());
            row["cancelText"] = p["cancelText"].clone();
            s.put("pickups", &key, row);
            return Ok(json!({"result":true}));
        }
        if text(p, &["refundId"]) != text(&old, &["refundId"]) {
            return fail("PARAM_ILLEGAL", "cannot change pickup refund");
        }
        appointment(p)?;
        let oid = number(&old, &["orderId"]).unwrap_or(0);
        let order = buyer_order(s, oid, owner)?;
        let (sender, receiver) = addresses(s, p, order)?;
        measures(p, Decimal::ONE)?;
        let mut row = old;
        for (key, value) in p.as_object().unwrap() {
            row[key] = value.clone();
        }
        row["senderInfo"] = sender;
        row["receiverInfo"] = receiver;
        row["gmtModified"] = json!(s.now());
        s.put("pickups", &key, row);
        return Ok(json!({"result":true}));
    }
    let rid = number(p, &["refundId"]).unwrap_or(0);
    let refund = s
        .refunds
        .get(&rid)
        .filter(|r| r.buyer == owner)
        .ok_or_else(|| Error {
            code: "REFUND_NOT_EXISTS".into(),
            message: "refund not found".into(),
        })?;
    let order = &s.orders[&refund.order_id];
    let ids: Vec<_> = if refund.entries.is_empty() {
        (0..order.lines.len())
            .map(|i| order.id * 100 + i as i64)
            .collect()
    } else {
        refund.entries.keys().copied().collect()
    };
    let quantity = ids
        .iter()
        .map(|id| {
            refund
                .entries
                .get(id)
                .map(|e| e.quantity)
                .unwrap_or(order.lines[(id - order.id * 100) as usize].quantity)
        })
        .sum();
    let (count, weight, volume) = measures(p, quantity)?;
    let (sender, receiver) = addresses(s, p, order)?;
    if name.ends_with("solution.get") {
        let offers: Vec<_> = ids
            .iter()
            .map(|id| {
                let l = &order.lines[(id - order.id * 100) as usize];
                json!({"offerId":l.offer_id,"offerName":s.products[&l.offer_id].title})
            })
            .collect();
        let tomorrow = crate::dates::task_time(s.now() + 86400000)[..10].to_owned();
        let solution = json!({"solutionCode":"pickup-standard","solutionName":"模拟上门取件","cpCode":"MOCK","cpName":"模拟物流","priority":1,"serviceType":"PICKUP","serviceTypeName":"上门揽收","serviceTypeDesc":"预约取件","appointmentTimeDTO":{"timeSlotDTOList":[{"dateStr":tomorrow,"gmtStart":"09:00","gmtEnd":"18:00"}]},"chargeDetailDTO":{"totalCost":0,"deliveryFee":0,"deliveryTotalFee":0,"firstFee":0,"firstTotalCost":0,"firstUnit":1000,"nextFee":0,"nextTotalCost":0,"nextUnit":1000,"originTotalCost":0,"vasTotalFee":0,"vasDetailDTOList":[],"officialCouponDTO":{"gmtEffect":null,"gmtExpired":null}},"deliveryTimeDTO":{"deliveryText":"预约时间上门","deliveryTime":"09:00-18:00","remark":""}});
        return Ok(
            json!({"orderId":order.id,"orderEntryIdList":ids,"offerInfoDTOList":offers,"senderInfoDTO":sender,"receiverInfoDTO":receiver,"totalWeight":weight,"totalVolume":volume,"height":5,"length":20,"width":10,"sceneCode":"RETURN_REFUND","solutionDTOList":[solution],"userSettled":true,"vipUser":false,"userAlipaySignUrl":format!("{}/payment/refund-{rid}",s.base_url)}),
        );
    }
    if !refund.return_goods || refund.status != "waitbuyersend" {
        return fail("ILLEGAL_REFUND_STATUS", "approved return required");
    }
    if s.rows("pickups", Some(owner))
        .iter()
        .any(|r| text(r, &["refundId"]) == rid.to_string() && r["status"] != "CANCELLED")
    {
        return fail("PICKUP_ALREADY_EXISTS", "active pickup already exists");
    }
    appointment(p)?;
    let oid = order.id;
    let pid = s.next_id();
    let now = s.now();
    let mut row = p.clone();
    let fields = json!({"id":pid,"officialDeliveryOrderId":pid,"orderId":oid.to_string(),"bizId":rid.to_string(),"refundId":rid.to_string(),"bizType":"RETURN_REFUND","outBizId":pid.to_string(),"status":"WAIT_PICKUP","logisticsStatus":"WAIT_PICKUP","lastTraceStatus":"WAIT_PICKUP","lastTraceStatusName":"等待上门揽收","lastTraceDetail":"已预约，等待模拟揽收","mailNo":"","signOfficial":true,"gmtModified":now,"senderInfo":sender,"receiverInfo":receiver,"packageCount":count,"totalWeight":weight,"totalVolume":volume,"features":{},"payRecordDTO":{"amount":0,"id":pid,"bizId":rid.to_string(),"bizType":"RETURN_REFUND","outBizId":pid.to_string(),"gmtCreate":now,"gmtModified":now,"status":"NOT_REQUIRED","payType":"FREE","payChannel":"","feature":{"totalFee":"0","chargeWeight":weight/Decimal::from(1000),"chargeVolume":volume/Decimal::from(1000000)}}});
    row.as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    s.put("pickups", &format!("{owner}:{pid}"), row);
    Ok(json!({"result":pid}))
}
