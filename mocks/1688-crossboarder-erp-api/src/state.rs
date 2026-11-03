use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub user_id: i64,
    pub id: String,
    pub login: String,
    pub balance: Decimal,
    pub frozen: bool,
    pub scopes: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Token {
    #[serde(default)]
    pub sub_account: Option<String>,
    #[serde(default)]
    pub parent_token: Option<String>,
    pub account: String,
    pub app_key: String,
    pub expires: i64,
    pub refresh: String,
    pub refresh_expires: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Code {
    pub account: String,
    pub app_key: String,
    pub redirect_uri: String,
    pub expires: i64,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Product {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freight: Option<crate::freight::ProductFreight>,
    #[serde(default)]
    pub merchandise: crate::search::Merchandise,
    pub id: i64,
    pub sku: String,
    pub seller: String,
    pub title: String,
    pub price: Decimal,
    pub stock: Decimal,
    pub minimum: Decimal,
    pub active: bool,
    pub category: i64,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    #[serde(default)]
    pub title: String,
    pub offer_id: i64,
    pub spec_id: String,
    pub quantity: Decimal,
    pub price: Decimal,
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OrderMetadata {
    #[serde(default)]
    pub cps: BTreeMap<i64, Value>,
    #[serde(default)]
    pub seller_order: bool,
    #[serde(default)]
    pub stock_at_payment: bool,
    #[serde(default)]
    pub pay_channels: Option<Vec<String>>,
    #[serde(default)]
    pub received: BTreeMap<i64, i64>,
    #[serde(default)]
    pub paid_invoice_types: BTreeMap<i64, Vec<String>>,
    pub modified: Option<i64>,
    pub paid: Option<i64>,
    pub shipped: Option<i64>,
    pub completed: Option<i64>,
    pub flow: String,
    pub trade_type: String,
    pub business_type: String,
    pub buyer_rate: Option<i32>,
    pub seller_rate: Option<i32>,
    pub historical: bool,
    pub complaint: bool,
    pub official_logistics: bool,
    pub close_reason: String,
    pub close_operator: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    #[serde(default)]
    pub metadata: OrderMetadata,
    pub id: i64,
    pub buyer: String,
    pub seller: String,
    pub status: String,
    pub lines: Vec<Line>,
    pub amount: Decimal,
    pub refunded: Decimal,
    pub address: Value,
    pub memo: String,
    pub feedback: Vec<String>,
    pub external_id: String,
    pub created: i64,
    pub tracking: Option<String>,
    pub deleted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RefundEntry {
    pub quantity: Decimal,
    pub amount: Decimal,
    pub restock: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Refund {
    #[serde(default)]
    pub entries: BTreeMap<i64, RefundEntry>,
    pub id: i64,
    pub order_id: i64,
    pub buyer: String,
    pub amount: Decimal,
    pub status: String,
    pub return_goods: bool,
    pub tracking: Option<String>,
    pub history: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Event {
    pub app_key: String,
    pub id: String,
    pub topic: String,
    pub account: String,
    pub data: Value,
    pub created: i64,
    pub attempts: u32,
    pub due: i64,
    pub status: String,
    pub last_error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Subscription {
    #[serde(default = "default_app")]
    pub app_key: String,
    pub topic: String,
    pub account: String,
    pub url: String,
    pub secret: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Fault {
    pub api: String,
    pub account: Option<String>,
    pub remaining: u32,
    #[serde(default)]
    pub delay_ms: u64,
    pub code: Option<String>,
    pub message: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Store {
    pub current_app: String,
    pub fixed_time: Option<i64>,
    pub offset_ms: i64,
    pub sequence: i64,
    pub generation: u64,
    pub apps: BTreeMap<String, String>,
    pub accounts: BTreeMap<String, Account>,
    pub tokens: BTreeMap<String, Token>,
    pub codes: BTreeMap<String, Code>,
    pub products: BTreeMap<i64, Product>,
    pub orders: BTreeMap<i64, Order>,
    pub refunds: BTreeMap<i64, Refund>,
    pub records: BTreeMap<String, BTreeMap<String, Value>>,
    pub events: Vec<Event>,
    pub subscriptions: Vec<Subscription>,
    pub faults: Vec<Fault>,
    pub base_url: String,
}
fn default_app() -> String {
    "test-app".into()
}
impl Store {
    pub fn seed(base_url: String) -> Self {
        let mut s = Self {
            current_app: "test-app".into(),
            fixed_time: None,
            offset_ms: 0,
            sequence: 10000,
            generation: 0,
            apps: BTreeMap::from([("test-app".into(), "test-secret".into())]),
            accounts: BTreeMap::new(),
            tokens: BTreeMap::new(),
            codes: BTreeMap::new(),
            products: BTreeMap::new(),
            orders: BTreeMap::new(),
            refunds: BTreeMap::new(),
            records: BTreeMap::new(),
            events: vec![],
            subscriptions: vec![],
            faults: vec![],
            base_url,
        };
        for (id, login, token) in [
            ("buyer-1", "buyer", "test-token"),
            ("buyer-2", "other-buyer", "other-token"),
            ("seller-1", "seller", "seller-token"),
        ] {
            s.accounts.insert(
                id.into(),
                Account {
                    domain: format!("{login}.1688.com"),
                    user_id: if id == "buyer-1" {
                        1
                    } else if id == "buyer-2" {
                        2
                    } else {
                        3
                    },
                    id: id.into(),
                    login: login.into(),
                    balance: Decimal::new(10000000, 2),
                    frozen: false,
                    scopes: vec!["*".into()],
                },
            );
            s.tokens.insert(
                token.into(),
                Token {
                    sub_account: None,
                    parent_token: None,
                    account: id.into(),
                    app_key: "test-app".into(),
                    expires: 4102444800000,
                    refresh: format!("refresh-{token}"),
                    refresh_expires: 4102444800000,
                },
            );
        }
        crate::freight::seed(&mut s);
        for (id, title, price, stock, minimum, active) in [
            (1001, "棉质短袖 T-shirt", 1990, 100, 1, true),
            (1002, "陶瓷杯 Ceramic cup", 1250, 50, 2, true),
            (1003, "缺货商品", 1000, 0, 1, true),
            (1004, "已下架商品", 1000, 10, 1, false),
        ] {
            s.products.insert(
                id,
                Product {
                    freight: crate::freight::product_seed(id),
                    merchandise: crate::search::Merchandise::sample(id, 1792886400000),
                    id,
                    sku: format!("sku-{id}"),
                    seller: "seller-1".into(),
                    title: title.into(),
                    price: Decimal::new(price, 2),
                    stock: Decimal::from(stock),
                    minimum: Decimal::from(minimum),
                    active,
                    category: 101,
                },
            );
        }
        s.put("addresses", "buyer-1:1", json!({"id":1,"addressId":1,"fullName":"测试买家","mobile":"13800000000","provinceText":"浙江省","cityText":"杭州市","areaText":"滨江区","address":"网商路699号","postCode":"310000","provinceCode":"330000","cityCode":"330100","areaCode":"330108"}));
        s.put("invoice_titles", "buyer-1:1", json!({"id":1,"titleId":1,"invoiceTitleId":1,"title":"测试公司","titleType":"COMPANY","taxpayerIdentify":"91330100TEST000001","companyName":"测试公司","taxpayerIdentifier":"91330100TEST000001","invoiceType":0}));
        s.put(
            "subaccounts",
            "buyer-1:sub-1",
            json!({"id":"sub-1","memberId":"sub-1","loginId":"buyer:采购","authorized":false}),
        );
        s.put(
            "models",
            "flagship",
            json!({"id":"flagship","displayName":"模拟旗舰模型","description":"固定采购结果","recommended":true,"iconUrl":"/assets/product.svg","isDefault":true,"sortOrder":1,"model":"flagship","name":"flagship","available":true}),
        );
        let sample_time = 1767225600000_i64;
        let address = s.records["addresses"]["buyer-1:1"].clone();
        for (id, status, tracking) in [
            (9001, "waitbuyerpay", None),
            (9002, "success", Some("SAMPLE-TRACK".to_owned())),
        ] {
            s.orders.insert(
                id,
                Order {
                    metadata: OrderMetadata {
                        modified: Some(sample_time),
                        paid: (status == "success").then_some(sample_time),
                        shipped: (status == "success").then_some(sample_time),
                        completed: (status == "success").then_some(sample_time),
                        ..Default::default()
                    },
                    id,
                    buyer: "buyer-1".into(),
                    seller: "seller-1".into(),
                    status: status.into(),
                    lines: vec![Line {
                        title: "棉质短袖".into(),
                        offer_id: 1001,
                        spec_id: "sku-1001".into(),
                        quantity: Decimal::ONE,
                        price: Decimal::new(1990, 2),
                    }],
                    amount: Decimal::new(1990, 2),
                    refunded: Decimal::ZERO,
                    address: address.clone(),
                    memo: "内置关联样例".into(),
                    feedback: vec![],
                    external_id: format!("sample-{id}"),
                    created: sample_time,
                    tracking,
                    deleted: false,
                },
            );
        }
        s.refunds.insert(
            9003,
            Refund {
                entries: BTreeMap::from([(
                    900200,
                    RefundEntry {
                        quantity: Decimal::ONE,
                        amount: Decimal::new(500, 2),
                        restock: false,
                    },
                )]),
                id: 9003,
                order_id: 9002,
                buyer: "buyer-1".into(),
                amount: Decimal::new(500, 2),
                status: "waitselleragree".into(),
                return_goods: false,
                tracking: None,
                history: vec![json!({"action":"apply","time":sample_time,"afterSales":true})],
            },
        );
        s.put(
            "follows",
            "buyer-1:1001",
            json!({"offerId":1001,"productId":1001}),
        );
        s.put(
            "distribution",
            "buyer-1:1001",
            json!({"offerId":1001,"status":"PUSHED"}),
        );
        s.put("funds","buyer-1:sample-fund",json!({"applyNo":"sample-fund","outBizNo":"sample-batch","outPayNo":"sample-pay","applyAmount":10000,"payAmount":10000,"payedAmount":10000,"availableAmount":10000,"status":"SUCCESS","assertNo":"sample-receipt","payNo":"sample-payment","cashierUrl":format!("{}/payment/sample-fund",s.base_url)}));
        s.put("contracts","buyer-1:sample-contract",json!({"draftNo":"sample-contract","contractId":"sample-contract","status":"SIGNED","contractCurrentStatus":"SIGNED","amount":100}));
        s.put("tasks","buyer-1:sample-task",json!({"taskId":"sample-task","sessionId":"sample-session","status":"END","createdAt":sample_time.to_string(),"finishedAt":crate::dates::task_time(sample_time),"outputStatus":"done","chunks":"[]","content":"[{\"offerId\":1001}]","rows":[{"offerId":1001}],"messages":[],"nextIndex":0}));
        let mut material = crate::business::product_value(
            &s.products[&1001],
            &s.base_url,
            &s.accounts[&s.products[&1001].seller],
            &s.current_app,
        );
        material["offerCollectionId"] = json!("sample-material");
        material["outerUserIdentity"] = json!("external-1");
        material["images"] = json!([format!("{}/assets/product.svg", s.base_url)]);
        s.put("materials", "buyer-1:sample-material", material);
        s.put(
            "repurchase",
            "buyer-1:sample-contract",
            json!({"contractId":9004,"offerId":1001,"price":19.90,"status":"ACTIVE"}),
        );
        s.put(
            "insurance",
            "buyer-1:sample-policy",
            json!({"policyNo":"sample-policy","insuranceId":8001,"orderId":9002,"tradeClaimList":[]}),
        );
        s.put("account_periods", "buyer-1:seller-1", json!({"owner":"buyer-1","seller":"seller-1","quota":"10000","granted":1792886400000_i64,"days":30,"status":"NORMAL"}));
        s.put(
            "payment_agreements",
            "buyer-1:ALIPAY",
            json!({"payChannel":"ALIPAY","bindingStatus":true,"signedStatus":true}),
        );
        s.put("app_configs","test-app",json!({"distribution":true,"product_active":true,"users":["buyer-1","buyer-2"],"webUnionKeys":["test-web-union"]}));
        s.put("supplier_relations","buyer-1:seller-1",json!({"memberId":"seller-1","consignCreateTime":sample_time,"consignStatus":"cooperating"}));
        s.put("app_orders","buyer-1:sample-app-order",json!({"orderItemNum":"sample-app-order","memberId":"buyer-1","loginId":"buyer","productName":"模拟 ERP 订阅","bizStatus":"S","bizStatusExt":"service","payStatus":"payment_success","paymentAmount":100,"executePrice":100,"gmtCreate":"20260101000000000+0000","gmtServiceBegin":"20260101000000000+0000","gmtServiceEnd":"20270101000000000+0000","gmtConfirm":"20260101000000000+0000","skuVersion":"V0","skuName":"基础版"}));
        s
    }
    pub fn finish_changes(&mut self, before: &Store) {
        let now = self.now();
        for (id, product) in &mut self.products {
            if before.products.get(id) != Some(product) {
                product.merchandise.modified = Some(now);
            }
        }
        for (id, order) in &mut self.orders {
            if before.orders.get(id) == Some(order) {
                continue;
            }
            order.metadata.modified = Some(now);
            if order.metadata.paid.is_none()
                && ["waitsellersend", "waitbuyerreceive", "success"]
                    .contains(&order.status.as_str())
            {
                order.metadata.paid = Some(now);
                order.metadata.paid_invoice_types = order
                    .lines
                    .iter()
                    .map(|l| {
                        (
                            l.offer_id,
                            self.products[&l.offer_id].merchandise.invoice_types.clone(),
                        )
                    })
                    .collect();
            }
            if order.metadata.shipped.is_none()
                && ["waitbuyerreceive", "success"].contains(&order.status.as_str())
            {
                order.metadata.shipped = Some(now);
            }
            if order.metadata.completed.is_none() && order.status == "success" {
                order.metadata.completed = Some(now);
            }
        }
        for request in self
            .records
            .get_mut("withhold_requests")
            .into_iter()
            .flat_map(|m| m.values_mut())
        {
            if let Some(order) = request["orderId"]
                .as_i64()
                .and_then(|id| self.orders.get(&id))
            {
                request["paySuccess"] = json!(order.metadata.paid.is_some());
            }
        }
    }
    pub fn now(&self) -> i64 {
        self.fixed_time
            .unwrap_or_else(|| Utc::now().timestamp_millis())
            + self.offset_ms
    }
    pub fn next_id(&mut self) -> i64 {
        self.sequence += 1;
        self.sequence
    }
    pub fn put(&mut self, table: &str, key: &str, value: Value) {
        self.records
            .entry(table.into())
            .or_default()
            .insert(key.into(), value);
    }
    pub fn rows(&self, table: &str, owner: Option<&str>) -> Vec<Value> {
        self.records
            .get(table)
            .into_iter()
            .flat_map(|m| m.iter())
            .filter(|(k, _)| owner.is_none_or(|o| k.starts_with(&format!("{o}:"))))
            .map(|(_, v)| v.clone())
            .collect()
    }
    pub fn emit(&mut self, topic: &str, account: &str, mut data: Value) {
        let now = self.now();
        if topic.starts_with("ORDER_BUYER_VIEW_") {
            let oid = data.get("orderId").and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|v| v.parse().ok()))
            });
            if let Some(o) = oid.and_then(|id| self.orders.get(&id)) {
                data["orderId"] = json!(o.id);
                data["currentStatus"] = json!(o.status);
                data["buyerMemberId"] = json!(o.buyer);
                data["sellerMemberId"] = json!(o.seller);
                let formatted = chrono::DateTime::from_timestamp_millis(now)
                    .unwrap()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                data["msgSendTime"] = json!(formatted);
                if topic == "ORDER_BUYER_VIEW_ORDER_SELLER_MODIFY_ADRESS" {
                    data["messageSendTime"] = json!(formatted);
                }
            }
        }
        let formatted = chrono::DateTime::from_timestamp_millis(now)
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        if let Some(refund) = data.get("refundId").cloned() {
            data["refundId"] = json!(refund
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| refund.to_string()));
        }
        if topic.starts_with("PRODUCT_RELATION_VIEW_") {
            let offer = data.get("offerId").and_then(Value::as_i64).unwrap_or(0);
            data["productIds"] = json!(offer.to_string());
            data["memberId"] = json!(self
                .products
                .get(&offer)
                .map(|p| p.seller.as_str())
                .unwrap_or(account));
            data["msgSendTime"] = json!(formatted);
        }
        if topic == "PRODUCT_PRODUCT_CHANGE" {
            if data.get("memberId").is_none() {
                data["memberId"] = json!(account);
            }
            data["offerIds"] = json!(data["offerId"].to_string());
            data["action"] = json!("UPDATE");
        }
        if topic == "PRODUCT_PFT_OFFER_PRICE_MODIFY" {
            data["minPrice"] = data["price"]
                .as_str()
                .and_then(|v| v.parse::<f64>().ok())
                .map(|v| json!(v))
                .unwrap_or_else(|| data["price"].clone());
            data["activeTime"] = json!(now);
        }
        if topic == "PRODUCT_PRODUCT_INVENTORY_CHANGE" {
            let stock: Decimal = data["stock"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(Decimal::ZERO);
            let previous: Decimal = data["previousStock"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(Decimal::ZERO);
            data = json!({"OfferInventoryChangeList":[{"offerId":data["offerId"],"skuId":data["offerId"],"offerOnSale":serde_json::Number::from_str(&stock.to_string()).unwrap(),"skuOnSale":serde_json::Number::from_str(&stock.to_string()).unwrap(),"quantity":serde_json::Number::from_str(&(stock-previous).to_string()).unwrap(),"bizTime":now}]});
        }
        if topic == "CROSSBOARD_CROSSBOARD_ADD_SUPPLY"
            || topic == "PRODUCT_PRODUCT_CROSSBOARD_INFORM"
        {
            data["offerId"] = json!(data["offerId"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| data["offerId"].to_string()));
        }
        if topic == "PRODUCT_PRODUCT_CROSSBOARD_INFORM" {
            data["userInfo"] = json!(account);
            data["action"] = json!("ADD");
        }
        if topic == "LOGISTICS_BUYER_VIEW_TRACE" || topic == "LOGISTICS_MAIL_NO_CHANGE" {
            let oid = crate::business::number(&data, &["orderId"]).unwrap_or(0);
            let pid = data
                .get("logisticsId")
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_else(|| oid.to_string());
            let entries: Vec<_> = if let Some(entries) = data["entries"].as_array() {
                entries
                    .iter()
                    .map(|e| json!({"orderId":oid,"orderEntryId":e["entryId"]}))
                    .collect()
            } else {
                self.orders
                    .get(&oid)
                    .map(|o| {
                        o.lines
                            .iter()
                            .enumerate()
                            .map(|(i, _)| json!({"orderId":oid,"orderEntryId":oid*100+i as i64}))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            data = if topic == "LOGISTICS_BUYER_VIEW_TRACE" {
                json!({"OrderLogisticsTracingModel":{"logisticsId":pid,"cpCode":data["cpCode"].as_str().unwrap_or("MOCK"),"mailNo":data["logisticsBillNo"],"statusChanged":"CONSIGN","changeTime":now,"orderLogsItems":entries}})
            } else {
                json!({"MailNoChangeModel":{"logisticsId":pid,"oldCpCode":data["oldCpCode"].as_str().unwrap_or("MOCK"),"newCpCode":data["newCpCode"].as_str().unwrap_or("MOCK"),"oldMailNo":data["oldMailNo"],"newMailNo":data["newMailNo"],"eventTime":now,"orderLogsItems":entries}})
            };
        }
        if topic.starts_with("FENXIAO_PAY_CREDIT_FUND_") {
            data["payerName"] = json!(self
                .accounts
                .get(account)
                .map(|a| a.login.as_str())
                .unwrap_or(account));
            if data.get("outBizNo").is_none() {
                data["outBizNo"] = data["applyNo"].clone();
            }
            if topic.ends_with("REFUND_RESULT") {
                data["outRefundNo"] = data["refundNo"].clone();
                data["refundAmount"] =
                    serde_json::from_str::<Value>(data["amount"].as_str().unwrap_or("0"))
                        .unwrap_or(json!(0));
            } else {
                data["amount"] =
                    serde_json::from_str::<Value>(data["applyAmount"].as_str().unwrap_or("0"))
                        .unwrap_or(json!(0));
                if data.get("outPayNo").is_none() {
                    data["outPayNo"] = data["applyNo"].clone();
                }
            }
        }
        if topic.starts_with("SYT_") {
            data["eventType"] = json!(topic.trim_start_matches("SYT_"));
            data["contractCurrentStatus"] = data["status"].clone();
            if topic == "SYT_CONTRACT_SIGN_SUCCESS" {
                data["signSuccessTime"] = json!(formatted);
            }
            if topic == "SYT_CONTRACT_PAY_SUCCESS" {
                data["payChannel"] = json!("mock");
            }
            if topic == "SYT_REFUND_FINISH_NOTICE" {
                data["refundStatus"] = json!("SUCCESS");
                data["refundOrderNo"] =
                    json!(format!("refund-{}", data["draftNo"].as_str().unwrap_or("")));
            }
        }
        self.enqueue(topic, account, data);
    }
    pub fn enqueue(&mut self, topic: &str, account: &str, data: Value) {
        let id = self.next_id().to_string();
        let now = self.now();
        self.events.push(Event {
            app_key: self.current_app.clone(),
            id,
            topic: topic.into(),
            account: account.into(),
            data,
            created: now,
            attempts: 0,
            due: now,
            status: "pending".into(),
            last_error: None,
        });
    }
    pub fn validate(&self) -> Result<(), String> {
        let base = self
            .fixed_time
            .unwrap_or_else(|| Utc::now().timestamp_millis());
        if self.offset_ms < 0
            || base
                .checked_add(self.offset_ms)
                .is_none_or(|t| !(0..=4102444800000).contains(&t))
        {
            return Err("clock must be between 1970 and 2100".into());
        }
        if self.apps.is_empty() || !(10000..=1000000000000).contains(&self.sequence) {
            return Err("apps and sequence are invalid".into());
        }
        crate::freight::validate(self)?;
        for (id, p) in &self.products {
            if id != &p.id
                || p.stock < Decimal::ZERO
                || p.price < Decimal::ZERO
                || p.price > Decimal::from(1000000000000_i64)
                || p.stock > Decimal::from(1000000000_i64)
                || p.minimum <= Decimal::ZERO
                || !(Decimal::ZERO..=Decimal::from(5)).contains(&p.merchandise.score)
                || !(Decimal::ZERO..=Decimal::from(5)).contains(&p.merchandise.dispute_score)
                || [p.merchandise.response_rate, p.merchandise.turnover_rate]
                    .iter()
                    .any(|v| !(Decimal::ZERO..=Decimal::ONE).contains(v))
                || [
                    p.merchandise.collection_rate24,
                    p.merchandise.collection_rate48,
                    p.merchandise.return_rate,
                ]
                .iter()
                .any(|v| !(Decimal::ZERO..=Decimal::from(100)).contains(v))
                || [p.merchandise.ratio, p.merchandise.old_buyer_ratio]
                    .iter()
                    .any(|v| !(Decimal::ZERO..=Decimal::ONE).contains(v))
                || p.merchandise
                    .sales
                    .values()
                    .any(|v| *v < 0 || *v > 1_000_000_000)
                || [
                    p.merchandise.mix_min_quantity,
                    p.merchandise.mix_min_amount,
                    p.merchandise.wholesale_minimum,
                ]
                .into_iter()
                .flatten()
                .any(|v| v <= Decimal::ZERO || v > Decimal::from(1000000000_i64))
                || !(0..=1_000_000_000).contains(&p.merchandise.order_count)
                || p.merchandise.buyer_count < 0
                || p.merchandise
                    .sale_start
                    .is_some_and(|t| !(0..=4102444800000).contains(&t))
                || p.merchandise
                    .max_quantity
                    .is_some_and(|q| q < p.minimum || q > Decimal::from(1000000000_i64))
                || p.merchandise.quantity_scale.is_some_and(|n| n > 9)
                || p.merchandise
                    .private_buyers
                    .iter()
                    .any(|a| !self.accounts.contains_key(a))
                || p.merchandise
                    .invoice_types
                    .iter()
                    .any(|v| !["VATAX_COMM", "VATAX_SPEC"].contains(&v.as_str()))
                || p.merchandise.trade_types.iter().any(|v| {
                    ![
                        "assureTrade",
                        "alipay",
                        "period",
                        "assure",
                        "creditBuy",
                        "bank",
                        "631staged",
                        "37staged",
                    ]
                    .contains(&v.as_str())
                })
                || p.merchandise.promotion_count < 0
                || p.merchandise.promotion_count > 1_000_000_000
                || p.merchandise
                    .modified
                    .is_some_and(|t| t < p.merchandise.created || t > 4102444800000)
                || !(0..=4102444800000).contains(&p.merchandise.created)
                || !self.accounts.contains_key(&p.seller)
            {
                return Err("invalid product".into());
            }
        }
        for (id, a) in &self.accounts {
            if id != &a.id
                || a.balance < Decimal::ZERO
                || a.balance > Decimal::from(1000000000000000_i64)
            {
                return Err("invalid account".into());
            }
        }
        for t in self.tokens.values() {
            if !self.accounts.contains_key(&t.account)
                || !self.apps.contains_key(&t.app_key)
                || t.parent_token.as_ref().is_some_and(|parent| {
                    self.tokens.get(parent).is_none_or(|p| {
                        p.account != t.account || p.app_key != t.app_key || p.sub_account.is_some()
                    })
                })
            {
                return Err("invalid token owner".into());
            }
        }
        for (id, o) in &self.orders {
            if !(0..=4102444800000).contains(&o.created)
                || o.metadata.pay_channels.as_ref().is_some_and(|channels| {
                    channels
                        .iter()
                        .any(|c| !["alipay", "bank", "crossBorderPay"].contains(&c.as_str()))
                })
                || id != &o.id
                || *id > self.sequence
                || !self.accounts.contains_key(&o.buyer)
                || !self.accounts.contains_key(&o.seller)
                || o.amount < Decimal::ZERO
                || o.amount > Decimal::from(1000000000000_i64)
                || ![
                    "waitbuyerpay",
                    "waitsellersend",
                    "waitbuyerreceive",
                    "success",
                    "cancel",
                ]
                .contains(&o.status.as_str())
                || o.lines.is_empty()
                || (o.amount > Decimal::ZERO && o.lines.iter().all(|l| l.price == Decimal::ZERO))
                || o.refunded > o.amount
                || o.refunded < Decimal::ZERO
                || o.lines.iter().any(|l| {
                    !self.products.contains_key(&l.offer_id)
                        || l.quantity <= Decimal::ZERO
                        || l.quantity > Decimal::from(1000000000_i64)
                        || l.price < Decimal::ZERO
                        || l.price > Decimal::from(1000000000000_i64)
                })
            {
                return Err("invalid order".into());
            }
        }
        for order in self.orders.values() {
            if order.metadata.received.iter().any(|(id, time)| {
                *id < order.id * 100
                    || *id >= order.id * 100 + order.lines.len() as i64
                    || *time < order.created
                    || *time > self.now()
            }) {
                return Err("invalid received entry".into());
            }
        }
        for (id, r) in &self.refunds {
            if id != &r.id
                || *id > self.sequence
                || !self.orders.contains_key(&r.order_id)
                || r.amount <= Decimal::ZERO
                || self
                    .orders
                    .get(&r.order_id)
                    .is_none_or(|o| o.buyer != r.buyer || r.amount > o.amount)
                || ![
                    "waitselleragree",
                    "waitbuyersend",
                    "waitsellerreceive",
                    "success",
                    "cancelled",
                    "rejected",
                ]
                .contains(&r.status.as_str())
            {
                return Err("invalid refund".into());
            }
        }
        for refund in self.refunds.values() {
            let order = &self.orders[&refund.order_id];
            if !refund.entries.is_empty()
                && (refund.entries.values().map(|e| e.amount).sum::<Decimal>() != refund.amount
                    || refund.entries.iter().any(|(id, e)| {
                        id.checked_sub(order.id * 100)
                            .and_then(|n| usize::try_from(n).ok())
                            .and_then(|i| order.lines.get(i))
                            .is_none_or(|l| {
                                e.quantity <= Decimal::ZERO
                                    || e.quantity > l.quantity
                                    || e.amount < Decimal::ZERO
                                    || e.amount
                                        > crate::business::line_payment(
                                            order,
                                            (*id - order.id * 100) as usize,
                                        )
                            })
                    }))
            {
                return Err("invalid refund entries".into());
            }
        }
        for order in self.orders.values() {
            for (i, line) in order.lines.iter().enumerate() {
                let (amount, quantity) =
                    crate::refunds::settled(self, order.id, order.id * 100 + i as i64);
                if amount > crate::business::line_payment(order, i) || quantity > line.quantity {
                    return Err("settled refunds exceed entry".into());
                }
            }
        }
        if self
            .records
            .values()
            .any(|rows| rows.values().any(|v| !v.is_object()))
        {
            return Err("record rows must be objects".into());
        }
        for (key, package) in self
            .records
            .get("packages")
            .into_iter()
            .flat_map(|m| m.iter())
        {
            let owner = key
                .split_once(':')
                .map(|(o, _)| o)
                .ok_or("package owner required")?;
            let order = crate::business::number(package, &["orderId"])
                .and_then(|id| self.orders.get(&id))
                .ok_or("package order does not exist")?;
            let entries = package["entries"]
                .as_array()
                .filter(|v| !v.is_empty())
                .ok_or("package entries required")?;
            if owner != order.buyer
                || package["created"]
                    .as_i64()
                    .is_none_or(|v| !(0..=4102444800000).contains(&v))
                || !package["receiver"].is_object()
            {
                return Err("invalid package owner/time/receiver".into());
            }
            for entry in entries {
                let index = crate::business::number(entry, &["entryId"])
                    .and_then(|id| id.checked_sub(order.id * 100))
                    .and_then(|n| usize::try_from(n).ok());
                let line = index
                    .and_then(|i| order.lines.get(i))
                    .ok_or("invalid package order entry")?;
                let qty = crate::business::decimal(entry, &["quantity"])
                    .map_err(|_| "invalid shipped quantity")?;
                if qty <= Decimal::ZERO || qty > line.quantity {
                    return Err("invalid package quantity".into());
                }
            }
        }
        for order in self.orders.values() {
            let sent = crate::logistics::sent(self, order.id);
            if order.lines.iter().enumerate().any(|(i, l)| {
                sent.get(&(order.id * 100 + i as i64))
                    .is_some_and(|q| *q > l.quantity)
            }) {
                return Err("total shipment exceeds order quantity".into());
            }
            if [
                order.metadata.modified,
                order.metadata.paid,
                order.metadata.shipped,
                order.metadata.completed,
            ]
            .into_iter()
            .flatten()
            .any(|t| t < order.created || t > 4102444800000)
            {
                return Err("invalid order event time".into());
            }
        }
        for (key, row) in self
            .records
            .get("supplier_relations")
            .into_iter()
            .flat_map(|m| m.iter())
        {
            if key
                .split_once(':')
                .is_none_or(|(owner, _)| !self.accounts.contains_key(owner))
                || !self
                    .accounts
                    .contains_key(&crate::business::text(row, &["memberId", "sellerMemberId"]))
            {
                return Err("invalid supplier relationship".into());
            }
        }
        for row in self
            .records
            .get("tasks")
            .into_iter()
            .flat_map(|m| m.values())
        {
            if !["INIT", "RUNNING", "WAIT_SKILL", "WAIT_USER", "END", "KILL"]
                .contains(&row["status"].as_str().unwrap_or(""))
            {
                return Err("invalid task status".into());
            }
            if let Some(chunks) = row.get("chunks") {
                if chunks
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .is_none_or(|v| !v.is_array())
                {
                    return Err("task chunks must be a JSON array string".into());
                }
            }
        }
        for (key, row) in self
            .records
            .get("payment_agreements")
            .into_iter()
            .flat_map(|m| m.iter())
        {
            let (owner, channel) = key.split_once(':').ok_or("invalid agreement key")?;
            if !self.accounts.contains_key(owner)
                || !["ALIPAY", "SHEGOU"].contains(&channel)
                || row["payChannel"] != channel
                || !row["bindingStatus"].is_boolean()
                || !row["signedStatus"].is_boolean()
            {
                return Err("invalid payment agreement".into());
            }
        }
        for table in [
            "contracts",
            "funds",
            "fund_refunds",
            "fund_payments",
            "staged_payments",
            "account_periods",
        ] {
            for (key, row) in self.records.get(table).into_iter().flat_map(|m| m.iter()) {
                let (owner, identifier) = key
                    .split_once(':')
                    .ok_or_else(|| format!("{table}: owner prefix required"))?;
                if !self.accounts.contains_key(owner) || identifier.is_empty() {
                    return Err(format!("{table}: invalid owner or identifier"));
                }
                let money = |field: &str, required: bool| -> Result<Decimal, String> {
                    if !required && row.get(field).is_none() {
                        return Ok(Decimal::ZERO);
                    }
                    let amount = crate::business::decimal(row, &[field])
                        .map_err(|_| format!("{table}.{field}: invalid amount"))?;
                    if amount < Decimal::ZERO
                        || amount > Decimal::from(100000000000000_i64)
                        || amount.scale() > 2
                    {
                        return Err(format!("{table}.{field}: amount outside range"));
                    }
                    Ok(amount)
                };
                match table {
                    "account_periods" => {
                        money("quota", true)?;
                        if row["owner"] != owner
                            || row["seller"] != identifier
                            || !self.accounts.contains_key(identifier)
                            || !["NORMAL", "FROZEN", "EXPIRED"]
                                .contains(&row["status"].as_str().unwrap_or(""))
                            || row["days"].as_i64().is_none_or(|n| !(1..=365).contains(&n))
                            || row["granted"]
                                .as_i64()
                                .is_none_or(|n| !(0..=4102444800000).contains(&n))
                        {
                            return Err("invalid account period".into());
                        }
                    }
                    "contracts" => {
                        let amount = money("amount", true)?;
                        let refunded = money("refunded", false)?;
                        if amount <= Decimal::ZERO
                            || refunded > amount
                            || money("refundAmount", false)? > amount
                        {
                            return Err("invalid contract amounts".into());
                        }
                        if row["status"] == "REFUND_PENDING"
                            && (money("refundAmount", true)? <= Decimal::ZERO
                                || refunded + money("refundAmount", true)? > amount)
                        {
                            return Err("invalid pending contract refund".into());
                        }
                    }
                    "funds" => {
                        let amount = money("applyAmount", true)?;
                        let available = money("availableAmount", true)?;
                        if amount <= Decimal::ZERO
                            || available > amount
                            || money("payedAmount", false)? > amount
                        {
                            return Err("invalid fund amounts".into());
                        }
                    }
                    "fund_refunds" | "fund_payments" => {
                        let amount = money("amount", true)?;
                        let fund_key =
                            format!("{owner}:{}", crate::business::text(row, &["applyNo"]));
                        let fund = self
                            .records
                            .get("funds")
                            .and_then(|m| m.get(&fund_key))
                            .ok_or("invalid fund reference")?;
                        let limit = crate::business::decimal(fund, &["applyAmount"])
                            .map_err(|_| "invalid fund amount")?;
                        if amount <= Decimal::ZERO || amount > limit {
                            return Err("invalid fund payment/refund amount".into());
                        }
                        if table == "fund_payments" {
                            let order = crate::business::number(row, &["orderId"])
                                .and_then(|id| self.orders.get(&id))
                                .ok_or("invalid fund payment order")?;
                            if order.buyer != owner || amount > order.amount * Decimal::from(100) {
                                return Err("invalid fund payment owner/amount".into());
                            }
                        }
                    }
                    "staged_payments" => {
                        let order = identifier
                            .parse::<i64>()
                            .ok()
                            .and_then(|id| self.orders.get(&id))
                            .ok_or("invalid staged payment order")?;
                        if order.buyer != owner || money("amount", true)? > order.amount {
                            return Err("invalid staged payment owner/amount".into());
                        }
                        if let Some(history) = row.get("history") {
                            let history =
                                history.as_array().ok_or("invalid installment history")?;
                            let mut total = Decimal::ZERO;
                            let mut previous = order.created;
                            for entry in history {
                                let amount = crate::business::decimal(entry, &["amount"])
                                    .map_err(|_| "invalid installment amount")?;
                                let cumulative = crate::business::decimal(entry, &["total"])
                                    .map_err(|_| "invalid installment total")?;
                                let time =
                                    entry["time"].as_i64().ok_or("invalid installment time")?;
                                total += amount;
                                if amount <= Decimal::ZERO
                                    || amount.scale() > 2
                                    || cumulative != total
                                    || time < previous
                                    || time > 4102444800000
                                {
                                    return Err("invalid installment history".into());
                                }
                                previous = time;
                            }
                            if total != money("amount", true)? {
                                return Err("installment history does not match paid amount".into());
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }
}
