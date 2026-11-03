use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub struct Contracts {
    pub apis: BTreeMap<String, Value>,
    pub messages: BTreeMap<String, Value>,
}
impl Contracts {
    pub fn load() -> Self {
        Self {
            apis: serde_json::from_str(include_str!("../contracts/apis.json"))
                .expect("frozen API contracts"),
            messages: serde_json::from_str(include_str!("../contracts/messages.json"))
                .expect("frozen message contracts"),
        }
    }
}

pub fn decode_parameters(
    contract: &Value,
    input: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let dates = crate::dates::Options::new(
        input.get("_aop_datePattern").map(String::as_str),
        input.get("_aop_timeZone").map(String::as_str),
    )?;
    let mut result = Map::new();
    for (key, value) in input {
        result.insert(key.clone(), Value::String(value.clone()));
    }
    for field in contract["detail"]["apiAppParamVOList"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = field["name"].as_str().unwrap_or_default();
        let raw = input.get(name);
        if raw.is_none() {
            if field["required"] == true {
                return Err(format!("{name} is required"));
            }
            continue;
        }
        let raw = raw.unwrap();
        let kind = field["type"]
            .as_str()
            .unwrap_or("String")
            .trim_start_matches("java.lang.")
            .trim_start_matches("java.math.");
        let value = if ["String", "Date", "java.util.Date"].contains(&kind)
            || (kind == "byte[]" && !raw.trim_start().starts_with('['))
        {
            Value::String(raw.clone())
        } else {
            serde_json::from_str(raw).map_err(|_| format!("{name}: invalid {kind}"))?
        };
        validate_value(contract, field, &value, name, 0, &dates)?;
        result.insert(name.into(), value);
    }
    let result = Value::Object(result);
    crate::rules::validate(
        contract["catalogue"]["name"].as_str().unwrap_or(""),
        &result,
    )?;
    Ok(result)
}

fn validate_value(
    contract: &Value,
    field: &Value,
    value: &Value,
    path: &str,
    depth: usize,
    dates: &crate::dates::Options,
) -> Result<(), String> {
    if depth > 32 {
        return Err(format!("{path}: nesting limit"));
    }
    let kind = field["type"]
        .as_str()
        .unwrap_or("String")
        .trim_start_matches("java.lang.")
        .trim_start_matches("java.math.");
    if value.is_null() {
        return if field["required"] == true {
            Err(format!("{path} cannot be null"))
        } else {
            Ok(())
        };
    }
    if kind == "byte[]" && value.is_string() {
        return Ok(());
    }
    if kind.ends_with("[]") {
        let values = value
            .as_array()
            .ok_or_else(|| format!("{path}: expected array"))?;
        let mut element = field.clone();
        element["type"] = json!(kind.trim_end_matches("[]"));
        if let Some(t) = field["typeName"].as_str() {
            element["typeName"] = json!(t.trim_end_matches("[]"));
        }
        for (i, v) in values.iter().enumerate() {
            validate_value(
                contract,
                &element,
                v,
                &format!("{path}[{i}]"),
                depth + 1,
                dates,
            )?;
        }
    } else if field["complexTypeFlag"] == true || kind.starts_with("message:") {
        if !value.is_object() {
            return Err(format!("{path}: expected object"));
        }
        let model = field["typeName"]
            .as_str()
            .unwrap_or(kind.trim_start_matches("message:"));
        if model == "alibaba.trade.fast.address" {
            if let Some(fields) = contract["models"][format!("1:{model}")]["fields"].as_array() {
                for child in fields {
                    let name = child["name"].as_str().unwrap_or_default();
                    if let Some(v) = value.get(name) {
                        validate_value(
                            contract,
                            child,
                            v,
                            &format!("{path}.{name}"),
                            depth + 1,
                            dates,
                        )?;
                    }
                }
            }
            if value.get("addressId").is_some() {
                return Ok(());
            }
            for required in ["fullName", "address"] {
                if value
                    .get(required)
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    return Err(format!("{path}.{required} is required"));
                }
            }
            if value
                .get("mobile")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                && value
                    .get("phone")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(format!("{path}: mobile or phone required"));
            }
            return Ok(());
        }
        if let Some(fields) = contract["models"][format!("1:{model}")]["fields"].as_array() {
            for child in fields {
                let name = child["name"].as_str().unwrap_or_default();
                let conditional_optional = (model.ends_with("ApiContractPayTerm")
                    && name == "payTimeContent"
                    && value["payTimeType"] == "IMMEDIATELY")
                    || (model.ends_with("ApiContractRoleInfoTerm")
                        && [
                            "counterpartyName",
                            "counterpartyLicenseNo",
                            "counterpartyLicenseType",
                        ]
                        .contains(&name)
                        && value["counterpartyOrigin"] == "LOGIN_ID_1688_MATCH")
                    || (model.ends_with("ApiContractConfirmTerm")
                        && name == "autoConfirmCondition"
                        && value["contractConfirmType"] == "BUYER_MANUAL_CONFIRM");
                if conditional_optional && value.get(name).is_none_or(Value::is_null) {
                    continue;
                }
                match value.get(name) {
                    None if child["required"] == true
                        && !(name == "specId"
                            && contract["catalogue"]["name"] == "alibaba.createOrder.preview")
                        && !(model.ends_with("ContractDraftAndSignApplyApiRequest")
                            && name == "draftNo") =>
                    {
                        return Err(format!("{path}.{name} is required"))
                    }
                    Some(v) => validate_value(
                        contract,
                        child,
                        v,
                        &format!("{path}.{name}"),
                        depth + 1,
                        dates,
                    )?,
                    _ => (),
                }
            }
        }
    } else {
        let valid = match kind.to_lowercase().as_str() {
            "string" => value.is_string(),
            "date" | "java.util.date" => value.as_str().is_some_and(|v| dates.parse(v).is_ok()),
            "boolean" | "bool" => value.is_boolean(),
            "long" | "integer" | "int" | "short" => value.as_i64().is_some(),
            "double" | "float" | "bigdecimal" | "java.math.bigdecimal" => value.is_number(),
            "map" | "java.util.map" | "object" => value.is_object(),
            "java.util.list" | "list" => value.is_array(),
            _ => true,
        };
        if !valid {
            return Err(format!("{path}: expected {kind}"));
        }
    }
    Ok(())
}

