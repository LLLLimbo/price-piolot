package store

import (
	"encoding/json"
	"path/filepath"
	"testing"
)

func TestInitialSnapshotOnlyRestoresAnEmptyDatabase(t *testing.T) {
	db, err := Open("sqlite", filepath.Join(t.TempDir(), "snapshot.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	records := []InitialRecord{
		{StoreID: "first", Kind: "products", ID: "same", Payload: `{"name":"第一店商品","price":47.5300}`, UpdatedAt: "2026-11-01T08:00:00Z"},
		{StoreID: "second", Kind: "products", ID: "same", Payload: `{"name":"第二店商品"}`, UpdatedAt: "2026-11-02T08:00:00Z"},
		{StoreID: "first", Kind: "runs", ID: "z-old", Payload: `{"profit":1.2300}`, UpdatedAt: "2026-11-01T08:00:00Z"},
		{StoreID: "first", Kind: "runs", ID: "a-new", Payload: `{"profit":8.7229}`, UpdatedAt: "2026-11-02T08:00:00Z"},
	}
	restored, err := db.RestoreInitial(t.Context(), records)
	if err != nil || !restored {
		t.Fatalf("%v %v", restored, err)
	}
	var raw json.RawMessage
	if err = db.ForStore("first").Get(t.Context(), "products", "same", &raw); err != nil || string(raw) != records[0].Payload {
		t.Fatalf("payload changed: %s %v", raw, err)
	}
	rows, err := db.ForStore("first").List(t.Context(), "runs")
	if err != nil || len(rows) != 2 || string(rows[0]) != records[3].Payload {
		t.Fatal("timestamps or history order lost", err)
	}
	if err = db.ForStore("first").Put(t.Context(), "products", "same", map[string]string{"name": "用户修改"}); err != nil {
		t.Fatal(err)
	}
	if err = db.ForStore("second").Delete(t.Context(), "products", "same"); err != nil {
		t.Fatal(err)
	}
	restored, err = db.RestoreInitial(t.Context(), records)
	if err != nil || restored {
		t.Fatalf("non-empty database reinitialised: %v %v", restored, err)
	}
	var got map[string]string
	if err = db.ForStore("first").Get(t.Context(), "products", "same", &got); err != nil || got["name"] != "用户修改" {
		t.Fatal("overwrote existing data", err)
	}
	if err = db.ForStore("second").Get(t.Context(), "products", "same", &got); err != ErrNotFound {
		t.Fatal("restored a record the user had deleted", err)
	}
}

func TestInvalidInitialSnapshotRollsBack(t *testing.T) {
	db, err := Open("sqlite", filepath.Join(t.TempDir(), "invalid.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	bad := []InitialRecord{
		{Kind: "stores", ID: "one", Payload: `{"id":"one"}`, UpdatedAt: "2026-11-03T00:00:00Z"},
		{Kind: "products", ID: "bad", Payload: `invalid`, UpdatedAt: "2026-11-03T00:00:00Z"},
	}
	if _, err = db.RestoreInitial(t.Context(), bad); err == nil {
		t.Fatal("invalid snapshot accepted")
	}
	var got map[string]string
	if err = db.Get(t.Context(), "stores", "one", &got); err != ErrNotFound {
		t.Fatal("partial restore survived", err)
	}
}
