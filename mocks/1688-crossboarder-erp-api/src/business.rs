use crate::state::*;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::str::FromStr;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    pub code: String,
    pub message: String,
}
pub fn fail<T>(code: &str, message: &str) -> Result<T> {
    Err(Error {
        code: code.into(),
        message: message.into(),
    })
}
pub fn find<'a>(p: &'a Value, names: &[&str]) -> Option<&'a Value> {
    for n in names {
        if let Some(v) = p.get(*n) {
            return Some(v);
        }
    }
    match p {
        Value::Object(m) => m.values().find_map(|v| find(v, names)),
        _ => None,
    }
}
pub fn text(p: &Value, names: &[&str]) -> String {
    find(p, names)
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default()
}
pub fn number(p: &Value, names: &[&str]) -> Option<i64> {
    find(p, names).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}
pub fn decimal(p: &Value, names: &[&str]) -> Result<Decimal> {
    Decimal::from_str(&text(p, names)).map_err(|_| Error {
        code: "PARAM_ILLEGAL".into(),
        message: format!("{} must be a decimal", names[0]),
    })
}
pub fn array<'a>(p: &'a Value, names: &[&str]) -> Option<&'a Vec<Value>> {
    find(p, names).and_then(Value::as_array)
}
pub fn id(p: &Value) -> Result<i64> {
    number(
        p,
        &["orderId", "tradeID", "mainOrderId", "tradeId", "sourceId"],
    )
    .ok_or_else(|| Error {
        code: "PARAM_ILLEGAL".into(),
        message: "orderId required".into(),
    })
}
pub fn owned_order<'a>(s: &'a Store, order_id: i64, owner: &str) -> Result<&'a Order> {
    let o = s
        .orders
        .get(&order_id)
        .filter(|o| !o.deleted)
        .ok_or_else(|| Error {
            code: "ORDER_NOT_EXISTS".into(),
            message: "order does not exist".into(),
        })?;
    if o.buyer != owner && o.seller != owner {
        return fail("ACCESS_DENIED", "order belongs to another account");
    }
    Ok(o)
}
pub fn buyer_order<'a>(s: &'a Store, order_id: i64, owner: &str) -> Result<&'a Order> {
    let o = owned_order(s, order_id, owner)?;
    if o.buyer != owner {
        return fail("ACCESS_DENIED", "buyer authorization required");
    }
    Ok(o)
}
pub fn page(items: Vec<Value>, p: &Value) -> Result<Value> {
    let size = number(p, &["pageSize", "quantity"]).unwrap_or(20);
    let index = number(
        p,
        &[
            "page",
            "pageNo",
            "pageNum",
            "pageIndex",
            "currentPage",
            "currentPageNum",
            "beginPage",
        ],
    )
    .unwrap_or(1);
    if !(1..=200).contains(&size) || index < 1 {
        return fail("PARAM_ILLEGAL", "invalid pagination");
    }
    let total = items.len();
    let start = number(p, &["startIndex"]).unwrap_or((index - 1).saturating_mul(size));
    if start < 0 {
        return fail("PARAM_ILLEGAL", "negative startIndex");
    }
    let rows = items
        .into_iter()
        .skip(start as usize)
        .take(size as usize)
        .collect::<Vec<_>>();
    Ok(
        json!({"items":rows,"total":total,"totalRecord":total,"totalRow":total,"page":index,"pageNo":index,"currentPage":index,"pageSize":size,"totalPage":(total as i64+size-1)/size,"hasMore":start.saturating_add(size)<(total as i64)}),
    )
}
pub fn product_value(p: &Product, base: &str, seller: &Account, app_key: &str) -> Value {
    let m = &p.merchandise;
    let has = |tag: &str| m.tags.iter().any(|v| v == tag);
    let image = format!("{base}/assets/product.svg");
    let range = json!([{"startQuantity":p.minimum,"price":p.price,"promotionPrice":p.price,"foreignCurrencyPrice":p.price,"foreignCurrencyPromotionPrice":p.price}]);
    let sku_attr = json!([{"attributeId":1,"attributeID":1,"attributeDisplayName":"规格","attributeValue":p.sku,"attrType":"enum","attributeName":"规格","attributeNameTrans":"Specification","value":p.sku,"valueTrans":p.sku,"skuImageUrl":image,"skuImageUrlTrans":image}]);
    let sku = json!({"skuId":p.id,"specId":p.sku,"cargoNumber":p.sku,"price":p.price,"jxhyPrice":p.price,"pfJxhyPrice":p.price,"pftPrice":p.price,"jxhyPfPrice":p.price,"consignPrice":p.price,"promotionPrice":p.price,"retailPrice":p.price,"foreignCurrencyPrice":p.price,"foreignCurrencyPromotionPrice":p.price,"foreignCurrencyRetailPrice":p.price,"fenxiaoPriceInfo":{"onePiecePrice":p.price,"offerPrice":p.price},"amountOnSale":p.stock,"skuAttributes":sku_attr,"attributes":sku_attr});
    let shipping = crate::freight::product_shipping(p);
    let seller_data = json!({"tradeMedalLevel":"5","compositeServiceScore":m.score,"logisticsExperienceScore":m.score,"disputeComplaintScore":m.dispute_score,"offerExperienceScore":m.score,"afterSalesExperienceScore":m.score,"consultingExperienceScore":m.score,"repeatPurchasePercent":m.old_buyer_ratio,"collect30DayWithin48HPercent":m.collection_rate48/Decimal::from(100),"qualityRefundWithin30Day":m.return_rate/Decimal::from(100),"tpYear":5});
    let price_info = json!({"price":p.price,"jxhyPrice":p.price,"pfJxhyPrice":p.price,"pftPrice":p.price,"jxhyPfPrice":p.price,"consignPrice":p.price,"promotionPrice":p.price});
    let tags: Vec<_> = m
        .tags
        .iter()
        .map(|key| json!({"key":key,"value":true}))
        .collect();
    let created = crate::dates::task_time(m.created);
    let mut value = json!({"id":p.id,"offerId":p.id,"productId":p.id,"productID":p.id,"subject":p.title,"subjectTrans":p.title,"title":p.title,"name":p.title,
        "categoryId":p.category,"categoryID":p.category,"categoryName":"日用百货","firstCateName":"家居用品","secondCateName":"日用百货","thirdCateName":"日用百货","topCategoryId":1,"secondCategoryId":p.category,"thirdCategoryId":p.category,
        "sellerId":seller.user_id,"supplierUserId":seller.user_id,"supplierLoginId":seller.login,"sellerOpenId":crate::accounts_business::open_uid(app_key, &p.seller),"memberId":p.seller,"companyName":p.seller,"status":if p.active{"published"}else{"expired"},
        "price":p.price,"salePrice":p.price,"priceInfo":price_info,"amountOnSale":p.stock,"stock":p.stock,"minOrderQuantity":p.minimum,"minPurchaseQuantity":p.minimum,"quantityBegin":p.minimum,"batchNumber":1,
        "productImage":{"images":[image],"whiteImage":image},"productImageTrans":{"images":[image],"whiteImage":image},"images":[image],"detailImages":[image],"imageUrl":image,"picUrl":image,"mainImageUrl":image,"aigcImageUrl":image,"whiteImage":image,"offerUrl":format!("{base}/product/{}",p.id),"detailUrl":format!("{base}/product/{}",p.id),"longClickUrl":format!("{base}/promotion/{}",p.id),"mainVideo":"","detailVideo":"",
        "productAttribute":[{"attributeId":"1","attributeID":1,"valueID":1,"attributeName":"规格","attributeNameTrans":"Specification","value":p.sku,"valueTrans":p.sku}],"productSkuInfos":[sku],"skuInfos":[sku],
        "saleInfo":{"priceRanges":range,"amountOnSale":p.stock,"minOrderQuantity":p.minimum,"beginAmount":p.minimum,"supportOnlineTrade":true,"consignPrice":p.price,"jxhyPrice":p.price,"retailPrice":p.price,"retailprice":p.price,"pftPrice":p.price,"jxhyPfPrice":p.price,"retailBeginAmount":p.minimum,"price":p.price},
        "productSaleInfo":{"amountOnSale":p.stock,"priceRangeList":range,"quoteType":1,"unitInfo":{"unit":"件","transUnit":"piece"},"fenxiaoSaleInfo":{"onePieceFreePostage":has("isOnePsaleFreePostage"),"startQuantity":p.minimum,"onePiecePrice":p.price,"offerPrice":p.price},"retailPrice":p.price,"consignPrice":p.price,"jxhyPrice":p.price,"foreignCurrencyPrice":(p.price*Decimal::from(100)).round(),"foreignCurrencyRetailPrice":p.price}
    });
    let details = json!({"productSimpleShippingInfo":{"shippingTimeGuarantee":m.shipping,"shippingOrigin":"浙江省杭州市","shippingOriginCode":"330100","perfectFulfillmentRate30d":"1","pickupWithin24hRate30d":m.collection_rate24/Decimal::from(100),"qualityReturnRate30d":m.return_rate/Decimal::from(100),"perfectFulfillmentRate7d":"1","pickupWithin24hRate7d":m.collection_rate24/Decimal::from(100),"delayedShippingRate7d":"0"},
        "productShippingInfo":shipping,
        "tagInfoList":tags,"description":p.title,"descriptionTrans":p.title,"language":"en","saleLimitAddress":{"status":0},"rangePrice":{"price":[p.price*Decimal::from(100)],"beginQuantity":[p.minimum]},
        "bookedCount":m.sales.get("totalSales").copied().unwrap_or(0),"saleCount":m.sales.get("sales30").copied().unwrap_or(0),"saleQuantity":m.sales.get("sales30").copied().unwrap_or(0),"monthSold":m.sales.get("sales30").copied().unwrap_or(0),"soldOut":m.sales.get("totalSales").copied().unwrap_or(0),"sales7d":m.sales.get("sales7").copied().unwrap_or(0),"ratio":m.ratio,"oldBuyerRatio":m.old_buyer_ratio,"tkCnt":m.promotion_count,"tkCommission":p.price*m.ratio*Decimal::from(m.promotion_count),"yhqDiscountFee":0,
        "repurchaseRate":m.old_buyer_ratio,"isOnePsale":has("isOnePsale"),"isJxhy":has("1688_yx"),"isSelect":has("select"),"sellerIdentities":if m.certified{vec!["certifiedFactory"]}else{vec![]},"offerIdentities":m.tags,"tradeScore":m.score,"sellerDataInfo":seller_data,"sellerMixSetting":{"generalHunpi":has("isSupportMix"),"mixAmount":m.mix_min_amount.unwrap_or(Decimal::ZERO),"mixNumber":m.mix_min_quantity.unwrap_or(Decimal::ONE)},"productCargoNumber":p.sku,"promotionModel":{"hasPromotion":false,"promotionType":""},"channelPrice":{"channelSkuPriceList":[{"skuId":p.id,"currentPrice":p.price}]},"sellingPoint":[p.title],"certificateList":[],"isPatentProduct":false,"createDate":created,"modifyDate":crate::dates::task_time(m.modified.unwrap_or(m.created)),"createTime":m.created,"modifyTime":m.modified.unwrap_or(m.created),"lastUpdateTime":m.modified.unwrap_or(m.created),"sellStartTime":m.sale_start.unwrap_or(m.created)
    });
    let commerce = json!({"itemId":p.id,"imgUrl":image,"salesCnt90d":m.sales.get("sales90").copied().unwrap_or(0),"minPrice":p.price*Decimal::from(100),"maxPrice":p.price*Decimal::from(100),"serviceList":m.tags.iter().map(|t|json!({"code":t,"name":t})).collect::<Vec<_>>(),"token":format!("mock-offer-{}",p.id),"traceInfo":format!("mock-offer-{}",p.id),"promotionURL":format!("{base}/promotion/{}",p.id),"promotionUrl":format!("{base}/promotion/{}",p.id),
        "ww30DResponseRate":m.response_rate,"newTowerDisputeScore":m.dispute_score,"order30DTurnoverRate":m.turnover_rate,"productTradeInfo":{"addCartCount7d":0,"payBuyerCount7d":m.buyer_count,"addCartCount30d":0,"payBuyerCount30d":m.buyer_count},
        "quantityPrice":[{"quantity":format!("≥{}",p.minimum),"value":p.price,"valueString":p.price.to_string(),"integer":p.price.trunc(),"decimals":p.price.fract()*Decimal::from(100)}],"priceUnderLine":p.price,"priceType":"NORMAL","consignPrice":p.price,"fenxiaoPrice":{"price":p.price,"pfPrice":p.price,"freepostage":has("isOnePsaleFreePostage")},"invoiceInfo":{"supportOnlineInvoice":!m.invoice_types.is_empty(),"supportFastInvoice":false,"invoiceTypes":m.invoice_types,"taxpayerType":"general"}});
    value
        .as_object_mut()
        .unwrap()
        .extend(details.as_object().unwrap().clone());
    value
        .as_object_mut()
        .unwrap()
        .extend(commerce.as_object().unwrap().clone());
    value["attributes"] = value["productAttribute"].clone();
    value["shippingInfo"] = value["productShippingInfo"].clone();
    value
}

