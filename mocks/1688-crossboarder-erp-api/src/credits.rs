use crate::{business::*, state::Store};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn handle(s: &mut Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    match name {
        "paycredit.batchFundCharge.decison" => {
            if decimal(p, &["applyAmount"])? <= Decimal::ZERO {
                return fail("PARAM_ILLEGAL", "positive amount required");
            }
            Ok(json!({"result":!s.accounts[owner].frozen}))
        }
        "paycredit.batchFundCharge.apply" => {
            let amount = decimal(p, &["applyAmount"])?;
            let out_pay = text(p, &["outPayNo"]);
            let out_biz = text(p, &["outBizNo"]);
            if amount <= Decimal::ZERO
                || out_pay.is_empty()
                || out_pay.len() > 64
                || out_biz.is_empty()
                || out_biz.len() > 64
            {
                return fail(
                    "PARAM_ILLEGAL",
                    "positive amount and identifiers of 1..64 bytes required",
                );
            }
            if let Some(prior) = s
                .rows("funds", Some(owner))
                .into_iter()
                .find(|r| r["outPayNo"] == out_pay)
            {
                if decimal(&prior, &["applyAmount"])? != amount || prior["outBizNo"] != out_biz {
                    return fail(
                        "IDEMPOTENCY_CONFLICT",
                        "outPayNo already used for another request",
                    );
                }
                return Ok(prior);
            }
            let id = format!("fund-{}", s.next_id());
            let row = json!({"applyNo":id,"applyAmount":amount,"payAmount":amount,"payedAmount":0,"availableAmount":0,"outPayNo":out_pay,"outBizNo":out_biz,"status":"INIT","assertNo":format!("asset-{id}"),"payNo":format!("pay-{id}"),"cashierUrl":format!("{}/payment/{id}",s.base_url)});
            s.put("funds", &format!("{owner}:{id}"), row.clone());
            Ok(row)
        }
        "paycredit.batchFundCharge.status" => s
            .records
            .get("funds")
            .and_then(|m| m.get(&format!("{owner}:{}", text(p, &["applyNo"]))))
            .cloned()
            .ok_or_else(|| Error {
                code: "FUND_NOT_EXISTS".into(),
                message: "fund not found".into(),
            }),
        "paycredit.orderVerification.pay" => {
            let oid = id(p)?;
            let order = buyer_order(s, oid, owner)?.clone();
            payment_stock(s, &order, true)?;
            let key = format!("{owner}:{}", text(p, &["applyNo"]));
            let fund = s
                .records
                .get_mut("funds")
                .and_then(|m| m.get_mut(&key))
                .ok_or_else(|| Error {
                    code: "FUND_NOT_EXISTS".into(),
                    message: "fund not found".into(),
                })?;
            verify_receipt(fund, p)?;
            if order.status != "waitbuyerpay" {
                return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting payment");
            }
            let balance = decimal(fund, &["availableAmount"])?;
            let amount = order.amount * Decimal::from(100);
            if balance < amount {
                return fail("ACCOUNT_BALANCE_NOT_ENOUGH", "fund balance insufficient");
            }
            fund["availableAmount"] = json!(balance - amount);
            let apply = fund["applyNo"].clone();
            s.put(
                "fund_payments",
                &format!("{owner}:{oid}"),
                json!({"orderId":oid,"applyNo":apply,"amount":amount}),
            );
            s.orders.get_mut(&oid).unwrap().status = "waitsellersend".into();
            s.emit("ORDER_BUYER_VIEW_ORDER_PAY", owner, json!({"orderId":oid}));
            Ok(json!({"result":true}))
        }
        "paycredit.fundCharge.refund" => {
            let identifier = text(p, &["identifier"]);
            let amount = decimal(p, &["applyRefundAmount"])?;
            let apply = text(p, &["applyNo"]);
            if identifier.is_empty() {
                return fail("PARAM_ILLEGAL", "identifier required");
            }
            if let Some(prior) = s
                .rows("fund_refunds", Some(owner))
                .into_iter()
                .find(|r| r["identifier"] == identifier)
            {
                if prior["applyNo"] != apply || decimal(&prior, &["amount"])? != amount {
                    return fail(
                        "IDEMPOTENCY_CONFLICT",
                        "identifier already used for another refund",
                    );
                }
                return Ok(prior);
            }
            let payments: Vec<_> = s
                .rows("fund_payments", Some(owner))
                .into_iter()
                .filter(|r| r["applyNo"] == apply)
                .collect();
            let paid = payments
                .iter()
                .map(|r| decimal(r, &["amount"]))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .sum::<Decimal>();
            if number(p, &["orderSuccessNums"]) != Some(payments.len() as i64)
                || decimal(p, &["totalPaySuccessAmount"])? != paid
                || number(p, &["orderTotalNums"]).is_none_or(|n| n < payments.len() as i64)
                || decimal(p, &["totalPayAmount"])? < paid
            {
                return fail(
                    "BATCH_STATISTICS_MISMATCH",
                    "paid order count or paid total does not match batch",
                );
            }
            let key = format!("{owner}:{apply}");
            let fund = s
                .records
                .get_mut("funds")
                .and_then(|m| m.get_mut(&key))
                .ok_or_else(|| Error {
                    code: "FUND_NOT_EXISTS".into(),
                    message: "fund not found".into(),
                })?;
            verify_receipt(fund, p)?;
            let balance = decimal(fund, &["availableAmount"])?;
            if amount <= Decimal::ZERO || amount > balance {
                return fail("REFUND_AMOUNT_EXCEEDED", "invalid refund amount");
            }
            fund["availableAmount"] = json!(balance - amount);
            let rid = format!("fund-refund-{}", s.next_id());
            // shortcut: SUCCEED acknowledges acceptance; settlement remains INIT until an explicit management action.
            let row = json!({"identifier":identifier,"applyNo":apply,"outBizNo":text(p,&["outBizNo"]),"assertNo":text(p,&["assertNo"]),"refundNo":rid,"amount":amount,"refundAmount":0,"refundStatus":"SUCCEED","status":"INIT"});
            s.put("fund_refunds", &format!("{owner}:{rid}"), row.clone());
            Ok(row)
        }
        "paycredit.refundCharge.status" => s
            .records
            .get("fund_refunds")
            .and_then(|m| m.get(&format!("{owner}:{}", text(p, &["refundNo"]))))
            .cloned()
            .ok_or_else(|| Error {
                code: "REFUND_NOT_EXISTS".into(),
                message: "fund refund not found".into(),
            }),
        _ => fail("UNHANDLED", "unhandled"),
    }
}
fn verify_receipt(fund: &Value, p: &Value) -> Result<()> {
    if fund["status"] != "SUCCESS" {
        return fail("ILLEGAL_FUND_STATUS", "fund not settled");
    }
    if fund["assertNo"] != text(p, &["assertNo"]) || fund["outBizNo"] != text(p, &["outBizNo"]) {
        return fail(
            "INVALID_RECEIPT",
            "assertNo or outBizNo does not match fund",
        );
    }
    Ok(())
}
pub fn settle(s: &mut Store, p: &Value, owner: &str) -> Result<Value> {
    let key = format!("{owner}:{}", text(p, &["applyNo"]));
    let fund = s
        .records
        .get_mut("funds")
        .and_then(|m| m.get_mut(&key))
        .ok_or_else(|| Error {
            code: "FUND_NOT_EXISTS".into(),
            message: "fund not found".into(),
        })?;
    if fund["status"] != "INIT" {
        return fail("ILLEGAL_FUND_STATUS", "fund already settled");
    }
    let success = p["success"] != false;
    fund["status"] = json!(if success { "SUCCESS" } else { "INVALID" });
    if success {
        fund["availableAmount"] = fund["applyAmount"].clone();
        fund["payedAmount"] = fund["applyAmount"].clone();
    }
    let row = fund.clone();
    s.emit("FENXIAO_PAY_CREDIT_FUND_CHARGE_RESULT", owner, row.clone());
    Ok(row)
}
pub fn settle_refund(s: &mut Store, p: &Value, owner: &str) -> Result<Value> {
    let key = format!("{owner}:{}", text(p, &["refundNo"]));
    let row = s
        .records
        .get_mut("fund_refunds")
        .and_then(|m| m.get_mut(&key))
        .ok_or_else(|| Error {
            code: "REFUND_NOT_EXISTS".into(),
            message: "refund not found".into(),
        })?;
    if row["status"] != "INIT" {
        return fail("ILLEGAL_REFUND_STATUS", "refund already settled");
    }
    let success = p["success"] != false;
    row["status"] = json!(if success { "SUCCESS" } else { "FAILED" });
    row["refundStatus"] = json!(if success { "SUCCEED" } else { "FAILED" });
    let amount = decimal(row, &["amount"])?;
    row["refundAmount"] = if success { json!(amount) } else { json!(0) };
    let v = row.clone();
    if !success {
        let key = format!("{owner}:{}", text(&v, &["applyNo"]));
        let fund = s
            .records
            .get_mut("funds")
            .and_then(|m| m.get_mut(&key))
            .ok_or_else(|| Error {
                code: "FUND_NOT_EXISTS".into(),
                message: "fund not found".into(),
            })?;
        fund["availableAmount"] = json!(decimal(fund, &["availableAmount"])? + amount);
    }
    s.emit(
        "FENXIAO_PAY_CREDIT_FUND_CHARGE_REFUND_RESULT",
        owner,
        v.clone(),
    );
    Ok(v)
}
