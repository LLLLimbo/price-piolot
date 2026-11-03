use super::{Template, TABLE};
use crate::{
    business::{fail, Error, Result},
    state::{Product, Store},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAX_QUANTITY: i64 = 1_000_000_000;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductFreight {
    template_id: i64,
    service_type: i32,
    package: Package,
    #[serde(default)]
    skus: BTreeMap<String, Package>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Package {
    weight: Decimal,
    width: Decimal,
    height: Decimal,
    length: Decimal,
}

impl Default for Package {
    fn default() -> Self {
        Self {
            weight: Decimal::new(2, 1),
            width: Decimal::from(10),
            height: Decimal::from(5),
            length: Decimal::from(20),
        }
    }
}

impl Default for ProductFreight {
    fn default() -> Self {
        Self {
            template_id: 1,
            service_type: 0,
            package: Package::default(),
            skus: BTreeMap::new(),
        }
    }
}

impl Package {
    fn measure(&self, charge: i32) -> Option<Decimal> {
        match charge {
            0 => self.weight.checked_mul(Decimal::from(1000)),
            1 => Some(Decimal::ONE),
            2 => self
                .length
                .checked_mul(self.width)?
                .checked_mul(self.height)?
                .checked_div(Decimal::from(1_000_000)),
            _ => None,
        }
    }

    fn valid(&self) -> bool {
        [self.weight, self.width, self.height, self.length]
            .iter()
            .all(|value| *value > Decimal::ZERO)
            && [0, 2].iter().all(|charge| {
                self.measure(*charge)
                    .is_some_and(|value| value > Decimal::ZERO)
            })
    }

    fn shipping(&self, sku_id: i64, spec_id: &str) -> Value {
        json!({"skuId":sku_id,"specId":spec_id,"weight":self.weight,
            "width":self.width,"height":self.height,"length":self.length,
            "pkgSizeSource":"mock_fixture","officialLength":self.length,
            "officialWidth":self.width,"officialHeight":self.height,
            "officialWeight":self.weight,"aiWeight":self.weight,"aiWeightAccuracy":"fixture"})
    }
}

impl ProductFreight {
    fn package(&self, sku_id: i64) -> &Package {
        self.skus.get(&sku_id.to_string()).unwrap_or(&self.package)
    }

    fn sku_id(&self, product: &Product, value: &str) -> Option<i64> {
        if value == product.sku || value == product.id.to_string() {
            return Some(product.id);
        }
        value
            .parse::<i64>()
            .ok()
            .filter(|id| value == id.to_string() && self.skus.contains_key(value))
    }

    fn template(&self, store: &Store, product: &Product) -> Result<Template> {
        let value = store
            .records
            .get(TABLE)
            .and_then(|rows| rows.get(&format!("{}:{}", product.seller, self.template_id)))
            .ok_or_else(unavailable)?;
        let template: Template =
            serde_json::from_value(value.clone()).map_err(|_| unavailable())?;
        if template.member_id != product.seller || template.id != self.template_id {
            return Err(unavailable());
        }
        Ok(template)
    }
}

impl Template {
    fn sub(&self, service: i32) -> Option<&super::SubTemplate> {
        match service {
            0 => self.express_sub_template.as_ref(),
            1 => self.logistics_sub_template.as_ref(),
            2 => self.cod_sub_template.as_ref(),
            _ => None,
        }
    }
}

fn unavailable() -> Error {
    Error {
        code: "FREIGHT_NOT_AVAILABLE".into(),
        message: "freight template, service or destination has no calculable rate".into(),
    }
}

fn arithmetic(value: Option<Decimal>) -> Result<Decimal> {
    value.ok_or_else(|| Error {
        code: "PARAM_ILLEGAL".into(),
        message: "freight calculation exceeds decimal range".into(),
    })
}

pub(super) fn area_code(value: &str) -> bool {
    value.len() == 6 && value.bytes().all(|b| b.is_ascii_digit()) && !value.starts_with("00")
}

fn destination(params: &Value) -> Result<[&str; 3]> {
    let codes = ["toProvinceCode", "toCityCode", "toCountryCode"]
        .map(|key| params[key].as_str().unwrap_or(""));
    if !codes.iter().all(|code| area_code(code))
        || !codes[0].ends_with("0000")
        || !codes[1].ends_with("00")
        || codes[0][..2] != codes[1][..2]
        || codes[1][..4] != codes[2][..4]
    {
        return fail(
            "PARAM_ILLEGAL",
            "consistent six-digit province, city and district codes required",
        );
    }
    Ok(codes)
}

pub(super) fn validate_products(store: &Store) -> std::result::Result<(), String> {
    for product in store.products.values() {
        let Some(config) = &product.freight else {
            continue;
        };
        if config.template_id < 0
            || !(0..=2).contains(&config.service_type)
            || !config.package.valid()
            || config.skus.iter().any(|(id, package)| {
                id.parse::<i64>().ok().is_none_or(|value| {
                    value <= 0
                        || value.to_string() != *id
                        || (id == &product.sku && value != product.id)
                }) || !package.valid()
            })
        {
            return Err(format!(
                "product {}: invalid freight configuration",
                product.id
            ));
        }
        let template = config
            .template(store, product)
            .map_err(|error| error.message)?;
        if config.template_id > 1 && template.sub(config.service_type).is_none() {
            return Err(format!(
                "product {}: freight subtemplate does not exist",
                product.id
            ));
        }
    }
    Ok(())
}

pub fn product_seed(id: i64) -> Option<ProductFreight> {
    match id {
        1001 => Some(ProductFreight::default()),
        1002 => Some(ProductFreight {
            template_id: 10001,
            skus: BTreeMap::from([
                ("1002".into(), Package::default()),
                (
                    "2002".into(),
                    Package {
                        weight: Decimal::new(8, 1),
                        ..Package::default()
                    },
                ),
            ]),
            ..ProductFreight::default()
        }),
        _ => None,
    }
}

pub fn product_shipping(product: &Product) -> Value {
    let config = product.freight.clone().unwrap_or_default();
    let base = &config.package;
    let mut packages: BTreeMap<_, _> = config
        .skus
        .iter()
        .map(|(id, package)| (id.parse::<i64>().expect("validated freight SKU"), package))
        .collect();
    packages.entry(product.id).or_insert(base);
    let mut details = Vec::new();
    let mut grams = Vec::new();
    for (id, package) in packages {
        let spec = if id == product.id {
            product.sku.clone()
        } else {
            id.to_string()
        };
        let detail = package.shipping(id, &spec);
        let mut gram = detail.clone();
        gram["weight"] = json!(package.measure(0).expect("validated package weight"));
        details.push(detail);
        grams.push(gram);
    }
    json!({"sendGoodsAddressText":"浙江省杭州市滨江区",
        "shippingTimeGuarantee":product.merchandise.shipping,
        "freightTemplateID":config.template_id,"unitWeight":base.weight,
        "weight":base.weight,"width":base.width,"height":base.height,"length":base.length,
        "officialWeight":base.weight,"officialWidth":base.width,
        "officialHeight":base.height,"officialLength":base.length,"pkgSizeSource":"mock_fixture",
        "skuShippingInfoList":grams,"skuShippingDetails":details})
}

pub fn calculate(store: &Store, input: &Value) -> Result<Value> {
    let params = &input["productFreightQueryParamsNew"];
    let id = params["offerId"].as_i64().unwrap_or(0);
    let product = store
        .products
        .get(&id)
        .filter(|product| product.active)
        .ok_or_else(|| Error {
            code: "PRODUCT_NOT_EXISTS".into(),
            message: "product not found".into(),
        })?;
    let config = product.freight.clone().unwrap_or_default();
    let codes = destination(params)?;
    let total = params["totalNum"].as_i64().unwrap_or(0);
    let Some(rows) = params["logisticsSkuNumModels"]
        .as_array()
        .filter(|rows| !rows.is_empty())
    else {
        return fail("PARAM_ILLEGAL", "nonempty SKU quantities required");
    };
    if !(1..=MAX_QUANTITY).contains(&total) {
        return fail(
            "PARAM_ILLEGAL",
            "totalNum must be positive and at most one billion",
        );
    }
    let mut quantities = BTreeMap::<i64, i64>::new();
    let mut count = 0_i64;
    for row in rows {
        let sku_id = config
            .sku_id(product, row["skuId"].as_str().unwrap_or(""))
            .ok_or_else(|| Error {
                code: "SKU_NOT_EXISTS".into(),
                message: "unknown skuId".into(),
            })?;
        let quantity = row["number"].as_i64().unwrap_or(0);
        if !(1..=MAX_QUANTITY).contains(&quantity) || count > MAX_QUANTITY - quantity {
            return fail(
                "PARAM_ILLEGAL",
                "SKU quantities must be positive and total at most one billion",
            );
        }
        count += quantity;
        *quantities.entry(sku_id).or_default() += quantity;
    }
    if count != total {
        return fail("PARAM_ILLEGAL", "SKU quantities must equal totalNum");
    }
    let base = &config.package;
    let sku_info: Vec<_> = quantities.keys().map(|sku| {
        let package = config.package(*sku);
        json!({"skuId":sku.to_string(),"singleSkuWeight":package.weight,
            "singleSkuWidth":package.width,"singleSkuHeight":package.height,"singleSkuLength":package.length})
    }).collect();
    let mut quote = json!({"success":true,"code":"S0000","message":"成功",
        "offerId":id,"freight":"0.00","freePostage":true,
        "templateId":config.template_id,"templateType":1,"templateName":"模拟包邮模板",
        "subTemplateType":config.service_type,"subTemplateName":service_name(config.service_type),
        "singleProductWeight":base.weight,"singleProductWidth":base.width,
        "singleProductHeight":base.height,"singleProductLength":base.length,
        "productFreightSkuInfoModels":sku_info,"sizeValueType":2,
        "firstFee":"0.00","firstUnit":"1","nextFee":"0.00","nextUnit":"1",
        "chargeType":"1","discount":"1"});
    let template = if product.freight.is_none() {
        config.template(store, product).ok()
    } else {
        Some(config.template(store, product)?)
    };
    let Some(template) = template else {
        return Ok(quote);
    };
    quote["templateName"] = json!(template.name);
    if config.template_id == 1 {
        return Ok(quote);
    }
    if config.template_id == 0 {
        return Err(unavailable());
    }
    quote["templateType"] = json!(if template.kind == "3" { 3 } else { 2 });
    let sub = template.sub(config.service_type).ok_or_else(unavailable)?;
    let service = if template.kind == "3" {
        3
    } else {
        config.service_type
    };
    quote["subTemplateType"] = json!(service);
    quote["subTemplateName"] = json!(service_name(service));
    let charge = sub.template.charge_type;
    quote["chargeType"] = json!(charge.to_string());
    if sub.template.service_charge_type == 0 {
        return Ok(quote);
    }
    let rate = sub
        .rate_list
        .iter()
        .flatten()
        .filter_map(|rate| {
            let pricing = rate.mock_pricing.as_ref()?;
            let rank = if pricing.to_area_codes.is_empty() {
                Some(0)
            } else {
                codes
                    .iter()
                    .enumerate()
                    .filter(|(_, code)| pricing.to_area_codes.iter().any(|area| area == **code))
                    .map(|(index, _)| index + 1)
                    .max()
            }?;
            Some((rank, rate))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, rate)| rate)
        .ok_or_else(unavailable)?;
    let pricing = rate.mock_pricing.as_ref().ok_or_else(unavailable)?;
    let (terms, next) = if let Some(system) = &rate.system {
        (&system.terms, system.next_unit)
    } else {
        (
            rate.ordinary.as_ref().ok_or_else(unavailable)?,
            pricing.next_unit.ok_or_else(unavailable)?,
        )
    };
    let mut measure = Decimal::ZERO;
    for (sku, quantity) in quantities {
        let units = arithmetic(config.package(sku).measure(charge))?;
        measure = arithmetic(
            measure.checked_add(arithmetic(units.checked_mul(Decimal::from(quantity)))?),
        )?;
    }
    let first = Decimal::from(terms.first_unit);
    let next = Decimal::from(next);
    let excess = arithmetic(measure.checked_sub(first))?.max(Decimal::ZERO);
    let remainder = arithmetic(excess.checked_rem(next))?;
    let whole = arithmetic(arithmetic(excess.checked_sub(remainder))?.checked_div(next))?;
    let steps = arithmetic(whole.checked_add(if remainder.is_zero() {
        Decimal::ZERO
    } else {
        Decimal::ONE
    }))?;
    let fee = arithmetic(
        arithmetic(steps.checked_mul(Decimal::from(pricing.next_unit_fee)))?
            .checked_add(Decimal::from(terms.first_unit_fee)),
    )?
    .max(Decimal::from(terms.least_expenses));
    let divisor = if charge == 0 {
        Decimal::from(1000)
    } else {
        Decimal::ONE
    };
    quote["freight"] = json!(money(fee)?);
    quote["freePostage"] = json!(fee.is_zero());
    quote["firstFee"] = json!(money(Decimal::from(terms.first_unit_fee))?);
    quote["nextFee"] = json!(money(Decimal::from(pricing.next_unit_fee))?);
    quote["firstUnit"] = json!(arithmetic(first.checked_div(divisor))?
        .normalize()
        .to_string());
    quote["nextUnit"] = json!(arithmetic(next.checked_div(divisor))?
        .normalize()
        .to_string());
    Ok(quote)
}

fn money(cents: Decimal) -> Result<String> {
    Ok(format!(
        "{:.2}",
        arithmetic(cents.checked_div(Decimal::from(100)))?
    ))
}

fn service_name(service: i32) -> &'static str {
    match service {
        0 => "模拟快递",
        1 => "模拟货运",
        2 => "模拟货到付款",
        _ => "模拟官方物流",
    }
}