pub fn order_trade_type(o: &Order) -> &str {
    match o.metadata.trade_type.as_str() {
        "alipay" => "1",
        "period" => "10",
        "creditBuy" => "9",
        "631staged" | "37staged" => "7",
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "50060" | "50070"
        | "50080" => &o.metadata.trade_type,
        _ => "50060",
    }
}
pub fn line_payment(o: &Order, index: usize) -> Decimal {
    let gross: Decimal = o.lines.iter().map(|l| l.price * l.quantity).sum();
    if gross == Decimal::ZERO {
        return Decimal::ZERO;
    }
    let shares: Vec<_> = o
        .lines
        .iter()
        .take(o.lines.len().saturating_sub(1))
        .map(|l| {
            ((l.price * l.quantity / gross) * o.amount)
                .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero)
        })
        .collect();
    shares
        .get(index)
        .copied()
        .unwrap_or_else(|| o.amount - shares.iter().copied().sum::<Decimal>())
}
pub fn order_value(o: &Order, s: &Store) -> Value {
    let buyer = &s.accounts[&o.buyer];
    let seller = &s.accounts[&o.seller];
    let modified = o.metadata.modified.unwrap_or(o.created);
    let dt = |v: i64| {
        chrono::DateTime::from_timestamp_millis(v)
            .unwrap()
            .format("%Y%m%d%H%M%S%3f%z")
            .to_string()
    };
    let last_refund = s
        .refunds
        .values()
        .filter(|r| r.order_id == o.id)
        .next_back();
    let refund_status = last_refund
        .map(|r| match r.status.as_str() {
            "success" => "refundsuccess",
            "cancelled" | "rejected" => "refundclose",
            other => other,
        })
        .unwrap_or("");
    let product_total = o
        .lines
        .iter()
        .map(|l| l.price * l.quantity)
        .sum::<Decimal>();
    let trade_type = order_trade_type(o);
    let buyer_contact = json!({"name":buyer.login,"phone":"057100000000","mobile":"13800000000","email":format!("{}@mock.invalid",buyer.login),"companyName":"模拟采购公司","imInPlatform":buyer.login});
    let seller_contact = json!({"name":seller.login,"phone":"057100000001","mobile":"13800000001","email":format!("{}@mock.invalid",seller.login),"companyName":"模拟供应商","imInPlatform":seller.login});
    let receiver = json!({"toFullName":o.address["fullName"],"toArea":format!("{}{}{}",text(&o.address,&["provinceText"]),text(&o.address,&["cityText"]),text(&o.address,&["areaText"])),"toMobile":o.address["mobile"],"toPhone":text(&o.address,&["phone"]),"toPost":text(&o.address,&["postCode"]),"toDivisionCode":text(&o.address,&["areaCode"]),"toTownCode":text(&o.address,&["townCode"]),"toAddress":o.address["address"]});
    let channels: Vec<_> = [
        ("alipay", "支付宝"),
        ("bank", "银行转账"),
        ("crossBorderPay", "跨境宝"),
    ]
    .into_iter()
    .filter(|(code, _)| channel_available(o, code))
    .collect();
    let mut base = json!({"id":o.id,"idOfStr":o.id.to_string(),"orderId":o.id,"status":o.status,"buyerID":o.buyer,"buyerMemberId":o.buyer,"buyerUserId":buyer.user_id,"buyerLoginId":buyer.login,"buyerOpenUid":crate::accounts_business::open_uid(&s.current_app, &o.buyer),"sellerOpenUid":crate::accounts_business::open_uid(&s.current_app, &o.seller),"sellerID":o.seller,"sellerMemberId":o.seller,"sellerUserId":seller.user_id,"sellerLoginId":seller.login,"totalAmount":o.amount,"sumProductPayment":product_total,"totalPayment":o.amount,"refund":o.refunded,"refundPayment":o.refunded*Decimal::from(100),"discount":(product_total-o.amount)*Decimal::from(100),"createTime":dt(o.created),"modifyTime":dt(modified),"payTime":o.metadata.paid.map(dt),"allDeliveredTime":o.metadata.shipped.map(dt),"receivingTime":o.metadata.completed.map(dt),"completeTime":o.metadata.completed.map(dt),"confirmedTime":o.metadata.completed.map(dt),"remark":o.memo,"buyerMemo":o.memo,"buyerFeedback":o.feedback.join("\n"),"shippingFee":0,"couponFee":0,"flowTemplateCode":if o.metadata.flow.is_empty(){"general"}else{&o.metadata.flow},"tradeType":trade_type,"tradeTypeCode":if o.metadata.trade_type.is_empty(){"assureTrade"}else{&o.metadata.trade_type},"businessType":if o.metadata.business_type.is_empty(){"cn"}else{&o.metadata.business_type},"outOrderId":o.external_id,"buyerContact":buyer_contact,"sellerContact":seller_contact,"receiverInfo":receiver,"refundStatus":refund_status,"refundId":last_refund.map(|r|r.id.to_string()).unwrap_or_default(),"closeReason":o.metadata.close_reason,"closeOperateType":o.metadata.close_operator,"sellerOrder":o.metadata.seller_order,"overSeaOrder":false,"stepPayAll":o.metadata.trade_type != "631staged" && o.metadata.trade_type != "37staged","payTimeout":86400,"payTimeoutType":0,"payChannelList":channels.iter().map(|(_,name)|*name).collect::<Vec<_>>(),"payChannelCodeList":channels.iter().map(|(code,_)|*code).collect::<Vec<_>>()});
    let steps = crate::payments::steps(s, o);
    base["newStepOrderList"] = json!(steps);
    base["stepOrderList"] = json!(steps);
    let entries:Vec<Value>=o.lines.iter().enumerate().map(|(i,l)|{
        let product=&s.products[&l.offer_id];let entry=o.id*100+i as i64;
        let refunded = crate::refunds::settled(s,o.id,entry).0;
        let entry_refund = s.refunds.values().filter(|r| r.order_id==o.id && (r.entries.is_empty() || r.entries.contains_key(&entry))).next_back();
        let entry_refund_status = entry_refund.map(|r|match r.status.as_str(){"success"=>"refundsuccess","cancelled"|"rejected"=>"refundclose",other=>other}).unwrap_or("");
        let received = o.metadata.received.get(&entry).copied().or(o.metadata.completed);
        let status = if refunded==line_payment(o,i) {"cancel"} else if received.is_some(){"success"}else{&o.status};
        json!({"id":entry,"subItemID":entry,"subItemIDString":entry.to_string(),"orderEntryId":entry,"productID":l.offer_id,"productId":l.offer_id,"specId":l.spec_id,"skuID":l.offer_id,"name":if l.title.is_empty(){&product.title}else{&l.title},"productImgUrl":[format!("{}/assets/product.svg",s.base_url)],"productSnapshotUrl":format!("{}/snapshot/{}/{}",s.base_url,o.id,entry),"quantity":l.quantity,"price":l.price,"itemAmount":line_payment(o,i),"entryDiscount":(line_payment(o,i)-l.price*l.quantity)*Decimal::from(100),"actualPayFee":line_payment(o,i),"status":status,"statusStr":status,"refund":refunded,"refundStatus":entry_refund_status,"refundId":entry_refund.map(|r|r.id.to_string()).unwrap_or_default(),"unit":"件","quantityFactor":1000,"weight":"0.2","weightUnit":"kg","sort":i,"gmtCreate":dt(o.created),"gmtModified":dt(modified),"gmtCompleted":received.map(dt),"logisticsStatus":match status{"success"=>3,"waitbuyerreceive"=>2,_=>1},"closeReason":o.metadata.close_reason,"sharePostage":0,"skuInfos":[{"name":"规格","value":l.spec_id}]})
    }).collect();
    json!({"id":o.id,"orderId":o.id,"orderIds":[o.id],"totalSuccessAmount":(o.amount*Decimal::from(100)).to_string(),"postFee":0,"failedOfferList":[],"accountPeriod":{"tapType":0,"tapDate":0,"tapOverdue":0},"orderList":[{"orderId":o.id.to_string(),"orderAmmount":(o.amount*Decimal::from(100)).to_string(),"postFee":0,"success":true,"resultCode":"200","payChannel":"alipay"}],"status":o.status,"totalAmount":o.amount,"baseInfo":base,"productItems":entries,"orderEntries":entries,"tradeTerms":[],"orderInvoiceInfo":crate::invoices::order_info(s,o),"invoicingSettingModel":crate::invoices::setting(s,o),"orderRateInfo":{"buyerRateStatus":o.metadata.buyer_rate.unwrap_or(5),"sellerRateStatus":o.metadata.seller_rate.unwrap_or(5)},"receiveAddress":o.address,"buyerMemo":o.memo,"orderMemo":o.memo,"extAttributes":{"outOrderId":o.external_id}})
}

