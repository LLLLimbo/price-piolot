use crate::{business::*, state::Store};
use serde_json::{json, Value};

pub fn detail(s: &Store, row: &Value, owner: &str) -> Value {
    let request = &row["request"]["request"];
    let seller_login = text(request, &["counterpartyLoginId"]);
    let seller = s.accounts.values().find(|a| a.login == seller_login);
    let status = match row["status"].as_str().unwrap_or("") {
        "WAIT_SIGN" => "SIGNING",
        "SIGNED" => "SIGN_SUCCESS",
        "COMPLETED" => "CONFIRMED",
        "REFUND_PENDING" => "REFUNDING",
        "REFUNDED" => "CLOSED",
        "REJECTED" => "SIGN_REJECT",
        other => other,
    };
    let buyer_name = s.accounts[owner].login.clone();
    let part_b = if text(request, &["counterpartyName"]).is_empty() {
        seller_login.clone()
    } else {
        text(request, &["counterpartyName"])
    };
    let address = &request["receiverAddressTerm"];
    let mut dto = row.clone();
    dto["status"] = json!(status);
    dto["partAName"] = json!(buyer_name);
    dto["partBName"] = json!(part_b);
    dto["contentTerm"] = request["contentTerm"].clone();
    dto["buyerContactName"] = json!(text(
        address,
        &["fullName", "name", "receiverName", "contactName"]
    ));
    dto["buyerContactPhone"] = json!(text(
        address,
        &["mobile", "phone", "receiverPhone", "contactPhone"]
    ));
    dto["sellerContactName"] = json!(seller.map(|a| a.login.as_str()).unwrap_or(&seller_login));
    dto["sellerContactPhone"] = json!("13800000001");
    dto["confirmReceiveRule"] = json!(text(request, &["contractConfirmType"]));
    dto["autoConfirmTime"] = row.get("autoConfirmTime").cloned().unwrap_or(Value::Null);
    dto["payableTime"] = row.get("payableTime").cloned().unwrap_or(Value::Null);
    dto["supplyVoucherList"] = row.get("supplyVoucherList").cloned().unwrap_or(json!([]));
    let payments=row.get("paidAt").map(|date|vec![json!({"payNo":format!("pay-{}",text(row,&["draftNo"])),"payTime":date,"payChannel":"mock"})]).unwrap_or_default();
    json!({"contractDTO":dto,"fundPayOrderList":payments,"traceId":format!("contract-{}",text(row,&["draftNo"])),"isSuccess":true})
}

pub fn paid(row: &mut Value, now: i64) -> Result<()> {
    row["paidAt"] = json!(now);
    let request = &row["request"]["request"];
    if text(request, &["contractConfirmType"]) == "AUTO_CONFIRM" {
        let condition = text(request, &["conditionEnum"]);
        if condition == "PAY_SUCCESS_TIME_PLUS_X_DAYS" {
            let days = number(request, &["xDay"])
                .filter(|n| (1..=3650).contains(n))
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "xDay must be 1..3650".into(),
                })?;
            row["autoConfirmTime"] = json!(now + days * 86400000);
        }
    }
    Ok(())
}
pub fn advance(s: &mut Store) {
    let now = s.now();
    for row in s
        .records
        .get_mut("contracts")
        .into_iter()
        .flat_map(|m| m.values_mut())
    {
        if row["status"] == "PAID" && row["autoConfirmTime"].as_i64().is_some_and(|t| t <= now) {
            row["status"] = json!("COMPLETED");
            row["contractCurrentStatus"] = json!("COMPLETED");
            row["confirmedAt"] = json!(now);
        }
    }
}

pub fn transfer(s: &mut Store, row: &mut Value, p: &Value, owner: &str) -> Result<Value> {
    let amount = decimal(row, &["amount"])?;
    if decimal(p, &["transferToAmount"])? != amount || amount.scale() > 2 {
        return fail(
            "PARAM_ILLEGAL",
            "transferToAmount must equal contract amount in yuan",
        );
    }
    if text(p, &["transferFromCurrency"]) != "USD"
        || text(p, &["transferFundChannel"]) != "WORLD_FIRST"
    {
        return fail("PARAM_ILLEGAL", "USD WORLD_FIRST transfer required");
    }
    let account = find(p, &["escrowAccount"]).cloned().unwrap_or(json!({}));
    if !["COMPANY", "PERSON"].contains(&text(&account, &["cardHolderType"]).as_str())
        || text(&account, &["accountNumber"]).is_empty()
        || text(&account, &["holder"]).is_empty()
    {
        return fail("PARAM_ILLEGAL", "valid escrow account required");
    }
    let cents = amount * rust_decimal::Decimal::from(100);
    row["payOrderNo"] = json!(text(p, &["payOrderNo"]));
    row["transferQuote"] = json!({"success":true,"transferToAmount":cents,"transferToCurrency":"CNY","transferFromAmount":cents,"transferFromCurrency":"USD","quotePrice":1,"feeAmount":0,"feeCurrency":"USD","escrowAccount":account,"traceId":format!("transfer-{}",text(row,&["draftNo"]))});
    s.put(
        "contracts",
        &format!("{owner}:{}", text(row, &["draftNo"])),
        row.clone(),
    );
    Ok(row["transferQuote"].clone())
}
