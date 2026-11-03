#![recursion_limit = "256"]
mod accounts_business;
mod admin;
mod analytics;
mod business;
mod catalog;
mod contract;
mod contracts_business;
mod cps;
mod credits;
mod dates;
mod freight;
mod invoices;
mod logistics;
mod messages;
mod oauth;
mod payments;
mod pickups;
mod protocol;
mod refunds;
mod rules;
mod search;
mod services;
mod state;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, FromRequest, Multipart, Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use state::Store;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct App {
    store: Arc<Mutex<Store>>,
    contracts: Arc<contract::Contracts>,
    client: reqwest::Client,
}
fn gateway(code: &str, message: &str) -> Response {
    Json(json!({"error_code":code,"error_message":message})).into_response()
}

async fn server_time(State(app): State<App>, Path(key): Path<String>) -> Response {
    let store = app.store.lock().unwrap();
    if !store.apps.contains_key(&key) {
        return gateway("INVALID_APP_KEY", "unknown appKey");
    }
    Json(store.now()).into_response()
}

async fn invoke(
    State(app): State<App>,
    Path((version, namespace, name, key)): Path<(String, String, String, String)>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    if version == "1"
        && namespace == "system.oauth2"
        && ["getToken", "postponeToken"].contains(&name.as_str())
    {
        return match axum::Form::<BTreeMap<String, String>>::from_request(request, &app).await {
            Ok(form) => oauth::token(State(app), Path((name, key)), form).await,
            Err(_) => gateway("INVALID_ARGUMENT", "malformed OAuth form data"),
        };
    }
    let content = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mut files = BTreeMap::new();
    let pairs: Vec<(String, String)> = if content.starts_with("multipart/form-data") {
        let mut multipart = match Multipart::from_request(request, &app).await {
            Ok(m) => m,
            Err(_) => return gateway("INVALID_ARGUMENT", "malformed multipart data"),
        };
        let mut pairs = vec![];
        loop {
            let field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(_) => return gateway("INVALID_ARGUMENT", "malformed multipart field"),
            };
            let field_name = field.name().unwrap_or("").to_owned();
            let binary = field.file_name().is_some();
            if binary {
                if name != "alibaba.trade.uploadRefundVoucher"
                    || field_name != "imageData"
                    || files.contains_key(&field_name)
                {
                    return gateway("INVALID_ARGUMENT", "unsupported or duplicate file field");
                }
                let bytes = match field.bytes().await {
                    Ok(bytes) => bytes,
                    Err(_) => return gateway("INVALID_ARGUMENT", "cannot read file"),
                };
                files.insert(field_name, STANDARD.encode(bytes));
            } else {
                let value = match field.text().await {
                    Ok(value) => value,
                    Err(_) => return gateway("INVALID_ARGUMENT", "invalid UTF-8 field"),
                };
                pairs.push((field_name, value));
            }
        }
        pairs
    } else if content.starts_with("application/x-www-form-urlencoded") {
        let body = match Bytes::from_request(request, &app).await {
            Ok(body) => body,
            Err(_) => return gateway("INVALID_REQUEST", "request body too large"),
        };
        match serde_urlencoded::from_bytes(&body) {
            Ok(p) => p,
            Err(_) => return gateway("INVALID_ARGUMENT", "malformed form data"),
        }
    } else {
        return gateway("INVALID_REQUEST", "form encoding required");
    };
    let mut input = query;
    for (k, v) in pairs {
        if input.insert(k, v).is_some() {
            return gateway("INVALID_ARGUMENT", "duplicate parameter");
        }
    }
    let api_key = format!("{namespace}:{name}-{version}");
    let Some(contract) = app.contracts.apis.get(&api_key) else {
        return gateway("API_NOT_EXISTS", "unknown API or version");
    };
    let path = format!("param2/{version}/{namespace}/{name}/{key}");
    let (owner, fault) = {
        let mut s = app.store.lock().unwrap();
        let Some(secret) = s.apps.get(&key) else {
            return gateway("INVALID_APP_KEY", "unknown appKey");
        };
        if contract["detail"]["needSignature"] != false
            && !protocol::valid_signature(&path, &input, secret)
        {
            return gateway("INVALID_SIGNATURE", "invalid signature");
        }
        if contract["detail"]["needTimestamp"] == true || input.contains_key("_aop_timestamp") {
            let Some(time) = input
                .get("_aop_timestamp")
                .and_then(|v| v.parse::<i64>().ok())
            else {
                return gateway("INVALID_TIMESTAMP", "timestamp required");
            };
            if (i128::from(s.now()) - i128::from(time)).abs() > 900000 {
                return gateway("INVALID_TIMESTAMP", "timestamp outside 15 minute window");
            }
        }
        let owner = if let Some(token) = input.get("access_token") {
            let Some(t) = s.tokens.get(token) else {
                return gateway("INVALID_ACCESS_TOKEN", "token is invalid");
            };
            if t.parent_token.as_ref().is_some_and(|parent| {
                s.tokens
                    .get(parent)
                    .is_none_or(|p| p.expires <= s.now() || p.app_key != key)
            }) {
                return gateway(
                    "INVALID_ACCESS_TOKEN",
                    "parent authorization revoked or expired",
                );
            }
            if t.sub_account.is_some() && name.starts_with("subaccount.auth.") {
                return gateway("ACCESS_DENIED", "parent authorization required");
            }
            if t.expires <= s.now() {
                return gateway("ACCESS_TOKEN_EXPIRED", "token expired");
            }
            if t.app_key != key {
                return gateway(
                    "INVALID_ACCESS_TOKEN",
                    "token belongs to another application",
                );
            }
            let Some(a) = s.accounts.get(&t.account) else {
                return gateway("INVALID_ACCESS_TOKEN", "account not found");
            };
            if !a
                .scopes
                .iter()
                .any(|scope| scope == "*" || scope == &api_key)
            {
                return gateway("ACCESS_DENIED", "API permission missing");
            }
            t.account.clone()
        } else if contract["detail"]["needAuth"] != false {
            return gateway("INVALID_ACCESS_TOKEN", "access_token required");
        } else {
            "buyer-1".into()
        };
        let fault = s
            .faults
            .iter_mut()
            .find(|f| {
                f.remaining > 0
                    && (f.api == api_key || f.api == name)
                    && f.account.as_deref().is_none_or(|a| a == owner)
            })
            .map(|f| {
                f.remaining -= 1;
                f.clone()
            });
        (owner, fault)
    };
    if let Some(fault) = fault {
        if fault.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(fault.delay_ms)).await;
        }
        if let Some(code) = fault.code {
            return Json(contract::business_error(
                contract,
                &code,
                fault.message.as_deref().unwrap_or("injected failure"),
            ))
            .into_response();
        }
    }
    for (name, file) in files {
        if input.insert(name, file).is_some() {
            return gateway("INVALID_ARGUMENT", "duplicate file parameter");
        }
    }
    let p = match contract::decode_parameters(contract, &input) {
        Ok(p) => p,
        Err(e) if e.contains("required") => return Json(json!({"error_code":"450","error_message":e,"exception":"com.alibaba.openapi.client.exception.ApiException"})).into_response(),
        Err(e) => return gateway("INVALID_ARGUMENT", &e),
    };
    let response = {
        let mut guard = app.store.lock().unwrap();
        let mut candidate = guard.clone();
        candidate.current_app = key.clone();
        let outcome = business::commerce(&mut candidate, &name, &p, &owner)
            .or_else(|| catalog::handle(&mut candidate, &name, &p, &owner))
            .or_else(|| services::handle(&mut candidate, &name, &p, &owner));
        match outcome {
            Some(Ok(data)) => {
                candidate.finish_changes(&guard);
                *guard = candidate;
                contract::response(contract, &data, &p)
            }
            Some(Err(error)) => contract::business_error(contract, &error.code, &error.message),
            None => return gateway("API_NOT_IMPLEMENTED", "API implementation is missing"),
        }
    };
    Json(response).into_response()
}
async fn asset() -> Response {
    ([(axum::http::header::CONTENT_TYPE,"image/svg+xml")],"<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><rect width='160' height='160' fill='#f2f2f2'/><text x='25' y='85'>Mock 1688</text></svg>").into_response()
}
async fn product_resource(State(app): State<App>, Path(id): Path<i64>) -> Response {
    let s = app.store.lock().unwrap();
    match s.products.get(&id) {
        Some(p) => Json(business::product_value(
            p,
            &s.base_url,
            &s.accounts[&p.seller],
            &s.current_app,
        ))
        .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn snapshot(State(app): State<App>, Path((oid, eid)): Path<(i64, i64)>) -> Response {
    let s = app.store.lock().unwrap();
    match s.orders.get(&oid).and_then(|o|eid.checked_sub(oid*100).and_then(|i|usize::try_from(i).ok()).and_then(|i|o.lines.get(i))) {
        Some(line)=>Json(json!({"productId":line.offer_id,"title":line.title,"sku":line.spec_id,"quantity":line.quantity,"price":line.price})).into_response(),
        None=>StatusCode::NOT_FOUND.into_response(),
    }
}
async fn shop(State(app): State<App>, Path(id): Path<String>) -> Response {
    let s = app.store.lock().unwrap();
    match s.accounts.get(&id) {
        Some(a)=>Json(json!({"memberId":a.id,"name":a.login,"products":s.products.values().filter(|p|p.seller==id && p.active).map(|p|business::product_value(p,&s.base_url,&s.accounts[&p.seller], &s.current_app)).collect::<Vec<_>>()})).into_response(),
        None=>StatusCode::NOT_FOUND.into_response(),
    }
}
async fn local_link(Path(id): Path<String>) -> Json<Value> {
    Json(json!({"id":id,"status":"awaiting_simulated_action"}))
}
async fn file(State(app): State<App>, Path(id): Path<String>) -> Response {
    let s = app.store.lock().unwrap();
    match s
        .records
        .get("files")
        .and_then(|m| m.iter().find(|(k, _)| k.ends_with(&format!(":{id}"))))
    {
        Some((_, v)) => match STANDARD.decode(v["content"].as_str().unwrap_or("")) {
            Ok(bytes) => (
                [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                bytes,
            )
                .into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn not_found() -> Response {
    gateway("API_NOT_EXISTS", "unknown endpoint")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("MOCK_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let mut store = Store::seed(base);
    if let Some(time) = std::env::var("MOCK_FIXED_TIME")
        .ok()
        .filter(|v| !v.is_empty())
    {
        store.fixed_time = Some(time.parse()?);
    }
    store.validate().map_err(std::io::Error::other)?;
    let app = App {
        store: Arc::new(Mutex::new(store)),
        contracts: Arc::new(contract::Contracts::load()),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(3))
            .build()?,
    };
    let api = Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route(
            "/openapi/param2/{version}/{namespace}/{name}/{key}",
            post(invoke),
        )
        .route(
            "/openapi/http/1/system.oauth2/{operation}/{key}",
            post(oauth::token),
        )
        .route(
            "/openapi/param2/1/system/currentTimeMillis/{key}",
            post(server_time),
        )
        .route("/oauth/authorize", get(oauth::authorize))
        .route("/auth/authorize.htm", get(oauth::authorize))
        .route("/assets/product.svg", get(asset))
        .route("/product/{id}", get(product_resource))
        .route("/snapshot/{oid}/{eid}", get(snapshot))
        .route("/files/{id}", get(file))
        .route("/payment/{id}", get(local_link))
        .route("/payment/contract/{id}", get(local_link))
        .route("/selection/{id}", get(local_link))
        .route("/promotion/{id}", get(local_link))
        .route("/contact/{id}", get(local_link))
        .route("/shop/{id}", get(shop))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(12 * 1024 * 1024))
        .with_state(app.clone());
    let management = Router::new()
        .route("/__mock/state", get(admin::state))
        .route("/__mock/reset", post(admin::reset))
        .route("/__mock/clock/advance", post(admin::advance))
        .route("/__mock/actions", post(admin::action))
        .route("/__mock/faults", put(admin::faults))
        .route("/__mock/subscriptions", put(admin::subscriptions))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(app.clone());
    let listener = tokio::net::TcpListener::bind(
        std::env::var("MOCK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into()),
    )
    .await?;
    let admin_listener = tokio::net::TcpListener::bind(
        std::env::var("MOCK_ADMIN_LISTEN").unwrap_or_else(|_| "0.0.0.0:8081".into()),
    )
    .await?;
    let worker = app.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            messages::deliver(&worker).await;
        }
    });
    eprintln!(
        "1688 mock: {} API contracts, {} message contracts",
        app.contracts.apis.len(),
        app.contracts.messages.len()
    );
    tokio::try_join!(
        axum::serve(listener, api)
            .with_graceful_shutdown(shutdown())
            .into_future(),
        axum::serve(admin_listener, management)
            .with_graceful_shutdown(shutdown())
            .into_future()
    )?;
    Ok(())
}
use std::future::IntoFuture;

async fn shutdown() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("termination signal");
    tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
}
