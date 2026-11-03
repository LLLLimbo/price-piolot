use crate::{
    query::{self, identity},
    state::Store,
};
use serde_json::{json, Value};
pub fn handle(s: &Store, id: &str, input: &Value) -> Value {
    let fuzzy = input["matchType"] == 1;
    let (start, end) = query::dates(input, s.now(), id);
    let historical = ["date", "searchDate", "historyDate"]
        .iter()
        .any(|k| input[*k].as_str().is_some_and(|v| !v.is_empty()));
    let mut rows: Vec<_> = s
        .market
        .get(id.strip_prefix("ozon-").unwrap())
        .into_iter()
        .flatten()
        .filter(|row| {
            let date = row["queryTime"].as_str().or_else(|| row["_date"].as_str());
            if date.is_some_and(|d| d < start.as_str() || d > end.as_str())
                || (historical && date.is_none())
            {
                return false;
            }
            let text = row["query"].as_str().unwrap_or("");
            if id == "ozon-keyword-mining"
                && !query::text_match(text, input["keyword"].as_str().unwrap(), true)
            {
                return false;
            }
            for (key, exclude) in [
                ("includeKeywords", false),
                ("excludeKeywords", true),
                ("keywords", false),
            ] {
                if let Some(words) = input[key].as_array().filter(|a| !a.is_empty()) {
                    let matches = words
                        .iter()
                        .any(|w| query::text_match(text, w.as_str().unwrap_or(""), fuzzy));
                    if matches == exclude {
                        return false;
                    }
                }
            }
            if let Some(ids) = input["categories"].as_array().filter(|a| !a.is_empty()) {
                if !ids
                    .iter()
                    .any(|id| row["categories"].as_array().is_some_and(|a| a.contains(id)))
                {
                    return false;
                }
            }
            if id == "ozon-keyword-back-search" {
                if input["skuIds"].as_array().is_none_or(|ids| {
                    !ids.iter().any(|id| {
                        row["_skuIds"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| identity(v) == identity(id)))
                    })
                }) {
                    return false;
                }
                if !query::selected(&input["type"], &row["dimension"]["type"]) {
                    return false;
                }
            }
            let mut r = (*row).clone();
            r["price"] = row["avgPrice"].clone();
            r["products"] = row["productCount"].clone();
            let mut filter = input.clone();
            filter.as_object_mut().unwrap().remove("keywords");
            query::filters(&r, &filter, s.now())
        })
        .cloned()
        .collect();
    if input["hasVariant"] == 1 {
        let mut seen = std::collections::BTreeSet::new();
        rows.retain(|r| seen.insert(identity(&r["query"])));
    }
    for row in &mut rows {
        row.as_object_mut()
            .unwrap()
            .retain(|key, _| !key.starts_with('_'));
    }
    let mut response = query::paginate(rows, input, false, "searchVolume");
    if response["orders"].is_null() {
        response["orders"] = json!([]);
    }
    response
}
