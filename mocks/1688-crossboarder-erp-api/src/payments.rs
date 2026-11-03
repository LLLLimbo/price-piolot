use crate::{
    business::*,
    state::{Order, Store},
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn agreements(s: &Store, owner: &str) -> Vec<Value> {
    s.rows("payment_agreements", Some(owner))
        .into_iter()
        .map(|mut row| {
            let channel = text(&row, &["payChannel"]);
            row["signUrl"] = json!(format!("{}/payment/agreement/{channel}", s.base_url));
            row["agreementNo"] = if row["signedStatus"] == true {
                json!(format!("agreement-{owner}-{channel}"))
            } else {
                json!("")
            };
            row
        })
        .collect()
}

pub fn prepare(s: &mut Store, p: &Value, owner: &str) -> Result<Value> {
    let order = buyer_order(s, id(p)?, owner)?.clone();
    if order.status != "waitbuyerpay" {
        return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting payment");
    }
    let requested = text(p, &["payChannel"]).to_ascii_uppercase();
    if !requested.is_empty() && !["SHEGOU", "ALIPAY"].contains(&requested.as_str()) {
        return fail("PARAM_ILLEGAL", "unsupported withholding channel");
    }
    let agreements = agreements(s, owner);
    let channel = ["SHEGOU", "ALIPAY"]
        .into_iter()
        .find(|channel| {
            (requested.is_empty() || requested == *channel)
                && agreements.iter().any(|a| {
                    a["payChannel"] == *channel
                        && a["bindingStatus"] == true
                        && a["signedStatus"] == true
                })
                && (*channel != "ALIPAY" || channel_available(&order, "alipay"))
        })
        .ok_or_else(|| Error {
            code: "PAYMENT_AGREEMENT_REQUIRED".into(),
            message: "bound and signed payment agreement required".into(),
        })?;
    let paid = s
        .records
        .get("staged_payments")
        .and_then(|m| m.get(&format!("{owner}:{}", order.id)))
        .and_then(|r| decimal(r, &["amount"]).ok())
        .unwrap_or_default();
    let amount = (order.amount - paid) * Decimal::from(100);
    if find(p, &["payAmount"]).is_some() && decimal(p, &["payAmount"])? != amount {
        return fail(
            "PARAM_ILLEGAL",
            "payAmount must equal outstanding amount in cents",
        );
    }
    payment_stock(s, &order, false)?;
    let request_id = s.next_id();
    s.put("withhold_requests", &format!("{owner}:{request_id}"), json!({"orderId":order.id,"payChannel":channel,"payAmount":amount,"opRequestId":text(p,&["opRequestId"]),"created":s.now(),"paySuccess":false}));
    Ok(json!({"success":true,"payChannel":channel,"paySuccess":false}))
}

pub fn steps(s: &Store, order: &Order) -> Vec<Value> {
    let ratios: &[i64] = match order.metadata.trade_type.as_str() {
        "37staged" => &[30, 70],
        "631staged" => &[60, 30, 10],
        _ => return vec![],
    };
    let payment = s
        .records
        .get("staged_payments")
        .and_then(|m| m.get(&format!("{}:{}", order.buyer, order.id)));
    let total_paid = if order.metadata.paid.is_some() {
        order.amount
    } else {
        payment
            .and_then(|r| decimal(r, &["amount"]).ok())
            .unwrap_or_default()
    };
    let cancelled = order.status == "cancel";
    let mut offset = Decimal::ZERO;
    ratios.iter().enumerate().map(|(index, ratio)| {
        let last = index + 1 == ratios.len();
        let amount = if last { order.amount - offset }
            else { (order.amount * Decimal::from(*ratio) / Decimal::from(100)).round_dp(2) };
        let paid = (total_paid - offset).max(Decimal::ZERO).min(amount);
        let active = total_paid >= offset;
        let history = payment.and_then(|r| r["history"].as_array());
        let reached = |target: Decimal| history.and_then(|rows| rows.iter()
            .find(|v| decimal(v, &["total"]).is_ok_and(|n| n >= target)))
            .and_then(|v| v["time"].as_i64());
        let start = if active { if index == 0 { Some(order.created) }
            else { reached(offset).or(order.metadata.paid).or(order.metadata.modified) } } else { None };
        let pay_time = if paid > Decimal::ZERO {
            if paid == amount { reached(offset + amount).or(order.metadata.paid).or(order.metadata.modified) }
            else { history.and_then(|rows| rows.last()).and_then(|v| v["time"].as_i64()).or(order.metadata.modified) }
        } else { None };
        let pay_status = if cancelled { 8 } else if paid == amount { 2 } else { 1 };
        let end = if paid == amount { pay_time } else { None };
        let status = if cancelled { if active { "cancel" } else { "inactiveandcancel" } }
            else if paid == amount { "success" } else if active { "waitbuyerpay" } else { "waitactivate" };
        let row = json!({
            "stepOrderId": order.id * 100 + index as i64, "stepNo": index + 1,
            "lastStep": last, "stepName": format!("第{}阶段", index + 1),
            "activeStatus": if active {1} else {0}, "payStatus": pay_status,
            "stepPayStatus": pay_status, "stepOrderStatus": status,
            "logisticsStatus": match order.status.as_str() {"cancel"=>7,"success"=>3,"waitbuyerreceive"=>2,_=>1},
            "payFee": amount, "paidFee": paid, "goodsFee": amount, "actualPayFee": amount,
            "price": amount, "amount": 1, "postFee": 0, "paidPostFee": 0,
            "adjustFee": 0, "discountFee": 0, "itemDiscountFee": 0,
            "gmtStart": start, "gmtPay": pay_time, "gmtEnd": end,
            "gmtCreate": order.created, "gmtModified": order.metadata.modified.unwrap_or(order.created),
            "enterTime": start, "payTime": pay_time, "endTime": end,
            "sellerActionTime": order.metadata.shipped, "hasDisbursed": order.status == "success",
            "needLogistics": last, "needSellerAction": false, "transferAfterConfirm": true,
            "needSellerCallNext": false, "instantPay": false, "buyerPayTimeout": 86400,
            "buyerConfirmTimeout": 86400, "templateId": if ratios.len() == 2 {37} else {631}
        });
        offset += amount;
        row
    }).collect()
}
