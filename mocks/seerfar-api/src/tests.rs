use crate::{admin, catalog, contract, query, state::Store, tasks};
use serde_json::json;

fn seed() -> Store {
    Store::seed("http://server:8080".into(), Some(1_793_232_000_000))
}
#[test]
fn frozen_contracts_and_seed_are_consistent() {
    let s = seed();
    s.validate().unwrap();
    assert_eq!(contract::endpoints().len(), 22);
    for e in contract::endpoints() {
        assert!(s.policies.contains_key(e["id"].as_str().unwrap()));
    }
}
#[test]
fn monetary_aggregates_and_long_identifiers_preserve_precision() {
    let rows = vec![json!({"revenue":0.1}), json!({"revenue":0.2})];
    assert_eq!(query::sum(&rows, "revenue").to_string(), "0.3");
    assert_eq!(
        query::identity(&json!(9_223_372_036_854_775_807u64)),
        "9223372036854775807"
    );
}
#[test]
fn empty_ranges_do_not_discard_null_metrics() {
    assert!(query::range_matches(
        &serde_json::Value::Null,
        &json!({"min":null,"max":null})
    ));
    assert!(!query::range_matches(
        &serde_json::Value::Null,
        &json!({"min":0})
    ));
    assert!(query::range_matches(&json!(0), &json!({"min":0,"max":0})));
}
#[test]
fn advancing_over_two_stages_settles_a_failed_task_once() {
    let mut s = seed();
    let before = s.keys["seerfar-test-key"].credit_used;
    tasks::review_add(&mut s, "seerfar-test-key", &json!({"skus":["999"]}))
        .ok()
        .unwrap();
    s.fixed_time = Some(s.now() + 5000);
    s.tick();
    s.tick();
    assert_eq!(s.keys["seerfar-test-key"].credit_used, before);
    assert_eq!(s.settlements.values().next().unwrap().refunded, 200);
    s.validate().unwrap();
}
#[test]
fn malformed_snapshot_and_fault_rules_are_rejected() {
    let mut s = seed();
    s.keys.get_mut("seerfar-test-key").unwrap().account = "missing".into();
    assert!(s.validate().is_err());
    assert!(admin::validate_faults(&[json!({"path":"/missing","remaining":1})]).is_err());
    let mut s = seed();
    tasks::review_add(&mut s, "seerfar-test-key", &json!({"skus":["149222760"]}))
        .ok()
        .unwrap();
    s.keys.get_mut("seerfar-test-key").unwrap().credit_used = 0;
    assert!(s.validate().is_err());
}
#[test]
fn list_summaries_match_filtered_products() {
    let s = seed();
    let result = catalog::handle(
        &s,
        "ozon-shop-search",
        &json!({"id":118364,"price":{"min":1800},"page":{"pageNumber":1,"pageSize":20}}),
    );
    assert_eq!(result["productCount"], 1);
    assert_eq!(result["products"][0]["sku"], 149222761);
    assert_eq!(result["totalRevenue"], result["products"][0]["revenue"]);
}
