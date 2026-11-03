package app

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestProfitCheckIsSavedAndStoreScoped(t *testing.T) {
	h, _ := testServer(t)
	_, boot := call(t, h, "GET", "/api/stores/default/bootstrap", nil)
	var products []map[string]any
	json.Unmarshal(boot["products"], &products)
	p := products[0]
	p["id"] = "profit-check-fixture"
	p["returns"].(map[string]any)["unclassified_rate"] = .05676186583558984
	sample := map[string]any{"order_ref": "sample-order", "date": "2026-10-23", "source": "订单截图", "carrier": "邮政", "freight_kind": "estimated", "fee_label": "原型基线 A", "fee_kind": "prototype", "amount": 29, "procurement": 7.7, "freight": 11.1, "commission_rate": .16, "fixed_fee": 1.36, "reported_profit": 4.2}
	p["profit_check"] = sample
	code, saved := call(t, h, "POST", "/api/stores/default/products", p)
	if code != 200 {
		t.Fatalf("%d %s", code, saved)
	}
	var got map[string]any
	json.Unmarshal(saved["product"], &got)
	if got["profit_check"] == nil {
		t.Fatal("source order sample lost on save")
	}
	req := map[string]any{"product_id": p["id"], "sample": sample}
	code, checked := call(t, h, "POST", "/api/stores/default/profit-checks/evaluate", req)
	if code != 200 || string(checked["status"]) != `"matched"` || string(checked["profit"]) != "4.2" {
		t.Fatalf("%d %s", code, checked)
	}
	code, _ = call(t, h, "POST", "/api/stores/demo-second/profit-checks/evaluate", req)
	if code != 404 {
		t.Fatalf("cross-store sample product accepted: %d", code)
	}
	// Product-level aggregate after-sales must remain intact and must not be used as this order's outcome.
	if got["returns"].(map[string]any)["unclassified_rate"] != .05676186583558984 {
		t.Fatal("changed product-wide after-sales data")
	}
	code, calc := call(t, h, "POST", "/api/stores/default/calculate", map[string]any{"product": got})
	if code != 200 || !strings.Contains(string(calc["result"]), "待分类") {
		t.Fatal("sample verification bypassed normal pricing safeguards")
	}
}
