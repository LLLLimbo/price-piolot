package initialdata

import (
	"encoding/json"
	"math"
	"path/filepath"
	"testing"
	"time"

	"pricepilot/internal/app"
	"pricepilot/internal/engine"
	"pricepilot/internal/store"
)

func TestBundledSnapshotPreservesBusinessDataAndHistory(t *testing.T) {
	db, err := store.Open("sqlite", filepath.Join(t.TempDir(), "seed.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	restored, err := Restore(t.Context(), db)
	if err != nil || !restored {
		t.Fatalf("%v %v", restored, err)
	}
	if err = app.Seed(t.Context(), db); err != nil {
		t.Fatal(err)
	}
	doc, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	for _, r := range doc.Records {
		var raw json.RawMessage
		if err = db.ForStore(r.StoreID).Get(t.Context(), r.Kind, r.ID, &raw); err != nil || string(raw) != r.Payload {
			t.Fatalf("changed initial record %s/%s/%s: %v", r.StoreID, r.Kind, r.ID, err)
		}
	}
	var p engine.Product
	if err = db.ForStore("default").Get(t.Context(), "products", "xzg-123703-Black-1PC", &p); err != nil {
		t.Fatal(err)
	}
	var settings engine.Settings
	if err = db.ForStore("default").Get(t.Context(), "settings", "default", &settings); err != nil {
		t.Fatal(err)
	}
	quotes, err := db.ForStore("default").List(t.Context(), "quotes")
	if err != nil {
		t.Fatal(err)
	}
	in := engine.Input{Product: p, Settings: settings, AsOf: time.Date(2026, 11, 3, 7, 0, 0, 0, time.UTC)}
	for _, raw := range quotes {
		var q engine.Quote
		json.Unmarshal(raw, &q)
		in.Quotes = append(in.Quotes, q)
	}
	r, err := engine.Solve(in)
	if err != nil || r.Recommended == nil || math.Abs(r.Recommended.Price-47.53) > .001 {
		t.Fatalf("snapshot pricing unusable: %+v %v", r, err)
	}
	if rows, _ := db.ForStore("default").List(t.Context(), "integrations"); len(rows) != 0 {
		t.Fatal("snapshot contained credentials")
	}
	if rows, _ := db.ForStore("default").List(t.Context(), "runs"); len(rows) != 0 {
		t.Fatal("snapshot included historical calculations")
	}
	p.Name = "部署后的修改"
	if err = db.ForStore("default").Put(t.Context(), "products", p.ID, p); err != nil {
		t.Fatal(err)
	}
	if restored, err = Restore(t.Context(), db); err != nil || restored {
		t.Fatalf("snapshot reapplied: %v %v", restored, err)
	}
	var after engine.Product
	db.ForStore("default").Get(t.Context(), "products", p.ID, &after)
	if after.Name != p.Name {
		t.Fatal("restart overwrote product")
	}
}
