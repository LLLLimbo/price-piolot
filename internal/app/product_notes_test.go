package app

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestProductNotesAndUnclassifiedReturnsRoundTrip(t *testing.T) {
	h, _ := testServer(t)
	_, boot := call(t, h, "GET", "/api/stores/default/bootstrap", nil)
	var products []map[string]any
	json.Unmarshal(boot["products"], &products)
	p := products[0]
	p["id"] = "source-reference"
	p["notes"] = "真实来源；统计范围2025-01-11至2026-11-03；合并售后比例不能作签收后退货率"
	returns := p["returns"].(map[string]any)
	returns["unclassified_rate"] = .05676186583558984
	code, saved := call(t, h, "POST", "/api/stores/default/products", p)
	if code != 200 || !strings.Contains(string(saved["product"]), "真实来源") || !strings.Contains(string(saved["product"]), "unclassified_rate") {
		t.Fatalf("lost source: %d %s", code, saved)
	}
	code, calc := call(t, h, "POST", "/api/stores/default/calculate", map[string]any{"product": json.RawMessage(saved["product"])})
	if code != 200 || !strings.Contains(string(calc["result"]), "待分类") {
		t.Fatalf("%d %s", code, calc)
	}
	var runID string
	json.Unmarshal(calc["id"], &runID)
	code, run := call(t, h, "GET", "/api/stores/default/runs/"+runID, nil)
	if code != 200 || !strings.Contains(string(run["input"]), "真实来源") || !strings.Contains(string(run["input"]), "unclassified_rate") {
		t.Fatal("reference omitted from frozen run")
	}
}
