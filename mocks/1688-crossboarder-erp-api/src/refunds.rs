use crate::{business::*, state::*};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::BTreeMap;

pub fn settled(s: &Store, oid: i64, eid: i64) -> (Decimal, Decimal) {
    let mut amount = Decimal::ZERO;
    let mut quantity = Decimal::ZERO;
    for r in s
        .refunds
        .values()
        .filter(|r| r.order_id == oid && r.status == "success")
    {
        if let Some(entry) = r.entries.get(&eid) {
            amount += entry.amount;
            if entry.restock {
                quantity += entry.quantity;
            }
        } else if r.entries.is_empty() {
            let order = &s.orders[&oid];
            let line = &order.lines[(eid - oid * 100) as usize];
            amount += r.amount * (line_payment(order, (eid - oid * 100) as usize) / order.amount);
            if r.return_goods || order.tracking.is_none() {
                quantity += line.quantity * r.amount / order.amount;
            }
        }
    }
    (amount, quantity)
}

pub fn entries(
    s: &Store,
    order: &Order,
    p: &Value,
    amount: Decimal,
) -> Result<BTreeMap<i64, RefundEntry>> {
    let selected = array(p, &["orderEntryIds"]).ok_or_else(|| Error {
        code: "5010".into(),
        message: "order entries required".into(),
    })?;
    if selected.is_empty() {
        return fail("5010", "order entries cannot be empty");
    }
    let returning = text(p, &["disputeRequest"]) == "returnRefund";
    let counts = array(p, &["orderEntryCountList"]);
    if counts.is_some() && (!returning || text(p, &["goodsStatus"]) != "refundBuyerReceived") {
        return fail("5013", "partial quantities require in-sale received return");
    }
    let mut entries = BTreeMap::new();
    let mut limits = BTreeMap::new();
    for value in selected {
        let Some(id) = value.as_i64() else {
            return fail("5010", "invalid order entry");
        };
        let Some(line) = id
            .checked_sub(order.id * 100)
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|i| order.lines.get(i))
        else {
            return fail("5010", "entry does not belong to order");
        };
        if entries.contains_key(&id) {
            return fail("5010", "duplicate order entry");
        }
        let (paid, returned) = settled(s, order.id, id);
        let quantity = line.quantity - returned;
        if quantity <= Decimal::ZERO {
            return fail("5013", "entry already returned");
        }
        entries.insert(
            id,
            RefundEntry {
                quantity,
                amount: Decimal::ZERO,
                restock: returning || order.status == "waitsellersend",
            },
        );
        limits.insert(
            id,
            (line_payment(order, (id - order.id * 100) as usize) - paid).max(Decimal::ZERO),
        );
    }
    if let Some(counts) = counts {
        let mut seen = std::collections::BTreeSet::new();
        if counts.is_empty() {
            return fail("5013", "return counts cannot be empty");
        }
        for count in counts {
            let id = number(count, &["id"]).unwrap_or(0);
            let quantity = decimal(count, &["count"]).map_err(|_| Error {
                code: "5013".into(),
                message: "invalid return quantity".into(),
            })?;
            let Some(entry) = entries.get_mut(&id) else {
                return fail("5013", "quantity entry must be selected");
            };
            if !seen.insert(id) || quantity <= Decimal::ZERO || quantity > entry.quantity {
                return fail("5013", "return quantity exceeds remaining quantity");
            }
            entry.quantity = quantity;
            let line = &order.lines[(id - order.id * 100) as usize];
            let price = line_payment(order, (id - order.id * 100) as usize) / line.quantity;
            limits.insert(id, limits[&id].min(price * quantity));
        }
    }
    let available: Decimal = limits.values().copied().sum();
    if amount <= Decimal::ZERO
        || amount > available
        || (order.status == "waitsellersend" && amount != available)
    {
        return fail(
            "5011",
            "refund amount exceeds selected entries or unshipped full amount",
        );
    }
    // Allocate cents deterministically, carrying the rounding remainder into later entries.
    let mut remaining = amount;
    let mut weight = available;
    for (id, entry) in &mut entries {
        let limit = limits[id];
        entry.amount = if weight == limit {
            remaining
        } else {
            (limit / weight * remaining)
                .round_dp(2)
                .min(limit)
                .min(remaining)
        };
        remaining -= entry.amount;
        weight -= limit;
    }
    Ok(entries)
}

