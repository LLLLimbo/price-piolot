use crate::{business::*, state::Store};
use rust_decimal::Decimal;
use serde_json::{json, Value};

pub fn handle(s: &mut Store, name: &str, p: &Value, owner: &str) -> Option<Result<Value>> {
    let result = (|| {
        crate::cps::validate(name, p)?;
        match name {
            "newtoncloud.model.list" => Ok(json!({"models":s.rows("models",None)})),
            "newtoncloud.points.query" => {
                if number(p, &["pageSize"]).is_some_and(|n| n > 100) {
                    return fail("PARAM_ILLEGAL", "points pageSize maximum is 100");
                }
                let rows = s.rows("point_usage", Some(owner));
                let consumed = rows
                    .iter()
                    .map(|r| decimal(r, &["creditsConsumed"]))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .sum::<Decimal>();
                let mut result = page(rows, p)?;
                result["availableCredits"] = json!(Decimal::from(1000) - consumed);
                Ok(result)
            }
            "newtoncloud.task.create" => {
                let model = text(p, &["model"]);
                if !model.is_empty() && model != "flagship" {
                    return fail("PARAM_ILLEGAL", "unknown model");
                }
                let files = array(p, &["fileUrls"]);
                let names = array(p, &["fileNames"]);
                if files.map(Vec::len) != names.map(Vec::len) {
                    return fail(
                        "PARAM_ILLEGAL",
                        "fileUrls and fileNames must have equal length",
                    );
                }
                if text(p, &["message"]).trim().is_empty() {
                    return fail("PARAM_ILLEGAL", "message is empty");
                }
                if s.rows("tasks", Some(owner))
                    .iter()
                    .filter(|r| !["END", "KILL"].contains(&r["status"].as_str().unwrap_or("")))
                    .count()
                    >= 10
                {
                    return fail("RATE_LIMIT_PENDING_TOTAL", "pending task limit reached");
                }
                let task_id = if text(p, &["taskId"]).is_empty() {
                    format!("task-{}", s.next_id())
                } else {
                    text(p, &["taskId"])
                };
                let key = format!("{owner}:{task_id}");
                if s.records.get("tasks").is_some_and(|m| m.contains_key(&key)) {
                    return fail("TASK_ALREADY_EXISTS", "taskId already exists");
                }
                let session = if text(p, &["sessionId"]).is_empty() {
                    format!("session-{}", s.next_id())
                } else {
                    text(p, &["sessionId"])
                };
                let row = json!({"taskId":task_id,"sessionId":session,"status":"INIT","taskType":"NORMAL","createdAt":s.now().to_string(),"message":text(p,&["message"]),"content":"","messages":[],"nextIndex":0,"outputStatus":"producing","finishedAt":"","workflowName":text(p,&["workflowName"]),"error":"","chunks":"[]","queuePosition":1,"queueLength":1,"queuedTotal":1,"runningTotal":0,"finishedTotal":0});
                s.put("tasks", &key, row.clone());
                Ok(row)
            }
            "newtoncloud.task.list" => {
                let rows = s.rows("tasks", Some(owner));
                if find(p, &["pageNo"]).is_none() {
                    Ok(json!({"items":rows,"total":rows.len()}))
                } else {
                    page(rows, p)
                }
            }
            "newtoncloud.task.get"
            | "newtoncloud.task.kill"
            | "newtoncloud.task.resume"
            | "newtoncloud.task.fetch"
            | "newtoncloud.batchInquiry.getResult" => {
                let task_id = text(p, &["taskId", "wwTaskId"]);
                let key = format!("{owner}:{task_id}");
                let mut task = s
                    .records
                    .get("tasks")
                    .and_then(|m| m.get(&key))
                    .cloned()
                    .ok_or_else(|| Error {
                        code: "TASK_NOT_EXISTS".into(),
                        message: "task not found".into(),
                    })?;
                if name.ends_with("kill") {
                    if ["END", "KILL"].contains(&task["status"].as_str().unwrap_or("")) {
                        return fail("ILLEGAL_TASK_STATUS", "task is terminal");
                    }
                    task["status"] = json!("KILL");
                    task["killed"] = json!(true);
                    task["finishedAt"] = json!(crate::dates::task_time(s.now()));
                    task["outputStatus"] = json!("error");
                    s.emit("AGENT_NEWTON_CLOUD_TASK_NOTIFY", owner, task.clone());
                }
                if name.ends_with("resume") {
                    if task["status"] != "WAIT_USER" {
                        return fail("ILLEGAL_TASK_STATUS", "task is not waiting for input");
                    }
                    if text(p, &["sessionId"]) != task["sessionId"].as_str().unwrap_or("") {
                        return fail("PARAM_ILLEGAL", "sessionId mismatch");
                    }
                    task["status"] = json!("RUNNING");
                    task["outputStatus"] = json!("producing");
                }
                if name.ends_with("fetch") || name.ends_with("getResult") {
                    if task["status"] != "END" {
                        return fail("TASK_NOT_FINISHED", "task has no final result");
                    }
                    return Ok(
                        json!({"data":task["content"],"items":task["rows"].as_array().cloned().unwrap_or_default(),"inquiryStatus":"END"}),
                    );
                }
                s.put("tasks", &key, task.clone());
                if name.ends_with("get") {
                    let index = number(p, &["fromIndex"]).unwrap_or(0);
                    if index < 0 {
                        return fail("PARAM_ILLEGAL", "negative fromIndex");
                    }
                    let messages = task["messages"].as_array().cloned().unwrap_or_default();
                    task["nextIndex"] = json!(messages.len());
                    task["messages"] = json!(messages
                        .into_iter()
                        .skip(index as usize)
                        .collect::<Vec<_>>());
                    let chunks: Vec<Value> =
                        serde_json::from_str(task["chunks"].as_str().unwrap_or("[]"))
                            .unwrap_or_default();
                    task["nextIndex"] = json!(chunks.len());
                    task["chunks"] = json!(serde_json::to_string(
                        &chunks.into_iter().skip(index as usize).collect::<Vec<_>>()
                    )
                    .unwrap());
                    if p["includeBlocks"] != true {
                        task["chunks"] = json!("[]");
                    }
                }
                Ok(task)
            }
            "alibaba.logistics.OpQueryLogisticCompanyList"
            | "alibaba.logistics.OpQueryLogisticCompanyList.offline" => page(
                vec![
                    json!({"id":1,"code":"MOCK","name":"模拟物流","companyNo":"MOCK","companyName":"模拟物流","supportPrint":false}),
                ],
                p,
            ),
            "alibaba.logistics.myFreightTemplate.list.get" => crate::freight::query(s, p, owner),
            "product.freight.estimate" => crate::freight::calculate(s, p),
            "alibaba.trade.receiveAddress.get" => {
                let rows = s.rows("addresses", Some(owner));
                Ok(json!({"items":rows,"receiveAddressList":rows,"addressList":rows}))
            }
            "alibaba.trade.addresscode.get" | "alibaba.trade.addresscode.getchild" => {
                let code = text(p, &["areaCode"]);
                let entries = [
                    ("1", "中国", "0"),
                    ("330000", "浙江省", "1"),
                    ("330100", "杭州市", "330000"),
                    ("330108", "滨江区", "330100"),
                ];
                let rows:Vec<Value>=entries.iter().filter(|(id,_,parent)|if name.ends_with("getchild"){*parent==if code.is_empty(){"0"}else{&code}}else{*id==code}).map(|(id,n,parent)|json!({"code":id,"areaCode":id,"name":n,"areaName":n,"parentCode":parent})).collect();
                if !name.ends_with("getchild") {
                    return rows.first().cloned().ok_or_else(|| Error {
                        code: "404".into(),
                        message: "无法找到相对应的地址信息。".into(),
                    });
                }
                page(rows, p)
            }
            "alibaba.trade.addresscode.parse" | "trade.address.parseText" => {
                let address = if name == "trade.address.parseText" {
                    format!(
                        "{}{}{}{}",
                        text(p, &["country"]),
                        text(p, &["province"]),
                        text(p, &["city"]),
                        text(p, &["area"])
                    )
                } else {
                    text(p, &["addressInfo", "address", "text"])
                };
                if address.trim().is_empty() {
                    return fail("PARAM_ILLEGAL", "address required");
                }
                if !address.contains("杭州") && !address.contains("330108") {
                    return fail("ADDRESS_NOT_RECOGNIZED", "no fixture matches this address");
                }
                Ok(
                    json!({"address":address,"provinceCode":"330000","cityCode":"330100","areaCode":"330108","districtCode":"330108","provinceText":"浙江省","cityText":"杭州市","areaText":"滨江区"}),
                )
            }
            "alibaba.trade.OpQueryMarketingMixConfig" => {
                let identity = text(p, &["sellerMemberId", "sellerLoginId"]);
                if identity.is_empty() {
                    return fail("500_1", "sellerMemberId or sellerLoginId required");
                }
                let seller = s
                    .accounts
                    .values()
                    .find(|a| a.id == identity || a.login == identity)
                    .ok_or_else(|| Error {
                        code: "ACCOUNT_NOT_EXIST".into(),
                        message: "seller not found".into(),
                    })?;
                let products: Vec<_> = s
                    .products
                    .values()
                    .filter(|p| p.seller == seller.id)
                    .collect();
                let mix = products
                    .iter()
                    .any(|p| p.merchandise.tags.iter().any(|t| t == "isSupportMix"));
                let quantity = products
                    .iter()
                    .filter_map(|p| p.merchandise.mix_min_quantity)
                    .max()
                    .unwrap_or(rust_decimal::Decimal::ONE);
                let amount = products
                    .iter()
                    .filter_map(|p| p.merchandise.mix_min_amount)
                    .max()
                    .unwrap_or_default();
                let created = products
                    .iter()
                    .map(|p| p.merchandise.created)
                    .min()
                    .unwrap_or(0);
                Ok(
                    json!({"generalHunpi":mix,"memberId":seller.id,"mixAmount":amount,"mixNumber":quantity,"gmtCreate":created,"gmtModified":created}),
                )
            }
            "shipping.insurance.get" => {
                let o = buyer_order(s, id(p)?, owner)?;
                let kind = text(p, &["type"]);
                let record = s.rows("insurance", Some(owner)).into_iter().find(|r| {
                    r["orderId"] == o.id && (r.get("type").is_none() || r["type"] == kind)
                });
                Ok(record
                    .unwrap_or_else(|| json!({"orderId":o.id,"insuranceId":0,"tradeClaimList":[]})))
            }
            "alibaba.trade.queryOrderByInsure" => {
                let key = text(p, &["insureId", "policyNo", "claimNo"]);
                let record = s
                    .records
                    .get("insurance")
                    .and_then(|m| m.get(&format!("{owner}:{key}")))
                    .ok_or_else(|| Error {
                        code: "INSURANCE_NOT_EXISTS".into(),
                        message: "insurance fixture not found".into(),
                    })?;
                Ok(record.clone())
            }
            "logistics.order.getOutOrderId" => {
                let tracking = text(p, &["shipmentId", "noMainPartCode"]);
                let order = s
                    .orders
                    .values()
                    .find(|o| o.buyer == owner && o.tracking.as_deref() == Some(&tracking))
                    .ok_or_else(|| Error {
                        code: "LOGISTICS_NOT_EXISTS".into(),
                        message: "shipment not found".into(),
                    })?;
                Ok(json!({"outOrderId":order.external_id,"orderId":order.id}))
            }
            "refundofficialdelivery.solution.get"
            | "refundofficialdelivery.order.create"
            | "refundofficialdelivery.order.get"
            | "refundofficialdelivery.order.modify"
            | "refundofficialdelivery.order.cancel" => crate::pickups::handle(s, name, p, owner),
            name if name.starts_with("paycredit.") => crate::credits::handle(s, name, p, owner),
            "trade.invoiceTitle.getPageList" => crate::invoices::titles(s, p, owner),
            "trade.invoiceAmount.getList" | "trade.invoice.consult" => {
                crate::invoices::amounts(s, name, p, owner)
            }
            "trade.invoice.apply" => {
                let requests = array(p, &["invoiceApplyModelList"])
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| Error {
                        code: "PARAM_ILLEGAL".into(),
                        message: "nonempty invoiceApplyModelList required".into(),
                    })?;
                let mut success = vec![];
                let mut failed = vec![];
                for request in requests {
                    let oid = number(request, &["orderId"]).unwrap_or(0);
                    let completed = s
                        .orders
                        .get(&oid)
                        .is_some_and(|o| o.buyer == owner && o.status == "success");
                    match crate::invoices::create(s,owner,&[oid],request) {
                    Ok(_)=>success.push(json!({"orderId":oid,"outBizId":oid.to_string(),"result":true,"tradeOrderCompleted":completed})),
                    Err(error)=>failed.push(json!({"orderId":oid,"outBizId":oid.to_string(),"result":false,"tradeOrderCompleted":completed,"errorCode":error.code,"errorDesc":error.message})),
                }
                }
                Ok(json!({"successList":success,"failedList":failed}))
            }
            "trade.invoice.mergeapply" => {
                let groups: Value = serde_json::from_str(&text(p, &["orderIdGroupsJsonList"]))
                    .map_err(|_| Error {
                        code: "PARAM_ILLEGAL".into(),
                        message: "invalid order groups".into(),
                    })?;
                let groups = groups
                    .as_array()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| Error {
                        code: "PARAM_ILLEGAL".into(),
                        message: "nonempty groups required".into(),
                    })?;
                for group in groups {
                    let ids = order_ids(&if group.is_array() {
                        json!({"orderIds":group})
                    } else {
                        json!({"orderIdsJsonList":format!("[{}]",group.as_str().unwrap_or(""))})
                    })?;
                    crate::invoices::create(s, owner, &ids, p)?;
                }
                Ok(json!({"result":true,"idempotent":false,"isIdempotent":false}))
            }
            "trade.invoiceApply.getPageListBuyerView"
            | "trade.invoice.getListBuyerView"
            | "trade.invoice.sellerqueryrelatedorders" => crate::invoices::list(s, name, p, owner),
            "repurchase.contract.get" => {
                let ids = array(p, &["offerIds"]).ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "offerIds required".into(),
                })?;
                let mut rows = vec![];
                for mut row in s
                    .rows("repurchase", Some(owner))
                    .into_iter()
                    .filter(|r| ids.contains(&r["offerId"]))
                {
                    let id = number(&row, &["offerId"]).unwrap_or(0);
                    let Some(product) = s.products.get(&id) else {
                        continue;
                    };
                    row["freightFee"] = json!(0);
                    row["freightDiscount"] = json!(1);
                    row["freightStrategy"] = json!("free");
                    row["freeAddressCode"] = json!("CN");
                    row["includeTax"] = json!(true);
                    row["invoiceTaxRate"] = json!("0");
                    row["invoiceRequired"] = json!(false);
                    row["deliveryDays"] =
                        json!(if product.merchandise.shipping == "shipIn24Hours" {
                            1
                        } else {
                            2
                        });
                    row["deliveryDateIsDefault"] = json!(true);
                    row["skuRepurchases"] = json!([{"skuId":id,"specId":product.sku,"priceRanges":[{"begin":product.minimum,"end":product.stock,"price":product.price*rust_decimal::Decimal::from(100)}]}]);
                    rows.push(row);
                }
                Ok(json!({"offerRepurchases":rows}))
            }
            "syt.customer.queryUserAuthStatus" => {
                Ok(json!({"authStatus":"AUTHENTICATED","authenticated":true,"userId":owner}))
            }
            "syt.buyer.draftPurchaseOrder" => {
                let request_no = text(p, &["requestNo"]);
                if request_no.is_empty() {
                    return fail("PARAM_ILLEGAL", "requestNo required");
                }
                if let Some(prior) = s
                    .rows("contracts", Some(owner))
                    .into_iter()
                    .find(|row| text(&row["request"], &["requestNo"]) == request_no)
                {
                    if prior["request"]["request"] != p["request"] {
                        return fail(
                            "IDEMPOTENCY_CONFLICT",
                            "requestNo already used for another draft",
                        );
                    }
                    return Ok(prior);
                }
                let id = format!("contract-{}", s.next_id());
                let amount = decimal(p, &["totalAmount", "amount", "contractAmount"])?;
                if amount <= Decimal::ZERO {
                    return fail("PARAM_ILLEGAL", "positive amount required");
                }
                let row = json!({"draftNo":id,"contractCurrentStatus":"WAIT_SIGN","contractId":id,"purchaseOrderId":id,"status":"WAIT_SIGN","amount":amount,"request":p,"createdAt":s.now(),"payableTime":if text(p,&["payTimeType"])=="IMMEDIATELY"{Some(s.now())}else{None}});
                s.put("contracts", &format!("{owner}:{id}"), row.clone());
                s.emit("SYT_CONTRACT_WAIT_SIGN", owner, row.clone());
                Ok(row)
            }
            "syt.buyer.confirmPurchaseOrder"
            | "syt.buyer.queryContractDetail"
            | "syt.contract.confirm"
            | "syt.contract.invalid"
            | "syt.contract.refund"
            | "syt.contract.pay"
            | "syt.contract.payTransfer" => {
                let cid = text(p, &["draftNo", "contractId", "purchaseOrderId"]);
                let key = format!("{owner}:{cid}");
                let mut row = s
                    .records
                    .get("contracts")
                    .and_then(|m| m.get(&key))
                    .cloned()
                    .ok_or_else(|| Error {
                        code: "CONTRACT_NOT_EXISTS".into(),
                        message: "contract not found".into(),
                    })?;
                if name == "syt.contract.refund" {
                    let request_no = text(p, &["requestNo"]);
                    let request_key = format!("{owner}:{request_no}");
                    if let Some(prior) = s
                        .records
                        .get("contract_refund_requests")
                        .and_then(|m| m.get(&request_key))
                    {
                        if prior["request"] != p["request"] {
                            return fail(
                                "IDEMPOTENCY_CONFLICT",
                                "requestNo already used for another refund",
                            );
                        }
                        return Ok(prior["response"].clone());
                    }
                    let amount = decimal(p, &["amount"])?;
                    let refunded = row
                        .get("refunded")
                        .map(|_| decimal(&row, &["refunded"]))
                        .transpose()?
                        .unwrap_or(Decimal::ZERO);
                    if amount <= Decimal::ZERO || amount > decimal(&row, &["amount"])? - refunded {
                        return fail(
                            "REFUND_AMOUNT_EXCEEDED",
                            "contract refund exceeds remaining amount",
                        );
                    }
                    row["refundAmount"] = json!(amount);
                }
                let status = row["status"].as_str().unwrap_or("");
                let next = match name {
                    "syt.buyer.queryContractDetail" => {
                        return Ok(crate::contracts_business::detail(s, &row, owner))
                    }
                    "syt.buyer.confirmPurchaseOrder" if status == "WAIT_SIGN" => "SIGNED",
                    "syt.contract.invalid" if ["WAIT_SIGN", "SIGNED"].contains(&status) => {
                        "INVALID"
                    }
                    "syt.contract.confirm" if status == "PAID" => "COMPLETED",
                    "syt.contract.refund" if ["PAID", "COMPLETED"].contains(&status) => {
                        "REFUND_PENDING"
                    }
                    "syt.contract.payTransfer" if status == "SIGNED" => {
                        return crate::contracts_business::transfer(s, &mut row, p, owner);
                    }
                    "syt.contract.pay" if status == "SIGNED" => {
                        row["payUrl"] = json!(format!("{}/payment/contract/{cid}", s.base_url));
                        return Ok(row);
                    }
                    _ => {
                        return fail(
                            "ILLEGAL_CONTRACT_STATUS",
                            "operation not allowed in current state",
                        )
                    }
                };
                row["status"] = json!(next);
                row["contractCurrentStatus"] = json!(next);
                s.put("contracts", &key, row.clone());
                if name == "syt.contract.refund" {
                    s.put(
                        "contract_refund_requests",
                        &format!("{owner}:{}", text(p, &["requestNo"])),
                        json!({"request":p["request"],"response":row}),
                    );
                }
                if next == "SIGNED" {
                    s.emit("SYT_CONTRACT_SIGN_SUCCESS", owner, row.clone());
                }
                Ok(row)
            }
            "alibaba.cps.listMediaInfo" => page(
                vec![
                    json!({"tkId":s.accounts[owner].user_id,"mediaId":1,"mediaName":"模拟媒体","mediaTitle":"模拟媒体","mediaType":1,"auditState":1,"auditReason":"","mediaZoneId":1,"mediaZoneName":"模拟推广位","mediaZoneTitle":"模拟推广位"}),
                ],
                p,
            ),
            "alibaba.cps.listActivityPageQuery" => page(
                vec![
                    json!({"id":1,"activityId":1,"name":"模拟联盟活动","activityName":"模拟联盟活动","url":format!("{}/promotion/1",s.base_url)}),
                ],
                p,
            ),
            "alibaba.cps.listShopPageQuery" => crate::cps::shops(s, p),
            "alibaba.cps.genClickUrl"
            | "alibaba.cps.generateIntroduceUrlByParam"
            | "alibaba.cps.genSearchPjjxIntroduceUrlByKeyword" => {
                crate::cps::links(s, name, p, owner)
            }
            "alibaba.cps.tradeBillList" | "alibaba.cps.webUnionTradeBillList" => {
                crate::cps::bills(s, name, p, owner)
            }
            "alibaba.cps.listCPSSettleInfoDetail" => crate::cps::settlement(s, p, owner),
            "alibaba.cps.getCPSSettleSummaryInfo" | "alibaba.cps.queryTradeInfoSum" => {
                crate::cps::summary(s, name, p, owner)
            }
            "alibaba.cps.queryRealtimeVisitor" => {
                let rows = s.rows("visitors", Some(owner));
                Ok(json!({"uv":rows.len(),"muv":rows.len()}))
            }
            "alibaba.app.pieceorder.get" | "app.order.get" | "app.expire.get" => {
                crate::accounts_business::app_orders(s, name, p, owner)
            }
            "push.query.messageList" | "push.cursor.messageList" => {
                if number(p, &["pageSize"]).is_some_and(|n| !(20..=200).contains(&n)) {
                    return fail("PARAM_ILLEGAL", "pageSize must be 20..200");
                }
                let start = date_millis(p, "createStartTime")?;
                let end = date_millis(p, "createEndTime")?;
                if start.zip(end).is_some_and(|(a, b)| a > b) {
                    return fail("PARAM_ILLEGAL", "message date range reversed");
                }
                let topic = text(p, &["type"]);
                let user = text(p, &["userInfo"]);
                let selected:Vec<_>=s.events.iter().filter(|e|e.app_key==s.current_app&&(user.is_empty()||e.account==user)&&e.status=="failed"&&start.is_none_or(|t|e.created>=t)&&end.is_none_or(|t|e.created<=t)&&(topic.is_empty()||e.topic==topic)).map(|e|json!({"id":e.id,"msgId":e.id,"type":e.topic,"userInfo":e.account,"data":e.data,"gmtBorn":e.created})).collect();
                let quantity = number(p, &["quantity"]).unwrap_or(20);
                if name.contains("cursor") && !(20..=200).contains(&quantity) {
                    return fail("PARAM_ILLEGAL", "quantity must be 20..200");
                }
                let out = page(selected, p)?;
                if name.contains("cursor") {
                    for row in out["items"].as_array().unwrap() {
                        let id = row["id"].as_str().unwrap();
                        if let Some(e) = s.events.iter_mut().find(|e| e.id == id) {
                            e.status = "confirmed".into();
                        }
                    }
                }
                Ok(out)
            }
            "push.message.confirm" => {
                let ids = array(p, &["msgIdList"]).cloned().unwrap_or_default();
                for mid in ids {
                    let mid = mid
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| mid.to_string());
                    let e = s
                        .events
                        .iter_mut()
                        .find(|e| e.id == mid && e.app_key == s.current_app)
                        .ok_or_else(|| Error {
                            code: "MESSAGE_NOT_EXISTS".into(),
                            message: "message not found".into(),
                        })?;
                    e.status = "confirmed".into();
                }
                Ok(json!({"isSuccess":true}))
            }
            _ => fail("UNHANDLED", "unhandled"),
        }
    })();
    if result.as_ref().err().is_some_and(|e| e.code == "UNHANDLED") {
        None
    } else {
        Some(result)
    }
}
