use crate::{
    contract,
    query::{self, identity, number},
    state::Store,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn public_product(mut p: Value) -> Value {
    for key in ["removed", "nameEn", "nameRu"] {
        p.as_object_mut().unwrap().remove(key);
    }
    p
}
fn summary(rows: &[Value]) -> Value {
    let mut fulfillment = json!({});
    for p in rows {
        for f in p["fulfillment"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            fulfillment[f] = json!(fulfillment[f].as_u64().unwrap_or(0) + 1);
        }
    }
    let total_sales = query::sum(rows, "sales");
    json!({
        "productCount": rows.len(),
        "rating": query::mean(rows, "reviewRating"),
        "totalRevenue": query::sum(rows, "revenue"),
        "totalSales": total_sales,
        "dailySales": (number(&total_sales) / 30.0).floor() as u64,
        "fulfillment": fulfillment,
    })
}
fn variants(s: &Store, sku: &str, lang: &str) -> Vec<Value> {
    let Some(product) = s.products.get(sku) else {
        return vec![];
    };
    let ids = product["variationIds"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![product["sku"].clone()]);
    ids.iter()
        .filter_map(|id| s.products.get(&identity(id)))
        .map(|p| {
            let mut p = p.clone();
            if let Some(a) = p["characteristics"].as_array_mut() {
                for c in a {
                    if lang == "en-US" {
                        c["name"] = c
                            .get("nameEn")
                            .cloned()
                            .unwrap_or_else(|| c["sourceName"].clone());
                        c["value"] = c
                            .get("valueEn")
                            .cloned()
                            .unwrap_or_else(|| c["value"].clone());
                    } else if lang == "ru-RU" {
                        c["name"] = c["sourceName"].clone();
                        c["value"] = c
                            .get("valueRu")
                            .cloned()
                            .unwrap_or_else(|| c["value"].clone());
                    }
                    for key in ["nameEn", "valueEn", "valueRu"] {
                        c.as_object_mut().unwrap().remove(key);
                    }
                }
            }
            public_product(p)
        })
        .collect()
}
pub fn handle(s: &Store, id: &str, input: &Value) -> Value {
    let sku = identity(&input["sku"]);
    let language = input["clientLanguage"].as_str().unwrap_or("zh-CN");
    match id {
        "ozon-product-detail-search" => {
            let Some(p) = s.products.get(&sku) else {
                return Value::Null;
            };
            let (start, end) = query::dates(input, s.now(), id);
            let history: Vec<_> = s
                .history
                .get(&sku)
                .into_iter()
                .flatten()
                .filter(|v| {
                    v["date"]
                        .as_str()
                        .is_some_and(|d| d >= start.as_str() && d <= end.as_str())
                })
                .cloned()
                .collect();
            let total_sales = query::sum(&history, "sales");
            let days = (chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d").unwrap()
                - chrono::NaiveDate::parse_from_str(&start, "%Y-%m-%d").unwrap())
            .num_days()
                + 1;
            json!({
                "product": public_product(p.clone()),
                "totalSales": total_sales,
                "totalRevenue": query::sum(&history, "revenue"),
                "dailySales": (number(&total_sales) / days as f64).floor() as u64,
                "stock": p["stock"],
                "startDate": start,
                "endDate": end,
                "categoryRanks": p.get("categoryRanks").cloned().unwrap_or_else(|| json!([])),
                "salesTrendVOList": history,
            })
        }
        "ozon-product-similar" => {
            let Some(p) = s.products.get(&sku) else {
                return json!([]);
            };
            json!(s
                .products
                .values()
                .filter(
                    |v| v["categoryInfo"]["category"]["id"] == p["categoryInfo"]["category"]["id"]
                )
                .cloned()
                .map(public_product)
                .collect::<Vec<_>>())
        }
        "ozon-product-monthly-sales-trend" => {
            json!(s.monthly.get(&sku).cloned().unwrap_or_default())
        }
        "ozon-product-variations-characteristics" => {
            let mut found = BTreeMap::new();
            for p in variants(s, &sku, language) {
                for c in p["characteristics"].as_array().into_iter().flatten() {
                    found.insert(
                        identity(&c["sourceName"]),
                        json!({"sourceName":c["sourceName"],"name":c["name"]}),
                    );
                }
            }
            json!(found.into_values().collect::<Vec<_>>())
        }
        "ozon-product-variations-search" => {
            let mut result = query::paginate(variants(s, &sku, language), input, false, "revenue");
            result["timeInterval"] = Value::Null;
            result
        }
        "ozon-product-variations-top" => {
            let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
            for p in variants(s, &sku, language) {
                if let Some(c) = p["characteristics"].as_array().and_then(|a| {
                    a.iter()
                        .find(|c| c["sourceName"] == input["characteristic"])
                }) {
                    groups.entry(identity(&c["value"])).or_default().push(p);
                }
            }
            let top = |field: &str| {
                let mut rows: Vec<_> = groups
                    .iter()
                    .map(|(name, rows)| json!({"name":name,"quantity":query::sum(rows,field)}))
                    .collect();
                query::sort(&mut rows, &Value::Null, "quantity");
                rows.truncate(10);
                rows
            };
            json!({"revenues":top("revenue"),"sales":top("sales")})
        }
        "ozon-shop-brand-search" | "ozon-shop-category-search" => shop_groups(s, id, input),
        _ => listings(s, id, input),
    }
}
fn listings(s: &Store, id: &str, input: &Value) -> Value {
    let mut rows = query::product_rows(s, input, id);
    if id == "ozon-shop-search" {
        rows.retain(|p| identity(&p["sellerId"]) == identity(&input["id"]));
    }
    if id == "ozon-category-search" {
        rows.retain(|p| {
            p["categoryInfo"]["fullCategoryId"]
                .as_array()
                .is_some_and(|a| a.iter().any(|c| *c == input["categoryId"]))
        });
    }
    if input["variationsMerge"] == 1 {
        let mut groups = std::collections::BTreeSet::new();
        rows.retain(|p| {
            groups.insert(identity(p.get("variationsRelationId").unwrap_or(&p["sku"])))
        });
    }
    let average_price = query::mean(&rows, "price");
    let summary = summary(&rows);
    let fallback = if id == "ozon-shop-search" {
        "upTime"
    } else {
        "revenue"
    };
    let mut page = query::paginate(
        rows.into_iter().map(public_product).collect(),
        input,
        false,
        fallback,
    );
    if id == "ozon-product-report-search" {
        let (start, end) = query::dates(input, s.now(), id);
        page["timeInterval"] = json!({"min":start,"max":end});
        return page;
    }
    let mut result = if id == "ozon-shop-search" && page["current"] != 1 {
        json!({})
    } else {
        summary
    };
    let items = if id == "ozon-shop-search" {
        "products"
    } else {
        "productList"
    };
    if id == "ozon-category-search" {
        let (start, end) = query::dates(input, s.now(), id);
        let info = s
            .products
            .values()
            .find(|p| p["categoryInfo"]["category"]["id"] == input["categoryId"])
            .map(|p| p["categoryInfo"].clone())
            .unwrap_or(Value::Null);
        result["id"] = input["categoryId"].clone();
        result["avgPrice"] = average_price;
        result["products"] = page["total"].clone();
        result["sellerType"] = result["fulfillment"].clone();
        result["startDate"] = json!(start);
        result["endDate"] = json!(end);
        result["categoryInfo"] = info;
        result["categoryLibraryName"] = Value::Null;
        result["seasonalityAmplitude"] = Value::Null;
        result["seasonalityCoef"] = Value::Null;
        for k in ["productCount", "fulfillment", "dailySales"] {
            result.as_object_mut().unwrap().remove(k);
        }
    }
    result[items] = page["records"].take();
    result["hasNextPage"] = json!(number(&page["current"]) < number(&page["totalPages"]));
    result
}
fn shop_groups(s: &Store, id: &str, input: &Value) -> Value {
    let brands = id == "ozon-shop-brand-search";
    let field = if brands { "brandId" } else { "categoryInfo" };
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    // Product ranges differ from aggregate ranges; only aggregate rows are filtered below.
    let period = json!({"date":input["date"]});
    for p in query::product_rows(s, &period, id)
        .into_iter()
        .filter(|p| identity(&p["sellerId"]) == identity(&input["id"]))
    {
        let key = if brands {
            identity(&p[field])
        } else {
            identity(&p[field]["category"]["id"])
        };
        groups.entry(key).or_default().push(p);
    }
    let mut rows = vec![];
    for (key, ps) in groups {
        let p = &ps[0];
        let mut row = json!({
            "id": key,
            "totalRevenues": query::sum(&ps, "revenue"),
            "totalSales": query::sum(&ps, "sales"),
            "revenueRate": query::mean(&ps, "revenueRate"),
            "salesRate": query::mean(&ps, "salesRate"),
            "products": ps.len(),
            "avgPrice": query::mean(&ps, "price"),
        });
        if brands {
            row["brandInfo"] = json!({"brandId":p["brandId"],"brandName":p["brandName"],"thumbnailUrl":p["imageUrl"]});
            for (target, source) in [
                ("avgRevenue", "revenue"),
                ("avgSales", "sales"),
                ("avgRating", "reviewRating"),
                ("avgReviewCount", "reviewCount"),
                ("conversionSharing", "convToCartPdp"),
                ("returnCancellationRate", "returnCancellationRate"),
            ] {
                let value = query::mean(&ps, source);
                if !value.is_null() {
                    row[target] = value;
                }
            }
            row["productsWithSales"] =
                json!(ps.iter().filter(|p| number(&p["sales"]) > 0.0).count());
            row["sellers"] = json!(1);
        } else {
            row["categoryInfo"] = p["categoryInfo"].clone();
        }
        let mut filter_row = row.clone();
        for (target, source) in [("sales", "totalSales"), ("revenue", "totalRevenues")] {
            filter_row[target] = row[source].clone();
        }
        let mut filter = input.clone();
        filter.as_object_mut().unwrap().remove("categoryIds");
        if query::filters(&filter_row, &filter, s.now())
            && (brands || query::selected(&input["categoryIds"], &row["id"]))
        {
            rows.push(row);
        }
    }
    let mut result = json!({});
    if query::page(input, false).unwrap().0 == 1 {
        let total_revenue = query::sum(&rows, "totalRevenues");
        let total_sales = query::sum(&rows, "totalSales");
        let top = |field: &str, total: &Value| {
            let mut sorted = rows.clone();
            query::sort(&mut sorted, &Value::Null, field);
            sorted.truncate(10);
            let top_total = query::sum(&sorted, field);
            sorted
                .into_iter()
                .map(|r| {
                    let name = if brands {
                        &r["brandInfo"]["brandName"]
                    } else {
                        &r["categoryInfo"]["category"]["title"]
                    };
                    let mut v = json!({"id":r["id"],"name":name,"quantity":r[field]});
                    if brands {
                        let share = if number(total) == 0.0 {
                            0.0
                        } else {
                            number(&r[field]) / number(total) * 100.0
                        };
                        let top_share = if number(&top_total) == 0.0 {
                            0.0
                        } else {
                            number(&r[field]) / number(&top_total) * 100.0
                        };
                        v["scopeShare"] = json!(share);
                        v["topShare"] = json!(top_share);
                    }
                    v
                })
                .collect::<Vec<_>>()
        };
        let revenues = top("totalRevenues", &total_revenue);
        let sales = top("totalSales", &total_sales);
        result["top"] = json!({"revenues":revenues,"sales":sales});
        if brands {
            for (k, v) in [
                ("total", json!(rows.len())),
                ("totalRevenue", total_revenue.clone()),
                ("totalSales", total_sales.clone()),
                ("revenueShareTop10", share(&revenues, &total_revenue)),
                ("salesShareTop10", share(&sales, &total_sales)),
            ] {
                result["top"][k] = v;
            }
        }
    }
    result[if brands { "brands" } else { "categories" }] =
        query::paginate(rows, input, false, "totalRevenues");
    result
}
fn share(rows: &[Value], total: &Value) -> Value {
    json!(if number(total) == 0.0 {
        0.0
    } else {
        number(&query::sum(rows, "quantity")) / number(total) * 100.0
    })
}
pub fn review_template(sku: &str) -> Value {
    let mut result = json!({});
    for lang in ["CN", "EN", "RU"] {
        let mut report = contract::example("ozon-review-analyse-report");
        report["sku"] = serde_json::from_str(sku).unwrap();
        let mut words = contract::example("ozon-review-analyse-word-frequency");
        words["sku"] = serde_json::from_str(sku).unwrap();
        result[lang] = json!({"report":report,"word_frequency":words});
    }
    let mut statistic = contract::example("ozon-review-analyse-review-statistic");
    statistic["sku"] = json!(sku);
    statistic["reviewStatistic"]["sku"] = json!(sku);
    let mut original = contract::example("ozon-review-analyse-original-review");
    original["skus"] = json!([sku]);
    for r in original["reviews"].as_array_mut().unwrap() {
        r["skus"] = json!(sku);
    }
    result["statistic"] = statistic;
    result["original"] = original;
    result
}
