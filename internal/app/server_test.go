package app

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"pricepilot/internal/engine"
	"pricepilot/internal/seerfar"
	"pricepilot/internal/store"
	"testing"
)

func testServer(t *testing.T) (http.Handler, *store.Store) {
	t.Helper()
	s, err := store.Open("sqlite", filepath.Join(t.TempDir(), "api.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	if err = Seed(t.Context(), s); err != nil {
		t.Fatal(err)
	}
	return New(s, seerfar.New("", ""), "", true), s
}
func call(t *testing.T, h http.Handler, method, path string, body any) (int, map[string]json.RawMessage) {
	t.Helper()
	b, _ := json.Marshal(body)
	r := httptest.NewRequest(method, path, bytes.NewReader(b))
	r.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	var v map[string]json.RawMessage
	if err := json.Unmarshal(w.Body.Bytes(), &v); err != nil {
		t.Fatalf("%s", w.Body.String())
	}
	return w.Code, v
}
func TestBootstrapAndPersistedCalculation(t *testing.T) {
	h, s := testServer(t)
	code, v := call(t, h, "GET", "/api/stores/default/bootstrap", nil)
	if code != 200 {
		t.Fatal(code)
	}
	var p []engine.Product
	json.Unmarshal(v["products"], &p)
	var snaps []engine.Snapshot
	json.Unmarshal(v["snapshots"], &snaps)
	code, v = call(t, h, "POST", "/api/stores/default/calculate", map[string]any{"product": p[0], "snapshot_id": snaps[0].ID})
	if code != 200 {
		t.Fatalf("%d %s", code, v["error"])
	}
	var res engine.Result
	json.Unmarshal(v["result"], &res)
	if res.Recommended == nil || !res.Demo {
		t.Fatalf("%+v", res)
	}
	runs, err := s.ForStore("default").List(t.Context(), "runs")
	if err != nil || len(runs) != 1 {
		t.Fatal("calculation not persisted")
	}
}
func TestExcludedPublicQuoteCannotBeActivated(t *testing.T) {
	h, s := testServer(t)
	var q engine.Quote
	s.Get(t.Context(), "public_quotes", "excluded-uni-offline", &q)
	q.StoreID = "default"
	s.ForStore("default").Put(t.Context(), "quotes", q.ID, q)
	q.Status = "active"
	q.Platforms = []string{"ozon"}
	code, _ := call(t, h, "PUT", "/api/stores/default/quotes/"+q.ID, q)
	if code != 400 {
		t.Fatal("excluded public tariff activated")
	}
}
func TestDiscoveryFailureDoesNotReplaceSnapshot(t *testing.T) {
	h, s := testServer(t)
	failing := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusServiceUnavailable) }))
	defer failing.Close()
	setService(t, h, "default", "seerfar", failing.URL)
	before, _ := s.ForStore("default").List(t.Context(), "snapshots")
	code, _ := call(t, h, "POST", "/api/stores/default/competitors/discover", map[string]any{"product": engine.Product{Name: "x", Keywords: []string{"x"}}})
	if code != 502 {
		t.Fatal(code)
	}
	after, _ := s.ForStore("default").List(t.Context(), "snapshots")
	if len(before) != len(after) {
		t.Fatal("failed discovery stored empty snapshot")
	}
}
func TestSnapshotEditsKeepObservedTime(t *testing.T) {
	h, s := testServer(t)
	raw, _ := s.ForStore("default").List(t.Context(), "snapshots")
	var snap engine.Snapshot
	json.Unmarshal(raw[0], &snap)
	original := snap.FetchedAt
	snap.FetchedAt = snap.FetchedAt.AddDate(1, 0, 0)
	snap.Competitors[0].Include = false
	code, _ := call(t, h, "PUT", "/api/stores/default/snapshots/"+snap.ID, snap)
	if code != 200 {
		t.Fatal(code)
	}
	var got engine.Snapshot
	s.ForStore("default").Get(t.Context(), "snapshots", snap.ID, &got)
	if !got.FetchedAt.Equal(original) || got.Competitors[0].Include {
		t.Fatal("snapshot edit changed observation time")
	}
}
