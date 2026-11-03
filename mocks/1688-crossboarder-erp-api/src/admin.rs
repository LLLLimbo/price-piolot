use crate::{
    business::*,
    state::{Fault, Store, Subscription},
    App,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

fn error(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error":message}))).into_response()
}
pub async fn state(State(app): State<App>) -> Json<Value> {
    Json(serde_json::to_value(&*app.store.lock().unwrap()).unwrap())
}
pub async fn reset(State(app): State<App>, Json(p): Json<Value>) -> Response {
    let mut old = app.store.lock().unwrap();
    let mut next = if let Some(value) = p.get("state") {
        match serde_json::from_value::<Store>(value.clone()) {
            Ok(s) => s,
            Err(e) => return error(&e.to_string()),
        }
    } else {
        Store::seed(old.base_url.clone())
    };
    if let Some(time) = p.get("time") {
        let Some(time) = time.as_i64() else {
            return error("time must be milliseconds");
        };
        next.fixed_time = Some(time);
    }
    if let Err(e) = next.validate() {
        return error(&e);
    }
    next.generation = old.generation + 1;
    next.events.clear();
    next.faults.clear();
    next.codes.clear();
    *old = next;
    Json(json!({"reset":true})).into_response()
}
pub async fn advance(State(app): State<App>, Json(p): Json<Value>) -> Response {
    let Some(ms) = p["milliseconds"].as_i64().filter(|ms| *ms >= 0) else {
        return error("milliseconds must be nonnegative");
    };
    {
        let mut s = app.store.lock().unwrap();
        let Some(offset) = s.offset_ms.checked_add(ms) else {
            return error("clock overflow");
        };
        if s.fixed_time
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
            .checked_add(offset)
            .is_none_or(|t| !(0..=4102444800000).contains(&t))
        {
            return error("clock must be between 1970 and 2100");
        }
        s.offset_ms = offset;
        crate::contracts_business::advance(&mut s);
    }
    crate::messages::deliver(&app).await;
    Json(json!({"now":app.store.lock().unwrap().now()})).into_response()
}
pub async fn faults(State(app): State<App>, Json(rules): Json<Vec<Fault>>) -> Response {
    if rules.iter().any(|r| {
        r.delay_ms > 60000
            || (!app.contracts.apis.contains_key(&r.api)
                && !app
                    .contracts
                    .apis
                    .values()
                    .any(|a| a["catalogue"]["name"].as_str() == Some(&r.api)))
    }) {
        return error("unknown API or delay over 60000 ms");
    }
    app.store.lock().unwrap().faults = rules;
    Json(json!({"updated":true})).into_response()
}
pub async fn subscriptions(
    State(app): State<App>,
    Json(rows): Json<Vec<Subscription>>,
) -> Response {
    let mut s = app.store.lock().unwrap();
    for row in &rows {
        if !app.contracts.messages.contains_key(&row.topic)
            || !s.accounts.contains_key(&row.account)
            || !s.apps.contains_key(&row.app_key)
            || row.secret.is_empty()
        {
            return error("unknown topic/account or empty secret");
        }
        let Ok(url) = reqwest::Url::parse(&row.url) else {
            return error("invalid URL");
        };
        if !["http", "https"].contains(&url.scheme()) {
            return error("HTTP callback required");
        }
    }
    s.subscriptions = rows;
    Json(json!({"updated":true})).into_response()
}
pub async fn action(State(app): State<App>, Json(p): Json<Value>) -> Response {
    let result = {
        let mut guard = app.store.lock().unwrap();
        let mut s = guard.clone();
        let result = apply(&app, &mut s, &p);
        if result.is_ok() {
            s.finish_changes(&guard);
            *guard = s;
        }
        result
    };
    match result {
        Ok(v) => {
            crate::messages::deliver(&app).await;
            Json(v).into_response()
        }
        Err(e) => error(&format!("{}: {}", e.code, e.message)),
    }
}
fn apply(app: &App, s: &mut Store, p: &Value) -> Result<Value> {
    let action = text(p, &["action"]);
    s.current_app = p
        .get("appKey")
        .and_then(Value::as_str)
        .unwrap_or("test-app")
        .to_string();
    if !s.apps.contains_key(&s.current_app) {
        return fail("INVALID_APP_KEY", "unknown appKey");
    }
    let owner = if text(p, &["account"]).is_empty() {
        "buyer-1".into()
    } else {
        text(p, &["account"])
    };
    if !s.accounts.contains_key(&owner) {
        return fail("ACCOUNT_NOT_EXIST", "account not found");
    }
    match action.as_str() {
        "payment" => pay(s, id(p)?, &owner, p["success"] != false),
        "shipment" => {
            let oid = id(p)?;
            let seller = s
                .orders
                .get(&oid)
                .map(|o| o.seller.clone())
                .ok_or_else(|| Error {
                    code: "ORDER_NOT_EXISTS".into(),
                    message: "order not found".into(),
                })?;
            ship(s, oid, &seller, text(p, &["tracking"]))
        }
        "refund_approve" | "refund_reject" | "refund_complete" => {
            let rid = number(p, &["refundId"]).unwrap_or(0);
            let r = s.refunds.get(&rid).cloned().ok_or_else(|| Error {
                code: "REFUND_NOT_EXISTS".into(),
                message: "refund not found".into(),
            })?;
            if action == "refund_reject" {
                if r.status != "waitselleragree" {
                    return fail("ILLEGAL_REFUND_STATUS", "refund is not awaiting approval");
                }
                s.refunds.get_mut(&rid).unwrap().status = "rejected".into();
            } else if action == "refund_approve" && r.return_goods {
                if r.status != "waitselleragree" {
                    return fail("ILLEGAL_REFUND_STATUS", "refund is not awaiting approval");
                }
                s.refunds.get_mut(&rid).unwrap().status = "waitbuyersend".into();
            } else {
                if !((!r.return_goods && r.status == "waitselleragree")
                    || (r.return_goods && r.status == "waitsellerreceive"))
                {
                    return fail("ILLEGAL_REFUND_STATUS", "refund cannot settle");
                }
                let o = s.orders.get_mut(&r.order_id).unwrap();
                if o.refunded + r.amount > o.amount {
                    return fail("REFUND_AMOUNT_EXCEEDED", "refund amount exceeded");
                }
                o.refunded += r.amount;
                if let Some(payment) = s
                    .records
                    .get("fund_payments")
                    .and_then(|m| m.get(&format!("{}:{}", r.buyer, r.order_id)))
                    .cloned()
                {
                    let key = format!("{}:{}", r.buyer, text(&payment, &["applyNo"]));
                    let fund = s
                        .records
                        .get_mut("funds")
                        .and_then(|m| m.get_mut(&key))
                        .ok_or_else(|| Error {
                            code: "FUND_NOT_EXISTS".into(),
                            message: "payment fund not found".into(),
                        })?;
                    fund["availableAmount"] =
                        json!(decimal(fund, &["availableAmount"])? + r.amount * Decimal::from(100));
                } else {
                    s.accounts.get_mut(&r.buyer).unwrap().balance += r.amount;
                }
                if o.refunded == o.amount {
                    o.status = "cancel".into();
                }
                if r.entries.is_empty() {
                    if o.refunded == o.amount && (r.return_goods || o.tracking.is_none()) {
                        for l in &o.lines {
                            s.products.get_mut(&l.offer_id).unwrap().stock += l.quantity;
                        }
                    }
                } else {
                    for (eid, entry) in &r.entries {
                        if entry.restock {
                            let line = &o.lines[(eid - o.id * 100) as usize];
                            s.products.get_mut(&line.offer_id).unwrap().stock += entry.quantity;
                        }
                    }
                }
                s.refunds.get_mut(&rid).unwrap().status = "success".into();
            }
            let now = s.now();
            let r = s.refunds.get_mut(&rid).unwrap();
            r.history
                .push(json!({"action":action,"time":now,"request":p,"afterStatus":r.status}));
            let r = r.clone();
            let v = crate::refunds::value(&r, s);
            let buyer = r.buyer.clone();
            let after_sales = r.history.first().is_some_and(|h| h["afterSales"] == true);
            s.emit(
                if after_sales {
                    "ORDER_BUYER_VIEW_ORDER_REFUND_AFTER_SALES"
                } else {
                    "ORDER_BUYER_VIEW_ORDER_BUYER_REFUND_IN_SALES"
                },
                &buyer,
                v.clone(),
            );
            Ok(v)
        }
        "task_transition" => {
            let tid = text(p, &["taskId"]);
            let key = format!("{owner}:{tid}");
            let next = text(p, &["status"]);
            let now = s.now();
            let defaults = s
                .products
                .values()
                .filter(|v| v.active)
                .take(2)
                .map(|v| json!({"offerId":v.id,"title":v.title}))
                .collect::<Vec<_>>();
            let task = s
                .records
                .get_mut("tasks")
                .and_then(|m| m.get_mut(&key))
                .ok_or_else(|| Error {
                    code: "TASK_NOT_EXISTS".into(),
                    message: "task not found".into(),
                })?;
            let current = task["status"].as_str().unwrap_or("");
            let valid = match current {
                "INIT" => ["RUNNING", "KILL"].contains(&next.as_str()),
                "RUNNING" => ["WAIT_SKILL", "WAIT_USER", "END", "KILL"].contains(&next.as_str()),
                "WAIT_SKILL" => ["RUNNING", "KILL"].contains(&next.as_str()),
                "WAIT_USER" => next == "KILL",
                _ => false,
            };
            if !valid {
                return fail("ILLEGAL_TASK_STATUS", "invalid transition");
            }
            task["status"] = json!(next);
            task["outputStatus"] = json!(match next.as_str() {
                "END" => "done",
                "KILL" => "error",
                "WAIT_USER" => "wait_user",
                _ => "producing",
            });
            let workflow = task["workflowName"].as_str().unwrap_or("");
            let invalid_workflow =
                !workflow.is_empty() && workflow != "1688-supplychain-procurement-search";
            if next == "END" && invalid_workflow {
                return fail(
                    "WORKFLOW_NOT_FOUND",
                    "unknown workflow cannot complete successfully; terminate with KILL",
                );
            }
            if next == "KILL" {
                task["error"] = json!(if invalid_workflow {
                    "workflow not found"
                } else {
                    "task terminated"
                });
            }
            if ["END", "KILL"].contains(&next.as_str()) {
                task["finishedAt"] = json!(crate::dates::task_time(now));
            }
            if next == "END" {
                task["rows"] = json!(defaults);
                task["content"] = json!(serde_json::to_string(&defaults).unwrap());
            }
            if let Some(content) = p.get("content") {
                task["content"] = content.clone();
            }
            if let Some(rows) = p.get("rows") {
                task["rows"] = rows.clone();
            }
            if next == "END" {
                let content = task["content"].clone();
                let mut messages = task["messages"].as_array().cloned().unwrap_or_default();
                messages.push(json!({"type":"text","content":content,"timestamp":now}));
                messages.push(json!({"type":"done","content":"","timestamp":now}));
                task["nextIndex"] = json!(messages.len());
                task["chunks"] = json!(serde_json::to_string(&messages).unwrap());
                task["messages"] = json!(messages);
            }
            let v = task.clone();
            if next == "END" {
                s.put("point_usage",&format!("{owner}:{tid}"),json!({"sessionId":v["sessionId"],"sessionName":v["message"],"creditsConsumed":1,"totalTokens":100,"time":now}));
            }
            s.emit("AGENT_NEWTON_CLOUD_TASK_NOTIFY", &owner, v.clone());
            Ok(v)
        }
        "product_update" => {
            let now = s.now();
            let oid = number(p, &["offerId"]).unwrap_or(0);
            let product = s.products.get_mut(&oid).ok_or_else(|| Error {
                code: "PRODUCT_NOT_EXISTS".into(),
                message: "product not found".into(),
            })?;
            let previous_stock = product.stock;
            let mut topics = vec![
                "PRODUCT_PRODUCT_CHANGE",
                "PRODUCT_RELATION_VIEW_PRODUCT_NEW_OR_MODIFY",
            ];
            if p.get("stock").is_some() {
                let stock = decimal(p, &["stock"])?;
                if stock < Decimal::ZERO || stock > Decimal::from(1000000000_i64) {
                    return fail("PARAM_ILLEGAL", "negative stock");
                }
                product.stock = stock;
                topics.push("PRODUCT_PRODUCT_INVENTORY_CHANGE");
            }
            if p.get("price").is_some() {
                let price = decimal(p, &["price"])?;
                if price < Decimal::ZERO || price > Decimal::from(1000000000000_i64) {
                    return fail("PARAM_ILLEGAL", "negative price");
                }
                product.price = price;
                topics.push("PRODUCT_PFT_OFFER_PRICE_MODIFY");
            }
            if let Some(active) = p.get("active").and_then(Value::as_bool) {
                product.active = active;
                topics.push(if active {
                    "PRODUCT_RELATION_VIEW_PRODUCT_REPOST"
                } else {
                    "PRODUCT_RELATION_VIEW_PRODUCT_EXPIRE"
                });
                if !active {
                    topics.push("PRODUCT_PFT_OFFER_QUIT");
                }
            }
            product.merchandise.modified = Some(now);
            let mut v = product_value(
                product,
                &s.base_url,
                &s.accounts[&product.seller],
                &s.current_app,
            );
            v["previousStock"] = json!(previous_stock);
            for topic in topics {
                s.emit(topic, &owner, v.clone());
            }
            Ok(v)
        }
        "fund_settle" => crate::credits::settle(s, p, &owner),
        "fund_refund_settle" => crate::credits::settle_refund(s, p, &owner),
        "invoice_issue" => {
            let iid = number(p, &["applyId"]).unwrap_or(0);
            let now = s.now();
            let row = s
                .records
                .get_mut("invoices")
                .and_then(|m| m.get_mut(&format!("{owner}:{iid}")))
                .ok_or_else(|| Error {
                    code: "INVOICE_NOT_EXISTS".into(),
                    message: "invoice not found".into(),
                })?;
            if row["status"] != "APPLIED" {
                return fail("ILLEGAL_INVOICE_STATUS", "invoice already issued");
            }
            row["status"] = json!("ISSUED");
            row["bizStatus"] = json!("ISSUED");
            row["gmtModified"] = json!(now);
            row["invoiceDate"] = json!(now);
            row["sellerInputInvoiceTime"] = json!(now);
            row["invoiceFileType"] = json!("XML");
            row["ossInvoiceFileName"] = json!(format!("invoice-{iid}.xml"));
            row["invoiceDownloadUrl"] = json!(format!("{}/files/invoice-{iid}", s.base_url));
            row["invoiceNo"] = json!(format!("INV-{iid}"));
            row["invoiceUrl"] = json!(format!("{}/files/invoice-{iid}", s.base_url));
            let value = row.clone();
            let content = format!(
                "<invoice><id>{iid}</id><amount>{}</amount><orders>{}</orders></invoice>",
                value["amount"], value["orderIds"]
            );
            use base64::Engine;
            s.put("files", &format!("{owner}:invoice-{iid}"), json!({"id":format!("invoice-{iid}"),"content":base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),"size":content.len(),"filename":format!("invoice-{iid}.xml")}));
            Ok(value)
        }
        "contract_pay" | "contract_refund" => {
            let now = s.now();
            let cid = text(p, &["contractId"]);
            let key = format!("{owner}:{cid}");
            let row = s
                .records
                .get_mut("contracts")
                .and_then(|m| m.get_mut(&key))
                .ok_or_else(|| Error {
                    code: "CONTRACT_NOT_EXISTS".into(),
                    message: "contract not found".into(),
                })?;
            let amount = decimal(row, &["amount"])?;
            if amount <= Decimal::ZERO {
                return fail("PARAM_ILLEGAL", "contract amount must be positive");
            }
            let a = s.accounts.get_mut(&owner).unwrap();
            if action == "contract_pay" {
                if row["status"] != "SIGNED" {
                    return fail("ILLEGAL_CONTRACT_STATUS", "contract must be signed");
                }
                if a.frozen || a.balance < amount {
                    return fail("ACCOUNT_BALANCE_NOT_ENOUGH", "cannot pay contract");
                }
                a.balance -= amount;
                row["status"] = json!("PAID");
                crate::contracts_business::paid(row, now)?;
            } else {
                if row["status"] != "REFUND_PENDING" {
                    return fail("ILLEGAL_CONTRACT_STATUS", "contract has no pending refund");
                }
                let refund = decimal(row, &["refundAmount"])?;
                let refunded = row
                    .get("refunded")
                    .map(|_| decimal(row, &["refunded"]))
                    .transpose()?
                    .unwrap_or(Decimal::ZERO);
                if refund <= Decimal::ZERO || refunded + refund > amount {
                    return fail(
                        "REFUND_AMOUNT_EXCEEDED",
                        "contract refund exceeds remaining amount",
                    );
                }
                a.balance += refund;
                row["refunded"] = json!(refunded + refund);
                row["status"] = json!(if refunded + refund == amount {
                    "REFUNDED"
                } else {
                    "PAID"
                });
            }
            row["contractCurrentStatus"] = row["status"].clone();
            let v = row.clone();
            s.emit(
                if action == "contract_pay" {
                    "SYT_CONTRACT_PAY_SUCCESS"
                } else {
                    "SYT_REFUND_FINISH_NOTICE"
                },
                &owner,
                v.clone(),
            );
            Ok(v)
        }
        "order_price" => {
            let oid = id(p)?;
            let amount = decimal(p, &["amount"])?;
            let o = s.orders.get_mut(&oid).ok_or_else(|| Error {
                code: "ORDER_NOT_EXISTS".into(),
                message: "order not found".into(),
            })?;
            if o.status != "waitbuyerpay" || amount <= Decimal::ZERO {
                return fail(
                    "ILLEGAL_ORDER_STATUS",
                    "only unpaid orders accept a positive revised amount",
                );
            }
            let paid = s
                .records
                .get("staged_payments")
                .and_then(|m| m.get(&format!("{}:{oid}", o.buyer)))
                .map(|v| decimal(v, &["amount"]))
                .transpose()?
                .unwrap_or(Decimal::ZERO);
            if amount < paid || amount.scale() > 2 || amount > Decimal::from(1000000000000_i64) {
                return fail(
                    "PARAM_ILLEGAL",
                    "revised amount must cover paid installments and use cents",
                );
            }
            o.amount = amount;
            let buyer = o.buyer.clone();
            let v = order_value(&s.orders[&oid], s);
            s.emit(
                "ORDER_BUYER_VIEW_ORDER_PRICE_MODIFY",
                &buyer,
                json!({"orderId":oid}),
            );
            Ok(v)
        }
        "seller_cancel" | "system_cancel" => {
            let oid = id(p)?;
            let o = s.orders.get(&oid).cloned().ok_or_else(|| Error {
                code: "ORDER_NOT_EXISTS".into(),
                message: "order not found".into(),
            })?;
            if o.status != "waitbuyerpay" {
                return fail("ILLEGAL_ORDER_STATUS", "only unpaid orders can close");
            }
            release_staged(s, &o)?;
            s.orders.get_mut(&oid).unwrap().status = "cancel".into();
            if !o.metadata.stock_at_payment {
                for l in o.lines {
                    s.products.get_mut(&l.offer_id).unwrap().stock += l.quantity;
                }
            }
            s.emit(
                if action == "seller_cancel" {
                    "ORDER_BUYER_VIEW_ORDER_SELLER_CLOSE"
                } else {
                    "ORDER_BUYER_VIEW_ORDER_BOPS_CLOSE"
                },
                &o.buyer,
                json!({"orderId":oid}),
            );
            Ok(json!({"closed":true}))
        }
        "seller_address" => {
            let oid = id(p)?;
            let address = p
                .get("address")
                .filter(|v| v.is_object())
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "address object required".into(),
                })?;
            let o = s.orders.get_mut(&oid).ok_or_else(|| Error {
                code: "ORDER_NOT_EXISTS".into(),
                message: "order not found".into(),
            })?;
            if !["waitbuyerpay", "waitsellersend"].contains(&o.status.as_str()) {
                return fail("ILLEGAL_ORDER_STATUS", "order already shipped");
            }
            o.address = address.clone();
            let buyer = o.buyer.clone();
            s.emit(
                "ORDER_BUYER_VIEW_ORDER_SELLER_MODIFY_ADRESS",
                &buyer,
                json!({"orderId":oid}),
            );
            Ok(json!({"updated":true}))
        }
        "tracking_update" => {
            let oid = id(p)?;
            let tracking = text(p, &["tracking"]);
            let o = s.orders.get_mut(&oid).ok_or_else(|| Error {
                code: "ORDER_NOT_EXISTS".into(),
                message: "order not found".into(),
            })?;
            if o.status != "waitbuyerreceive" || tracking.is_empty() {
                return fail(
                    "ILLEGAL_ORDER_STATUS",
                    "shipped order and tracking required",
                );
            }
            let buyer = o.buyer.clone();
            let requested = text(p, &["logisticsId"]);
            let mut changes = vec![];
            if let Some(packages) = s.records.get_mut("packages") {
                for package in packages.values_mut().filter(|r| {
                    r["orderId"] == oid
                        && (requested.is_empty() || text(r, &["logisticsId"]) == requested)
                }) {
                    let old = package["tracking"].clone();
                    package["tracking"] = json!(tracking);
                    changes.push(json!({"orderId":oid,"logisticsId":package["logisticsId"],"oldMailNo":old,"newMailNo":tracking,"entries":package["entries"],"oldCpCode":package["cpCode"].as_str().unwrap_or("MOCK"),"newCpCode":package["cpCode"].as_str().unwrap_or("MOCK")}));
                }
            }
            if changes.is_empty() && !requested.is_empty() {
                return fail("LOGISTICS_NOT_EXISTS", "package does not belong to order");
            }
            let old = s
                .orders
                .get_mut(&oid)
                .unwrap()
                .tracking
                .replace(tracking.clone());
            if changes.is_empty() {
                changes.push(
                    json!({"orderId":oid,"logisticsId":oid,"oldMailNo":old,"newMailNo":tracking}),
                );
            }
            for change in changes {
                s.emit("LOGISTICS_MAIL_NO_CHANGE", &buyer, change);
            }
            Ok(json!({"updated":true}))
        }
        "partial_shipment" => {
            let oid = id(p)?;
            let quantity = decimal(p, &["quantity"])?;
            let o = s.orders.get(&oid).ok_or_else(|| Error {
                code: "ORDER_NOT_EXISTS".into(),
                message: "order not found".into(),
            })?;
            let sent = crate::logistics::sent(s, oid);
            let remaining: Decimal = o
                .lines
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    l.quantity
                        - sent
                            .get(&(oid * 100 + i as i64))
                            .copied()
                            .unwrap_or_default()
                })
                .sum();
            if quantity <= Decimal::ZERO || quantity >= remaining {
                return fail(
                    "ILLEGAL_ORDER_STATUS",
                    "partial quantity must remain below unshipped total",
                );
            }
            let seller = o.seller.clone();
            let mut left = quantity;
            let mut entries = vec![];
            for (i, line) in o.lines.iter().enumerate() {
                let id = oid * 100 + i as i64;
                let amount = (line.quantity - sent.get(&id).copied().unwrap_or_default()).min(left);
                if amount > Decimal::ZERO {
                    entries.push(json!({"sourceEntryId":id,"amount":amount}));
                    left -= amount;
                }
            }
            crate::logistics::send(
                s,
                oid,
                &seller,
                format!("PARTIAL-{oid}-{}", s.sequence),
                Some(&entries),
                None,
            )
        }
        "staged_payment" => {
            let oid = id(p)?;
            let amount = decimal(p, &["amount"])?;
            let o = buyer_order(s, oid, &owner)?.clone();
            let key = format!("{owner}:{oid}");
            let paid = s
                .records
                .get("staged_payments")
                .and_then(|m| m.get(&key))
                .map(|v| decimal(v, &["amount"]))
                .transpose()?
                .unwrap_or(Decimal::ZERO);
            if o.status != "waitbuyerpay"
                || amount <= Decimal::ZERO
                || amount.scale() > 2
                || amount > o.amount - paid
            {
                return fail("ILLEGAL_ORDER_STATUS", "invalid payment installment");
            }
            if paid + amount == o.amount {
                payment_stock(s, &o, true)?;
            }
            let a = s.accounts.get_mut(&owner).unwrap();
            if a.frozen || a.balance < amount {
                return fail("ACCOUNT_BALANCE_NOT_ENOUGH", "insufficient balance");
            }
            a.balance -= amount;
            let mut history = s
                .records
                .get("staged_payments")
                .and_then(|m| m.get(&key))
                .and_then(|r| r["history"].as_array())
                .cloned()
                .unwrap_or_default();
            history.push(json!({"amount":amount,"total":paid+amount,"time":s.now()}));
            s.put(
                "staged_payments",
                &key,
                json!({"amount":paid+amount,"history":history}),
            );
            if paid + amount == o.amount {
                s.orders.get_mut(&oid).unwrap().status = "waitsellersend".into();
            }
            s.emit(
                "ORDER_BUYER_VIEW_ORDER_STEP_PAY",
                &owner,
                json!({"orderId":oid}),
            );
            Ok(json!({"paidAmount":paid+amount}))
        }
        "product_delete" | "product_audit" => {
            let now = s.now();
            let oid = number(p, &["offerId"]).unwrap_or(0);
            let product = s.products.get_mut(&oid).ok_or_else(|| Error {
                code: "PRODUCT_NOT_EXISTS".into(),
                message: "product not found".into(),
            })?;
            product.active = false;
            product.merchandise.modified = Some(now);
            let mut v = product_value(
                product,
                &s.base_url,
                &s.accounts[&product.seller],
                &s.current_app,
            );
            v["status"] = json!(if action == "product_delete" {
                "deleted"
            } else {
                "audit"
            });
            s.emit(
                if action == "product_delete" {
                    "PRODUCT_RELATION_VIEW_PRODUCT_DELETE"
                } else {
                    "PRODUCT_RELATION_VIEW_PRODUCT_AUDIT"
                },
                &owner,
                v,
            );
            Ok(json!({"updated":true}))
        }
        "lp_distribute" => {
            let oid = number(p, &["offerId"]).unwrap_or(0);
            if !s.products.contains_key(&oid) {
                return fail("PRODUCT_NOT_EXISTS", "product not found");
            }
            let record = json!({"offerId":oid,"status":"PUSHED"});
            s.put("distribution", &format!("{owner}:{oid}"), record.clone());
            let timestamp = s.now().to_string();
            let sign = crate::protocol::signature(
                "",
                &std::collections::BTreeMap::from([("timestamp".into(), timestamp.clone())]),
                &s.apps[&s.current_app],
            );
            s.emit("CROSSBOARD_LP_DISTRIBUTION",&owner,json!({"customerId":owner,"userKey":owner,"appkey":s.current_app,"userToken":"mock-selection-token","offerList":[{"offerId":oid.to_string(),"pic":format!("{}/assets/product.svg",s.base_url)}],"sign":sign,"timestamp":timestamp}));
            Ok(json!({"distributed":true}))
        }
        "contract_content" | "contract_reject" => {
            let cid = text(p, &["draftNo", "contractId"]);
            let key = format!("{owner}:{cid}");
            let now = s.now();
            let row = s
                .records
                .get_mut("contracts")
                .and_then(|m| m.get_mut(&key))
                .ok_or_else(|| Error {
                    code: "CONTRACT_NOT_EXISTS".into(),
                    message: "contract not found".into(),
                })?;
            if action == "contract_reject" {
                if row["status"] != "WAIT_SIGN" {
                    return fail(
                        "ILLEGAL_CONTRACT_STATUS",
                        "contract is not awaiting signature",
                    );
                }
                row["status"] = json!("REJECTED");
                row["signRejectReason"] = json!(text(p, &["reason"]));
                row["signRejectTime"] = json!(chrono::DateTime::from_timestamp_millis(now)
                    .unwrap()
                    .to_rfc3339());
            } else {
                if ["INVALID", "REJECTED", "REFUNDED"]
                    .contains(&row["status"].as_str().unwrap_or(""))
                {
                    return fail("ILLEGAL_CONTRACT_STATUS", "contract is closed");
                }
                row["content"] = json!(text(p, &["content"]));
                row["submitterRole"] = json!("BUYER");
                row["addTime"] = json!(now);
            }
            row["contractCurrentStatus"] = row["status"].clone();
            let v = row.clone();
            s.emit(
                if action == "contract_reject" {
                    "SYT_CONTRACT_REJECT"
                } else {
                    "SYT_ADD_CONTRACT_CONTENT"
                },
                &owner,
                v.clone(),
            );
            Ok(v)
        }
        "token_revoke" => {
            let token = text(p, &["accessToken"]);
            let Some(t) = s.tokens.remove(&token) else {
                return fail("INVALID_ACCESS_TOKEN", "token not found");
            };
            let revoked: Vec<String> = s
                .tokens
                .iter()
                .filter(|(_, x)| !t.refresh.is_empty() && x.refresh == t.refresh)
                .map(|(k, _)| k.clone())
                .chain(std::iter::once(token))
                .collect();
            s.tokens.retain(|key, x| {
                !revoked.contains(key)
                    && x.parent_token
                        .as_ref()
                        .is_none_or(|parent| !revoked.contains(parent))
            });
            Ok(json!({"revoked":true}))
        }
        "message_emit" => {
            let topic = text(p, &["topic"]);
            if !app.contracts.messages.contains_key(&topic) {
                return fail("PARAM_ILLEGAL", "unknown topic");
            }
            let data = p.get("data").cloned().unwrap_or(json!({}));
            if p["validate"] != false {
                crate::messages::validate(&app.contracts.messages[&topic], &data)?;
            }
            s.enqueue(&topic, &owner, data);
            Ok(json!({"queued":true}))
        }
        "message_replay" => {
            let mid = text(p, &["messageId"]);
            let event = s
                .events
                .iter()
                .find(|e| e.id == mid)
                .cloned()
                .ok_or_else(|| Error {
                    code: "MESSAGE_NOT_EXISTS".into(),
                    message: "message not found".into(),
                })?;
            s.enqueue(&event.topic, &event.account, event.data);
            Ok(json!({"queued":true}))
        }
        _ => fail("UNKNOWN_ACTION", "unknown management action"),
    }
}
