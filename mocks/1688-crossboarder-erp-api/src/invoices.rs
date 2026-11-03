use crate::{
    business::*,
    state::{Order, Store},
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn related<'a>(s: &'a Store, o: &Order) -> Vec<&'a Value> {
    s.records
        .get("invoices")
        .into_iter()
        .flat_map(|r| r.iter())
        .filter(|(key, r)| {
            key.starts_with(&format!("{}:", o.buyer))
                && r["orderIds"]
                    .as_array()
                    .is_some_and(|ids| ids.contains(&json!(o.id)))
        })
        .map(|(_, r)| r)
        .collect()
}

pub fn setting(s: &Store, o: &Order) -> Value {
    let supports = |kind: &str| {
        o.lines.iter().all(|l| {
            o.metadata
                .paid_invoice_types
                .get(&l.offer_id)
                .unwrap_or(&s.products[&l.offer_id].merchandise.invoice_types)
                .iter()
                .any(|v| v == kind)
        })
    };
    let common = supports("VATAX_COMM");
    let special = supports("VATAX_SPEC");
    let rows = related(s, o);
    let status = if rows.iter().any(|r| r["bizStatus"] == "ISSUED") {
        "2"
    } else if rows.iter().any(|r| {
        !["FAILED", "CLOSED", "RETURNED", "DEPRECATED"].contains(&text(r, &["bizStatus"]).as_str())
    }) {
        "1"
    } else if o.status == "success" && o.amount > o.refunded && (common || special) {
        "0"
    } else {
        "-1"
    };
    json!({"tradeInvoiceStatus":status,"sellerInvoiceType":match(common,special){(true,true)=>"2",(true,false)=>"0",(false,true)=>"1",_=>""}})
}

pub fn order_info(s: &Store, o: &Order) -> Value {
    let rows = related(s, o);
    let Some(row) = rows
        .into_iter()
        .max_by_key(|r| number(r, &["id"]).unwrap_or(0))
    else {
        return json!({});
    };
    let title = &row["purchaserInvoiceTitle"];
    let mut out = title.clone();
    if !out.is_object() {
        out = json!({});
    }
    out["OpenUid"] = json!(crate::accounts_business::open_uid(&s.current_app, &o.buyer));
    out["invoiceCompanyName"] = json!(text(title, &["title"]));
    out["invoiceType"] = json!(if row["invoiceType"] == "VATAX_SPEC" {
        1
    } else {
        0
    });
    out["localInvoiceId"] = row["id"].clone();
    out["orderId"] = json!(o.id);
    out["taxpayerIdentify"] = json!(text(title, &["taxpayerIdentify"]));
    out["amount"] = json!(invoice_order_amount(row, o.id));
    out["status"] = row["bizStatus"].clone();
    out
}

fn invoice_order_amount(row: &Value, oid: i64) -> Decimal {
    row["invoiceDetailDTOList"]
        .as_array()
        .map(|details| {
            details
                .iter()
                .filter(|line| {
                    text(line, &["subOutBizId"])
                        .parse::<i64>()
                        .ok()
                        .is_some_and(|id| id / 100 == oid)
                })
                .map(|line| decimal(line, &["invoiceAmountWithTax"]).unwrap_or_default())
                .sum()
        })
        .unwrap_or_else(|| decimal(row, &["amount"]).unwrap_or_default())
}

