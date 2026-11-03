use crate::{business::*, state::Store};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
pub fn handle(s: &mut Store, name: &str, p: &Value, owner: &str) -> Option<Result<Value>> {
    let result = execute(s, name, p, owner);
    if result.as_ref().err().is_some_and(|e| e.code == "UNHANDLED") {
        None
    } else {
        Some(result)
    }
}
fn execute(s: &mut Store, name: &str, p: &Value, owner: &str) -> Result<Value> {
    if [
        "alibaba.cross.syncProductListPushed",
        "alibaba.product.push.syncPushProductResult",
    ]
    .contains(&name)
    {
        let config = s
            .records
            .get("app_configs")
            .and_then(|r| r.get(&s.current_app));
        if config.is_none_or(|c| c["distribution"] != true) {
            return fail(
                "APPKEY_ISV_CONFIG_IS_NOT_EXIST",
                "application distribution configuration missing",
            );
        }
        if name == "alibaba.cross.syncProductListPushed"
            && config
                .and_then(|c| c["users"].as_array())
                .is_none_or(|a| !a.contains(&json!(owner)))
        {
            return fail("ISVUSERS_IS_NULL", "ISV account has not been synchronized");
        }
    }
    match name {
        "alibaba.account.basic"
        | "alibaba.account.agent.crossBasic"
        | "alibaba.member.getRelationUserInfo" => {
            let login = text(p, &["loginId", "domain"]);
            if name == "alibaba.account.agent.crossBasic" && login.is_empty() {
                return fail("PARAM_ILLEGAL", "loginId or domain required");
            }
            let a = if login.is_empty() {
                s.accounts.get(owner)
            } else {
                s.accounts.values().find(|a| {
                    a.login == login
                        || a.id == login
                        || (!a.domain.is_empty()
                            && (a.domain == login
                                || a.domain.trim_end_matches(".1688.com") == login))
                })
            }
            .ok_or_else(|| Error {
                code: "ACCOUNT_NOT_EXIST".into(),
                message: "account not found".into(),
            })?;
            Ok(
                json!({"memberId":a.id,"loginId":a.login,"userId":a.user_id,"companyName":format!("{}模拟商家",a.login),"supplierName":a.login,"sellerName":a.login,"sellerId":a.id,"isEnterprise":true,"enterpriseAccount":true,"personAccount":false,"status":if a.frozen{"disabled"}else{"enabled"},"shopUrl":format!("{}/shop/{}",s.base_url,a.id),"homepageUrl":format!("{}/shop/{}",s.base_url,a.id),"categoryName":"日用百货","kuaJingBao":true,"crossBorder":true,"phoneNo":"057100000000","mobileNo":"13800000000"}),
            )
        }
        "alibaba.subAccount.list" => Ok(
            json!({"mainUserId":s.accounts[owner].user_id,"mainMemberId":owner,"mainLoginId":s.accounts[owner].login,"subAccountList":s.rows("subaccounts",Some(owner))}),
        ),
        "loginid.openuid.encrypt" => {
            let login = text(p, &["loginId"]);
            let a = s
                .accounts
                .values()
                .find(|a| a.login == login)
                .ok_or_else(|| Error {
                    code: "ACCOUNT_NOT_EXIST".into(),
                    message: "account not found".into(),
                })?;
            Ok(json!({"openUid":crate::accounts_business::open_uid(&s.current_app, &a.id)}))
        }
        "wangwangnick.openuid.decrypt" | "account.wangwangUrl.get" => {
            let uid = text(p, &["openUid", "toOpenUid"]);
            let a = s
                .accounts
                .values()
                .find(|a| crate::accounts_business::open_uid(&s.current_app, &a.id) == uid)
                .ok_or_else(|| Error {
                    code: "ACCOUNT_NOT_EXIST".into(),
                    message: "unknown openUid".into(),
                })?;
            Ok(
                json!({"wangwangNick":a.login,"url":format!("{}/contact/{}",s.base_url,a.id),"data":format!("{}/contact/{}",s.base_url,a.id)}),
            )
        }
        "account.user.register" => {
            let login = text(p, &["loginId", "account", "email", "outMemberId"]);
            if login.is_empty() {
                return fail("PARAM_ILLEGAL", "account identifier required");
            }
            if s.accounts.values().any(|a| a.login == login) {
                return fail("ACCOUNT_ALREADY_EXISTS", "account already exists");
            }
            let user_id = s.next_id();
            let id = format!("buyer-{user_id}");
            s.accounts.insert(
                id.clone(),
                crate::state::Account {
                    domain: String::new(),
                    user_id,
                    id: id.clone(),
                    login: login.clone(),
                    balance: rust_decimal::Decimal::ZERO,
                    frozen: false,
                    scopes: vec!["*".into()],
                },
            );
            Ok(
                json!({"memberId":id,"loginId":login,"userId":user_id,"data":{"memberId":id,"loginId":login}}),
            )
        }
        "subaccount.auth.add" | "subaccount.auth.cancel" | "subaccount.auth.list" => {
            let identities = array(p, &["subUserIdentityList"])
                .filter(|v| !v.is_empty())
                .ok_or_else(|| Error {
                    code: "PARAM_ILLEGAL".into(),
                    message: "nonempty subUserIdentityList required".into(),
                })?;
            let mut results = serde_json::Map::new();
            for ident in identities {
                let sub = ident.as_str().unwrap_or_default();
                let key = format!("{owner}:{sub}");
                let mut row = s
                    .records
                    .get("subaccounts")
                    .and_then(|m| m.get(&key))
                    .cloned()
                    .ok_or_else(|| Error {
                        code: "SUB_ACCOUNT_NOT_EXIST".into(),
                        message: "subaccount does not belong to principal".into(),
                    })?;
                if name.ends_with("add") {
                    let parent_token = text(p, &["access_token"]);
                    let parent = s.tokens.get(&parent_token).cloned().ok_or_else(|| Error {
                        code: "INVALID_ACCESS_TOKEN".into(),
                        message: "parent authorization required".into(),
                    })?;
                    s.tokens.retain(|_, t| {
                        !(t.account == owner
                            && t.app_key == s.current_app
                            && t.sub_account.as_deref() == Some(sub))
                    });
                    let token = format!("sub-token-{}", s.next_id());
                    let expires = (s.now() + 36000000).min(parent.expires);
                    s.tokens.insert(
                        token.clone(),
                        crate::state::Token {
                            account: owner.into(),
                            app_key: s.current_app.clone(),
                            expires,
                            refresh: String::new(),
                            refresh_expires: 0,
                            sub_account: Some(sub.into()),
                            parent_token: Some(parent_token),
                        },
                    );
                    row["authorized"] = json!(true);
                    row["accessToken"] = json!(token);
                    row["access_token"] = json!(token);
                    row["gmtExpired"] = json!(chrono::DateTime::from_timestamp_millis(expires)
                        .unwrap()
                        .format("%Y%m%d%H%M%S%3f%z")
                        .to_string());
                } else if name.ends_with("cancel") {
                    s.tokens.retain(|_, t| {
                        !(t.account == owner
                            && t.app_key == s.current_app
                            && t.sub_account.as_deref() == Some(sub))
                    });
                    row["authorized"] = json!(false);
                    row.as_object_mut().unwrap().remove("accessToken");
                    row.as_object_mut().unwrap().remove("access_token");
                }
                row["adminOwnerId"] = json!(s.accounts[owner].login);
                row["adminUserId"] = json!(s.accounts[owner].user_id);
                row["clientId"] = json!(s.current_app);
                row["clientName"] = json!("Mock application");
                row["ownerId"] = row["loginId"].clone();
                row["subOwnerId"] = row["loginId"].clone();
                row["subUserId"] = row
                    .get("subUserId")
                    .cloned()
                    .unwrap_or(json!(s.accounts[owner].user_id * 1000 + 1));
                row["userId"] = row["subUserId"].clone();
                row["resourceScopes"] = json!(s.accounts[owner].scopes.join(","));
                row["site"] = json!("china");
                row["subAuth"] = json!(true);
                row["status"] = json!(if row["authorized"] == true {
                    "authorized"
                } else {
                    "cancelled"
                });
                s.put("subaccounts", &key, row.clone());
                results.insert(sub.into(), row);
            }
            let value = if name.ends_with("cancel") {
                json!(true)
            } else if name.ends_with("list") {
                json!(results.values().collect::<Vec<_>>())
            } else {
                json!(results)
            };
            Ok(json!({"success":true,"returnValue":value}))
        }
        "category.translation.getById"
        | "category.translation.getByKeyword"
        | "alibaba.category.get"
        | "alibaba.category.searchByKeyword"
        | "alibaba.category.attribute.get" => {
            let id = number(p, &["categoryId", "categoryID"]).unwrap_or(101);
            if ![0, 1, 101].contains(&id) {
                return fail("CATEGORY_NOT_EXISTS", "category not found");
            }
            let language = text(p, &["language"]);
            let leaf = json!({"id":101,"categoryId":101,"categoryID":101,"name":"日用百货","chineseName":"日用百货","translatedName":"Daily necessities","language":language,"leaf":true,"isLeaf":true,"level":2,"parentIDs":[1],"parentCateId":1,"childIDs":[],"childCategorys":[],"children":[],"minOrderQuantity":1,"categoryType":"normal"});
            let parent = json!({"id":1,"categoryId":1,"categoryID":1,"name":"家居用品","chineseName":"家居用品","translatedName":"Home supplies","language":language,"leaf":false,"isLeaf":false,"level":1,"parentIDs":[],"parentCateId":0,"childIDs":[101],"childCategorys":[leaf],"children":[leaf],"minOrderQuantity":1,"categoryType":"normal"});
            let selected = if id == 0 || id == 1 {
                parent.clone()
            } else {
                leaf.clone()
            };
            let keyword = text(p, &["keyword", "keywords"]);
            let rows = if name.contains("Keyword") {
                [parent, leaf]
                    .into_iter()
                    .filter(|c| {
                        keyword.is_empty()
                            || c["name"].as_str().unwrap_or("").contains(&keyword)
                            || c["translatedName"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&keyword.to_lowercase())
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![selected.clone()]
            };
            if name == "alibaba.category.searchByKeyword" && rows.is_empty() {
                return fail("查询失败", "查询失败");
            }
            let mut out = selected;
            out["items"] = json!(rows);
            out["categoryInfo"] = json!(rows);
            out["attributes"] = if id == 101 {
                json!([{"attrID":1,"name":"颜色","enName":"Color","required":false,"isSKUAttribute":true,"inputType":"select","attrValues":[{"attrValueID":1,"name":"白色","enName":"White","isSKU":true,"childAttrs":[]}]}])
            } else {
                json!([])
            };
            out["levelAttrRelList"] = json!([]);
            out["attributeLevelMapStr"] = json!({});
            out["succes"] = json!("true");
            Ok(out)
        }
        "product.search.queryProductDetail"
        | "alibaba.cross.productInfo"
        | "alibaba.product.simple.get"
        | "alibaba.fenxiao.productInfo.get"
        | "product.skuinfo.get"
        | "alibaba.cps.queryOfferDetailActivity" => {
            let id = number(p, &["offerId", "productId", "productID"]).unwrap_or(0);
            let product = s
                .products
                .get(&id)
                .filter(|v| v.active)
                .ok_or_else(|| Error {
                    code: format!("商品[{id}]不存在"),
                    message: format!("商品[{id}]不存在"),
                })?;
            let language = crate::search::language(p)?;
            let mut value = product_value(
                product,
                &s.base_url,
                &s.accounts[&product.seller],
                &s.current_app,
            );
            value["language"] = json!(language);
            value["subjectTrans"] = json!(product
                .merchandise
                .translations
                .get(&language)
                .unwrap_or(&product.title));
            if name == "alibaba.cross.productInfo"
                && !s
                    .records
                    .get("follows")
                    .is_some_and(|m| m.contains_key(&format!("{owner}:{id}")))
            {
                return fail("没有该商品的查询权限", "没有该商品的查询权限");
            }
            let mut out = value.clone();
            out["productInfo"] = value.clone();
            out["data"] = value;
            Ok(out)
        }
        "alibaba.cross.productList" | "alibaba.pifatuan.product.detail.list" => {
            let ids = find(p, &["productIdList", "offerIds"])
                .cloned()
                .unwrap_or(json!([]));
            let ids: Vec<i64> = if let Some(a) = ids.as_array() {
                a.iter()
                    .filter_map(|v| {
                        v.as_i64()
                            .or_else(|| v.as_str().and_then(|x| x.parse().ok()))
                    })
                    .collect()
            } else {
                ids.as_str()
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|x| x.parse().ok())
                    .collect()
            };
            let mut rows = vec![];
            for id in ids {
                let product = s.products.get(&id).ok_or_else(|| Error {
                    code: "PRODUCT_NOT_EXISTS".into(),
                    message: format!("商品不存在: {id}"),
                })?;
                rows.push(product_value(
                    product,
                    &s.base_url,
                    &s.accounts[&product.seller],
                    &s.current_app,
                ));
            }
            let mut out = page(rows.clone(), p)?;
            out["productList"] = json!(rows);
            Ok(out)
        }
        "product.search.keywordQuery"
        | "product.search.imageQuery"
        | "product.search.imageQueryBasic"
        | "product.search.querySellerOfferList"
        | "product.search.offerRecommend"
        | "product.related.recommend"
        | "alibaba.product.suggest.crossBorder"
        | "alibaba.public.image.similar.offer.search"
        | "product.keyword.search"
        | "product.keywords.search"
        | "alibaba.pifatuan.product.list"
        | "alibaba.pifatuan.product.match.get"
        | "jxhy.product.getPageList"
        | "pool.product.pull"
        | "alibaba.cps.listOfferPageQuery"
        | "alibaba.cps.getCpsRecommendOfferList"
        | "alibaba.cps.getCpsRecommendSameOfferList" => {
            crate::cps::validate(name, p)?;
            crate::search::query(s, name, p)
        }
        "pool.product.total" => {
            let pool = text(p, &["palletId"]);
            let category = text(p, &["categoryId"]);
            if !s
                .products
                .values()
                .any(|v| v.merchandise.pools.contains(&pool))
            {
                return fail("POOL_ACCESS_DENIED", "unknown pool");
            }
            let count = s
                .products
                .values()
                .filter(|v| {
                    v.active
                        && v.merchandise.pools.contains(&pool)
                        && (category.is_empty()
                            || category == "1"
                            || category == v.category.to_string())
                })
                .count();
            Ok(json!({"model":count,"total":count,"success":"true"}))
        }
        "product.topList.query" => crate::analytics::rank(s, p),
        "product.search.topKeyword" => {
            if text(p, &["hotKeywordType"]) != "cate" {
                return fail("PARAM_ILLEGAL", "hotKeywordType must be cate");
            }
            let category = text(p, &["sourceId"]);
            let language = text(p, &["country"]);
            let rows=s.products.values().filter(|v|v.active && (category=="1" || v.category.to_string()==category)).map(|v|json!({"seKeyword":v.title,"seKeywordTranslation":v.merchandise.translations.get(&language).unwrap_or(&v.title)})).collect::<Vec<_>>();
            Ok(json!({"items":rows}))
        }
        "product.search.keywordSNQuery" => {
            crate::search::language(p)?;
            let query = crate::search::query(s, name, p)?;
            let rows = if query["total"].as_u64().unwrap_or(0) > 0 {
                vec![
                    json!({"id":"978","name":"日用百货","translateName":"Daily necessities","children":[{"id":"1352","name":"家居用品","translateName":"Home supplies"}]}),
                ]
            } else {
                vec![]
            };
            Ok(json!({"items":rows}))
        }
        "alibaba.pifatuan.product.search.tag.list" => {
            let ids: std::collections::BTreeSet<_> = s
                .products
                .values()
                .flat_map(|p| p.merchandise.rules.iter())
                .collect();
            Ok(
                json!({"items":ids.into_iter().filter_map(|id|id.parse::<i64>().ok()).map(|id|json!({"tagId":id,"ruleId":id,"tagName":format!("模拟规则{id}"),"ruleName":format!("模拟规则{id}"),"ruleDesc":"内置规则商品集合"})).collect::<Vec<_>>()}),
            )
        }
        "jxhy.productFilter.get" => Ok(
            json!({"items":[{"key":"jxhy","desc":"精选货源"},{"key":"isOnePsale","desc":"支持一件代发"},{"key":"isOnePsaleFreePostage","desc":"一件代发包邮"}]}),
        ),
        "product.analyze.getPerdaySellQuantityTrend"
        | "product.analyze.getPerdaySellQuantityTrendNew"
        | "product.analyze.getRepurchaseRateTrend"
        | "product.analyze.getRepurchaseRateTrendNew"
        | "product.analyze.getThirtyDayMedianPriceTrend"
        | "product.analyze.getThirtyDayMedianPriceTrendNew" => crate::analytics::trend(s, name, p),
        "alibaba.product.follow"
        | "alibaba.product.follow.crossborder"
        | "product.kjdistribute.addRelation"
        | "alibaba.fenxiao.relationadd"
        | "alibaba.product.unfollow.crossborder" => {
            let id = number(p, &["productId", "offerId"]).unwrap_or(0);
            if !s.products.contains_key(&id) {
                return fail("PRODUCT_NOT_EXISTS", "product not found");
            }
            let key = format!("{owner}:{id}");
            if name.contains("unfollow") {
                if let Some(m) = s.records.get_mut("follows") {
                    m.remove(&key);
                }
            } else {
                s.put("follows", &key, json!({"offerId":id,"productId":id}));
                let seller = s.products[&id].seller.clone();
                if !s
                    .records
                    .get("supplier_relations")
                    .is_some_and(|r| r.contains_key(&format!("{owner}:{seller}")))
                {
                    s.put("supplier_relations",&format!("{owner}:{seller}"),json!({"memberId":seller,"consignCreateTime":s.now(),"consignStatus":"cooperating"}));
                }
                s.emit(
                    "CROSSBOARD_CROSSBOARD_ADD_SUPPLY",
                    owner,
                    json!({"offerId":id}),
                );
            }
            Ok(json!({"success":true,"code":0,"message":"success","data":true}))
        }
        "alibaba.cross.syncProductListPushed" => {
            let ids = array(p, &["productIdList"])
                .filter(|a| !a.is_empty())
                .ok_or_else(|| Error {
                    code: "OFFERIDS_IS_NULL".into(),
                    message: "productIdList required".into(),
                })?;
            for id in ids {
                let id = id.as_i64().unwrap_or(0);
                if !s.products.contains_key(&id) {
                    return fail("PRODUCT_NOT_EXISTS", "product not found");
                }
                s.put("follows", &format!("{owner}:{id}"), json!({"offerId":id}));
                s.put(
                    "distribution",
                    &format!("{owner}:{id}"),
                    json!({"offerId":id,"status":"pending"}),
                );
                s.emit(
                    "PRODUCT_PRODUCT_CROSSBOARD_INFORM",
                    owner,
                    json!({"offerId":id}),
                );
            }
            Ok(json!({"success":true,"data":true}))
        }
        "alibaba.product.push.syncPushProductResult" => {
            let result = p.get("pushProductResults").ok_or_else(|| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "pushProductResults required".into(),
            })?;
            if text(p, &["definitionId"]).is_empty() {
                return fail("getPlatformDefId_IS_EMPTY", "platform definition required");
            }
            if text(result, &["productIdInSource"]).is_empty() {
                return fail("getProductIdInSource_IS_EMPTY", "source product required");
            }
            if text(result, &["productIdInTargetPlatform"]).is_empty() {
                return fail(
                    "getProductIdInTargetPlatform_IS_EMPTY",
                    "target product required",
                );
            }
            for result in std::iter::once(result) {
                let id = number(
                    result,
                    &[
                        "productIdInSource",
                        "productId",
                        "offerId",
                        "sourceProductId",
                    ],
                )
                .unwrap_or(0);
                s.put("distribution", &format!("{owner}:{id}"), result.clone());
            }
            Ok(json!({"success":true}))
        }
        "fenxiao.distributebill.getList" | "alibaba.fenxiao.chosen.offerlist.get" => {
            let unique = text(p, &["uniqueKey"]);
            let rows = s
                .rows("distribution", Some(owner))
                .into_iter()
                .filter(|r| unique.is_empty() || r["uniqueKey"] == unique)
                .collect();
            page(rows, p)
        }
        "fenxiao.distributebill.removeall" | "alibaba.fenxiao.chosen.offerlist.removeall" => {
            if let Some(rows) = s.records.get_mut("distribution") {
                let unique = text(p, &["uniqueKey"]);
                rows.retain(|k, r| {
                    !k.starts_with(&format!("{owner}:"))
                        || (!unique.is_empty() && r["uniqueKey"] != unique)
                });
            }
            Ok(json!({"success":true,"data":true}))
        }
        "product.distribute.cnt.put" => {
            let count = number(p, &["count"]).unwrap_or(-1);
            if count < 0 {
                return fail("PARAM_ILLEGAL", "count must be nonnegative");
            }
            s.put("distribution_counts", owner, json!({"count":count}));
            Ok(json!({"success":true,"data":true}))
        }
        "product.distribute.cnt.get" => {
            let count = s
                .records
                .get("distribution_counts")
                .and_then(|m| m.get(owner))
                .and_then(|r| r.get("count"))
                .cloned()
                .unwrap_or(json!(0));
            Ok(json!({"count":count,"data":count}))
        }
        "alibaba.fenxiao.buyer.outshop.add"
        | "alibaba.fenxiao.buyer.outshop.delete"
        | "alibaba.fenxiao.buyer.outproduct.relation.add"
        | "alibaba.fenxiao.buyer.outproduct.relation.delete"
        | "alibaba.fenxiao.buyer.outproduct.relation.get" => {
            let shop = text(p, &["outShopCode"]);
            let channel = text(p, &["channel"]);
            let item = text(p, &["outItemCode"]);
            let table = if name.contains("outproduct") {
                "outproducts"
            } else {
                "outshops"
            };
            let key = format!("{owner}:{channel}:{shop}:{item}");
            if name.ends_with("add") {
                if table == "outproducts"
                    && !s
                        .records
                        .get("outshops")
                        .is_some_and(|m| m.contains_key(&format!("{owner}:{channel}:{shop}:")))
                {
                    return fail("SHOP_NOT_EXISTS", "register shop first");
                }
                s.put(table, &key, p.clone());
                if table == "outproducts" {
                    s.emit("PRODUCT_FENXIAO_ADD_SUPPLY", owner, p.clone());
                }
                Ok(json!({"success":true,"data":true}))
            } else if name.ends_with("delete") {
                let deleted = s
                    .records
                    .get_mut(table)
                    .and_then(|m| m.remove(&key))
                    .is_some();
                if table == "outshops" {
                    if let Some(m) = s.records.get_mut("outproducts") {
                        m.retain(|k, _| !k.starts_with(&key));
                    }
                }
                Ok(json!({"success":true,"data":deleted}))
            } else {
                let row = s
                    .records
                    .get(table)
                    .and_then(|m| m.get(&key))
                    .filter(|r| number(r, &["offerId"]) == number(p, &["offerId"]))
                    .cloned()
                    .ok_or_else(|| Error {
                        code: "RELATION_NOT_EXISTS".into(),
                        message: "relation not found for offer".into(),
                    })?;
                Ok(json!({"data":serde_json::to_string(&row).unwrap(),"success":true}))
            }
        }
        "alibaba.search.relation.supply" | "alibaba.relation.querySuppliers" => {
            crate::accounts_business::suppliers(s, p, owner)
        }
        "alibaba.product.genDijiaFxUrl"
        | "alibaba.product.genTongkuanFxUrl"
        | "dkey.get"
        | "alibaba.lpAuthToken.generate" => {
            let id = s.next_id();
            let key = format!("mock-{id}");
            s.put("links", &format!("{owner}:{key}"), p.clone());
            Ok(
                json!({"success":true,"dkey":key,"token":key,"timeLimit":3600,"data":key,"url":format!("{}/selection/{key}",s.base_url)}),
            )
        }
        "alibaba.lpIsvInfo.createSubcustomer" => {
            let customer = text(p, &["customerId"]);
            let sub = text(p, &["subCustomerId"]);
            if sub.is_empty() || !sub.chars().all(|c| c.is_ascii_alphanumeric()) {
                return fail("PARAM_ILLEGAL", "subCustomerId must be alphanumeric");
            }
            let encrypted = format!("mock-sub-{customer}-{sub}");
            let row = json!({"customerId":customer,"subCustomerId":sub,"subCustomerName":text(p,&["subCustomerName"]),"encryptSubCustomerId":encrypted,"landingPageUrl":format!("{}/selection/{encrypted}",s.base_url)});
            s.put(
                "subcustomers",
                &format!("{owner}:{customer}:{sub}"),
                row.clone(),
            );
            Ok(row)
        }
        "trade.cross.orderSync" | "order.relation.write" => {
            let row = p
                .get(if name == "trade.cross.orderSync" {
                    "orderParam"
                } else {
                    "orderRelationParam"
                })
                .unwrap_or(p);
            let key = text(row, &["orderId"]);
            if key.is_empty() {
                return fail("PARAM_ILLEGAL", "external orderId required");
            }
            if name == "order.relation.write" {
                let oid = number(row, &["purchaseParentOrderId"]).unwrap_or(0);
                let eid = number(row, &["purchaseOrderId"]).unwrap_or(0);
                let order = buyer_order(s, oid, owner)?;
                if eid < oid * 100 || eid >= oid * 100 + order.lines.len() as i64 {
                    return fail(
                        "ORDER_ENTRY_NOT_EXISTS",
                        "purchaseOrderId must be a child of purchaseParentOrderId",
                    );
                }
            } else {
                if !["payed", "success", "close"].contains(&text(row, &["status"]).as_str()) {
                    return fail("PARAM_ILLEGAL", "unsupported downstream order status");
                }
                let mut rows = vec![row];
                if let Some(children) = array(row, &["subOrderParamList"]) {
                    rows.extend(children);
                }
                if rows
                    .iter()
                    .any(|r| number(r, &["refundStatus"]).is_some_and(|v| ![0, 1].contains(&v)))
                {
                    return fail("PARAM_ILLEGAL", "refundStatus must be 0 or 1");
                }
            }
            s.put(name, &format!("{owner}:{key}"), row.clone());
            Ok(json!({"success":true,"data":true,"result":"success"}))
        }
        "account.business.save"
        | "trade.cross.logisticsOrderSync"
        | "alibaba.feedback.out.product.add"
        | "global.agent.auth" => {
            let ext = text(
                p,
                &[
                    "outOrderId",
                    "outMemberId",
                    "userIdentity",
                    "outUserId",
                    "accountId",
                ],
            );
            let key = if ext.is_empty() {
                s.next_id().to_string()
            } else {
                ext
            };
            s.put(name, &format!("{owner}:{key}"), p.clone());
            Ok(json!({"success":true,"saveResult":true,"id":key,"data":true,"result":"success"}))
        }
        "material.query.sync" => {
            let key = text(p, &["offerCollectionId", "materialId", "id"]);
            let row = s
                .records
                .get("materials")
                .and_then(|m| m.get(&format!("{owner}:{key}")))
                .ok_or_else(|| Error {
                    code: "NO_AUTH".into(),
                    message: "material is not authorized for this account".into(),
                })?;
            if number(p, &["offerId"]).is_some_and(|id| number(row, &["offerId"]) != Some(id))
                || row
                    .get("outerUserIdentity")
                    .is_some_and(|token| token != &json!(text(p, &["outerUserIdentity"])))
            {
                return fail("NO_AUTH", "material identity does not match");
            }
            Ok(row.clone())
        }
        "product.image.upload"
        | "alibaba.trade.uploadRefundVoucher"
        | "newtoncloud.file.upload" => {
            let data = text(p, &["imageData", "contentBase64", "imageBase64", "base64"]);
            let bytes = if let Some(values) = array(p, &["imageData"]) {
                values
                    .iter()
                    .map(|v| {
                        v.as_i64()
                            .filter(|n| (-128..=255).contains(n))
                            .map(|n| n as u8)
                            .ok_or_else(|| Error {
                                code: if name == "alibaba.trade.uploadRefundVoucher" {
                                    "4000"
                                } else {
                                    "PARAM_ILLEGAL"
                                }
                                .into(),
                                message: "invalid image byte".into(),
                            })
                    })
                    .collect::<Result<Vec<u8>>>()?
            } else {
                crate::catalog::decode_base64(&data).map_err(|_| Error {
                    code: if name == "alibaba.trade.uploadRefundVoucher" {
                        "4000"
                    } else {
                        "PARAM_ILLEGAL"
                    }
                    .into(),
                    message: "invalid base64 content".into(),
                })?
            };
            if bytes.is_empty() {
                return fail(
                    if name == "alibaba.trade.uploadRefundVoucher" {
                        "4000"
                    } else {
                        "PARAM_ILLEGAL"
                    },
                    "file content required",
                );
            }
            if bytes.len() > 8 * 1024 * 1024 {
                return fail("FILE_TOO_LARGE", "file exceeds 8 MiB");
            }
            if name == "alibaba.trade.uploadRefundVoucher"
                && (bytes.len() >= 1024 * 1024
                    || !bytes.starts_with(&[255, 216])
                    || !bytes.ends_with(&[255, 217]))
            {
                return fail("4000", "voucher must be JPEG smaller than 1 MiB");
            }
            let fid = s.next_id().to_string();
            let url = format!("{}/files/{fid}", s.base_url);
            let encoded = STANDARD.encode(&bytes);
            s.put(
                "files",
                &format!("{owner}:{fid}"),
                json!({"id":fid,"content":encoded,"filename":text(p,&["filename"]),"size":bytes.len(),"voucher":name=="alibaba.trade.uploadRefundVoucher"}),
            );
            let mut upload = json!({"imageId":fid,"fileId":fid,"url":url,"imageUrl":url,"fileUrl":url,"downloadUrl":url,"agentDownloadUrl":url,"imageDomain":s.base_url,"imageRelativeUrl":format!("/files/{fid}"),"relativePath":format!("uploads/{fid}"),"fileSize":bytes.len()});
            if name == "product.image.upload" {
                upload["result"] = json!(fid);
            }
            Ok(upload)
        }
        "coupon.optimal.claim" => {
            let ids = array(p, &["offerIds"]).ok_or_else(|| Error {
                code: "PARAM_ILLEGAL".into(),
                message: "offerIds required".into(),
            })?;
            let mut rows = vec![];
            for id in ids {
                let id = id.as_i64().unwrap_or(0);
                if !s.products.contains_key(&id) {
                    return fail("PRODUCT_NOT_EXISTS", "product not found");
                }
                let coupon = json!({"offerId":id,"couponId":format!("coupon-{id}"),"discount":1,"claimed":true});
                s.put("coupons", &format!("{owner}:{id}"), coupon.clone());
                rows.push(coupon);
            }
            Ok(json!({"couponIds":rows.iter().map(|r|r["couponId"].clone()).collect::<Vec<_>>()}))
        }
        _ => fail("UNHANDLED", "unhandled"),
    }
}

pub fn decode_base64(data: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    let content = if data.starts_with("data:") {
        data.split_once(',').map(|(_, s)| s).unwrap_or(data)
    } else {
        data
    };
    STANDARD
        .decode(content)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(content))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(content))
}
