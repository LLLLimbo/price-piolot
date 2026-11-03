mod admin;
mod catalog;
mod contract;
mod market;
mod query;
mod state;
mod tasks;
#[cfg(test)]
mod tests;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::{json, Value};
use state::Store;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct App {
    store: Arc<Mutex<Store>>,
    review_locks: Arc<Mutex<BTreeSet<String>>>,
}
pub struct Failure {
    status: u16,
    code: i32,
    message: String,
}
impl Failure {
    pub fn new(status: u16, code: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}
pub type ApiResult = Result<Value, Failure>;
fn response(result: ApiResult) -> Response {
    match result {
        Ok(data) => Json(json!({"code":200,"msg":"SUCCESS","data":data})).into_response(),
        Err(e) => (
            StatusCode::from_u16(e.status).unwrap(),
            Json(json!({"code":e.code,"msg":e.message})),
        )
            .into_response(),
    }
}
fn authenticate(s: &Store, headers: &HeaderMap, id: &str) -> Result<String, Failure> {
    let key = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Failure::new(401, 401, "Unauthorized"))?;
    let k = s
        .keys
        .get(key)
        .filter(|k| k.active)
        .ok_or_else(|| Failure::new(401, 401, "Unauthorized"))?;
    let a = &s.accounts[&k.account];
    if !a.active || !a.terms_accepted || (id != "quota" && !a.ozon_authorized) {
        return Err(Failure::new(403, 403, "Forbidden"));
    }
    if a.partner_type != 0
        && (id == "ozon-collect-box-page" || id.starts_with("ozon-review-analyse"))
    {
        return Err(Failure::new(
            if id == "ozon-review-analyse-task-add" {
                200
            } else {
                403
            },
            403,
            "forbidden",
        ));
    }
    Ok(key.into())
}
fn rate_limit(s: &mut Store, key: &str, id: &str) -> Result<(), Failure> {
    let p = s.policies[id].clone();
    let now = s.now();
    let account = &s.keys[key].account;
    let scopes = [
        (format!("key:{id}:{key}"), p.limit, p.window_ms),
        (
            format!("account:{id}:{account}"),
            p.account_limit,
            p.window_ms,
        ),
        (format!("global:{id}"), p.global_limit, 1000),
    ];
    for (name, limit, window) in &scopes {
        let entries = s.rate_windows.entry(name.clone()).or_default();
        entries.retain(|v| *v > now - window);
        if *limit > 0 && entries.len() >= *limit {
            return Err(Failure::new(429, 429, "Too Many Requests"));
        }
    }
    for (name, limit, _) in scopes {
        if limit > 0 {
            s.rate_windows.get_mut(&name).unwrap().push(now);
        }
    }
    Ok(())
}
struct ReviewGuard {
    locks: Arc<Mutex<BTreeSet<String>>>,
    account: String,
}
impl Drop for ReviewGuard {
    fn drop(&mut self) {
        self.locks.lock().unwrap().remove(&self.account);
    }
}

