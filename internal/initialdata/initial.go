package initialdata

import (
	"bytes"
	"compress/gzip"
	"context"
	_ "embed"
	"encoding/json"
	"fmt"

	"pricepilot/internal/store"
)

//go:embed seed.snapshot.json.gz
var bundledSnapshot []byte

type Snapshot struct {
	SchemaVersion int                   `json:"schema_version"`
	ExportedAt    string                `json:"exported_at"`
	Records       []store.InitialRecord `json:"records"`
}

func Load() (Snapshot, error) {
	var snapshot Snapshot
	r, err := gzip.NewReader(bytes.NewReader(bundledSnapshot))
	if err != nil {
		return snapshot, err
	}
	defer r.Close()
	if err = json.NewDecoder(r).Decode(&snapshot); err != nil {
		return snapshot, err
	}
	if snapshot.SchemaVersion != 1 || len(snapshot.Records) == 0 {
		return snapshot, fmt.Errorf("unsupported or empty initial snapshot")
	}
	return snapshot, nil
}

func Restore(ctx context.Context, db *store.Store) (bool, error) {
	snapshot, err := Load()
	if err != nil {
		return false, err
	}
	return db.RestoreInitial(ctx, snapshot.Records)
}