pub fn channel_available(o: &Order, channel: &str) -> bool {
    ["alipay", "bank", "crossBorderPay"].contains(&channel)
        && o.metadata
            .pay_channels
            .as_ref()
            .is_none_or(|channels| channels.iter().any(|c| c == channel))
}

pub fn period_available(s: &Store, owner: &str, seller: &str) -> Decimal {
    let Some(row) = s
        .records
        .get("account_periods")
        .and_then(|r| r.get(&format!("{owner}:{seller}")))
    else {
        return Decimal::ZERO;
    };
    if row["status"] != "NORMAL" {
        return Decimal::ZERO;
    }
    let used: Decimal = s
        .orders
        .values()
        .filter(|o| {
            o.buyer == owner
                && o.seller == seller
                && o.metadata.trade_type == "period"
                && (o.metadata.paid.is_some()
                    || ["waitsellersend", "waitbuyerreceive", "success"]
                        .contains(&o.status.as_str()))
        })
        .map(|o| o.amount - o.refunded)
        .sum();
    (decimal(row, &["quota"]).unwrap_or_default() - used).max(Decimal::ZERO)
}

pub fn payment_stock(s: &mut Store, o: &Order, consume: bool) -> Result<()> {
    if o.metadata.trade_type == "period" && period_available(s, &o.buyer, &o.seller) < o.amount {
        return fail(
            "ACCOUNT_PERIOD_QUOTA_NOT_ENOUGH",
            "insufficient account period quota",
        );
    }
    if !o.metadata.stock_at_payment {
        return Ok(());
    }
    let mut quantities = std::collections::BTreeMap::new();
    for line in &o.lines {
        *quantities.entry(line.offer_id).or_insert(Decimal::ZERO) += line.quantity;
    }
    if quantities.iter().any(|(id, q)| s.products[id].stock < *q) {
        return fail(
            "操作库存失败:PRODUCT_TRADE_STAT_ERROR",
            "payment inventory deduction failed",
        );
    }
    if consume {
        for (id, q) in quantities {
            s.products.get_mut(&id).unwrap().stock -= q;
        }
    }
    Ok(())
}

pub fn pay(s: &mut Store, order_id: i64, owner: &str, success: bool) -> Result<Value> {
    let o = buyer_order(s, order_id, owner)?.clone();
    if o.status != "waitbuyerpay" {
        return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting payment");
    }
    if !success {
        s.emit("ORDER_BATCH_PAY",owner,json!({"batchPay":[{"orderId":order_id.to_string(),"status":"ACCOUNT_BALANCE_NOT_ENOUGH"}]}));
        return Ok(json!({"orderId":order_id,"paid":false}));
    }
    payment_stock(s, &o, true)?;
    let a = s.accounts.get_mut(owner).unwrap();
    if a.frozen {
        return fail("ACCOUNT_FROZEN", "account is frozen");
    }
    let staged = s
        .records
        .get("staged_payments")
        .and_then(|m| m.get(&format!("{owner}:{order_id}")))
        .map(|r| decimal(r, &["amount"]))
        .transpose()?
        .unwrap_or(Decimal::ZERO);
    let payable = o.amount - staged;
    if a.balance < payable {
        return fail("ACCOUNT_BALANCE_NOT_ENOUGH", "insufficient balance");
    }
    a.balance -= payable;
    s.orders.get_mut(&order_id).unwrap().status = "waitsellersend".into();
    s.emit(
        "ORDER_BATCH_PAY",
        owner,
        json!({"batchPay":[{"orderId":order_id.to_string(),"status":"successed"}]}),
    );
    s.emit(
        "ORDER_BUYER_VIEW_ORDER_PAY",
        owner,
        json!({"orderId":order_id.to_string()}),
    );
    Ok(order_value(&s.orders[&order_id], s))
}
pub fn ship(s: &mut Store, order_id: i64, seller: &str, tracking: String) -> Result<Value> {
    crate::logistics::send(s, order_id, seller, tracking, None, None)
}

