use crate::{
    business::*,
    state::{Product, Store},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Merchandise {
    pub tags: Vec<String>,
    pub invoice_types: Vec<String>,
    pub private_buyers: Vec<String>,
    pub sale_start: Option<i64>,
    pub max_quantity: Option<Decimal>,
    pub quantity_scale: Option<u32>,
    pub trade_types: Vec<String>,
    pub mix_min_quantity: Option<Decimal>,
    pub mix_min_amount: Option<Decimal>,
    pub wholesale_minimum: Option<Decimal>,
    pub shipping: String,
    pub score: Decimal,
    pub response_rate: Decimal,
    pub dispute_score: Decimal,
    pub turnover_rate: Decimal,
    pub collection_rate24: Decimal,
    pub collection_rate48: Decimal,
    pub return_rate: Decimal,
    pub certified: bool,
    pub created: i64,
    pub modified: Option<i64>,
    pub collections: Vec<String>,
    pub regions: Vec<String>,
    pub navigation: Vec<String>,
    pub sales: BTreeMap<String, i64>,
    pub ratio: Decimal,
    pub old_buyer_ratio: Decimal,
    pub promotion_count: i64,
    pub buyer_count: i64,
    pub order_count: i64,
    pub rules: Vec<String>,
    pub pools: Vec<String>,
    pub translations: BTreeMap<String, String>,
}
impl Merchandise {
    pub fn sample(id: i64, now: i64) -> Self {
        Self {
            invoice_types: vec!["VATAX_COMM".into(), "VATAX_SPEC".into()],
            private_buyers: vec![],
            sale_start: None,
            max_quantity: None,
            quantity_scale: None,
            trade_types: vec![],
            mix_min_quantity: None,
            mix_min_amount: None,
            wholesale_minimum: None,
            tags: if id == 1001 {
                vec![
                    "noReason7DReturn",
                    "isOnePsale",
                    "isOnePsaleFreePostage",
                    "isSupportMix",
                    "1688_yx",
                    "isQqyx",
                    "select",
                    "JPFL",
                ]
            } else {
                vec!["isOnePsale", "USFL"]
            }
            .into_iter()
            .map(str::to_owned)
            .collect(),
            shipping: if id == 1001 {
                "shipIn24Hours"
            } else {
                "shipIn48Hours"
            }
            .into(),
            score: Decimal::new(if id == 1001 { 50 } else { 45 }, 1),
            response_rate: Decimal::new(98, 2),
            order_count: if id == 1001 { 120 } else { 60 },
            dispute_score: Decimal::new(48, 1),
            turnover_rate: Decimal::new(15, 2),
            collection_rate24: Decimal::from(if id == 1001 { 99 } else { 94 }),
            collection_rate48: Decimal::from(99),
            return_rate: Decimal::ZERO,
            certified: id == 1001,
            created: now - 86400000,
            modified: None,
            collections: vec![if id == 1001 { "262105288" } else { "262105286" }.into()],
            regions: vec![if id == 1001 { "JP" } else { "US" }.into()],
            navigation: vec!["978".into(), "978:1352".into()],
            sales: [
                ("sales7", 10),
                ("sales14", 20),
                ("sales30", 40),
                ("sales90", 120),
                ("totalSales", 100),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect(),
            ratio: Decimal::new(1, 2),
            old_buyer_ratio: Decimal::new(1, 2),
            promotion_count: 10,
            buyer_count: 5,
            rules: vec!["1".into(), "228169".into(), "357954".into()],
            pools: vec!["1".into()],
            translations: BTreeMap::new(),
        }
    }
    pub fn matches(&self, filter: &str, now: i64) -> Option<bool> {
        let d = Decimal::from;
        Some(match filter {
            "totalEpScoreLv1" => self.score == d(5),
            "totalEpScoreLv2" => self.score >= Decimal::new(45, 1) && self.score < d(5),
            "totalEpScoreLv3" => self.score >= d(4) && self.score < Decimal::new(45, 1),
            "totalEpScoreLv4" => self.score < d(4),
            "certifiedFactory" => self.certified,
            "getRate24HLv1" => self.collection_rate24 < d(95),
            "getRate24HLv2" => self.collection_rate24 >= d(95),
            "getRate24HLv3" => self.collection_rate24 >= d(99),
            "getRate48HLv1" => self.collection_rate48 < d(95),
            "getRate48HLv2" => self.collection_rate48 >= d(95),
            "getRate48HLv3" => self.collection_rate48 >= d(99),
            "shipInToday" | "shipIn24Hours" | "shipIn48Hours" => self.shipping == filter,
            "qrr10" => self.return_rate >= d(5) && self.return_rate <= d(10),
            "qrr5" => self.return_rate >= d(1) && self.return_rate < d(5),
            "qrr1" => self.return_rate > d(0) && self.return_rate < d(1),
            "qrr0" => self.return_rate == d(0),
            "new7" => now >= self.created && now - self.created <= 7 * 86400000,
            "new30" => now >= self.created && now - self.created <= 30 * 86400000,
            "hasAIMaterials" | "fxBrandOffer" | "ksCiphertext" => {
                self.tags.iter().any(|t| t == filter)
            }
            "YX_SCORE_LEVEL_1" => self.score == d(5),
            "YX_SCORE_LEVEL_2" => self.score >= d(4) && self.score < d(5),
            "isOnePsaleFreePost" => self.tags.iter().any(|t| t == "isOnePsaleFreePostage"),
            "1688Selection" => self.tags.iter().any(|t| t == "1688_yx"),
            "noReason7DReturn" | "isOnePsale" | "isQqyx" | "JPFL" | "USFL" | "KRFL" | "VNFL"
            | "SAFL" | "RUFL" | "KZFL" | "HKFL" | "MOFL" | "TWFL" => {
                self.tags.iter().any(|t| t == filter)
            }
            _ => return None,
        })
    }
}

pub fn language(p: &Value) -> Result<String> {
    let lang = text(p, &["country", "language"]);
    if !lang.is_empty()
        && ![
            "en", "ja", "ko", "ru", "vi", "fr", "pt", "zh-tw", "es", "id", "th", "zh",
        ]
        .contains(&lang.as_str())
    {
        return fail("PARAM_ILLEGAL", "unsupported language");
    }
    Ok(if lang.is_empty() { "en".into() } else { lang })
}

fn metric(product: &Product, key: &str) -> Option<Decimal> {
    let m = &product.merchandise;
    Some(match key {
        "price" => product.price,
        "saleQuantity" => Decimal::from(*m.sales.get("sales30").unwrap_or(&0)),
        "order1m" => product.price * Decimal::from(*m.sales.get("sales30").unwrap_or(&0)),
        "buyer1m" => Decimal::from(m.buyer_count),
        "tkCnt" => Decimal::from(m.promotion_count),
        "ratio" => m.ratio,
        "oldBuyerRatio" => m.old_buyer_ratio,
        "tkCommission" => product.price * m.ratio * Decimal::from(m.promotion_count),
        "yhqDiscountFee" => Decimal::ZERO,
        "ww30DResponseRate" => m.response_rate,
        "newTowerDisputeScore" => m.dispute_score,
        "order30DTurnoverRate" => m.turnover_rate,
        _ => return None,
    })
}

pub fn query(s: &Store, name: &str, p: &Value) -> Result<Value> {
    let lang = language(p)?;
    let image = text(p, &["imgBase64"]);
    if !image.is_empty() {
        let bytes = crate::catalog::decode_base64(&image).map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "invalid image base64".into(),
        })?;
        if bytes.is_empty() || bytes.len() > 3 * 1024 * 1024 {
            return fail("PARAM_ILLEGAL", "image must be between 1 byte and 3 MiB");
        }
    }
    let mut keyword = text(p, &["keyword", "keywords", "keyWord", "feedInfo"]);
    if find(p, &["keywordTranslate"]) != Some(&json!(true)) {
        keyword = match keyword.to_lowercase().as_str() {
            "t-shirt" | "cotton" => "棉".into(),
            "cup" => "杯".into(),
            _ => keyword,
        };
    }
    let seller = text(p, &["sellerId", "sellerMemberId"]);
    let mut categories = vec![];
    if let Some(id) = number(p, &["categoryId"]) {
        categories.push(id);
    }
    let category_list = text(p, &["categoryIdList"]);
    for part in category_list.split(',').filter(|v| !v.is_empty()) {
        categories.push(part.parse::<i64>().map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "invalid categoryIdList".into(),
        })?);
    }
    if let Some(ids) = array(p, &["categoryIds"]) {
        for id in ids {
            categories.push(
                id.as_i64()
                    .or_else(|| id.as_str().and_then(|v| v.parse().ok()))
                    .ok_or_else(|| Error {
                        code: "PARAM_ILLEGAL".into(),
                        message: "invalid categoryIds".into(),
                    })?,
            );
        }
    }
    if let Some(id) = number(p, &["cateId"]) {
        categories.push(id);
    }
    let filters = array(p, &["filter", "filters"])
        .map(|v| {
            v.iter()
                .map(|v| v.as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| text(p, &["filter"]));
    let pool = text(p, &["offerPoolId"]);
    if !pool.is_empty()
        && !s
            .products
            .values()
            .any(|v| v.merchandise.pools.contains(&pool))
    {
        return fail("POOL_ACCESS_DENIED", "unknown or unauthorized pool");
    }
    let rule = text(p, &["ruleId"]);
    let rules = array(p, &["ruleIds"]);
    let seed_offer = number(p, &["offerId"]);
    if name == "product.related.recommend"
        && seed_offer.is_some_and(|id| !s.products.contains_key(&id))
    {
        return fail("PRODUCT_NOT_EXISTS", "reference product not found");
    }
    let mut pickup_ranges = vec![];
    for key in ["filterOrder24HPickUpRate", "filterOrder48HPickUpRate"] {
        let value = text(p, &[key]);
        if !value.is_empty() {
            let parts: Vec<_> = value.split('-').collect();
            if parts.len() != 2 {
                return fail("PARAM_ILLEGAL", "pickup rate requires min-max");
            }
            let a: Decimal = parts[0].parse().map_err(|_| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "invalid pickup rate".into(),
            })?;
            let b: Decimal = parts[1].parse().map_err(|_| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "invalid pickup rate".into(),
            })?;
            if a < Decimal::ZERO || b > Decimal::ONE || a > b {
                return fail("PARAM_ILLEGAL", "pickup range outside 0..1");
            }
            pickup_ranges.push((key, a * Decimal::from(100), b * Decimal::from(100)));
        }
    }
    for filter in filters.split(',').filter(|v| !v.is_empty()) {
        if Merchandise::default().matches(filter, s.now()).is_none() {
            return fail("PARAM_ILLEGAL", "unknown search filter");
        }
    }
    let mut ranges = vec![];
    for (key, lo, hi) in [
        ("price", "priceStart", "priceEnd"),
        ("price", "filterMinPrice", "filterMaxPrice"),
        (
            "quantity",
            "filterQuantityBeginMin",
            "filterQuantityBeginMax",
        ),
        (
            "saleQuantity",
            "filterSaleQuantityMin",
            "filterSaleQuantityMax",
        ),
        ("ratio", "filterRatioMin", "filterRatioMax"),
        ("price", "minPrice", "maxPrice"),
    ] {
        let low = find(p, &[lo]).map(|_| decimal(p, &[lo])).transpose()?;
        let high = find(p, &[hi]).map(|_| decimal(p, &[hi])).transpose()?;
        if low.zip(high).is_some_and(|(a, b)| a > b) {
            return fail("PARAM_ILLEGAL", "range is reversed");
        }
        ranges.push((key.to_owned(), low, high));
    }
    let old = text(p, &["filterOldBuyerRatio"]);
    if !old.is_empty() {
        let (a, b) = old.split_once('~').ok_or_else(|| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "ratio range requires ~".into(),
        })?;
        let parse = |v: &str| -> Result<Option<Decimal>> {
            if v.is_empty() {
                Ok(None)
            } else {
                v.parse().map(Some).map_err(|_| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "invalid ratio".into(),
                })
            }
        };
        let (a, b) = (parse(a)?, parse(b)?);
        if a.zip(b).is_some_and(|(a, b)| a > b) {
            return fail("PARAM_ILLEGAL", "range is reversed");
        }
        ranges.push(("oldBuyerRatio".into(), a, b));
    }
    for sale in array(p, &["saleFilterList"]).into_iter().flatten() {
        let kind = text(sale, &["saleType"]);
        if !["sales7", "sales14", "sales30", "totalSales"].contains(&kind.as_str()) {
            return fail("PARAM_ILLEGAL", "invalid saleType");
        }
        let low = find(sale, &["saleStart"])
            .map(|_| decimal(sale, &["saleStart"]))
            .transpose()?;
        let high = find(sale, &["saleEnd"])
            .map(|_| decimal(sale, &["saleEnd"]))
            .transpose()?;
        if low.zip(high).is_some_and(|(a, b)| a > b) {
            return fail("PARAM_ILLEGAL", "sale range is reversed");
        }
        ranges.push((kind, low, high));
    }
    let mut sorts = vec![];
    let sort = text(p, &["sort"]);
    if !sort.is_empty() {
        let parsed: BTreeMap<String, String> = serde_json::from_str(&sort).map_err(|_| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "sort must be a JSON field/direction object".into(),
        })?;
        sorts.extend(parsed);
    }
    let sort_field = text(p, &["sortField", "sortByField"]);
    if !sort_field.is_empty() {
        if name.starts_with("alibaba.cps.") {
            for part in sort_field.split(';').filter(|v| !v.is_empty()) {
                let (key, dir) = part.split_once('^').ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "sortField requires field^direction".into(),
                })?;
                sorts.push((key.into(), dir.into()));
            }
        } else {
            let dir = text(p, &["sortType", "sortByType"]).to_lowercase();
            sorts.push((sort_field, if dir.is_empty() { "desc".into() } else { dir }));
        }
    }
    for (key, dir) in &sorts {
        if !["asc", "desc"].contains(&dir.as_str())
            || s.products
                .values()
                .next()
                .and_then(|v| metric(v, key))
                .is_none()
        {
            return fail("PARAM_ILLEGAL", "unknown sort field or direction");
        }
    }
    let collection = text(p, &["productCollectionId"]);
    let region = text(p, &["regionOpp"]);
    let navigation = text(p, &["snId"]);
    let tags = text(p, &["defineTags"]);
    let mut tag_rules = vec![];
    for tag in tags.split(',').filter(|v| !v.is_empty()) {
        let (key, val) = tag.split_once('=').ok_or_else(|| Error {
            code: "PARAM_ILLEGAL".into(),
            message: "defineTags requires key=true/false".into(),
        })?;
        if ![
            "slsj_flag",
            "sdrd_flag",
            "yjdf_flag",
            "jkhy_flag",
            "yhq_flag",
        ]
        .contains(&key)
            || !["true", "false"].contains(&val)
        {
            return fail("PARAM_ILLEGAL", "unknown defineTags value");
        }
        tag_rules.push((key, val == "true"));
    }
    let mut products: Vec<_> = s
        .products
        .values()
        .filter(|v| {
            let m = &v.merchandise;
            v.active
                && (pool.is_empty() || m.pools.contains(&pool))
                && (rule.is_empty() || m.rules.contains(&rule))
                && rules.is_none_or(|r| {
                    r.is_empty() || r.iter().any(|r| m.rules.iter().any(|v| r == v))
                })
                && number(p, &["quantityBegin"]).is_none_or(|n| v.minimum <= Decimal::from(n))
                && pickup_ranges.iter().all(|(key, a, b)| {
                    let rate = if *key == "filterOrder24HPickUpRate" {
                        m.collection_rate24
                    } else {
                        m.collection_rate48
                    };
                    rate >= *a && rate <= *b
                })
                && [
                    ("filterFreePostage", "isOnePsaleFreePostage"),
                    ("filterYjdf", "isOnePsale"),
                    ("filter7dNoReasonReturn", "noReason7DReturn"),
                    ("filterDyEncryptOrder", "dyCiphertext"),
                    ("filterKsEncryptOrder", "ksCiphertext"),
                ]
                .iter()
                .all(|(key, tag)| {
                    find(p, &[key]) != Some(&json!(true)) || m.tags.iter().any(|v| v == tag)
                })
                && (find(p, &["filter48hShip"]) != Some(&json!(true))
                    || ["shipInToday", "shipIn24Hours", "shipIn48Hours"]
                        .contains(&m.shipping.as_str()))
                && (find(p, &["filterCommission"]) != Some(&json!(true)) || m.ratio > Decimal::ZERO)
                && (keyword.is_empty()
                    || v.title.to_lowercase().contains(&keyword.to_lowercase())
                    || v.id.to_string() == keyword
                    || m.translations
                        .values()
                        .any(|t| t.to_lowercase().contains(&keyword.to_lowercase())))
                && (seller.is_empty()
                    || v.seller == seller
                    || s.accounts[&v.seller].user_id.to_string() == seller)
                && (categories.is_empty() || categories.contains(&v.category))
                && filters
                    .split(',')
                    .filter(|v| !v.is_empty())
                    .all(|f| m.matches(f, s.now()) == Some(true))
                && (collection.is_empty() || m.collections.contains(&collection))
                && (region.is_empty() || m.regions.contains(&region))
                && (navigation.is_empty() || m.navigation.contains(&navigation))
                && ranges.iter().all(|(key, lo, hi)| {
                    let n = if key == "quantity" {
                        v.minimum
                    } else {
                        m.sales
                            .get(key)
                            .map(|v| Decimal::from(*v))
                            .or_else(|| metric(v, key))
                            .unwrap_or_default()
                    };
                    lo.is_none_or(|x| n >= x) && hi.is_none_or(|x| n <= x)
                })
                && tag_rules.iter().all(|(key, val)| {
                    let actual = match *key {
                        "slsj_flag" | "sdrd_flag" => m.certified,
                        "yjdf_flag" => m.tags.iter().any(|t| t == "isOnePsale"),
                        _ => false,
                    };
                    actual == *val
                })
        })
        .collect();
    products.sort_by(|a, b| {
        for (key, dir) in &sorts {
            let cmp = metric(a, key).cmp(&metric(b, key));
            if !cmp.is_eq() {
                return if dir == "desc" { cmp.reverse() } else { cmp };
            }
        }
        a.id.cmp(&b.id)
    });
    if name == "alibaba.pifatuan.product.list" && number(p, &["pageNo"]).unwrap_or(1) == 1 {
        if let Some(top) = array(p, &["topOfferIds"]) {
            if top.len() > number(p, &["pageSize"]).unwrap_or(20) as usize {
                return fail("PARAM_ILLEGAL", "topOfferIds exceeds pageSize");
            }
            products.sort_by_key(|v| {
                top.iter()
                    .position(|id| id.as_i64() == Some(v.id))
                    .unwrap_or(usize::MAX)
            });
        }
    }
    let rows = products
        .into_iter()
        .map(|v| {
            let mut out = product_value(v, &s.base_url, &s.accounts[&v.seller], &s.current_app);
            if name == "alibaba.product.suggest.crossBorder" {
                out["bookedCount"] = json!(v.merchandise.order_count);
            }
            out["language"] = json!(lang);
            out["subjectTrans"] = json!(v.merchandise.translations.get(&lang).unwrap_or(&v.title));
            out
        })
        .collect();
    page(rows, p)
}