pub fn operations(r: &Refund, s: &Store) -> Vec<Value> {
    let order = &s.orders[&r.order_id];
    let mut previous = "";
    r.history.iter().enumerate().map(|(i,h)| {
        let action = text(h, &["action"]);
        let seller = action.starts_with("refund_");
        let account = &s.accounts[if seller {&order.seller}else{&r.buyer}];
        let status = h["afterStatus"].as_str().unwrap_or_else(|| if action=="apply" {"waitselleragree"} else {r.status.as_str()});
        let value = serde_json::json!({"id":r.id*100+i as i64,"refundId":r.id.to_string(),"beforeOperateStatus":previous,"afterOperateStatus":status,"discription":text(&h["request"],&["description"]),"operateRemark":action,"operatorId":account.id,"operatorLoginId":account.login,"operatorUserId":account.user_id,"operatorRoleId":if seller{2}else{1},"gmtCreate":h["time"],"gmtModified":h["time"],"messageStatus":1,"msgType":7,"freightBill":r.tracking.as_deref().unwrap_or(""),"vouchers":h["request"].get("vouchers").cloned().unwrap_or(serde_json::json!([]))});
        previous=status;
        value
    }).collect()
}
pub fn value(r: &Refund, s: &Store) -> Value {
    use serde_json::json;
    let order = &s.orders[&r.order_id];
    let buyer = &s.accounts[&r.buyer];
    let seller = &s.accounts[&order.seller];
    let initial = r.history.first().unwrap_or(&Value::Null);
    let request = &initial["request"];
    let created = initial["time"].as_i64().unwrap_or(order.created);
    let modified = r
        .history
        .last()
        .and_then(|h| h["time"].as_i64())
        .unwrap_or(created);
    let paid = if r.status == "success" {
        r.amount * Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    let entries: Vec<_> = if r.entries.is_empty() {
        (0..order.lines.len())
            .map(|i| order.id * 100 + i as i64)
            .collect()
    } else {
        r.entries.keys().rev().copied().collect()
    };
    let counts: BTreeMap<_, _> = entries
        .iter()
        .map(|id| {
            (
                id.to_string(),
                r.entries
                    .get(id)
                    .map(|e| e.quantity)
                    .unwrap_or(order.lines[(id - order.id * 100) as usize].quantity),
            )
        })
        .collect();
    let title = entries
        .iter()
        .map(|id| {
            let l = &order.lines[(id - order.id * 100) as usize];
            if l.title.is_empty() {
                s.products[&l.offer_id].title.clone()
            } else {
                l.title.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("、");
    let received = text(request, &["goodsStatus"]).ends_with("Received");
    let mut value = json!({"id":r.id,"refundId":r.id.to_string(),"orderId":r.order_id,"status":r.status,"refundStatus":r.status,"applyPayment":r.amount*Decimal::from(100),"applyExpect":r.amount*Decimal::from(100),"applyCarriage":0,"refundPayment":paid,"refundCarriage":0,"disputeId":r.id,"refundOperationList":operations(r,s),"logisticsNo":r.tracking.as_deref().unwrap_or(""),"freightBill":r.tracking.as_deref().unwrap_or(""),"orderEntryIdList":entries,"orderEntryCountMap":counts,"productName":title,"buyerMemberId":r.buyer,"buyerUserId":buyer.user_id,"buyerLoginId":buyer.login,"sellerMemberId":order.seller,"sellerUserId":seller.user_id,"sellerLoginId":seller.login,"sellerRealName":seller.login,"sellerReceiveAddress":"测试发货仓","sellerMobile":"13800000001","sellerTel":"057100000001"});
    let status = json!({"gmtCreate":created,"gmtApply":created,"gmtModified":modified,"gmtCompleted":if r.status=="success"{Some(modified)}else{None},"gmtFreezed":null,"gmtTimeOut":null,"onlyRefund":!r.return_goods,"refundGoods":r.return_goods,"buyerSendGoods":r.tracking.is_some(),"goodsReceived":received,"goodsStatus":if r.tracking.is_some(){3}else if received{2}else{1},"disputeRequest":if r.return_goods{2}else{1},"disputeType":if initial["afterSales"]==true{2}else{1},"applyReasonId":number(request,&["applyReasonId"]).unwrap_or(1),"applyReason":if number(request,&["applyReasonId"])==Some(2){"商品质量问题"}else{"不想要了"},"canRefundPayment":(order.amount-order.refunded)*Decimal::from(100),"frozenFund":if ["waitselleragree","waitbuyersend","waitsellerreceive"].contains(&r.status.as_str()){r.amount*Decimal::from(100)}else{Decimal::ZERO},"rejectTimes":r.history.iter().filter(|h|h["action"]=="refund_reject").count(),"newRefundReturn":r.return_goods,"tradeTypeStr":order_trade_type(order),"extInfo":{"refundRemark":text(request,&["refundRemark"])}});
    value
        .as_object_mut()
        .unwrap()
        .extend(status.as_object().unwrap().clone());
    for name in [
        "goodsReceived",
        "onlyRefund",
        "refundGoods",
        "newRefundReturn",
        "buyerSendGoods",
    ] {
        let mut chars = name.chars();
        let alias = format!(
            "is{}{}",
            chars.next().unwrap().to_ascii_uppercase(),
            chars.as_str()
        );
        value[alias] = value[name].clone();
    }
    value
}