/// Shape domain results to each endpoint's documented envelope and scalar types.
pub fn response(contract: &Value, data: &Value, input: &Value) -> Value {
    if contract["catalogue"]["name"] == "alibaba.logistics.myFreightTemplate.list.get" {
        return data.clone();
    }
    let dates = crate::dates::Options::from_value(input).expect("validated date options");
    let Some(fields) = contract["detail"]["apiReturnParamVOList"].as_array() else {
        return data.clone();
    };
    project(fields, contract, data, 0, &dates)
}
fn project(
    fields: &[Value],
    contract: &Value,
    data: &Value,
    depth: usize,
    dates: &crate::dates::Options,
) -> Value {
    let mut object = Map::new();
    for field in fields {
        let name = field["name"].as_str().unwrap_or_default();

        if contract["catalogue"]["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("newtoncloud.task."))
            && [
                "queuePosition",
                "queueLength",
                "queuedTotal",
                "runningTotal",
                "finishedTotal",
            ]
            .contains(&name)
            && data["status"] != "QUEUED"
        {
            continue;
        }
        let kind = field["type"]
            .as_str()
            .unwrap_or("String")
            .trim_start_matches("java.lang.")
            .trim_start_matches("java.math.");
        if kind.starts_with("message:") {
            let model = field["typeName"]
                .as_str()
                .unwrap_or("")
                .trim_end_matches("[]");
            let nested = contract["models"][format!("2:{model}")]["fields"].as_array();
            let source = data
                .get(name)
                .filter(|v| v.is_object() || v.is_array())
                .unwrap_or(data);
            let value = if kind.ends_with("[]") {
                let rows = source.as_array().or_else(|| {
                    if name.to_lowercase().contains("fail") {
                        None
                    } else {
                        data.get("items").and_then(Value::as_array)
                    }
                });
                json!(rows
                    .map(|rows| rows
                        .iter()
                        .map(|row| if depth < 16 {
                            nested
                                .map(|f| project(f, contract, row, depth + 1, dates))
                                .unwrap_or_else(|| row.clone())
                        } else {
                            json!({})
                        })
                        .collect::<Vec<_>>())
                    .unwrap_or_default())
            } else if depth < 16 {
                nested
                    .map(|f| project(f, contract, source, depth + 1, dates))
                    .unwrap_or_else(|| source.clone())
            } else {
                json!({})
            };
            object.insert(name.into(), value);
        } else if let Some(value) = data.get(name).cloned().or_else(|| scalar(name, kind, data)) {
            let value = if ["java.util.Date", "Date"].contains(&kind) {
                crate::dates::internal_millis(&value)
                    .and_then(|ms| dates.format(ms).ok())
                    .map(|v| json!(v))
                    .unwrap_or(value)
            } else {
                cast(value, kind)
            };
            object.insert(name.into(), value);
        } else if field["required"] == true {
            // shortcut: unmodeled required metadata uses typed defaults; replace when its domain fixture is defined.
            let value = match kind.to_lowercase().as_str() {
                "date" | "java.util.date" => Value::Null,
                "boolean" | "bool" => json!(false),
                "long" | "integer" | "int" | "short" | "double" | "float" | "bigdecimal"
                | "biginteger" => json!(0),
                "map" | "java.util.map" | "object" => json!({}),
                _ if kind.ends_with("[]") => json!([]),
                _ => json!(""),
            };
            object.insert(name.into(), value);
        }
    }
    Value::Object(object)
}
fn cast(value: Value, kind: &str) -> Value {
    let kind = kind
        .trim_start_matches("java.lang.")
        .trim_start_matches("java.math.");
    if kind.ends_with("[]") {
        return value
            .as_array()
            .map(|a| {
                json!(a
                    .iter()
                    .map(|v| cast(v.clone(), kind.trim_end_matches("[]")))
                    .collect::<Vec<_>>())
            })
            .unwrap_or(value);
    }
    match kind.to_lowercase().as_str() {
        "string" => {
            if value.is_string() || value.is_null() {
                value
            } else {
                json!(value.to_string())
            }
        }
        "long" | "integer" | "int" | "short" => value
            .as_i64()
            .map(|v| json!(v))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|v| v.parse::<Decimal>().ok())
                    .filter(|v| v.fract().is_zero())
                    .and_then(|v| v.to_i64())
                    .map(|v| json!(v))
            })
            .unwrap_or(value),
        "bigdecimal" | "biginteger" => {
            if value.is_number() {
                value
            } else {
                value
                    .as_str()
                    .and_then(|v| v.parse::<serde_json::Number>().ok())
                    .map(Value::Number)
                    .unwrap_or(value)
            }
        }
        "double" | "float" => value
            .as_f64()
            .map(|v| json!(v))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| json!(v))
            })
            .unwrap_or(value),
        _ => value,
    }
}
fn scalar(name: &str, kind: &str, data: &Value) -> Option<Value> {
    match name {
        "success" | "isSuccess" | "successed" => Some(json!(true)),
        "errorCode" | "errorMessage" | "errorMsg" | "error" | "code" | "message" => None,
        "total" | "totalCount" | "totalRecords" | "count" | "totalNum" => {
            Some(data.get("total").cloned().unwrap_or(json!(0)))
        }
        "page" | "pageNo" | "pageIndex" => Some(data.get("page").cloned().unwrap_or(json!(1))),
        "pageSize" | "sizePerPage" => Some(data.get("pageSize").cloned().unwrap_or(json!(20))),
        "offerIds" => Some(json!(data
            .get("items")
            .and_then(Value::as_array)
            .map(|rows| rows
                .iter()
                .filter_map(|r| r.get("offerId"))
                .collect::<Vec<_>>())
            .unwrap_or_default())),
        "result" | "data" | "returnValue" if kind.eq_ignore_ascii_case("boolean") => {
            Some(json!(true))
        }
        _ if kind.ends_with("[]") => Some(
            data.get("items")
                .filter(|v| {
                    v.as_array()
                        .is_some_and(|rows| rows.iter().all(|r| !r.is_object()))
                })
                .cloned()
                .unwrap_or(json!([])),
        ),
        _ => None,
    }
}

