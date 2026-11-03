package app

import (
	"encoding/json"
	"net/http"
	"pricepilot/internal/engine"
	"testing"
)

func TestStoresAndContextRunsAreIsolatedAndFrozen(t *testing.T) {
	h, db := testServer(t)
	code, boot := call(t, h, "GET", "/api/stores/default/bootstrap", nil)
	if code != 200 {
		t.Fatalf("bootstrap %d", code)
	}
	var products []engine.Product
	var snaps []engine.Snapshot
	var promos []engine.PromotionScenario
	json.Unmarshal(boot["products"], &products)
	json.Unmarshal(boot["snapshots"], &snaps)
	json.Unmarshal(boot["promotions"], &promos)
	if len(products) == 0 || len(promos) == 0 {
		t.Fatal("missing demo scenarios")
	}
	p := products[0]
	code, result := call(t, h, "POST", "/api/stores/default/calculate", map[string]any{"product": p, "snapshot_id": snaps[0].ID, "promotion_ids": []string{promos[0].ID}})
	if code != 200 {
		t.Fatalf("calculate %d %s", code, result["error"])
	}
	var contexts []engine.ContextResult
	json.Unmarshal(result["contexts"], &contexts)
	if len(contexts) != 2 || contexts[0].Result.Recommended == nil {
		t.Fatalf("%s", result["contexts"])
	}
	var id string
	json.Unmarshal(result["id"], &id)
	original, _ := db.ForStore("default").List(t.Context(), "runs")
	p.StoreID = "default"
	if code, _ = call(t, h, "POST", "/api/stores/demo-second/calculate", map[string]any{"product": p, "snapshot_id": snaps[0].ID}); code != 400 {
		t.Fatal("cross store product accepted", code)
	}
	p.StoreID = "demo-second"
	if code, _ = call(t, h, "POST", "/api/stores/demo-second/calculate", map[string]any{"product": p, "snapshot_id": snaps[0].ID}); code != 404 {
		t.Fatal("cross store snapshot accepted", code)
	}
	if code, _ = call(t, h, "GET", "/api/stores/demo-second/runs/"+id, nil); code != 404 {
		t.Fatal("cross store run leaked")
	}
	code, record := call(t, h, "GET", "/api/stores/default/runs/"+id, nil)
	if code != 200 || len(record["contexts"]) == 0 || len(record["store"]) == 0 {
		t.Fatal("incomplete frozen run")
	}
	after, _ := db.ForStore("default").List(t.Context(), "runs")
	if len(after) != len(original) {
		t.Fatal("failed cross-store request saved result")
	}
	// A new store starts with unknown costs, not confirmed demo rates.
	code, newStore := call(t, h, "POST", "/api/stores", map[string]any{"name": "新店"})
	if code != 200 {
		t.Fatal(code)
	}
	var profile engine.StoreProfile
	json.Unmarshal(newStore["store"], &profile)
	code, blank := call(t, h, "GET", "/api/stores/"+profile.ID+"/bootstrap", nil)
	var settings engine.Settings
	json.Unmarshal(blank["settings"], &settings)
	if code != 200 || settings.Fees[0].Rate != nil || settings.Fees[0].Status != "pending" {
		t.Fatal("new store inherited confirmed assumptions")
	}
}

func TestStoreArchiveBlocksWritesAndPublicCopyIsPending(t *testing.T) {
	h, _ := testServer(t)
	code, v := call(t, h, "POST", "/api/stores/default/quotes/copy-public", map[string]any{"public_id": "public-ogl"})
	var q engine.Quote
	json.Unmarshal(v["quote"], &q)
	if code != 200 || q.Status != "pending" || q.PublicID != "public-ogl" || q.StoreID != "default" {
		t.Fatalf("%d %+v", code, q)
	}
	if len(q.Fees) != 2 || q.Fees[0].Status != "pending" || q.Fees[0].Minimum == nil || *q.Fees[0].Minimum != 15 || !contains(q.Includes, "tail_delivery") {
		t.Fatalf("OGL carrier rate lost its separate platform costs or duplicated the last mile: %+v", q)
	}
	if code, _ = call(t, h, "PUT", "/api/stores/demo-second", map[string]any{"name": "二店", "archived": true}); code != 200 {
		t.Fatal(code)
	}
	if code, _ = call(t, h, "POST", "/api/stores/demo-second/demo", map[string]any{}); code != http.StatusConflict {
		t.Fatal("archived store changed", code)
	}
	if code, _ = call(t, h, "GET", "/api/stores/demo-second/bootstrap", nil); code != 200 {
		t.Fatal("archived history unavailable")
	}
}

func TestOzonReferenceUpgradePreservesHistoryAndIsIdempotent(t *testing.T) {
	_, db := testServer(t)
	ctx := t.Context()
	q := publicQuotes()[0]
	q.ID, q.PublicID, q.StoreID = "legacy-ogl", "public-ogl", "default"
	q.Version, q.Fees, q.Includes, q.Status = 3, nil, nil, "active"
	if err := db.ForStore("default").Put(ctx, "quotes", q.ID, q); err != nil {
		t.Fatal(err)
	}
	history := publicQuotes()[1]
	history.ID, history.PublicID, history.StoreID = "legacy-partner", "historical-CEL", "default"
	history.Version, history.ValidTo, history.Fees = 3, "2026-10-31", nil
	if err := db.ForStore("default").Put(ctx, "quotes", history.ID, history); err != nil {
		t.Fatal(err)
	}
	frozen := map[string]any{"id": "frozen", "old_result": 123.45}
	if err := db.ForStore("default").Put(ctx, "runs", "frozen", frozen); err != nil {
		t.Fatal(err)
	}
	if err := db.Delete(ctx, "meta", "ozon-rules-2026-11-03"); err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 2; i++ {
		if err := Seed(ctx, db); err != nil {
			t.Fatal(err)
		}
		var updated engine.Quote
		if err := db.ForStore("default").Get(ctx, "quotes", q.ID, &updated); err != nil {
			t.Fatal(err)
		}
		if updated.Version != 4 || updated.Status != "pending" || len(updated.Fees) != 2 {
			t.Fatalf("%+v", updated)
		}
		var updatedHistory engine.Quote
		if err := db.ForStore("default").Get(ctx, "quotes", history.ID, &updatedHistory); err != nil {
			t.Fatal(err)
		}
		if updatedHistory.Version != 4 || updatedHistory.Status != "historical" || updatedHistory.ValidTo != "" || len(updatedHistory.Fees) != 2 {
			t.Fatalf("unverified expiry survived: %+v", updatedHistory)
		}
		var old map[string]any
		if err := db.ForStore("default").Get(ctx, "runs", "frozen", &old); err != nil || old["old_result"] != 123.45 {
			t.Fatal("historical result changed", old, err)
		}
	}
}

func TestHistoricalPartnerQuotesDoNotClaimAnOfficialShutdownDate(t *testing.T) {
	providers := map[string]bool{}
	for _, q := range publicQuotes() {
		if q.ID == "historical-chinapost" && contains(q.Includes, "tail_delivery") {
			t.Fatal("historical postal quote inherited OGL's included last-mile service")
		}
		if q.Source != partnerSource {
			continue
		}
		providers[q.Provider] = true
		if q.Status != "historical" || q.ValidTo != "" || len(q.Fees) != 2 {
			t.Fatalf("%+v", q)
		}
	}
	if len(providers) != 10 {
		t.Fatal("missing historical providers", providers)
	}
}
