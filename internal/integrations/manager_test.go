package integrations

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pricepilot/internal/store"
)

func TestProfilesSecretsIsolationAndRestart(t *testing.T) {
	db, err := store.Open("sqlite", filepath.Join(t.TempDir(), "config.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	key, err := LoadKey(filepath.Join(t.TempDir(), "key"))
	if err != nil {
		t.Fatal(err)
	}
	m := New(db, key)
	cfg, err := m.Get(t.Context(), "a")
	if err != nil {
		t.Fatal(err)
	}
	_, err = m.Save(t.Context(), "a", Update{Provider: "1688", Environment: "real", Version: cfg.Version, BaseURL: "https://gw.open.1688.com", Credentials: map[string]string{"app_key": "real-key", "app_secret": "sensitive-secret", "access_token": "sensitive-token"}, Activate: true})
	if err != nil {
		t.Fatal(err)
	}
	rt, err := m.Resolve(t.Context(), "a", "1688")
	if err != nil || rt.Credentials["access_token"] != "sensitive-token" || rt.Environment != "real" {
		t.Fatalf("%+v %v", rt, err)
	}
	raw, _ := db.ForStore("a").List(t.Context(), "integrations")
	if strings.Contains(string(raw[0]), "sensitive-") {
		t.Fatal("plaintext stored")
	}
	view, _ := m.View(t.Context(), "a")
	b, _ := json.Marshal(view)
	if strings.Contains(string(b), "sensitive-") || strings.Contains(string(b), "real-key") {
		t.Fatal("secret returned")
	}
	other, _ := m.Resolve(t.Context(), "b", "1688")
	if other.Environment != "mock" || other.Credentials["access_token"] != "test-token" {
		t.Fatal("cross store settings inherited")
	}
	m = New(db, key)
	after, _ := m.Resolve(t.Context(), "a", "1688")
	if after.Credentials["access_token"] != "sensitive-token" {
		t.Fatal("restart lost config")
	}
	cfg, _ = m.Get(t.Context(), "a")
	_, err = m.Save(t.Context(), "a", Update{Provider: "1688", Environment: "real", Version: cfg.Version, BaseURL: "https://gw.open.1688.com", Credentials: map[string]string{"access_token": ""}})
	if err != nil {
		t.Fatal(err)
	}
	after, _ = m.Resolve(t.Context(), "a", "1688")
	if after.Credentials["access_token"] != "sensitive-token" {
		t.Fatal("blank erased credential")
	}
	cfg, _ = m.Get(t.Context(), "a")
	_, err = m.Save(t.Context(), "a", Update{Provider: "1688", Environment: "real", Version: cfg.Version, BaseURL: "https://gw.open.1688.com", Clear: []string{"access_token"}})
	if err != nil {
		t.Fatal(err)
	}
	after, _ = m.Resolve(t.Context(), "a", "1688")
	if after.Configured() {
		t.Fatal("explicit clear ignored")
	}
	if _, err = m.Save(t.Context(), "a", Update{Provider: "1688", Environment: "real", Version: 1, BaseURL: "https://example.com"}); err != ErrConflict {
		t.Fatalf("stale save %v", err)
	}
}

func TestKeyPersistsAndInvalidKeyIsNotReplaced(t *testing.T) {
	p := filepath.Join(t.TempDir(), "key")
	a, err := LoadKey(p)
	if err != nil {
		t.Fatal(err)
	}
	b, err := LoadKey(p)
	if err != nil || string(a) != string(b) {
		t.Fatal("key not persistent")
	}
	if err = os.WriteFile(p, []byte("broken"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err = LoadKey(p); err == nil {
		t.Fatal("corrupt key silently regenerated")
	}
	for _, s := range []string{"file:///etc/passwd", "https://user:pass@example.com", "https://example.com?token=x", "javascript:abc"} {
		if ValidateURL(s) == nil {
			t.Fatalf("bad url %s", s)
		}
	}
}