pub fn business_error(contract: &Value, code: &str, message: &str) -> Value {
    let code = match (contract["catalogue"]["name"].as_str().unwrap_or(""), code) {
        ("alibaba.trade.cancel", "ILLEGAL_ORDER_STATUS") => "ORDER_STATUS_ERROR",
        ("alibaba.trade.cancel", "ACCESS_DENIED") => "400_3",
        ("alibaba.trade.cancel", "ORDER_NOT_EXISTS") => "ORDER_NOT_EXIST",
        ("alibaba.createOrder.preview", "PRODUCT_OFF_SHELF") => "500_001",
        ("alibaba.createOrder.preview", "MULTIPLE_SELLERS") => "500_002",
        ("alibaba.createOrder.preview", "SKU_NOT_EXISTS") => "500_003",
        ("alibaba.createOrder.preview", "STOCK_NOT_ENOUGH") => "500_004",
        ("alibaba.createOrder.preview", "MIN_ORDER_QUANTITY") => "500_005",
        ("alibaba.createOrder.preview", "MIX_MINIMUM" | "MIX_UNSUPPORTED") => "500_006",
        ("alibaba.createOrder.preview", "WHOLESALE_MINIMUM") => "500_009",
        ("alibaba.trade.fastCreateOrder", "MIX_UNSUPPORTED") => "FAIL_BIZ_FAIL_BIZ_UNSUPPORT_MIX",
        ("alibaba.trade.fastCreateOrder", "MIX_MINIMUM") => "FAIL_BIZ_FAIL_BIZ_LESS_THAN_MIX_BEGIN",
        ("alibaba.trade.fastCreateOrder", "WHOLESALE_MINIMUM") => {
            "FAIL_BIZ_FAIL_BIZ_BOOKED_LESS_THAN_LEAST_QUANTITY"
        }
        ("alibaba.createOrder.preview", "RELATION_NOT_EXISTS") => "500_007",
        ("alibaba.createOrder.preview", "PRICE_ZERO") => "500_008",
        (
            "alibaba.trade.fastCreateOrder",
            "PARAM_ILLEGAL" | "INVALID_ADDRESS" | "not support tradeType:【XXXX】",
        ) => "400",
        ("alibaba.trade.fastCreateOrder", "MIN_ORDER_QUANTITY") => {
            "FAIL_BIZ_FAIL_BIZ_BOOKED_LESS_THAN_LEAST_QUANTITY"
        }
        ("alibaba.trade.fastCreateOrder", "PRODUCT_OFF_SHELF") => "FAIL_BIZ_FAIL_BIZ_ILLEGAL_OFFER",
        ("alibaba.trade.fastCreateOrder", "PRODUCT_NOT_EXISTS" | "SKU_NOT_EXISTS") => {
            "FAIL_BIZ_FAIL_BIZ_ILLEGAL_CARGO_DATA"
        }
        ("alibaba.trade.fastCreateOrder", "STOCK_NOT_ENOUGH") => {
            "FAIL_BIZ_PRODUCT_TRADE_STAT_ERROR"
        }
        ("alibaba.trade.fastCreateOrder", "RELATION_NOT_EXISTS") => {
            "FAIL_BIZ_FAIL_BIZ_PROXY_RELATED_NOT_FUND"
        }
        ("alibaba.trade.addFeedback", "ORDER_NOT_EXISTS" | "ACCESS_DENIED") => "500_2",
        ("alibaba.trade.addFeedback", "PARAM_ILLEGAL") => "400_1",
        ("alibaba.trade.createRefund", "ORDER_NOT_EXISTS") => "5001",
        ("alibaba.trade.createRefund", "ACCESS_DENIED") => "5002",
        ("alibaba.trade.createRefund", "ILLEGAL_ORDER_STATUS") => "5010",
        ("alibaba.trade.createRefund", "REFUND_AMOUNT_INVALID" | "REFUND_AMOUNT_EXCEEDED") => {
            "5011"
        }
        ("alibaba.trade.refund.returnGoods", "REFUND_NOT_EXISTS") => "4005",
        ("alibaba.trade.refund.returnGoods", "ACCESS_DENIED") => "4006",
        ("alibaba.trade.refund.returnGoods", "ILLEGAL_REFUND_STATUS") => "4007",
        (
            "alibaba.trade.refund.OpQueryOrderRefund"
            | "alibaba.trade.refund.OpQueryOrderRefundOperationList",
            "REFUND_NOT_EXISTS" | "ACCESS_DENIED",
        ) => "500",
        ("alibaba.trade.refund.OpQueryBatchRefundByOrderIdAndStatus", "ORDER_NOT_EXISTS") => "500",
        ("alibaba.trade.getLogisticsTraceInfo.buyerView", "ACCESS_DENIED") => {
            "order.nopermission.buyer"
        }
        ("alibaba.trade.getLogisticsTraceInfo.buyerView", "ORDER_NOT_EXISTS" | "500_2") => "404",
        ("alibaba.trade.payWay.query", "ACCESS_DENIED") => "500_2",
        ("alibaba.cross.productList", "PRODUCT_NOT_EXISTS") => "商品不存在",
        ("app.order.get", "PARAM_ILLEGAL") => "11002",
        _ => code,
    };
    fn envelope(
        fields: &[Value],
        contract: &Value,
        code: &str,
        message: &str,
        depth: usize,
    ) -> Value {
        let mut out = Map::new();
        for f in fields {
            let name = f["name"].as_str().unwrap_or("");
            let kind = f["type"].as_str().unwrap_or("String");
            let v = match name {
                "success" | "isSuccess" | "successed" | "succes" => Some(cast(json!(false), kind)),
                "code" | "errorCode" | "errCode" | "retCode" => Some(
                    if ["int", "Integer", "Long", "java.lang.Integer"].contains(&kind) {
                        json!(code.parse::<i64>().unwrap_or(-1))
                    } else {
                        json!(code)
                    },
                ),
                "message" | "errorMessage" | "errorMsg" | "errMsg" | "error" | "errorInfo"
                | "erroMsg" | "retMsg" | "errorDesc" => Some(json!(message)),
                _ => None,
            };
            if let Some(v) = v {
                out.insert(name.into(), v);
            }
        }
        if out.is_empty() && depth < 8 {
            for f in fields {
                if f["complexTypeFlag"] == true && !f["type"].as_str().unwrap_or("").ends_with("[]")
                {
                    let model = f["typeName"].as_str().unwrap_or("");
                    if let Some(nested) =
                        contract["models"][format!("2:{model}")]["fields"].as_array()
                    {
                        let value = envelope(nested, contract, code, message, depth + 1);
                        if value.as_object().is_some_and(|m| !m.is_empty()) {
                            out.insert(f["name"].as_str().unwrap_or("result").into(), value);
                            break;
                        }
                    }
                }
            }
        }
        Value::Object(out)
    }
    let fields = contract["detail"]["apiReturnParamVOList"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let result = envelope(fields, contract, code, message, 0);
    if result.as_object().is_some_and(|m| m.is_empty()) {
        json!({"error_code":code,"error_message":message})
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_wire_does_not_lose_precision() {
        assert_eq!(
            cast(json!("999999999999.99"), "BigDecimal").to_string(),
            "999999999999.99"
        );
    }
    #[test]
    fn cents_are_integer_without_truncating_fractional_values() {
        assert_eq!(cast(json!("1990.00"), "java.lang.Long"), json!(1990));
        assert_eq!(cast(json!("1.5"), "Long"), json!("1.5"));
    }
    #[test]
    fn errors_preserve_documented_string_boolean() {
        let c = Contracts::load();
        let api = &c.apis["com.alibaba.trade:alibaba.trade.payWay.query-1"];
        assert_eq!(
            business_error(api, "ORDER_NOT_EXISTS", "missing")["success"],
            json!("false")
        );
    }
    #[test]
    fn address_id_is_a_valid_alternative_to_text_address() {
        let c = Contracts::load();
        let api = &c.apis["com.alibaba.trade:alibaba.trade.createCrossOrder-1"];
        let p = BTreeMap::from([
            ("flow".into(), "general".into()),
            ("addressParam".into(), "{\"addressId\":1}".into()),
            (
                "cargoParamList".into(),
                "[{\"offerId\":1001,\"specId\":\"sku-1001\",\"quantity\":1}]".into(),
            ),
        ]);
        assert!(decode_parameters(api, &p).is_ok());
    }
}