pub fn titles(s: &Store, p: &Value, owner: &str) -> Result<Value> {
    let kind = text(p, &["titleType"]);
    if !kind.is_empty()
        && ![
            "PERSONAL",
            "COMPANY",
            "SOCIAL_ORGANIZATION",
            "PARENT_VIRTUAL",
        ]
        .contains(&kind.as_str())
    {
        return fail("PARAM_INVALID", "invalid titleType");
    }
    let rows = s
        .rows("invoice_titles", Some(owner))
        .into_iter()
        .filter(|r| kind.is_empty() || r["titleType"] == kind)
        .collect();
    let mut data = page(rows, p)?;
    let rows = data["items"].as_array().unwrap();
    let companies: Vec<_> = rows
        .iter()
        .filter(|r| r["titleType"] == "COMPANY" || r["titleType"] == "PARENT_VIRTUAL")
        .cloned()
        .collect();
    let personal: Vec<_> = rows
        .iter()
        .filter(|r| r["titleType"] == "PERSONAL" || r["titleType"] == "SOCIAL_ORGANIZATION")
        .cloned()
        .collect();
    data["companyInvoiceTitles"] = json!(companies);
    data["personalInvoiceTitles"] = json!(personal);
    Ok(data)
}
fn available(s: &Store, owner: &str, oid: i64) -> Result<Decimal> {
    let o = buyer_order(s, oid, owner)?;
    if o.status != "success" {
        return Ok(Decimal::ZERO);
    }
    let reserved = s
        .rows("invoices", Some(owner))
        .iter()
        .filter(|r| {
            !["FAILED", "CLOSED", "RETURNED", "DEPRECATED"]
                .contains(&text(r, &["bizStatus"]).as_str())
        })
        .filter(|r| {
            r["orderIds"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(oid)))
        })
        .map(|r| invoice_order_amount(r, oid))
        .sum::<Decimal>();
    Ok(((o.amount - o.refunded) * Decimal::from(100) - reserved).max(Decimal::ZERO))
}
pub fn amounts(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    let ids = order_ids(p)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut rows = vec![];
    let mut groups: BTreeMap<String, (Vec<String>, Decimal)> = BTreeMap::new();
    for oid in ids {
        if !seen.insert(oid) {
            return fail("PARAM_ILLEGAL", "duplicate orderId");
        }
        let o = buyer_order(s, oid, owner)?;
        let amount = available(s, owner, oid)?;
        rows.push(json!({"orderId":oid,"amount":amount,"invoiceAmount":amount,"canInvoice":amount>Decimal::ZERO}));
        if amount > Decimal::ZERO {
            let group = groups.entry(o.seller.clone()).or_default();
            group.0.push(oid.to_string());
            group.1 += amount;
        }
    }
    if name == "trade.invoice.consult" {
        let groups:Vec<_>=groups.into_iter().map(|(seller,(ids,amount))|json!({"orderIds":ids,"sellerLoginId":s.accounts[&seller].login,"totalAmount":amount})).collect();
        return Ok(
            json!({"supportMerge":groups.iter().any(|g|g["orderIds"].as_array().unwrap().len()>1),"groups":groups}),
        );
    }
    page(rows, p)
}
pub fn list(s: &Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    let oid = number(p, &["orderId"]);
    let out_id = text(p, &["outBizId"]);
    if name == "trade.invoice.sellerqueryrelatedorders" {
        let rows = s
            .records
            .get("invoices")
            .into_iter()
            .flat_map(|r| r.values());
        let row = rows
            .filter(|r| {
                r["sellerMemberId"] == owner
                    || r["orderIds"].as_array().is_some_and(|ids| {
                        ids.iter()
                            .filter_map(Value::as_i64)
                            .any(|id| s.orders.get(&id).is_some_and(|o| o.buyer == owner))
                    })
            })
            .find(|r| {
                r["outBizId"] == out_id
                    && oid.is_none_or(|id| {
                        r["orderIds"]
                            .as_array()
                            .is_some_and(|a| a.contains(&json!(id)))
                    })
            })
            .ok_or_else(|| Error {
                code: "INVOICE_NOT_EXISTS".into(),
                message: "invoice not found for account and order".into(),
            })?;
        return Ok(
            json!({"orderIds":row["orderIds"],"mergeInvoice":row["orderIds"].as_array().is_some_and(|a|a.len()>1),"outBizId":row["outBizId"]}),
        );
    }
    let title = text(p, &["fuzzyInvoiceTitle"]);
    let states = array(p, &["bizStatusList"]);
    let allowed = [
        "ISSUING",
        "ISSUED",
        "VERIFYING",
        "VERIFY_FAILED",
        "RETURNING",
        "RETURNED",
        "INVALIDING",
        "DEPRECATED",
        "RED_ISSUING",
        "RED_PART_ISSUED",
        "RED_ALL_ISSUED",
        "FAILED",
        "CLOSED",
    ];
    if states.is_some_and(|s| {
        s.iter()
            .any(|v| !allowed.contains(&v.as_str().unwrap_or("")))
    }) {
        return fail("PARAM_ILLEGAL", "invalid bizStatusList");
    }
    for (a, b) in [
        ("createMillTimeStart", "createMillTimeEnd"),
        ("modifyMillTimeStart", "modifyMillTimeEnd"),
    ] {
        if number(p, &[a])
            .zip(number(p, &[b]))
            .is_some_and(|(a, b)| a > b)
        {
            return fail("PARAM_ILLEGAL", "date range reversed");
        }
    }
    let rows = s
        .rows("invoices", Some(owner))
        .into_iter()
        .filter(|r| {
            (name != "trade.invoice.getListBuyerView" || r["bizStatus"] == "ISSUED")
                && oid.is_none_or(|id| {
                    r["orderIds"]
                        .as_array()
                        .is_some_and(|a| a.contains(&json!(id)))
                })
                && (out_id.is_empty() || r["outBizId"] == out_id)
                && (title.is_empty()
                    || text(&r["purchaserInvoiceTitleModel"], &["title"]).contains(&title))
                && states.is_none_or(|states| states.is_empty() || states.contains(&r["bizStatus"]))
                && find(p, &["isRedInvoice"]).is_none_or(|v| v == &r["isRedInvoice"])
                && [
                    ("createMillTimeStart", "gmtCreate", true),
                    ("createMillTimeEnd", "gmtCreate", false),
                    ("modifyMillTimeStart", "gmtModified", true),
                    ("modifyMillTimeEnd", "gmtModified", false),
                ]
                .iter()
                .all(|(key, field, start)| {
                    number(p, &[key]).is_none_or(|limit| {
                        r[field]
                            .as_i64()
                            .is_some_and(|t| if *start { t >= limit } else { t <= limit })
                    })
                })
        })
        .map(|mut row| {
            let seller = text(&row, &["sellerMemberId"]);
            let buyer_uid = crate::accounts_business::open_uid(&s.current_app, owner);
            let seller_uid = crate::accounts_business::open_uid(&s.current_app, &seller);
            row["purchaserOpenUid"] = json!(buyer_uid);
            row["sellerOpenUid"] = json!(seller_uid);
            for key in ["purchaserInvoiceTitle", "purchaserInvoiceTitleModel"] {
                row[key]["OpenUid"] = json!(buyer_uid);
            }
            row["sellerInvoiceTitleModel"]["OpenUid"] = json!(seller_uid);
            if let Some(details) = row["invoiceDetailDTOList"].as_array_mut() {
                for detail in details {
                    detail["purchaserOpenUid"] = json!(buyer_uid);
                    detail["sellerOpenUid"] = json!(seller_uid);
                }
            }
            row
        })
        .collect();
    page(rows, p)
}

