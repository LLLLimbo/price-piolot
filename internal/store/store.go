package store

import (
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	_ "github.com/jackc/pgx/v5/stdlib"
	_ "github.com/mattn/go-sqlite3"
	"strings"
	"time"
)

var ErrNotFound = errors.New("record not found")

type Store struct {
	db     *sql.DB
	driver string
	scope  string
	tx     *sql.Tx
}

func ID() string {
	b := make([]byte, 12)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return hex.EncodeToString(b)
}
func Open(driver, dsn string) (*Store, error) {
	name := driver
	if driver == "sqlite" {
		name = "sqlite3"
		if !strings.Contains(dsn, "?") {
			dsn += "?_journal_mode=WAL&_busy_timeout=5000&_foreign_keys=on"
		}
	} else if driver == "postgres" {
		name = "pgx"
	} else {
		return nil, fmt.Errorf("unsupported database driver %q", driver)
	}
	db, err := sql.Open(name, dsn)
	if err != nil {
		return nil, err
	}
	if driver == "sqlite" {
		db.SetMaxOpenConns(1)
	} else {
		db.SetMaxOpenConns(8)
	}
	s := &Store{db: db, driver: driver}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err = db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	if err = s.migrate(ctx); err != nil {
		db.Close()
		return nil, err
	}
	return s, nil
}
func (s *Store) query(q string) string {
	if s.driver != "postgres" {
		return q
	}
	n := 0
	return replaceParams(q, &n)
}
func replaceParams(q string, n *int) string {
	parts := strings.Split(q, "?")
	var b strings.Builder
	for i, p := range parts {
		if i > 0 {
			*n++
			fmt.Fprintf(&b, "$%d", *n)
		}
		b.WriteString(p)
	}
	return b.String()
}
func (s *Store) Close() error                   { return s.db.Close() }
func (s *Store) Ping(ctx context.Context) error { return s.db.PingContext(ctx) }
func (s *Store) Put(ctx context.Context, kind, id string, value any) error {
	b, err := json.Marshal(value)
	if err != nil {
		return err
	}
	_, err = s.executor().ExecContext(ctx, s.query(`INSERT INTO records(store_id,kind,id,payload,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(store_id,kind,id) DO UPDATE SET payload=excluded.payload, updated_at=excluded.updated_at`), s.scope, kind, id, string(b), time.Now().UTC().Format(time.RFC3339Nano))
	return err
}
func (s *Store) Get(ctx context.Context, kind, id string, out any) error {
	var b string
	err := s.executor().QueryRowContext(ctx, s.query(`SELECT payload FROM records WHERE store_id=? AND kind=? AND id=?`), s.scope, kind, id).Scan(&b)
	if errors.Is(err, sql.ErrNoRows) {
		return ErrNotFound
	}
	if err != nil {
		return err
	}
	return json.Unmarshal([]byte(b), out)
}
func (s *Store) List(ctx context.Context, kind string) ([]json.RawMessage, error) {
	rows, err := s.executor().QueryContext(ctx, s.query(`SELECT payload FROM records WHERE store_id=? AND kind=? ORDER BY updated_at DESC,id`), s.scope, kind)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []json.RawMessage{}
	for rows.Next() {
		var v string
		if err = rows.Scan(&v); err != nil {
			return nil, err
		}
		out = append(out, json.RawMessage(v))
	}
	return out, rows.Err()
}
func (s *Store) Delete(ctx context.Context, kind, id string) error {
	_, err := s.executor().ExecContext(ctx, s.query(`DELETE FROM records WHERE store_id=? AND kind=? AND id=?`), s.scope, kind, id)
	return err
}

const recordsDDL = `CREATE TABLE IF NOT EXISTS records (store_id TEXT NOT NULL DEFAULT '',kind TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(store_id,kind,id))`

type executor interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func (s *Store) executor() executor {
	if s.tx != nil {
		return s.tx
	}
	return s.db
}
func (s *Store) ForStore(id string) *Store { out := *s; out.scope = id; return &out }
func (s *Store) Scope() string             { return s.scope }
func (s *Store) Atomic(ctx context.Context, fn func(*Store) error) error {
	if s.tx != nil {
		return fn(s)
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	scoped := *s
	scoped.tx = tx
	if err = fn(&scoped); err != nil {
		return err
	}
	return tx.Commit()
}
func (s *Store) migrate(ctx context.Context) error {
	if _, err := s.db.ExecContext(ctx, recordsDDL); err != nil {
		return err
	}
	rows, err := s.db.QueryContext(ctx, `SELECT * FROM records LIMIT 0`)
	if err != nil {
		return err
	}
	cols, err := rows.Columns()
	rows.Close()
	if err != nil {
		return err
	}
	for _, col := range cols {
		if col == "store_id" {
			return nil
		}
	}
	return s.Atomic(ctx, func(tx *Store) error {
		for _, q := range []string{`ALTER TABLE records RENAME TO records_legacy`, recordsDDL, `INSERT INTO records(store_id,kind,id,payload,updated_at) SELECT 'default',kind,id,payload,updated_at FROM records_legacy`, `DROP TABLE records_legacy`} {
			if _, err := tx.executor().ExecContext(ctx, q); err != nil {
				return err
			}
		}
		return nil
	})
}
