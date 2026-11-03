use crate::{
    business::{fail, Error, Result},
    state::Store,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

mod estimate;
pub use estimate::{calculate, product_seed, product_shipping, ProductFreight};

const TABLE: &str = "freight_templates";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    address_code_text: String,
    from_area_code: String,
    id: i64,
    member_id: String,
    name: String,
    remark: String,
    status: i32,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    express_sub_template: Option<SubTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logistics_sub_template: Option<SubTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cod_sub_template: Option<SubTemplate>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubTemplate {
    #[serde(rename = "subTemplateDTO")]
    template: SubTemplateDetails,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rate_list: Option<Vec<Rate>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubTemplateDetails {
    charge_type: i32,
    is_sys_template: bool,
    service_charge_type: i32,
    service_type: i32,
    #[serde(rename = "type")]
    kind: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rate {
    is_sys_rate: bool,
    to_area_code_text: String,
    #[serde(rename = "rateDTO", default, skip_serializing_if = "Option::is_none")]
    ordinary: Option<RateTerms>,
    #[serde(
        rename = "sysRateDTO",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    system: Option<SystemRateTerms>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mock_pricing: Option<Pricing>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pricing {
    to_area_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_unit: Option<i64>,
    next_unit_fee: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateTerms {
    first_unit: i64,
    first_unit_fee: i64,
    least_expenses: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemRateTerms {
    #[serde(flatten)]
    terms: RateTerms,
    next_unit: i64,
}

impl RateTerms {
    fn valid(&self) -> bool {
        self.first_unit > 0 && self.first_unit_fee >= 0 && self.least_expenses >= 0
    }
}

impl SubTemplate {
    fn valid(&self, service_type: i32) -> bool {
        let details = &self.template;
        let mut areas = BTreeSet::new();
        (0..=2).contains(&details.charge_type)
            && (0..=1).contains(&details.service_charge_type)
            && details.service_type == service_type
            && (0..=1).contains(&details.kind)
            && self.rate_list.iter().flatten().all(|rate| {
                rate.is_sys_rate == details.is_sys_template
                    && match (rate.is_sys_rate, &rate.ordinary, &rate.system) {
                        (false, Some(terms), None) => terms.valid(),
                        (true, None, Some(system)) => system.terms.valid() && system.next_unit > 0,
                        _ => false,
                    }
                    && rate.mock_pricing.as_ref().is_none_or(|pricing| {
                        pricing.next_unit_fee >= 0
                            && if rate.is_sys_rate {
                                pricing.next_unit.is_none()
                            } else {
                                pricing.next_unit.is_some_and(|unit| unit > 0)
                            }
                            && if pricing.to_area_codes.is_empty() {
                                areas.insert("*")
                            } else {
                                pricing.to_area_codes.iter().all(|area| {
                                    estimate::area_code(area) && areas.insert(area.as_str())
                                })
                            }
                    })
            })
    }
}

pub fn validate(store: &Store) -> std::result::Result<(), String> {
    let mut ids = BTreeSet::new();
    for (key, value) in store.records.get(TABLE).into_iter().flat_map(|m| m.iter()) {
        let template: Template = serde_json::from_value(value.clone())
            .map_err(|error| format!("{TABLE}.{key}: {error}"))?;
        if template.id < 0
            || key != &format!("{}:{}", template.member_id, template.id)
            || !store.accounts.contains_key(&template.member_id)
            || !["", "2", "3"].contains(&template.kind.as_str())
            || (template.id > 1 && !ids.insert(template.id))
        {
            return Err(format!("{TABLE}.{key}: invalid identity or type"));
        }
        for (service_type, sub) in [
            &template.express_sub_template,
            &template.logistics_sub_template,
            &template.cod_sub_template,
        ]
        .into_iter()
        .enumerate()
        {
            if sub.as_ref().is_some_and(|sub| {
                !sub.valid(service_type as i32)
                    || (template.id == 1 && sub.template.service_charge_type != 0)
            }) {
                return Err(format!("{TABLE}.{key}: invalid subtemplate or rate"));
            }
        }
    }
    estimate::validate_products(store)
}

pub fn query(store: &Store, params: &Value, owner: &str) -> Result<Value> {
    let id = params["templateId"].as_i64();
    if id.is_some_and(|id| id < 0) {
        return fail("PARAM_ILLEGAL", "templateId must be nonnegative");
    }
    let account = id.is_none_or(|id| id <= 1).then_some(owner);
    let mut templates = store
        .rows(TABLE, account)
        .into_iter()
        .map(|row| {
            serde_json::from_value::<Template>(row).map_err(|error| Error {
                code: "SYSTEM_ERROR".into(),
                message: error.to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    templates.retain(|template| {
        id.is_none_or(|id| template.id == id)
            && account.is_none_or(|account| template.member_id == account)
    });
    templates.sort_by_key(|template| template.id);
    for template in &mut templates {
        for sub in [
            &mut template.express_sub_template,
            &mut template.logistics_sub_template,
            &mut template.cod_sub_template,
        ] {
            if params["querySubTemplate"] != true {
                *sub = None;
            } else if params["queryRate"] != true {
                if let Some(sub) = sub {
                    sub.rate_list = None;
                }
            }
            if let Some(sub) = sub {
                for rate in sub.rate_list.iter_mut().flatten() {
                    rate.mock_pricing = None;
                }
            }
        }
    }
    Ok(json!({"result": templates, "errorCode": "", "errorMsg": ""}))
}

fn sample(member_id: &str, id: i64, name: &str, remark: &str) -> Template {
    Template {
        address_code_text: "浙江省 杭州市 滨江区".into(),
        from_area_code: "330108".into(),
        id,
        member_id: member_id.into(),
        name: name.into(),
        remark: remark.into(),
        status: 1,
        kind: "2".into(),
        express_sub_template: None,
        logistics_sub_template: None,
        cod_sub_template: None,
    }
}

pub fn seed(store: &mut Store) {
    let mut templates = Vec::new();
    for member in store.accounts.keys() {
        templates.push(sample(member, 0, "模拟运费说明", "运费以卖家说明为准"));
        templates.push(sample(member, 1, "模拟包邮模板", "卖家承担运费"));
    }
    let mut ordinary = sample("seller-1", 10001, "模拟普通运费模板", "买家承担运费");
    for (service_type, sub) in [
        &mut ordinary.express_sub_template,
        &mut ordinary.logistics_sub_template,
        &mut ordinary.cod_sub_template,
    ]
    .into_iter()
    .enumerate()
    {
        let fee = [800, 500, 1200][service_type];
        *sub = Some(SubTemplate {
            template: SubTemplateDetails {
                charge_type: service_type as i32,
                is_sys_template: false,
                service_charge_type: 1,
                service_type: service_type as i32,
                kind: 0,
            },
            rate_list: Some(vec![Rate {
                is_sys_rate: false,
                to_area_code_text: "浙江省、江苏省、上海".into(),
                ordinary: Some(RateTerms {
                    first_unit: if service_type == 0 { 1000 } else { 1 },
                    first_unit_fee: fee,
                    least_expenses: fee,
                }),
                system: None,
                mock_pricing: Some(Pricing {
                    to_area_codes: vec!["330000".into(), "320000".into(), "310000".into()],
                    next_unit: Some(if service_type == 0 { 1000 } else { 1 }),
                    next_unit_fee: [200, 100, 500][service_type],
                }),
            }]),
        });
    }
    let mut official = sample("seller-1", 10002, "模拟官方运费模板", "官方物流");
    official.kind = "3".into();
    official.express_sub_template = Some(SubTemplate {
        template: SubTemplateDetails {
            charge_type: 0,
            is_sys_template: true,
            service_charge_type: 1,
            service_type: 0,
            kind: 0,
        },
        rate_list: Some(vec![Rate {
            is_sys_rate: true,
            to_area_code_text: "浙江省、江苏省、上海".into(),
            ordinary: None,
            system: Some(SystemRateTerms {
                terms: RateTerms {
                    first_unit: 1000,
                    first_unit_fee: 1000,
                    least_expenses: 1000,
                },
                next_unit: 500,
            }),
            mock_pricing: Some(Pricing {
                to_area_codes: vec!["330000".into(), "320000".into(), "310000".into()],
                next_unit: None,
                next_unit_fee: 200,
            }),
        }]),
    });
    templates.extend([ordinary, official]);
    for template in templates {
        store.put(
            TABLE,
            &format!("{}:{}", template.member_id, template.id),
            serde_json::to_value(template).expect("freight template seed"),
        );
    }
}
