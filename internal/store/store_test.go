package store

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"testing"
)

func TestLegacyMigrationAndScopes(t *testing.T) {
	legacyContract(t, "sqlite", filepath.Join(t.TempDir(), "legacy.db"))
}
func legacyContract(t *testing.T, driver, dsn string) {
	name := driver
	if name == "sqlite" {
		name = "sqlite3"
	} else {
		name = "pgx"
	}
	old, err := sql.Open(name, dsn)
	if err != nil {
		t.Fatal(err)
	}
	_, err = old.Exec(`CREATE TABLE records(kind TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(kind,id)); INSERT INTO records VALUES('products','same','{"name":"旧商品"}','2026-11-02')`)
	if err != nil {
		t.Fatal(err)
	}
	old.Close()
	for i := 0; i < 2; i++ {
		s, err := Open(driver, dsn)
		if err != nil {
			t.Fatal(err)
		}
		var p map[string]string
		if err = s.ForStore("default").Get(t.Context(), "products", "same", &p); err != nil || p["name"] != "旧商品" {
			t.Fatalf("legacy lost: %v %+v", err, p)
		}
		if err = s.ForStore("other").Get(t.Context(), "products", "same", &p); i == 0 && err != ErrNotFound {
			t.Fatal("cross-store read", err)
		}
		if err = s.ForStore("other").Put(t.Context(), "products", "same", map[string]string{"name": "另一店商品"}); err != nil {
			t.Fatal(err)
		}
		err = s.ForStore("default").Atomic(t.Context(), func(tx *Store) error {
			if e := tx.Put(t.Context(), "products", "rollback", p); e != nil {
				return e
			}
			return fmt.Errorf("abort")
		})
		if err == nil {
			t.Fatal("expected transaction failure")
		}
		if err = s.ForStore("default").Get(t.Context(), "products", "rollback", &p); err != ErrNotFound {
			t.Fatal("transaction failed to roll back")
		}
		s.Close()
	}
}

func contract(t *testing.T, driver, dsn string) {
	t.Helper()
	s, err := Open(driver, dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	ctx := context.Background()
	kind := "test-" + ID()
	value := map[string]string{"name": "人民币商品", "formula": "60 / 0.55"}
	if err = s.Put(ctx, kind, "one", value); err != nil {
		t.Fatal(err)
	}
	var got map[string]string
	if err = s.Get(ctx, kind, "one", &got); err != nil || got["name"] != value["name"] {
		t.Fatalf("%v %+v", err, got)
	}
	value["name"] = "更新后"
	if err = s.Put(ctx, kind, "one", value); err != nil {
		t.Fatal(err)
	}
	items, err := s.List(ctx, kind)
	if err != nil || len(items) != 1 {
		t.Fatalf("%v %+v", err, items)
	}
	if err = s.Delete(ctx, kind, "one"); err != nil {
		t.Fatal(err)
	}
	if err = s.Get(ctx, kind, "one", &got); err != ErrNotFound {
		t.Fatalf("expected not found, got %v", err)
	}
}
func TestSQLiteContract(t *testing.T) { contract(t, "sqlite", filepath.Join(t.TempDir(), "test.db")) }
func TestPostgresContract(t *testing.T) {
	dsn := os.Getenv("TEST_POSTGRES_DSN")
	if dsn == "" {
		t.Skip("Postgres contract runs in the Compose integration environment")
	}
	contract(t, "postgres", dsn)
}

func TestPostgresLegacyMigration(t *testing.T) {
	dsn := os.Getenv("TEST_POSTGRES_DSN")
	if dsn == "" {
		t.Skip("Compose Postgres required")
	}
	root, err := sql.Open("pgx", dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	schema := "migration_" + ID()
	if _, err = root.Exec(`CREATE SCHEMA ` + schema); err != nil {
		t.Fatal(err)
	}
	defer root.Exec(`DROP SCHEMA ` + schema + ` CASCADE`)
	u, err := url.Parse(dsn)
	if err != nil {
		t.Fatal(err)
	}
	q := u.Query()
	q.Set("search_path", schema)
	u.RawQuery = q.Encode()
	legacyContract(t, "postgres", u.String())
}
