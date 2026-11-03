use crate::{
    business::{fail, Result},
    protocol, App,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn validate(contract: &Value, data: &Value) -> Result<()> {
    fn fields(schema: &[Value], data: &Value, path: &str) -> Result<()> {
        if !data.is_object() {
            return fail("INVALID_MESSAGE", &format!("{path} must be an object"));
        }
        for field in schema {
            let name = field["name"].as_str().unwrap_or("");
            let location = format!("{path}.{name}");
            let Some(value) = data.get(name).filter(|v| !v.is_null()) else {
                if field["required"] == true {
                    return fail("INVALID_MESSAGE", &format!("{location} required"));
                }
                continue;
            };
            let kind = field["type"].as_str().unwrap_or("");
            let valid = match kind {
                "String" => value.is_string(),
                "Number" | "Date" => value.is_number(),
                "Boolean" => value.is_boolean(),
                "Object" => value.is_object(),
                "Object[]" => value.is_array(),
                _ => true,
            };
            if !valid {
                return fail("INVALID_MESSAGE", &format!("{location} must be {kind}"));
            }
            if let Some(children) = field["children"].as_array() {
                if let Some(rows) = value.as_array() {
                    for row in rows {
                        fields(children, row, &location)?;
                    }
                } else {
                    fields(children, value, &location)?;
                }
            }
        }
        Ok(())
    }
    fields(
        contract["detail"]["messageDocs"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        data,
        "data",
    )
}
pub async fn deliver(app: &App) {
    let jobs = {
        let mut s = app.store.lock().unwrap();
        let now = s.now();
        let generation = s.generation;
        let subscriptions = s.subscriptions.clone();
        s.events.retain(|e| now - e.created <= 7 * 86400000);
        s.events
            .iter_mut()
            .filter_map(|e| {
                if !["pending", "retry"].contains(&e.status.as_str()) || e.due > now {
                    return None;
                }
                let subscription = subscriptions
                    .iter()
                    .find(|sub| {
                        sub.topic == e.topic && sub.account == e.account && sub.app_key == e.app_key
                    })?
                    .clone();
                e.status = "delivering".into();
                e.attempts += 1;
                Some((generation, e.clone(), subscription))
            })
            .collect::<Vec<_>>()
    };
    for (generation, e, sub) in jobs {
        let message=json!({"msgId":e.id,"type":e.topic,"data":e.data,"userInfo":e.account,"gmtBorn":e.created}).to_string();
        let mut form = BTreeMap::from([("message".into(), message)]);
        form.insert(
            "_aop_signature".into(),
            protocol::signature("", &form, &sub.secret),
        );
        let result = app.client.post(&sub.url).form(&form).send().await;
        let mut s = app.store.lock().unwrap();
        if s.generation != generation {
            continue;
        }
        let now = s.now();
        let Some(event) = s.events.iter_mut().find(|x| x.id == e.id) else {
            continue;
        };
        if event.status != "delivering" || event.attempts != e.attempts {
            continue;
        }
        match result {
            Ok(r) if r.status().as_u16() == 200 => {
                event.status = "delivered".into();
                event.last_error = None;
            }
            result => {
                event.last_error = Some(match result {
                    Ok(r) => format!("HTTP {}", r.status()),
                    Err(e) => e.to_string(),
                });
                event.status = if event.attempts >= 4 {
                    "failed"
                } else {
                    "retry"
                }
                .into();
                event.due = now + 1000 * 2_i64.pow(event.attempts - 1);
            }
        }
    }
}
