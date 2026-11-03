package store

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

type InitialRecord struct {
	StoreID   string `json:"store_id"`
	Kind      string `json:"kind"`
	ID        string `json:"id"`
	Payload   string `json:"payload"`
	UpdatedAt string `json:"updated_at"`
}

// RestoreInitial imports the original JSON and timestamps atomically, only when
// the entire database is empty. Existing installations never receive a merge.
func (s *Store) RestoreInitial(ctx context.Context, records []InitialRecord) (bool, error) {
	if s.scope != "" {
		return false, fmt.Errorf("initial snapshot requires the root store")
	}
	restored := false
	err := s.Atomic(ctx, func(tx *Store) error {
		var count int
		if err := tx.executor().QueryRowContext(ctx, "SELECT COUNT(*) FROM records").Scan(&count); err != nil {
			return err
		}
		if count > 0 {
			return nil
		}
		if len(records) == 0 {
			return fmt.Errorf("initial snapshot is empty")
		}
		for _, r := range records {
			if r.Kind == "" || r.ID == "" || !json.Valid([]byte(r.Payload)) {
				return fmt.Errorf("invalid initial record: %s/%s", r.Kind, r.ID)
			}
			if r.Kind == "integrations" || strings.Contains(r.Kind, "preview") {
				return fmt.Errorf("credentials and temporary previews cannot be initial records")
			}
			if _, err := time.Parse(time.RFC3339Nano, r.UpdatedAt); err != nil {
				return fmt.Errorf("invalid initial record timestamp: %s/%s", r.Kind, r.ID)
			}
			if _, err := tx.executor().ExecContext(ctx, s.query(`INSERT INTO records(store_id,kind,id,payload,updated_at) VALUES(?,?,?,?,?)`), r.StoreID, r.Kind, r.ID, r.Payload, r.UpdatedAt); err != nil {
				return err
			}
		}
		restored = true
		return nil
	})
	if err != nil {
		return false, err
	}
	return restored, nil
}
