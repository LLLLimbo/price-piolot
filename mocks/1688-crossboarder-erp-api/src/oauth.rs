use crate::{
    gateway, protocol,
    state::{Code, Token},
    App,
};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Form, Json,
};
use serde_json::json;
use std::collections::BTreeMap;

pub async fn authorize(
    State(app): State<App>,
    Query(p): Query<BTreeMap<String, String>>,
) -> Response {
    let mut s = app.store.lock().unwrap();
    let key = p.get("client_id").cloned().unwrap_or_default();
    let Some(secret) = s.apps.get(&key) else {
        return gateway("invalid_client", "unknown client_id");
    };
    if p.get("site").is_some_and(|site| site == "china")
        && !protocol::valid_signature("", &p, secret)
    {
        return gateway("invalid_signature", "invalid authorization signature");
    }
    let Some(uri) = p.get("redirect_uri") else {
        return gateway("invalid_request", "redirect_uri required");
    };
    let Ok(mut redirect) = reqwest::Url::parse(uri) else {
        return gateway("invalid_request", "invalid redirect URI");
    };
    if (uri != "urn:ietf:wg:oauth:2.0:oob" && !["http", "https"].contains(&redirect.scheme()))
        || redirect.fragment().is_some()
    {
        return gateway("invalid_request", "invalid redirect URI");
    }
    let account = p
        .get("mock_account")
        .map(String::as_str)
        .unwrap_or("buyer-1");
    if !s.accounts.contains_key(account) {
        return gateway("access_denied", "unknown account");
    }
    let account = account.to_owned();
    let code = format!("code-{}", s.next_id());
    let expires = s.now() + 120000;
    s.codes.insert(
        code.clone(),
        Code {
            account,
            app_key: key,
            redirect_uri: uri.clone(),
            expires,
        },
    );
    if uri == "urn:ietf:wg:oauth:2.0:oob" {
        return Json(json!({"code":code,"state":p.get("state")})).into_response();
    }
    redirect.query_pairs_mut().append_pair("code", &code);
    if let Some(state) = p.get("state") {
        redirect.query_pairs_mut().append_pair("state", state);
    }
    (
        axum::http::StatusCode::FOUND,
        [(axum::http::header::LOCATION, redirect.to_string())],
    )
        .into_response()
}
pub async fn token(
    State(app): State<App>,
    Path((operation, key)): Path<(String, String)>,
    Form(p): Form<BTreeMap<String, String>>,
) -> Response {
    let mut s = app.store.lock().unwrap();
    if s.apps.get(&key) != p.get("client_secret") || p.get("client_id") != Some(&key) {
        return gateway("invalid_client", "invalid client credentials");
    }
    let now = s.now();
    if operation == "postponeToken" {
        let refresh = p.get("refresh_token").cloned().unwrap_or_default();
        let token = s
            .tokens
            .values_mut()
            .find(|t| t.app_key == key && t.refresh == refresh && t.refresh_expires > now);
        let Some(token) = token else {
            return gateway("invalid_grant", "refresh token expired or invalid");
        };
        token.refresh_expires = now + 90 * 86400000;
        return Json(json!({"refresh_token":refresh,"refresh_token_timeout":chrono::DateTime::from_timestamp_millis(token.refresh_expires).unwrap().format("%Y%m%d%H%M%S+0000").to_string()})).into_response();
    }
    if operation != "getToken" {
        return gateway("invalid_request", "unknown token operation");
    }
    let (account, refresh, refresh_expires) = match p.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let code = p.get("code").cloned().unwrap_or_default();
            let Some(c) = s.codes.get(&code) else {
                return gateway("invalid_grant", "authorization code invalid or used");
            };
            if c.expires <= now
                || c.app_key != key
                || p.get("redirect_uri") != Some(&c.redirect_uri)
            {
                return gateway(
                    "invalid_grant",
                    "authorization code expired or redirect mismatch",
                );
            }
            let account = c.account.clone();
            s.codes.remove(&code);
            (
                account,
                format!("refresh-{}", s.next_id()),
                now + 90 * 86400000,
            )
        }
        Some("refresh_token") => {
            let refresh = p.get("refresh_token").cloned().unwrap_or_default();
            let Some(t) = s.tokens.values().find(|t| {
                t.app_key == key
                    && !t.refresh.is_empty()
                    && t.refresh == refresh
                    && t.refresh_expires > now
            }) else {
                return gateway("invalid_grant", "refresh token expired or invalid");
            };
            (t.account.clone(), refresh, t.refresh_expires)
        }
        _ => return gateway("unsupported_grant_type", "unsupported grant_type"),
    };
    let access = format!("access-{}", s.next_id());
    s.tokens.insert(
        access.clone(),
        Token {
            sub_account: None,
            parent_token: None,
            account: account.clone(),
            app_key: key,
            expires: now + 36000000,
            refresh: refresh.clone(),
            refresh_expires,
        },
    );
    let mut response = json!({"access_token":access,"expires_in":"36000","refresh_token":refresh,"refresh_token_timeout":chrono::DateTime::from_timestamp_millis(refresh_expires).unwrap().format("%Y%m%d%H%M%S+0000").to_string(),"memberId":account,"aliId":account,"resource_owner":account,"token_type":"bearer"});
    if p.get("need_refresh_token").is_some_and(|v| v == "false") {
        response.as_object_mut().unwrap().remove("refresh_token");
        response
            .as_object_mut()
            .unwrap()
            .remove("refresh_token_timeout");
        if p.get("grant_type")
            .is_some_and(|v| v == "authorization_code")
        {
            s.tokens.get_mut(&access).unwrap().refresh.clear();
        }
    }
    Json(response).into_response()
}
