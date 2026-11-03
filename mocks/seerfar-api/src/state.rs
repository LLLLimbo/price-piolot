use crate::{contract, tasks};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type Records = BTreeMap<String, Value>;
pub const MAX_TIME: i64 = 253_370_764_799_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub name: String,
    pub active: bool,
    pub terms_accepted: bool,
    pub ozon_authorized: bool,
    pub partner_type: u8,
    pub sub_accounts: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub account: String,
    pub active: bool,
    pub credit_limit: u64,
    pub credit_used: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub cost: u64,
    pub limit: usize,
    pub window_ms: i64,
    pub account_limit: usize,
    pub global_limit: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub account: String,
    pub key: String,
    pub sku: String,
    pub kind: String,
    pub status: String,
    pub due: i64,
    pub cost: u64,
    pub charged: bool,
    pub refunded: bool,
    pub reason: Option<i32>,
    pub collection_id: Option<String>,
    pub settlement_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub account: String,
    pub creator: String,
    pub sku: String,
    pub status: u8,
    pub fail_type: Option<String>,
    pub request_id: String,
    pub create_time: i64,
    pub expire_at: i64,
    pub data: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub id: String,
    pub account: String,
    pub key: String,
    pub skus: Vec<String>,
    pub request_skus: Vec<String>,
    pub unit_cost: u64,
    pub total_cost: u64,
    pub refunded: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    pub base_url: String,
    pub fixed_time: Option<i64>,
    pub generation: u64,
    pub sequence: u64,
    pub task_delay_ms: i64,
    pub accounts: BTreeMap<String, Account>,
    pub keys: BTreeMap<String, Key>,
    pub policies: BTreeMap<String, Policy>,
    pub products: Records,
    pub shops: Records,
    pub categories: Records,
    pub market: BTreeMap<String, Vec<Value>>,
    pub history: BTreeMap<String, Vec<Value>>,
    pub monthly: BTreeMap<String, Vec<Value>>,
    pub review_results: Records,
    pub tasks: BTreeMap<String, Task>,
    pub collections: BTreeMap<String, Collection>,
    pub settlements: BTreeMap<String, Settlement>,
    pub faults: Vec<Value>,
    #[serde(skip)]
    pub rate_windows: BTreeMap<String, Vec<i64>>,
}
impl Store {
    pub fn now(&self) -> i64 {
        self.fixed_time
            .unwrap_or_else(|| Utc::now().timestamp_millis())
    }
    pub fn next_id(&mut self, prefix: &str) -> String {
        self.sequence += 1;
        format!("{prefix}-{:020}", self.sequence)
    }
    pub fn seed(base_url: String, fixed_time: Option<i64>) -> Self {
        let mut s: Self = serde_json::from_str(include_str!("../fixtures/seed.json")).unwrap();
        s.base_url = base_url;
        s.fixed_time = fixed_time;
        let now = s.now();
        for p in s.products.values_mut() {
            p["imageUrl"] = json!(format!("{}/assets/product.svg", s.base_url));
            p["imageUrls"] = json!([p["imageUrl"]]);
        }
        for result in s.review_results.values_mut() {
            result["statistic"]["imageUrl"] = json!(format!("{}/assets/product.svg", s.base_url));
        }
        for rows in s.market.values_mut() {
            for row in rows {
                row["_date"] = json!(date(now));
                if row.get("queryTime").is_some() {
                    row["queryTime"] = json!(date(now));
                }
            }
        }
        // Histories follow the simulated clock; historical monthly fixtures remain explicit.
        for rows in s.history.values_mut() {
            for (i, row) in rows.iter_mut().enumerate() {
                row["date"] = json!(date(now - (29 - i as i64) * 86_400_000));
            }
        }
        s
    }
    pub fn tick(&mut self) {
        tasks::tick(self);
    }
    pub fn validate(&self) -> Result<(), String> {
        let bad = || "invalid snapshot or action".to_string();
        if self
            .fixed_time
            .is_some_and(|v| !(0..=MAX_TIME - 30 * 86_400_000).contains(&v))
            || !(1..=60_000).contains(&self.task_delay_ms)
            || self.sequence > u64::MAX - 1_000_000
            || self
                .base_url
                .parse::<axum::http::Uri>()
                .ok()
                .is_none_or(|u| {
                    !matches!(u.scheme_str(), Some("http" | "https")) || u.authority().is_none()
                })
        {
            return Err(bad());
        }
        for key in self.keys.values() {
            if !self.accounts.contains_key(&key.account) || key.credit_used > key.credit_limit {
                return Err(bad());
            }
        }
        if self
            .accounts
            .keys()
            .any(|id| id.parse::<u64>().ok().is_none_or(|n| n == 0))
        {
            return Err(bad());
        }
        for (token, key) in &self.keys {
            let pending = self
                .tasks
                .values()
                .filter(|t| {
                    t.key == *token
                        && t.charged
                        && !t.refunded
                        && !["success", "failed"].contains(&t.status.as_str())
                })
                .try_fold(0u64, |n, t| n.checked_add(t.cost));
            if pending.is_none_or(|n| n > key.credit_used) {
                return Err(bad());
            }
        }
        for e in contract::endpoints() {
            let p = self
                .policies
                .get(e["id"].as_str().unwrap())
                .ok_or_else(bad)?;
            if !(1..=86_400_000).contains(&p.window_ms) || p.cost > 1_000_000 {
                return Err(bad());
            }
        }
        for (sku, p) in &self.products {
            if p["sku"]
                .as_u64()
                .is_none_or(|v| v == 0 || v.to_string() != *sku)
                || !p["title"].is_string()
                || !p["price"].is_number()
                || p["price"].as_f64().is_none_or(|v| v < 0.0)
                || ["stock", "sales", "reviewCount"]
                    .iter()
                    .any(|k| !p[*k].is_null() && p[*k].as_u64().is_none())
                || (!p["revenue"].is_null() && p["revenue"].as_f64().is_none_or(|v| v < 0.0))
            {
                return Err(bad());
            }
        }
        for records in [&self.shops, &self.categories] {
            for (id, record) in records {
                if !record.is_object() || crate::query::identity(&record["id"]) != *id {
                    return Err(bad());
                }
            }
        }
        for (sku, rows) in &self.history {
            if !self.products.contains_key(sku)
                || rows.iter().any(|r| {
                    r["date"]
                        .as_str()
                        .is_none_or(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err())
                        || !r["sales"].is_number()
                        || !r["revenue"].is_number()
                })
            {
                return Err(bad());
            }
        }
        for (sku, rows) in &self.monthly {
            if !self.products.contains_key(sku)
                || rows.iter().any(|r| {
                    r["date"].as_str().is_none_or(|v| {
                        chrono::NaiveDate::parse_from_str(&format!("{v}-01"), "%Y-%m-%d").is_err()
                    })
                })
            {
                return Err(bad());
            }
        }
        for (id, t) in &self.tasks {
            if *id != t.id
                || !self.accounts.contains_key(&t.account)
                || self.keys.get(&t.key).is_none_or(|k| k.account != t.account)
                || !["collection", "review"].contains(&t.kind.as_str())
                || ![
                    "collecting",
                    "getting_reviews",
                    "analyzing",
                    "success",
                    "failed",
                ]
                .contains(&t.status.as_str())
                || !(0..=MAX_TIME).contains(&t.due)
                || (t.refunded && (t.status != "failed" || !t.charged))
                || (t.kind == "collection"
                    && (t.collection_id.is_none()
                        || t.settlement_id.is_some()
                        || !["collecting", "success", "failed"].contains(&t.status.as_str())))
                || (t.kind == "review"
                    && (t.settlement_id.is_none()
                        || t.collection_id.is_some()
                        || t.status == "collecting"))
                || t.collection_id.as_ref().is_some_and(|id| {
                    self.collections
                        .get(id)
                        .is_none_or(|c| c.account != t.account || c.sku != t.sku)
                })
                || t.settlement_id.as_ref().is_some_and(|id| {
                    self.settlements.get(id).is_none_or(|c| {
                        c.account != t.account || c.key != t.key || !c.skus.contains(&t.sku)
                    })
                })
            {
                return Err(bad());
            }
        }
        for (id, c) in &self.collections {
            if *id != c.id
                || !self.accounts.contains_key(&c.account)
                || c.status > 2
                || c.sku.parse::<u64>().ok().is_none_or(|v| v == 0)
                || !c.data.is_object()
                || c.create_time < 0
                || c.expire_at < c.create_time
                || c.expire_at > MAX_TIME
            {
                return Err(bad());
            }
        }
        for (id, c) in &self.settlements {
            if *id != c.id
                || self.keys.get(&c.key).is_none_or(|k| k.account != c.account)
                || c.refunded > c.total_cost
                || c.unit_cost.checked_mul(c.skus.len() as u64) != Some(c.total_cost)
                || c.skus.iter().any(|v| !c.request_skus.contains(v))
            {
                return Err(bad());
            }
        }
        for (kind, rows) in &self.market {
            if ![
                "market-keyword-search",
                "keyword-mining",
                "keyword-back-search",
            ]
            .contains(&kind.as_str())
                || rows
                    .iter()
                    .any(|r| !r.is_object() || !r["id"].is_string() || !r["query"].is_string())
            {
                return Err(bad());
            }
        }
        for (sku, result) in &self.review_results {
            if !self.products.contains_key(sku) || !result.is_object() {
                return Err(bad());
            }
            for key in ["CN", "EN", "RU", "statistic", "original"] {
                if result
                    .get(key)
                    .is_some_and(|v| !v.is_object() && !v.is_null())
                {
                    return Err(bad());
                }
            }
        }
        let max_id = self
            .tasks
            .keys()
            .chain(self.collections.keys())
            .chain(self.settlements.keys())
            .filter_map(|v| v.rsplit('-').next()?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        if self.sequence < max_id {
            return Err(bad());
        }
        crate::admin::validate_faults(&self.faults)?;
        Ok(())
    }
}
pub fn date(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}