pub fn commerce(s: &mut Store, name: &str, p: &Value, owner: &str) -> Option<Result<Value>> {
    let result = (|| match name {
        "alibaba.createOrder.preview"
        | "alibaba.trade.createCrossOrder"
        | "alibaba.trade.fastCreateOrder" => {
            let preview = name == "alibaba.createOrder.preview";
            let external = text(p, &["outOrderId"]);
            if name == "alibaba.trade.fastCreateOrder"
                && text(p, &["message"]).chars().count() >= 500
            {
                return fail(
                    "FAIL_BIZ_MESSAGE_LENGTH_IS_ILLEGA",
                    "message must be shorter than 500 characters",
                );
            }
            if name == "alibaba.trade.fastCreateOrder" && !external.is_empty() {
                if let Some(saved) = s
                    .records
                    .get("fast_order_requests")
                    .and_then(|r| r.get(&format!("{owner}:{external}")))
                {
                    return Ok(saved.clone());
                }
            }
            let flow = text(p, &["flow"]);
            if !flow.is_empty()
                && ![
                    "general",
                    "fenxiao",
                    "saleproxy",
                    "paired",
                    "repurchase",
                    "boutiquefenxiao",
                    "boutiquepifa",
                ]
                .contains(&flow.as_str())
            {
                return fail("PARAM_ILLEGAL", "unsupported flow");
            }
            let trade_type = text(p, &["tradeType"]);
            if !trade_type.is_empty()
                && ![
                    "assureTrade",
                    "alipay",
                    "period",
                    "assure",
                    "creditBuy",
                    "bank",
                    "631staged",
                    "37staged",
                ]
                .contains(&trade_type.as_str())
            {
                return fail("not support tradeType:【XXXX】", "unsupported tradeType");
            }
            let cargo = array(p, &["cargoParamList"])
                .filter(|a| !a.is_empty())
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "cargoParamList must not be empty".into(),
                })?;
            if cargo.len() > 50 {
                return fail("PARAM_ILLEGAL", "at most 50 SKUs per order");
            }
            let mut address = p.get("addressParam").cloned().unwrap_or(json!({}));
            if let Some(address_id) = number(&address, &["addressId"]) {
                address = s
                    .records
                    .get("addresses")
                    .and_then(|rows| rows.get(&format!("{owner}:{address_id}")))
                    .cloned()
                    .ok_or_else(|| Error {
                        code: "INVALID_ADDRESS".into(),
                        message: "address not owned by buyer".into(),
                    })?;
            }
            if text(&address, &["fullName"]).is_empty() || text(&address, &["address"]).is_empty() {
                return fail("PARAM_ILLEGAL", "recipient and address required");
            }
            if name == "alibaba.trade.fastCreateOrder" {
                if !(2..=25).contains(&text(&address, &["fullName"]).chars().count()) {
                    return fail("PARAM_ILLEGAL", "fullName must be 2..25 characters");
                }
                if text(&address, &["phone"]).is_empty() && text(&address, &["mobile"]).is_empty() {
                    return fail(
                        "FAIL_BIZ_RECEIVE_ADDRESS_MOBILE_PHONE_NULL",
                        "phone or mobile required",
                    );
                }
                for field in ["phone", "mobile"] {
                    if text(&address, &[field]).chars().count() > 30 {
                        return fail("PARAM_ILLEGAL", "phone/mobile cannot exceed 30 characters");
                    }
                }
            }
            if p.get("useOfficialSolution") == Some(&json!(true))
                && array(p, &["useOfficialSolutionModelList"]).is_none_or(Vec::is_empty)
            {
                return fail("PARAM_ILLEGAL", "useOfficialSolutionModelList is required");
            }
            let mut lines = vec![];
            let mut quantities = std::collections::BTreeMap::new();
            let mut total = Decimal::ZERO;
            let mut seller = String::new();
            for c in cargo {
                let offer_id = number(c, &["offerId"]).ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "offerId required".into(),
                })?;
                let product = s.products.get(&offer_id).ok_or_else(|| Error {
                    code: "PRODUCT_NOT_EXISTS".into(),
                    message: "product not found".into(),
                })?;
                if !product.active {
                    return fail("PRODUCT_OFF_SHELF", "product is not available");
                }
                let m = &product.merchandise;
                if m.sale_start.is_some_and(|t| s.now() < t) {
                    return fail(
                        "FAIL_BIZ_FAIL_BIZ_OFFER_PREHEATING",
                        "product sale has not started",
                    );
                }
                if !m.private_buyers.is_empty() && !m.private_buyers.iter().any(|a| a == owner) {
                    return fail(
                        "FAIL_BIZ_FAIL_BIZ_CARGO_CANNOT_AUTH",
                        "private product is not authorized",
                    );
                }
                if !trade_type.is_empty()
                    && !m.trade_types.is_empty()
                    && !m.trade_types.contains(&trade_type)
                {
                    return fail(
                        "not support tradeType:【XXXX】",
                        "product does not support tradeType",
                    );
                }
                let q = decimal(c, &["quantity"])?;
                if m.max_quantity.is_some_and(|max| q > max) {
                    return fail(
                        "FAIL_BIZ_FAIL_BIZ_BOOKED_BEYOND_THE_MAX_QUANTITY",
                        "quantity exceeds product maximum",
                    );
                }
                if m.quantity_scale
                    .is_some_and(|scale| q.normalize().scale() > scale)
                {
                    return fail(
                        "FAIL_BIZ_QUANTITY_UNMATCH_SELLUNIT_SCALE",
                        "quantity precision exceeds sale unit",
                    );
                }
                if q <= Decimal::ZERO || q > Decimal::from(1000000000_i64) {
                    return fail(
                        "FAIL_BIZ_FAIL_BIZ_QUANTITY_OUT_OF_SCOPE",
                        "quantity must be positive and at most one billion",
                    );
                }
                if product.price == Decimal::ZERO {
                    return fail("PRICE_ZERO", "price must be positive");
                }
                if s.accounts[&product.seller].frozen {
                    return fail(
                        "FAIL_BIZ_FAIL_BIZ_ILLEGAL_SELLER_NOT_ENABLED",
                        "seller is disabled",
                    );
                }
                if name == "alibaba.trade.fastCreateOrder" && quantities.contains_key(&offer_id) {
                    return fail("PARAM_ILLEGAL", "duplicate cargo; combine quantities");
                }
                if preview && text(c, &["specId"]).is_empty() {
                    return fail("500_002", "specId is required");
                }
                if text(c, &["specId"]) != product.sku {
                    return fail("SKU_NOT_EXISTS", "unknown specId");
                }
                if !seller.is_empty() && seller != product.seller {
                    return fail("MULTIPLE_SELLERS", "split orders by seller");
                }
                seller = product.seller.clone();
                *quantities.entry(offer_id).or_insert(Decimal::ZERO) += q;
                total += product.price * q;
                lines.push(Line {
                    title: product.title.clone(),
                    offer_id,
                    spec_id: product.sku.clone(),
                    quantity: q,
                    price: product.price,
                });
            }
            if total > Decimal::from(1000000000000_i64) {
                return fail("PARAM_ILLEGAL", "order total exceeds supported amount");
            }
            if trade_type == "period" && period_available(s, owner, &seller) < total {
                return fail("PARAM_ILLEGAL", "insufficient account period quota");
            }
            let combined_quantity: Decimal = quantities.values().copied().sum();
            for (id, q) in &quantities {
                let product = &s.products[id];
                let m = &product.merchandise;
                if m.wholesale_minimum.is_some_and(|n| *q < n) {
                    return fail("WHOLESALE_MINIMUM", "quantity below wholesale minimum");
                }
                let mix = m.tags.iter().any(|t| t == "isSupportMix");
                let has_rule = m.mix_min_quantity.is_some() || m.mix_min_amount.is_some();
                if mix && has_rule {
                    if m.mix_min_quantity.is_none_or(|n| combined_quantity < n)
                        && m.mix_min_amount.is_none_or(|n| total < n)
                    {
                        return fail(
                            "MIX_MINIMUM",
                            "mixed quantity and amount below supplier minimum",
                        );
                    }
                } else if *q < product.minimum {
                    if !mix && quantities.len() > 1 {
                        return fail(
                            "MIX_UNSUPPORTED",
                            "seller does not permit below-minimum mixed purchase",
                        );
                    }
                    return fail("MIN_ORDER_QUANTITY", "quantity below minimum");
                }
            }
            // shortcut: official fractional-quantity rounding is unpublished; round each order half up to cents.
            total = total
                .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
            for (id, q) in &quantities {
                if s.products[id].stock < *q {
                    return fail("STOCK_NOT_ENOUGH", "insufficient inventory");
                }
            }
            if flow == "saleproxy"
                && s.records
                    .get("supplier_relations")
                    .and_then(|r| r.get(&format!("{owner}:{seller}")))
                    .is_none_or(|r| r["consignStatus"] != "cooperating")
            {
                return fail(
                    "RELATION_NOT_EXISTS",
                    "active supplier relationship required",
                );
            }
            let modes: Vec<_> = [
                "assureTrade",
                "alipay",
                "period",
                "assure",
                "creditBuy",
                "bank",
                "631staged",
                "37staged",
            ]
            .into_iter()
            .filter(|mode| {
                lines.iter().all(|l| {
                    let supported = &s.products[&l.offer_id].merchandise.trade_types;
                    supported.is_empty() || supported.iter().any(|v| v == mode)
                })
            })
            .collect();
            if modes.is_empty()
                || (!preview && trade_type.is_empty() && !modes.contains(&"assureTrade"))
            {
                return fail(
                    "not support tradeType:【XXXX】",
                    "no supported default trade mode",
                );
            }
            if preview {
                let cents = total * Decimal::from(100);
                let cargos: Vec<_> = lines.iter().map(|l| json!({"offerId":l.offer_id,"openOfferId":l.offer_id.to_string(),"specId":l.spec_id,"skuId":l.offer_id,"amount":l.quantity*l.price,"finalUnitPrice":l.price,"resultCode":"200","message":"","cargoPromotionList":[],"extPairList":[],"totalFundUsageAmount":0,"totalPostFundUsageAmount":0,"tradeServiceList":[]})).collect();
                let trades: Vec<_> = modes
                    .iter()
                    .map(|m| json!({"name":m,"description":m,"tradeType":m,"opSupport":true}))
                    .collect();
                let item = json!({"status":true,"flowFlag":if flow.is_empty(){"general"}else{&flow},"resultCode":"200","message":"","taoSampleSinglePromotion":true,"sumPayment":cents,"sumProductPayment":cents,"sumPaymentNoCarriage":cents,"sumCarriage":0,"additionalFee":0,"tradeModeNameList":modes,"tradeModelList":trades,"cargoList":cargos,"sellerMemberId":seller,"discountFee":0,"shopPromotionList":[],"payChannelInfos":[{"name":"alipay","amountLimit":cents}],"tradeServiceList":[],"extPairList":[],"orderGroup":seller,"canUseOfficialSolution":true,"officialSolutionModelList":[{"solutionCode":"MOCK","name":"模拟物流","totalCost":0}],"totalFundUsageAmount":0,"totalPostFundUsageAmount":0});
                return Ok(
                    json!({"items":[item],"totalAmount":total,"multiOrderPriceInfo":{"sumGoodsAmount":cents,"sumLogisticFee":0,"sumShopPromotionAmount":0,"sumShopRebateAmount":0,"sumCrossShopPromotionAmount":0,"sumCrossShopAllowanceAmount":0,"sumFundUsageAmount":0,"sumPaymentAmount":cents,"sumDiscountAmount":0,"sumCrossShopYuanBaoDiscountAmount":0}}),
                );
            }
            let order_id = s.next_id();
            let order = Order {
                metadata: OrderMetadata {
                    cps: lines
                        .iter()
                        .filter_map(|line| {
                            s.records
                                .get("promotion_links")
                                .and_then(|m| m.get(&format!("{owner}:{}", line.offer_id)))
                                .map(|link| (line.offer_id, link.clone()))
                        })
                        .collect(),
                    flow: flow.clone(),
                    trade_type: if trade_type.is_empty() {
                        "assureTrade".into()
                    } else {
                        trade_type
                    },
                    business_type: match flow.as_str() {
                        "fenxiao" => "distribution",
                        "saleproxy" => "sp",
                        "repurchase" => "ag",
                        "paired" => "hp",
                        _ => "cn",
                    }
                    .into(),
                    official_logistics: p["useOfficialSolution"] == true,
                    ..Default::default()
                },
                id: order_id,
                buyer: owner.into(),
                seller,
                status: "waitbuyerpay".into(),
                lines,
                amount: total,
                refunded: Decimal::ZERO,
                address,
                memo: text(p, &["message"]),
                feedback: vec![],
                external_id: text(p, &["outOrderId"]),
                created: s.now(),
                tracking: None,
                deleted: false,
            };
            for (id, q) in quantities {
                s.products.get_mut(&id).unwrap().stock -= q;
            }
            let data = order_value(&order, s);
            s.orders.insert(order_id, order);
            if name == "alibaba.trade.fastCreateOrder" && !external.is_empty() {
                s.put(
                    "fast_order_requests",
                    &format!("{owner}:{external}"),
                    data.clone(),
                );
            }
            s.emit(
                "ORDER_BUYER_VIEW_BUYER_MAKE",
                owner,
                json!({"orderId":order_id.to_string()}),
            );
            Ok(data)
        }
        "alibaba.trade.get.buyerView" => Ok(order_value(buyer_order(s, id(p)?, owner)?, s)),
        "alibaba.trade.getBuyerOrderList" | "alibaba.trade.getSellerOrderList" => {
            let seller = name == "alibaba.trade.getSellerOrderList";
            let status = text(p, &["orderStatus"]);
            let external = text(p, &["outOrderId"]);
            let start = date_millis(p, "createStartTime")?;
            let end = date_millis(p, "createEndTime")?;
            if start.zip(end).is_some_and(|(a, b)| a > b) {
                return fail("PARAM_ILLEGAL", "start time exceeds end time");
            }
            let modified_start = date_millis(p, "modifyStartTime")?;
            let modified_end = date_millis(p, "modifyEndTime")?;
            if modified_start.zip(modified_end).is_some_and(|(a, b)| a > b) {
                return fail("PARAM_ILLEGAL", "modifyStartTime exceeds modifyEndTime");
            }
            if seller && number(p, &["pageSize"]).is_some_and(|n| n > 20) {
                return fail("PARAM_ILLEGAL", "seller pageSize maximum is 20");
            }
            let trade_type = text(p, &["tradeType"]);
            if !trade_type.is_empty()
                && ![
                    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "50060", "50070", "50080",
                ]
                .contains(&trade_type.as_str())
            {
                return fail("PARAM_ILLEGAL", "invalid tradeType");
            }
            for field in ["buyerRateStatus", "sellerRateStatus"] {
                if number(p, &[field]).is_some_and(|n| ![4, 5, 6].contains(&n)) {
                    return fail("PARAM_ILLEGAL", "invalid rate status");
                }
            }
            let biz_types = array(p, &["bizTypes"]);
            let ids = array(p, &["orderIds"]);
            let member = text(
                p,
                &[if seller {
                    "buyerMemberId"
                } else {
                    "sellerMemberId"
                }],
            );
            let login = text(
                p,
                &[if seller {
                    "buyerLoginId"
                } else {
                    "sellerLoginId"
                }],
            );
            let product_name = text(p, &["productName"]);
            let refund_status = text(p, &["refundStatus"]);
            page(
                s.orders
                    .values()
                    .filter(|o| {
                        !o.deleted
                            && (if seller {
                                o.seller == owner
                            } else {
                                o.buyer == owner
                            })
                            && o.metadata.historical == (p["isHis"] == true)
                            && (status.is_empty() || o.status == status)
                            && modified_start
                                .is_none_or(|t| o.metadata.modified.unwrap_or(o.created) >= t)
                            && modified_end
                                .is_none_or(|t| o.metadata.modified.unwrap_or(o.created) <= t)
                            && (trade_type.is_empty() || order_trade_type(o) == trade_type)
                            && biz_types.is_none_or(|kinds| {
                                kinds.is_empty()
                                    || kinds.iter().any(|k| {
                                        k.as_str()
                                            == Some(if o.metadata.business_type.is_empty() {
                                                "cn"
                                            } else {
                                                &o.metadata.business_type
                                            })
                                    })
                            })
                            && number(p, &["buyerRateStatus"])
                                .is_none_or(|r| r == i64::from(o.metadata.buyer_rate.unwrap_or(5)))
                            && number(p, &["sellerRateStatus"])
                                .is_none_or(|r| r == i64::from(o.metadata.seller_rate.unwrap_or(5)))
                            && (p["tousuStatus"] != true || o.metadata.complaint)
                            && (p["needOfficialLogisticOrder"] != true
                                || o.metadata.official_logistics)
                            && (external.is_empty() || o.external_id == external)
                            && start.is_none_or(|time| o.created >= time)
                            && end.is_none_or(|time| o.created <= time)
                            && ids.is_none_or(|ids| ids.iter().any(|id| id.as_i64() == Some(o.id)))
                            && (member.is_empty()
                                || &member == if seller { &o.buyer } else { &o.seller })
                            && (login.is_empty()
                                || s.accounts
                                    .get(if seller { &o.buyer } else { &o.seller })
                                    .is_some_and(|a| a.login == login))
                            && (product_name.is_empty()
                                || o.lines.iter().any(|l| {
                                    s.products
                                        .get(&l.offer_id)
                                        .is_some_and(|v| v.title.contains(&product_name))
                                }))
                            && (refund_status.is_empty()
                                || s.refunds.values().any(|r| {
                                    r.order_id == o.id
                                        && match r.status.as_str() {
                                            "success" => "refundsuccess",
                                            "cancelled" | "rejected" => "refundclose",
                                            other => other,
                                        } == refund_status
                                }))
                    })
                    .map(|order| {
                        let mut value = order_value(order, s);
                        if p["needBuyerAddressAndPhone"] == false {
                            value["baseInfo"]["receiverInfo"] = json!({});
                            value["receiveAddress"] = json!({});
                            value["baseInfo"]["buyerContact"] = json!({});
                        }
                        if p["needMemoInfo"] == false {
                            value["baseInfo"]["buyerMemo"] = json!("");
                            value["buyerMemo"] = json!("");
                            value["orderMemo"] = json!("");
                        }
                        value
                    })
                    .collect(),
                p,
            )
        }
        "alibaba.trade.cancel" => {
            let reason = text(p, &["cancelReason"]);
            if !["buyerCancel", "sellerGoodsLack", "other"].contains(&reason.as_str()) {
                return fail("PARAM_ILLEGAL", "invalid cancelReason");
            }
            let order_id = id(p)?;
            let o = buyer_order(s, order_id, owner)?.clone();
            if o.status != "waitbuyerpay" {
                return fail(
                    "ILLEGAL_ORDER_STATUS",
                    "only unpaid orders can be cancelled",
                );
            }
            release_staged(s, &o)?;
            let order = s.orders.get_mut(&order_id).unwrap();
            order.status = "cancel".into();
            order.metadata.close_reason = reason;
            order.metadata.close_operator = "CLOSE_TRADE_BY_BUYER".into();
            if o.metadata.trade_type == "period"
                && period_available(s, &o.buyer, &o.seller) < o.amount
            {
                return fail(
                    "ACCOUNT_PERIOD_QUOTA_NOT_ENOUGH",
                    "insufficient account period quota",
                );
            }
            if !o.metadata.stock_at_payment {
                for l in o.lines {
                    s.products.get_mut(&l.offer_id).unwrap().stock += l.quantity;
                }
            }
            s.emit(
                "ORDER_BUYER_VIEW_ORDER_BUYER_CLOSE",
                owner,
                json!({"orderId":order_id.to_string()}),
            );
            Ok(json!({"success":true}))
        }
        "trade.order.buyerdelete" => {
            let order_id = id(p)?;
            let o = buyer_order(s, order_id, owner)?;
            if !["cancel", "success"].contains(&o.status.as_str()) {
                return fail("ILLEGAL_ORDER_STATUS", "order is not closed");
            }
            s.orders.get_mut(&order_id).unwrap().deleted = true;
            Ok(json!({"success":true,"data":true}))
        }
        "trade.receivegoods.confirm" => {
            let order_id = id(p)?;
            let o = buyer_order(s, order_id, owner)?;
            if o.status != "waitbuyerreceive" {
                return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting receipt");
            }
            let entries = array(p, &["orderEntryIds"])
                .filter(|a| !a.is_empty())
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "orderEntryIds must not be empty".into(),
                })?;
            let mut selected = std::collections::BTreeSet::new();
            for value in entries {
                let eid = value.as_i64().unwrap_or(0);
                if eid < order_id * 100
                    || eid >= order_id * 100 + o.lines.len() as i64
                    || !selected.insert(eid)
                {
                    return fail(
                        "ORDER_ENTRY_NOT_EXISTS",
                        "unique entries belonging to order required",
                    );
                }
                if o.metadata.received.contains_key(&eid) {
                    return fail("ILLEGAL_ORDER_STATUS", "entry already received");
                }
            }
            let now = s.now();
            let closed: Vec<_> = o
                .lines
                .iter()
                .enumerate()
                .filter_map(|(i, _)| {
                    let eid = order_id * 100 + i as i64;
                    (crate::refunds::settled(s, order_id, eid).0 >= line_payment(o, i))
                        .then_some(eid)
                })
                .collect();
            let current = s.orders.get_mut(&order_id).unwrap();
            for eid in selected {
                current.metadata.received.insert(eid, now);
            }
            let complete = (0..current.lines.len()).all(|i| {
                current
                    .metadata
                    .received
                    .contains_key(&(order_id * 100 + i as i64))
                    || closed.contains(&(order_id * 100 + i as i64))
            });
            if complete {
                current.status = "success".into();
                current.metadata.completed = Some(now);
            }
            s.emit(
                "ORDER_BUYER_VIEW_ORDER_COMFIRM_RECEIVEGOODS",
                owner,
                json!({"orderId":order_id,"orderEntryIds":entries}),
            );
            if complete {
                s.emit(
                    "ORDER_BUYER_VIEW_ORDER_SUCCESS",
                    owner,
                    json!({"orderId":order_id.to_string()}),
                );
            }
            Ok(json!({"success":true,"result":true}))
        }
        "alibaba.order.memoAdd" => {
            let order_id = id(p)?;
            buyer_order(s, order_id, owner)?;
            s.orders.get_mut(&order_id).unwrap().memo = text(p, &["memo"]);
            s.emit(
                "ORDER_BUYER_VIEW_MODIFY_MEMO",
                owner,
                json!({"orderId":order_id.to_string()}),
            );
            Ok(json!({"success":true}))
        }
        "alibaba.trade.addFeedback" => {
            let order_id = id(p)?;
            buyer_order(s, order_id, owner)?;
            let message = text(p, &["feedback", "message", "content"]);
            if message.trim().is_empty() {
                return fail("PARAM_ILLEGAL", "feedback must not be empty");
            }
            s.orders.get_mut(&order_id).unwrap().feedback.push(message);
            Ok(json!({"success":true}))
        }
        "order.receiveAddress.buyerUpdate" => {
            let order_id = id(p)?;
            let o = buyer_order(s, order_id, owner)?;
            if !["waitbuyerpay", "waitsellersend"].contains(&o.status.as_str()) {
                return fail("ILLEGAL_ORDER_STATUS", "order already shipped");
            }
            let address = find(p, &["receiveAddress", "addressParam"])
                .cloned()
                .unwrap_or_else(|| p["param"].clone());
            s.orders.get_mut(&order_id).unwrap().address = address;
            Ok(json!({"success":true,"data":true}))
        }
        "alibaba.alipay.url.get"
        | "alibaba.creditPay.url.get"
        | "alibaba.crossBorderPay.url.get"
        | "alibaba.trade.grouppay.url.get"
        | "trade.orderpay.analysis" => {
            let ids = order_ids(p)?;
            for oid in &ids {
                if buyer_order(s, *oid, owner)?.status != "waitbuyerpay" {
                    return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting payment");
                }
            }
            if name == "alibaba.alipay.url.get" {
                for oid in &ids {
                    let order = s.orders[oid].clone();
                    if order.metadata.seller_order
                        && (text(&order.address, &["address", "addressDetail"]).is_empty()
                            || text(&order.address, &["fullName", "receiver", "contactName"])
                                .is_empty())
                    {
                        return fail(
                            "Batch pay : not surport MANUAL-TRADE!",
                            "invitation order requires receiver information for batch payment",
                        );
                    }
                    payment_stock(s, &order, false)?;
                }
            }
            if name == "alibaba.crossBorderPay.url.get" {
                let (eligible, rejected): (Vec<_>, Vec<_>) = ids
                    .iter()
                    .copied()
                    .partition(|id| channel_available(&s.orders[id], "crossBorderPay"));
                if eligible.is_empty() {
                    return fail("400_4", "无可使用支付渠道[跨境宝]付款的订单");
                }
                return Ok(
                    json!({"success":true,"payUrl":format!("{}/payment/{}",s.base_url,eligible.iter().map(i64::to_string).collect::<Vec<_>>().join(",")),"cantPayOrderList":rejected}),
                );
            }
            if name == "trade.orderpay.analysis" {
                let channel = text(p, &["payChannel"]);
                let amount: Decimal = ids
                    .iter()
                    .map(|id| {
                        s.orders[id].amount
                            - s.records
                                .get("staged_payments")
                                .and_then(|m| m.get(&format!("{owner}:{id}")))
                                .and_then(|r| decimal(r, &["amount"]).ok())
                                .unwrap_or_default()
                    })
                    .sum();
                return Ok(
                    json!({"orderTotalCount":ids.len(),"totalPayFee":amount*Decimal::from(100),"orderIds":ids,"payChannel":{"channel":channel,"avaliable":ids.iter().all(|id|channel_available(&s.orders[id],match channel.as_str(){"1"=>"alipay","15"=>"bank",c=>c}))}}),
                );
            }
            Ok(
                json!({"success":true,"payUrl":format!("{}/payment/{}",s.base_url,ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")),"url":format!("{}/payment/{}",s.base_url,ids[0]),"orderIds":ids,"payFailureOrderList":[],"cantPayOrderList":[],"payChannelList":["alipay","bank"],"data":true}),
            )
        }
        "alibaba.trade.payWay.query" => {
            if s.tokens
                .get(&text(p, &["access_token"]))
                .is_some_and(|t| t.sub_account.is_some())
            {
                return fail("500_2", "buyer main account required");
            }
            let order = buyer_order(s, id(p)?, owner)?;
            let channels: Vec<_> = [("alipay", 1, "支付宝"), ("bank", 15, "银行转账")]
                .iter()
                .filter(|(c, _, _)| channel_available(order, c))
                .map(|(_, code, name)| json!({"code":code,"name":name}))
                .collect();
            Ok(
                json!({"channels":channels,"orderId":order.id.to_string(),"payFee":(order.amount*Decimal::from(100)).to_string(),"timeout":chrono::DateTime::from_timestamp_millis(order.created+86400000).unwrap().format("%Y-%m-%d %H:%M:%S").to_string()}),
            )
        }
        "alibaba.trade.pay.protocolPay.isopen" => {
            Ok(json!({"success":true,"paymentAgreements":crate::payments::agreements(s,owner)}))
        }
        "alibaba.trade.pay.protocolPay.preparePay" => crate::payments::prepare(s, p, owner),
        "alibaba.accountPeriod.list.buyerView" => {
            let seller_login = text(p, &["sellerLoginId"]);
            let rows = s.records.get("account_periods").into_iter().flat_map(|m| m.values())
                .filter(|r| r["owner"] == owner)
                .filter_map(|r| {
                    let seller = r["seller"].as_str()?;
                    let account = s.accounts.get(seller)?;
                    if !seller_login.is_empty() && seller_login != account.login { return None; }
                    let quota = decimal(r, &["quota"]).ok()?;

                    Some(json!({"sellerLoginId":account.login,"sellerCompanyName":format!("{}模拟商家",account.login),"gmtQuota":chrono::DateTime::from_timestamp_millis(r["granted"].as_i64()?)?.format("%Y%m%d%H%M%S%3f%z").to_string(),"quota":quota*Decimal::from(100),"surplusQuota":period_available(s,owner,seller)*Decimal::from(100),"statusStr":match r["status"].as_str(){Some("NORMAL")=>"有效",Some("FROZEN")=>"冻结",_=>"失效"},"tapDateStr":format!("{}天",r["days"]),"tapOverdue":0}))
                }).collect::<Vec<_>>();
            let mut paging = p.clone();
            paging["pageSize"] = json!(10);
            let mut result = page(rows, &paging)?;
            result["accountPeriodList"] = result["items"].clone();
            Ok(result)
        }
        "alibaba.trade.createRefund" => {
            let oid = id(p)?;
            let o = buyer_order(s, oid, owner)?;
            if !["waitsellersend", "waitbuyerreceive", "success"].contains(&o.status.as_str()) {
                return fail("ILLEGAL_ORDER_STATUS", "order is not refundable");
            }
            let amount = (decimal(p, &["applyPayment"])? + decimal(p, &["applyCarriage"])?)
                / Decimal::from(100);
            let goods = text(p, &["goodsStatus"]);
            let allowed = match o.status.as_str() {
                "waitsellersend" => goods == "refundWaitSellerSend",
                "waitbuyerreceive" => {
                    ["refundWaitBuyerReceive", "refundBuyerReceived"].contains(&goods.as_str())
                }
                "success" => ["aftersaleBuyerNotReceived", "aftersaleBuyerReceived"]
                    .contains(&goods.as_str()),
                _ => false,
            };
            if ![
                "refundWaitSellerSend",
                "refundWaitBuyerReceive",
                "refundBuyerReceived",
                "aftersaleBuyerNotReceived",
                "aftersaleBuyerReceived",
            ]
            .contains(&goods.as_str())
            {
                return fail("5003", "invalid goodsStatus");
            }
            if ![1, 2].contains(&number(p, &["applyReasonId"]).unwrap_or(0)) {
                return fail("5004", "unknown refund reason");
            }
            if number(p, &["applyReasonId"]) == Some(2)
                && array(p, &["vouchers"]).is_none_or(Vec::is_empty)
            {
                return fail("5005", "quality reason requires voucher");
            }
            if decimal(p, &["applyCarriage"])? > Decimal::ZERO {
                return fail(
                    "5006",
                    "sample free shipping order has no refundable freight",
                );
            }
            if !allowed
                || (text(p, &["disputeRequest"]) == "returnRefund"
                    && !["refundBuyerReceived", "aftersaleBuyerReceived"].contains(&goods.as_str()))
            {
                return fail(
                    "5010",
                    "goodsStatus does not match order or return eligibility",
                );
            }
            if let Some(vouchers) = array(p, &["vouchers"]) {
                if vouchers.iter().any(|url| {
                    !s.rows("files", Some(owner)).iter().any(|file| {
                        file["voucher"] == true
                            && url.as_str()
                                == Some(&format!("{}/files/{}", s.base_url, text(file, &["id"])))
                    })
                }) {
                    return fail(
                        "INVALID_VOUCHER",
                        "voucher must come from uploadRefundVoucher",
                    );
                }
            }
            let description = text(p, &["description"]);
            if !(2..=150).contains(&description.chars().count()) {
                return fail("5012", "description length must be 2..150");
            }
            let entries = crate::refunds::entries(s, o, p, amount)?;
            let reserved = s
                .refunds
                .values()
                .filter(|r| {
                    r.order_id == oid
                        && !["cancelled", "rejected", "success"].contains(&r.status.as_str())
                })
                .map(|r| r.amount)
                .sum::<Decimal>();
            if reserved > Decimal::ZERO {
                return fail("5010", "refund already in progress");
            }
            if amount <= Decimal::ZERO || amount > o.amount - o.refunded - reserved {
                return fail("REFUND_AMOUNT_EXCEEDED", "refund exceeds available amount");
            }
            let after_sales = o.status == "success";
            let rid = s.next_id();
            let r = Refund {
                entries,
                id: rid,
                order_id: oid,
                buyer: owner.into(),
                amount,
                status: "waitselleragree".into(),
                return_goods: text(p, &["disputeRequest"]) == "returnRefund",
                tracking: None,
                history: vec![
                    json!({"action":"apply","time":s.now(),"afterSales":after_sales,"request":p,"beforeStatus":"","afterStatus":"waitselleragree"}),
                ],
            };
            let value = crate::refunds::value(&r, s);
            s.refunds.insert(rid, r);
            s.emit(
                if after_sales {
                    "ORDER_BUYER_VIEW_ORDER_REFUND_AFTER_SALES"
                } else {
                    "ORDER_BUYER_VIEW_ORDER_BUYER_REFUND_IN_SALES"
                },
                owner,
                json!({"orderId":oid,"refundId":rid.to_string(),"refundAction":"apply"}),
            );
            Ok(value)
        }
        "alibaba.trade.cancelRefund" | "alibaba.trade.refund.returnGoods" => {
            let rid = number(p, &["refundId"]).ok_or_else(|| Error {
                code: "4008".into(),
                message: "invalid refundId".into(),
            })?;
            if s.refunds.get(&rid).is_some_and(|r| r.buyer != owner) {
                return fail("ACCESS_DENIED", "refund belongs to another buyer");
            }
            if name.ends_with("returnGoods") {
                if text(p, &["logisticsCompanyNo"]).is_empty()
                    || text(p, &["freightBill"]).is_empty()
                {
                    return fail("4002", "logistics company and tracking required");
                }
                if text(p, &["freightBill"]).chars().any(char::is_whitespace) {
                    return fail("4004", "tracking contains whitespace");
                }
                if let Some(vouchers) = array(p, &["vouchers"]) {
                    if vouchers.len() > 10
                        || vouchers.iter().any(|url| {
                            !s.rows("files", Some(owner)).iter().any(|file| {
                                file["voucher"] == true
                                    && url
                                        == &json!(format!(
                                            "{}/files/{}",
                                            s.base_url,
                                            text(file, &["id"])
                                        ))
                            })
                        })
                    {
                        return fail("4003", "invalid refund vouchers");
                    }
                }
            }
            let now = s.now();
            let r = s
                .refunds
                .get_mut(&rid)
                .filter(|r| r.buyer == owner)
                .ok_or_else(|| Error {
                    code: "REFUND_NOT_EXISTS".into(),
                    message: "refund not found".into(),
                })?;
            if name.ends_with("cancelRefund") {
                if ["success", "cancelled", "rejected"].contains(&r.status.as_str()) {
                    return fail("ILLEGAL_REFUND_STATUS", "refund already closed");
                }
                r.status = "cancelled".into();
            } else {
                if r.status != "waitbuyersend" {
                    return fail("ILLEGAL_REFUND_STATUS", "refund is not awaiting return");
                }
                let tracking = text(p, &["freightBill"]);
                if tracking.is_empty() {
                    return fail("PARAM_ILLEGAL", "freightBill required");
                }
                r.tracking = Some(tracking);
                r.status = "waitsellerreceive".into();
            }
            r.history
                .push(json!({"action":r.status,"time":now,"request":p,"afterStatus":r.status}));
            let r = r.clone();
            Ok(crate::refunds::value(&r, s))
        }
        "alibaba.trade.refund.OpQueryOrderRefund"
        | "alibaba.trade.refund.OpQueryOrderRefundOperationList" => {
            let rid = number(p, &["refundId"]).unwrap_or(0);
            let r = s
                .refunds
                .get(&rid)
                .filter(|r| r.buyer == owner)
                .ok_or_else(|| Error {
                    code: "REFUND_NOT_EXISTS".into(),
                    message: "refund not found".into(),
                })?;
            if name.ends_with("OperationList") {
                page(crate::refunds::operations(r, s), p)
            } else {
                Ok(crate::refunds::value(r, s))
            }
        }
        "alibaba.trade.refund.OpQueryBatchRefundByOrderIdAndStatus"
        | "alibaba.trade.refund.buyer.queryOrderRefundList" => {
            let oid = number(p, &["orderId"]);
            if let Some(id) = oid {
                buyer_order(s, id, owner)?;
            }
            let query_type = text(p, &["queryType"]);
            if !query_type.is_empty() && !["1", "3"].contains(&query_type.as_str()) {
                return fail("PARAM_ILLEGAL", "queryType must be 1 or 3");
            }
            let seller = text(p, &["sellerMemberId"]);
            let tracking = text(p, &["logisticsNo"]);
            if !tracking.is_empty() && seller.is_empty() {
                return fail("PARAM_ILLEGAL", "sellerMemberId required with logisticsNo");
            }
            let start = date_millis(p, "applyStartTime")?;
            if start.is_some_and(|t| t < s.now() - 730 * 86400000) {
                return fail("APPLY_TIME_ERROR", "refund history is limited to 730 days");
            }
            let end = date_millis(p, "applyEndTime")?;
            let modified_start = date_millis(p, "modifyStartTime")?;
            let modified_end = date_millis(p, "modifyEndTime")?;
            if start.zip(end).is_some_and(|(a, b)| a > b)
                || modified_start.zip(modified_end).is_some_and(|(a, b)| a > b)
            {
                return fail("PARAM_ILLEGAL", "start time exceeds end time");
            }
            let statuses = array(p, &["refundStatusSet"]);
            let dispute = number(p, &["dipsuteType"]).unwrap_or(0);
            if !(0..=2).contains(&dispute) {
                return fail("PARAM_ILLEGAL", "dipsuteType must be 0, 1 or 2");
            }
            page(
                s.refunds
                    .values()
                    .filter(|r| {
                        let created = r
                            .history
                            .first()
                            .and_then(|h| h["time"].as_i64())
                            .unwrap_or(0);
                        let modified = r
                            .history
                            .last()
                            .and_then(|h| h["time"].as_i64())
                            .unwrap_or(created);
                        let active =
                            !["success", "cancelled", "rejected"].contains(&r.status.as_str());
                        let after_sales =
                            r.history.first().is_some_and(|h| h["afterSales"] == true);
                        r.buyer == owner
                            && oid.is_none_or(|id| r.order_id == id)
                            && (query_type.is_empty()
                                || (query_type == "1" && active)
                                || (query_type == "3" && r.status == "success"))
                            && (seller.is_empty()
                                || s.orders
                                    .get(&r.order_id)
                                    .is_some_and(|o| o.seller == seller))
                            && (tracking.is_empty() || r.tracking.as_deref() == Some(&tracking))
                            && start.is_none_or(|t| created >= t)
                            && end.is_none_or(|t| created <= t)
                            && modified_start.is_none_or(|t| modified >= t)
                            && modified_end.is_none_or(|t| modified <= t)
                            && statuses.is_none_or(|values| {
                                values.iter().any(|v| v.as_str() == Some(&r.status))
                            })
                            && (dispute == 0 || (dispute == 2) == after_sales)
                    })
                    .map(|r| crate::refunds::value(r, s))
                    .collect(),
                p,
            )
        }
        "alibaba.trade.getMaxRefundFee" => {
            let o = buyer_order(s, id(p)?, owner)?;
            if let Some(rid) = number(p, &["refundId"]) {
                if !s.refunds.get(&rid).is_some_and(|r| {
                    r.order_id == o.id
                        && r.buyer == owner
                        && !["success", "cancelled", "rejected"].contains(&r.status.as_str())
                }) {
                    return fail(
                        "ILLEGAL_REFUND_STATUS",
                        "refundId must belong to an active refund of this order",
                    );
                }
            }
            let counts = array(p, &["refundGoodsCountList"])
                .filter(|a| !a.is_empty())
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "refundGoodsCountList required".into(),
                })?;
            let mut seen = std::collections::BTreeSet::new();
            let mut maximum = Decimal::ZERO;
            for count in counts {
                let eid = number(count, &["id"]).unwrap_or(0);
                let quantity = decimal(count, &["count"])?;
                let Some(line) = eid
                    .checked_sub(o.id * 100)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| o.lines.get(i))
                else {
                    return fail("ORDER_ENTRY_NOT_EXISTS", "entry does not belong to order");
                };
                let (paid, returned) = crate::refunds::settled(s, o.id, eid);
                if !seen.insert(eid)
                    || quantity <= Decimal::ZERO
                    || quantity > line.quantity - returned
                {
                    return fail("PARAM_ILLEGAL", "invalid return quantity");
                }
                maximum += (quantity / line.quantity
                    * line_payment(o, (eid - o.id * 100) as usize))
                .min(line_payment(o, (eid - o.id * 100) as usize) - paid);
            }
            let cents = maximum.min(o.amount - o.refunded) * Decimal::from(100);
            Ok(
                json!({"maxRefundFee":cents,"maxGoodsFee":cents,"maxRefundPayment":cents,"maxRefundCarriage":0,"applyPayment":cents}),
            )
        }
        "alibaba.trade.getRefundReasonList" => {
            let order = buyer_order(s, id(p)?, owner)?;
            if ![
                "refundWaitSellerSend",
                "refundWaitBuyerReceive",
                "refundBuyerReceived",
                "aftersaleBuyerNotReceived",
                "aftersaleBuyerReceived",
            ]
            .contains(&text(p, &["goodsStatus"]).as_str())
            {
                return fail("5003", "invalid goodsStatus");
            }
            let entries = array(p, &["orderEntryIds"]).ok_or_else(|| Error {
                code: "5010".into(),
                message: "order entries required".into(),
            })?;
            let mut seen = std::collections::BTreeSet::new();
            if entries.is_empty()
                || entries.iter().any(|v| {
                    v.as_i64().is_none_or(|id| {
                        id < order.id * 100
                            || id >= order.id * 100 + order.lines.len() as i64
                            || !seen.insert(id)
                    })
                })
            {
                return fail("5010", "invalid order entries");
            }
            Ok(
                json!({"items":[{"id":1,"reasonId":1,"name":"不想要了","reason":"不想要了","needVoucher":false,"noRefundCarriage":true},{"id":2,"reasonId":2,"name":"商品质量问题","reason":"商品质量问题","needVoucher":true,"noRefundCarriage":true}]}),
            )
        }
        "refund.address.get" => {
            buyer_order(s, id(p)?, owner)?;
            Ok(
                json!({"fullName":"测试卖家","mobile":"13800000001","address":"测试退货仓","province":"浙江省","city":"杭州市","area":"滨江区"}),
            )
        }
        "alibaba.trade.getLogisticsInfos.buyerView"
        | "alibaba.trade.getLogisticsTraceInfo.buyerView" => {
            crate::logistics::query(s, name, p, owner)
        }
        "logistics.delivery.urge" => {
            let oid = id(p)?;
            if buyer_order(s, oid, owner)?.status != "waitsellersend" {
                return fail("ILLEGAL_ORDER_STATUS", "order is not awaiting shipment");
            }
            s.put(
                "urges",
                &format!("{owner}:{oid}"),
                json!({"orderId":oid,"time":s.now()}),
            );
            Ok(json!({"success":true,"data":true}))
        }
        "alibaba.logistics.OpDeliverySendOrder.offline"
        | "alibaba.logistics.OpDeliverySendOrder.dummy" => {
            let goods = array(p, &["sendGoods"]).ok_or_else(|| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "sendGoods required".into(),
            })?;
            if goods.is_empty() {
                return fail("PARAM_ILLEGAL", "sendGoods is empty");
            }
            let multi = p["multiPackage"] == true && name.ends_with("offline");
            let ext: Value = serde_json::from_str(if multi {
                "{}"
            } else {
                p["extBody"].as_str().unwrap_or("{}")
            })
            .map_err(|_| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "extBody must be a JSON object".into(),
            })?;
            if !ext.is_object() {
                return fail("PARAM_ILLEGAL", "extBody must be a JSON object");
            }
            if name.ends_with("dummy") {
                let condition = text(&ext, &["noLogisticsCondition"]);
                let required: &[&str] = match condition.as_str() {
                    "1" | "3" => &["noLogisticsName", "noLogisticsTel"],
                    "2" => &["noLogisticsBillNo"],
                    "4" => &[],
                    "5" => &["remarks"],
                    _ => return fail("PARAM_ILLEGAL", "noLogisticsCondition must be 1..5"),
                };
                for field in required {
                    let value = if *field == "remarks" {
                        text(p, &[*field])
                    } else {
                        text(&ext, &[*field])
                    };
                    if value.trim().is_empty() {
                        return fail(
                            "PARAM_ILLEGAL",
                            &format!("{field} required for noLogisticsCondition {condition}"),
                        );
                    }
                }
            }
            let mut out = vec![];
            for g in goods {
                let oid = id(g)?;
                if multi {
                    let entries = array(g, &["sendGoodEntries"])
                        .filter(|a| !a.is_empty())
                        .ok_or_else(|| Error {
                            code: "PARAM_ILLEGAL".into(),
                            message: "sendGoodEntries required".into(),
                        })?;
                    let mut packages: std::collections::BTreeMap<
                        (String, String, String),
                        Vec<Value>,
                    > = std::collections::BTreeMap::new();
                    let mut seen = std::collections::BTreeSet::new();
                    for entry in entries {
                        let eid = number(entry, &["sourceEntryId"]).unwrap_or(0);
                        if !seen.insert(eid) {
                            return fail("PARAM_ILLEGAL", "duplicate entry");
                        }
                        let rows: Value = serde_json::from_str(&text(entry, &["extBody"]))
                            .map_err(|_| Error {
                                code: "PARAM_ILLEGAL".into(),
                                message: "entry extBody must be JSON array".into(),
                            })?;
                        let rows =
                            rows.as_array()
                                .filter(|a| !a.is_empty())
                                .ok_or_else(|| Error {
                                    code: "PARAM_ILLEGAL".into(),
                                    message: "package array required".into(),
                                })?;
                        let mut total = Decimal::ZERO;
                        for row in rows {
                            let code = text(row, &["cpCode"]);
                            let company = text(row, &["logisticsCpName"]);
                            let tracking = text(row, &["mailNo"]);
                            let quantity = decimal(row, &["quantity"])?;
                            if code.is_empty()
                                || company.is_empty()
                                || tracking.is_empty()
                                || quantity <= Decimal::ZERO
                            {
                                return fail(
                                    "PARAM_ILLEGAL",
                                    "package carrier, tracking and positive quantity required",
                                );
                            }
                            total += quantity;
                            packages
                                .entry((code, company, tracking))
                                .or_default()
                                .push(json!({"sourceEntryId":eid,"amount":quantity}));
                        }
                        if total != decimal(entry, &["amount"])? {
                            return fail(
                                "PARAM_ILLEGAL",
                                "package quantities must equal entry amount",
                            );
                        }
                    }
                    for ((code, company, tracking), entries) in packages {
                        let mut options = p.clone();
                        options["cpCode"] = json!(code);
                        options["logisticsCpName"] = json!(company);
                        out.push(crate::logistics::send(
                            s,
                            oid,
                            owner,
                            tracking,
                            Some(&entries),
                            Some(&options),
                        )?);
                    }
                } else {
                    let tracking = if name.ends_with("dummy") {
                        format!("DUMMY-{oid}")
                    } else {
                        text(&ext, &["logisticsBillNo", "logisticsNo", "mailNo"])
                    };
                    let mut options = p.clone();
                    options["cpCode"] = ext["cpCode"].clone();
                    options["logisticsCpName"] = ext["logisticsCpName"].clone();
                    out.push(crate::logistics::send(
                        s,
                        oid,
                        owner,
                        tracking,
                        array(g, &["sendGoodEntries"]),
                        Some(&options),
                    )?);
                }
            }
            Ok(json!({"items":out,"success":true}))
        }
        _ => fail("UNHANDLED", "unhandled"),
    })();
    if result.as_ref().err().is_some_and(|e| e.code == "UNHANDLED") {
        None
    } else {
        Some(result)
    }
}
pub fn order_ids(p: &Value) -> Result<Vec<i64>> {
    for name in ["orderIdsJsonList", "orderIdGroupsJsonList"] {
        if let Some(v) = find(p, &[name]) {
            let parsed: Value =
                serde_json::from_str(v.as_str().unwrap_or("")).map_err(|_| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: format!("{name} must be JSON"),
                })?;
            let flat = parsed
                .as_array()
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "order ID array required".into(),
                })?
                .iter()
                .flat_map(|v| v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]))
                .collect::<Vec<_>>();
            return order_ids(&json!({"orderIds":flat}));
        }
    }
    if let Some(rows) = array(p, &["invoiceApplyModelList"]) {
        if rows.is_empty() {
            return fail("PARAM_ILLEGAL", "invoiceApplyModelList is empty");
        }
        return rows.iter().map(id).collect();
    }
    let values = find(p, &["orderIdList", "orderIds", "tradeIds"])
        .cloned()
        .unwrap_or_else(|| json!([number(p, &["orderId", "tradeId", "sourceId"]).unwrap_or(0)]));
    let ids = if let Some(a) = values.as_array() {
        a.iter()
            .filter_map(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .collect::<Vec<_>>()
    } else {
        values
            .as_str()
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    if ids.is_empty() || ids.contains(&0) {
        return fail("PARAM_ILLEGAL", "orderIds required");
    }
    Ok(ids)
}

pub fn date_millis(p: &Value, name: &str) -> Result<Option<i64>> {
    let Some(value) = find(p, &[name]).and_then(Value::as_str) else {
        return Ok(None);
    };
    let parsed = crate::dates::Options::from_value(p).and_then(|options| options.parse(value));
    parsed.map(Some).map_err(|_| Error {
        code: "PARAM_ILLEGAL".into(),
        message: format!("invalid {name}"),
    })
}

pub fn release_staged(s: &mut Store, order: &Order) -> Result<()> {
    if let Some(payment) = s
        .records
        .get_mut("staged_payments")
        .and_then(|m| m.remove(&format!("{}:{}", order.buyer, order.id)))
    {
        let amount = decimal(&payment, &["amount"])?;
        s.accounts.get_mut(&order.buyer).unwrap().balance += amount;
    }
    Ok(())
}
