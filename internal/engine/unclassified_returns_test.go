package engine

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestUnclassifiedAfterSalesBlocksCompletePricing(t *testing.T) {
	in := basic()
	if err := json.Unmarshal([]byte(`{"unclassified_rate":0.05676186583558984,"cancel_rate":0.032187196648444286}`), &in.Product.Returns); err != nil {
		t.Fatal(err)
	}
	r, err := Solve(in)
	if err == nil || r.Recommended != nil || !strings.Contains(err.Error(), "待分类") {
		t.Fatalf("unclassified returns produced pricing: %+v %v", r, err)
	}
}

func TestUnclassifiedAfterSalesCannotBeNegative(t *testing.T) {
	in := basic()
	json.Unmarshal([]byte(`{"unclassified_rate":-0.01}`), &in.Product.Returns)
	if err := Validate(in); err == nil {
		t.Fatal("negative unknown rate accepted")
	}
}