pub fn create(s: &mut Store, owner: &str, ids: &[i64], p: &Value) -> Result<Value> {
    let title =
        find(p, &["purchaserInvoiceTitleModel", "purchaserInvoiceTitle"]).ok_or_else(|| Error {
            code: "PARAM_INVALID".into(),
            message: "invoice title required".into(),
        })?;
    let title_type = text(title, &["titleType"]);
    if ![
        "COMPANY",
        "PERSONAL",
        "SOCIAL_ORGANIZATION",
        "PARENT_VIRTUAL",
        "0",
        "1",
    ]
    .contains(&title_type.as_str())
        || text(title, &["title"]).trim().is_empty()
    {
        return fail("PARAM_INVALID", "invalid invoice title type or name");
    }
    let kind = text(p, &["invoiceType"]);
    if !["VATAX_SPEC", "VATAX_COMM", "1", "2"].contains(&kind.as_str()) {
        return fail("PARAM_ILLEGAL", "invalid invoiceType");
    }
    if ["COMPANY", "1"].contains(&title_type.as_str()) {
        if text(title, &["taxpayerIdentify"]).is_empty() {
            return fail("PARAM_INVALID", "company taxpayerIdentify required");
        }
        if ["VATAX_SPEC", "2"].contains(&kind.as_str())
            && [
                "bankName",
                "bankAccountId",
                "registerAddress",
                "registerPhone",
            ]
            .iter()
            .any(|field| text(title, &[field]).is_empty())
        {
            return fail(
                "PARAM_INVALID",
                "company special invoice requires banking and registration details",
            );
        }
    }
    let mut total = Decimal::ZERO;
    let mut seller = String::new();
    let mut seen = std::collections::BTreeSet::new();
    for oid in ids {
        if !seen.insert(*oid) {
            return fail("PARAM_ILLEGAL", "duplicate orderId");
        }
        let order = buyer_order(s, *oid, owner)?;
        if !seller.is_empty() && seller != order.seller {
            return fail("MULTIPLE_SELLERS", "merge group must have the same seller");
        }
        seller = order.seller.clone();
        let invoice_type = if kind == "2" {
            "VATAX_SPEC"
        } else if kind == "1" {
            "VATAX_COMM"
        } else {
            &kind
        };
        if order.lines.iter().any(|l| {
            let kinds = order
                .metadata
                .paid_invoice_types
                .get(&l.offer_id)
                .unwrap_or(&s.products[&l.offer_id].merchandise.invoice_types);
            !kinds.iter().any(|v| v == invoice_type)
        }) {
            return fail(
                if invoice_type == "VATAX_SPEC" {
                    "INVOICE_NOT_SUPPORT_SPEC"
                } else {
                    "INVOICE_NOT_SUPPORT_COMMON"
                },
                "product does not support requested invoice type",
            );
        }
        if order.status == "cancel" {
            return fail("ORDER_CANCELED_ERROR", "cancelled order cannot be invoiced");
        }
        if order.status != "success" {
            return fail("ILLEGAL_ORDER_STATUS", "completed order required");
        }
        if s.rows("invoices", Some(owner)).iter().any(|r| {
            r["orderIds"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(oid)))
        }) {
            return fail("INVOICE_ALREADY_APPLIED", "invoice already applied");
        }
        total += (order.amount - order.refunded) * Decimal::from(100);
    }
    let amount = if find(p, &["amount"]).is_some() {
        decimal(p, &["amount"])?
    } else {
        total
    };
    if amount <= Decimal::ZERO || amount > total {
        return fail(
            "INVOICE_AMOUNT_EXCEEDED",
            "invoice amount exceeds available cents",
        );
    }
    let iid = s.next_id();
    let mut title = title.clone();
    title["titleType"] = json!(match title_type.as_str() {
        "0" => "PERSONAL",
        "1" => "COMPANY",
        _ => &title_type,
    });
    title["OpenUid"] = json!(crate::accounts_business::open_uid(&s.current_app, owner));
    let mut seller_title = s.rows("invoice_titles", Some(&seller)).into_iter().next().unwrap_or_else(||json!({"titleType":"COMPANY","title":s.accounts[&seller].login,"taxpayerIdentify":"MOCK-SELLER-TAX","OpenUid":crate::accounts_business::open_uid(&s.current_app, &seller)}));
    seller_title["OpenUid"] = json!(crate::accounts_business::open_uid(&s.current_app, &seller));
    let mut details = vec![];
    let mut allocated = Decimal::ZERO;
    let gross = ids
        .iter()
        .flat_map(|id| s.orders[id].lines.iter())
        .map(|l| l.quantity * l.price * Decimal::from(100))
        .sum::<Decimal>();
    let line_count: usize = ids.iter().map(|id| s.orders[id].lines.len()).sum();
    for oid in ids {
        for (i, line) in s.orders[oid].lines.iter().enumerate() {
            let line_amount = if details.len() + 1 == line_count {
                amount - allocated
            } else {
                (line.quantity * line.price * Decimal::from(100) / gross * amount).floor()
            };
            allocated += line_amount;
            details.push(json!({"subOutBizId":(oid*100+i as i64).to_string(),"cargoName":if line.title.is_empty(){&s.products[&line.offer_id].title}else{&line.title},"skuId":line.offer_id.to_string(),"specifications":line.spec_id,"specId":line.spec_id,"invoiceAmountWithTax":line_amount,"quantity":line.quantity,"unit":"件","currency":"CNY","taxRate":0,"purchaserOpenUid":crate::accounts_business::open_uid(&s.current_app, owner),"sellerOpenUid":crate::accounts_business::open_uid(&s.current_app, &seller),"relatedInvoiceId":iid}));
        }
    }
    let row = json!({"id":iid,"applyId":iid,"invoiceApplyId":iid,"orderIds":ids,"amount":amount,"invoiceAmount":amount,"status":"APPLIED","bizStatus":"ISSUING","invoiceType":if kind=="2"{"VATAX_SPEC"}else if kind=="1"{"VATAX_COMM"}else{&kind},"purchaserInvoiceTitle":title,"purchaserInvoiceTitleModel":title,"sellerInvoiceTitleModel":seller_title,"sellerMemberId":seller,"sellerOpenUid":crate::accounts_business::open_uid(&s.current_app, &seller),"purchaserOpenUid":crate::accounts_business::open_uid(&s.current_app, owner),"gmtCreate":s.now(),"gmtModified":s.now(),"invoiceDate":null,"invoiceDeadline":s.now()+30*86400000,"invoiceErrMsg":"","invoiceMaterial":"ELECTRON","isRedInvoice":false,"outBizId":iid.to_string(),"requestNo":ids.iter().map(i64::to_string).collect::<Vec<_>>().join(","),"sellerInputInvoiceTime":null,"source":"1688","invoiceDetailDTOList":details});
    s.put("invoices", &format!("{owner}:{iid}"), row.clone());
    Ok(row)
}