async fn gateway(
    State(app): State<App>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let Some(endpoint) = contract::endpoints()
        .iter()
        .find(|e| e["path"] == uri.path())
    else {
        return response(Err(Failure::new(404, 404, "Not Found")));
    };
    if endpoint["method"] != method.as_str() {
        return response(Err(Failure::new(405, 405, "Method Not Allowed")));
    }
    let input = if method == Method::GET {
        json!({})
    } else {
        match body
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            Some(v) if v.is_object() => v,
            _ => return response(Err(Failure::new(400, 400, "Bad Request"))),
        }
    };
    let id = endpoint["id"].as_str().unwrap();
    let mut review_guard = None;
    let (result, delay) = {
        let mut s = app.store.lock().unwrap();
        s.tick();
        let key = match authenticate(&s, &headers, id) {
            Ok(k) => k,
            Err(e) => return response(Err(e)),
        };
        let account = s.keys[&key].account.clone();
        if id == "ozon-review-analyse-task-add" {
            if !app.review_locks.lock().unwrap().insert(account.clone()) {
                return response(Err(Failure::new(
                    200,
                    6102,
                    "Conflict caused by multiple people operating simultaneously.",
                )));
            }
            review_guard = Some(ReviewGuard {
                locks: app.review_locks.clone(),
                account: account.clone(),
            });
        }
        if let Err(e) = contract::validate(endpoint, &input).and_then(|_| {
            if id == "ozon-review-analyse-task-add" {
                Ok(())
            } else {
                query::validate(&input, endpoint, s.now())
            }
        }) {
            return response(Err(Failure::new(400, 400, e)));
        }
        if input["subAccountPhone"]
            .as_str()
            .is_some_and(|v| v.chars().count() > 30)
        {
            return response(Err(Failure::new(400, 400, "Bad Request")));
        }
        if let Err(e) = rate_limit(&mut s, &key, id) {
            return response(Err(e));
        }
        let mut fault = None;
        let mut delay = 0;
        for f in &mut s.faults {
            if f["remaining"].as_u64().unwrap_or(0) > 0
                && f["path"] == uri.path()
                && f["method"].as_str().is_none_or(|v| v == method.as_str())
                && f["account"].as_str().is_none_or(|v| v == account)
                && f["api_key"].as_str().is_none_or(|v| v == key)
            {
                f["remaining"] = json!(f["remaining"].as_u64().unwrap() - 1);
                delay = f["delay_ms"].as_u64().unwrap_or(0);
                if f.get("body").is_some() || f.get("status").is_some() {
                    fault = Some(f.clone());
                }
                break;
            }
        }
        if let Some(f) = fault {
            let status = f["status"].as_u64().unwrap_or(200) as u16;
            let charge = if status == 200
                && (f["body"]["code"] == 200 || id == "ozon-product-monthly-sales-trend")
            {
                s.policies[id].cost
            } else {
                0
            };
            let result = match tasks::charge(&mut s, &key, charge) {
                Ok(()) => (
                    StatusCode::from_u16(status).unwrap(),
                    Json(
                        f.get("body")
                            .cloned()
                            .unwrap_or_else(|| json!({"code":status,"msg":"Injected fault"})),
                    ),
                )
                    .into_response(),
                Err(e) => response(Err(e)),
            };
            (result, delay)
        } else {
            // Commit an entire operation, including its debit and created records, atomically.
            let mut candidate = s.clone();
            let result = dispatch(&mut candidate, &key, id, &input);
            if result.is_ok() {
                *s = candidate;
            }
            (response(result), delay)
        }
    };
    if review_guard.is_some() {
        tokio::task::yield_now().await;
    }
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
    drop(review_guard);
    result
}
fn dispatch(s: &mut Store, key: &str, id: &str, input: &Value) -> ApiResult {
    let account = s.keys[key].account.clone();
    if id == "quota" {
        let k = &s.keys[key];
        return Ok(
            json!({"creditLimit":k.credit_limit,"creditUsed":k.credit_used,"freeCreditLimitMonth":0,"freeCreditUsedMonth":0}),
        );
    }
    if id == "ozon-product-collect" {
        return tasks::collect(s, key, input).map_err(|mut e| {
            if e.code == 8000 {
                e.status = 200;
            }
            e
        });
    }
    if id == "ozon-review-analyse-task-add" {
        return tasks::review_add(s, key, input);
    }
    let result = if id == "ozon-collect-box-page" {
        Ok(tasks::collection_page(s, &account, input))
    } else if id.starts_with("ozon-review-analyse-") {
        tasks::review_result(s, &account, id, input)
    } else if [
        "ozon-market-keyword-search",
        "ozon-keyword-mining",
        "ozon-keyword-back-search",
    ]
    .contains(&id)
    {
        Ok(market::handle(s, id, input))
    } else {
        Ok(catalog::handle(s, id, input))
    };
    if result.is_ok() {
        let cost = s.policies[id].cost;
        tasks::charge(s, key, cost).map_err(|mut e| {
            if id != "ozon-collect-box-page" {
                e.status = 200;
            }
            e
        })?;
    }
    result
}
#[tokio::main]
async fn main() {
    let base = std::env::var("MOCK_BASE_URL").unwrap_or_else(|_| "http://localhost:8084".into());
    let fixed = std::env::var("MOCK_FIXED_TIME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().expect("MOCK_FIXED_TIME milliseconds"));
    assert!(
        fixed.is_none_or(|v| (0..=state::MAX_TIME - 30 * 86_400_000).contains(&v)),
        "MOCK_FIXED_TIME is out of range"
    );
    let store = Store::seed(base, fixed);
    store.validate().expect("valid initial configuration");
    let app = App {
        store: Arc::new(Mutex::new(store)),
        review_locks: Arc::new(Mutex::new(BTreeSet::new())),
    };
    let api = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/assets/product.svg",
            get(|| async {
                (
                    [("content-type", "image/svg+xml")],
                    include_str!("../fixtures/product.svg"),
                )
            }),
        )
        .fallback(gateway)
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(app.clone());
    let admin = Router::new()
        .route("/__mock/state", get(admin::state))
        .route("/__mock/actions", post(admin::actions))
        .route("/__mock/reset", post(admin::reset))
        .route("/__mock/clock/advance", post(admin::advance))
        .route("/__mock/faults", put(admin::faults))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(app);
    let api_listener = tokio::net::TcpListener::bind(
        std::env::var("MOCK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into()),
    )
    .await
    .unwrap();
    let admin_listener = tokio::net::TcpListener::bind(
        std::env::var("MOCK_ADMIN_LISTEN").unwrap_or_else(|_| "0.0.0.0:8081".into()),
    )
    .await
    .unwrap();
    eprintln!(
        "Seerfar mock: API {}, admin {}",
        api_listener.local_addr().unwrap(),
        admin_listener.local_addr().unwrap()
    );
    tokio::select! {
        r=axum::serve(api_listener,api)=>r.unwrap(),
        r=axum::serve(admin_listener,admin)=>r.unwrap(),
        _=tokio::signal::ctrl_c()=>(),
    }
}
