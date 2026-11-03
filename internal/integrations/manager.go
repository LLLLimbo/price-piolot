package integrations

import (
	"context"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"pricepilot/internal/external"
	"pricepilot/internal/store"
)

var ErrConflict = errors.New("配置已变化，请刷新后重试")
var ValidateURL = external.ValidateURL

type Binding struct {
	Provider      string `json:"provider"`
	Environment   string `json:"environment"`
	Version       int    `json:"version"`
	ConfigVersion int    `json:"config_version"`
}
type Profile struct {
	BaseURL        string          `json:"base_url"`
	Version        int             `json:"version"`
	TestProductID  string          `json:"test_product_id"`
	Ciphertext     string          `json:"ciphertext,omitempty"`
	CredentialsSet map[string]bool `json:"credentials_set,omitempty"`
	Configured     bool            `json:"configured"`
}
type Service struct {
	Active   string             `json:"active"`
	Profiles map[string]Profile `json:"profiles"`
}
type Config struct {
	Version  int                `json:"version"`
	Services map[string]Service `json:"services"`
}
type Update struct {
	Provider      string            `json:"provider"`
	Environment   string            `json:"environment"`
	Version       int               `json:"version"`
	BaseURL       string            `json:"base_url"`
	TestProductID string            `json:"test_product_id"`
	Credentials   map[string]string `json:"credentials"`
	Clear         []string          `json:"clear"`
	Activate      bool              `json:"activate"`
}
type Runtime struct {
	Binding
	BaseURL     string
	Credentials map[string]string
}

func (r Runtime) Configured() bool {
	if r.BaseURL == "" {
		return false
	}
	for _, k := range fields(r.Provider) {
		if r.Provider != "frankfurter" && r.Credentials[k] == "" {
			return false
		}
	}
	return true
}

type Manager struct {
	db   *store.Store
	aead cipher.AEAD
	mu   sync.Mutex
}

func New(db *store.Store, key []byte) *Manager {
	b, err := aes.NewCipher(key)
	if err != nil {
		panic("invalid integration encryption key")
	}
	aead, err := cipher.NewGCM(b)
	if err != nil {
		panic(err)
	}
	return &Manager{db: db, aead: aead}
}
func LoadKey(path string) ([]byte, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return nil, err
	}
	if b, err := os.ReadFile(path); err == nil {
		if len(b) != 32 {
			return nil, fmt.Errorf("外部服务加密密钥长度无效，请恢复原密钥文件")
		}
		return b, nil
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return nil, err
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if os.IsExist(err) {
		return LoadKey(path)
	}
	if err != nil {
		return nil, err
	}
	_, err = f.Write(b)
	if err == nil {
		err = f.Sync()
	}
	closeErr := f.Close()
	if err != nil {
		return nil, err
	}
	if closeErr != nil {
		return nil, closeErr
	}
	return b, nil
}
func fields(provider string) []string {
	switch provider {
	case "1688":
		return []string{"app_key", "app_secret", "access_token"}
	case "seerfar":
		return []string{"api_key"}
	case "frankfurter":
		return []string{"bearer_token"}
	}
	return nil
}
func aad(storeID, provider, env string) []byte {
	return []byte(storeID + "\x00" + provider + "\x00" + env)
}
func (m *Manager) encrypt(storeID, provider, env string, secrets map[string]string) (string, error) {
	b, err := json.Marshal(secrets)
	if err != nil {
		return "", err
	}
	n := make([]byte, m.aead.NonceSize())
	if _, err = rand.Read(n); err != nil {
		return "", err
	}
	return base64.StdEncoding.EncodeToString(m.aead.Seal(n, n, b, aad(storeID, provider, env))), nil
}
func (m *Manager) decrypt(storeID, provider, env string, p Profile) (map[string]string, error) {
	if p.Ciphertext == "" {
		return map[string]string{}, nil
	}
	b, err := base64.StdEncoding.DecodeString(p.Ciphertext)
	if err != nil || len(b) < m.aead.NonceSize() {
		return nil, fmt.Errorf("服务凭据无法解密，请检查应用密钥文件")
	}
	n := m.aead.NonceSize()
	b, err = m.aead.Open(nil, b[:n], b[n:], aad(storeID, provider, env))
	if err != nil {
		return nil, fmt.Errorf("服务凭据无法解密，请恢复应用密钥文件")
	}
	v := map[string]string{}
	if json.Unmarshal(b, &v) != nil {
		return nil, fmt.Errorf("服务凭据存储无效")
	}
	return v, nil
}
func (m *Manager) defaults(storeID string) (Config, error) {
	cfg := Config{Version: 1, Services: map[string]Service{}}
	for _, provider := range []string{"1688", "seerfar", "frankfurter"} {
		s := Service{Active: "mock", Profiles: map[string]Profile{"mock": {Version: 1}, "real": {Version: 1}}}
		mock, real := s.Profiles["mock"], s.Profiles["real"]
		var creds map[string]string
		switch provider {
		case "1688":
			mock.BaseURL = "http://mock-1688:8080"
			mock.TestProductID = "1001"
			real.BaseURL = "https://gw.open.1688.com"
			creds = map[string]string{"app_key": "test-app", "app_secret": "test-secret", "access_token": "test-token"}
		case "seerfar":
			mock.BaseURL = "http://mock-seerfar:8080"
			creds = map[string]string{"api_key": "seerfar-test-key"}
		case "frankfurter":
			mock.BaseURL = "http://mock-fx:8080"
			real.BaseURL = "https://api.frankfurter.dev"
			creds = map[string]string{}
		}
		var err error
		mock.Ciphertext, err = m.encrypt(storeID, provider, "mock", creds)
		if err != nil {
			return cfg, err
		}
		s.Profiles["mock"], s.Profiles["real"] = mock, real
		cfg.Services[provider] = s
	}
	return cfg, nil
}
func (m *Manager) Get(ctx context.Context, storeID string) (Config, error) {
	var cfg Config
	err := m.db.ForStore(storeID).Get(ctx, "integrations", "default", &cfg)
	if err == nil {
		return cfg, nil
	}
	if !errors.Is(err, store.ErrNotFound) {
		return cfg, err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	if err = m.db.ForStore(storeID).Get(ctx, "integrations", "default", &cfg); err == nil {
		return cfg, nil
	} else if !errors.Is(err, store.ErrNotFound) {
		return cfg, err
	}
	cfg, err = m.defaults(storeID)
	if err == nil {
		err = m.db.ForStore(storeID).Put(ctx, "integrations", "default", cfg)
	}
	return cfg, err
}
func (m *Manager) View(ctx context.Context, storeID string) (Config, error) {
	cfg, err := m.Get(ctx, storeID)
	if err != nil {
		return cfg, err
	}
	for provider, s := range cfg.Services {
		for env, p := range s.Profiles {
			secrets, e := m.decrypt(storeID, provider, env, p)
			if e != nil {
				return Config{}, e
			}
			p.Ciphertext = ""
			p.CredentialsSet = map[string]bool{}
			for _, k := range fields(provider) {
				p.CredentialsSet[k] = secrets[k] != ""
			}
			p.Configured = (Runtime{Binding: Binding{Provider: provider}, BaseURL: p.BaseURL, Credentials: secrets}).Configured()
			s.Profiles[env] = p
		}
		cfg.Services[provider] = s
	}
	return cfg, nil
}
func (m *Manager) Profile(ctx context.Context, storeID, provider, env string) (Runtime, error) {
	cfg, err := m.Get(ctx, storeID)
	if err != nil {
		return Runtime{}, err
	}
	s, ok := cfg.Services[provider]
	if !ok {
		return Runtime{}, fmt.Errorf("未知外部服务")
	}
	p, ok := s.Profiles[env]
	if !ok {
		return Runtime{}, fmt.Errorf("请选择Mock或真实环境")
	}
	secrets, err := m.decrypt(storeID, provider, env, p)
	return Runtime{Binding{provider, env, p.Version, cfg.Version}, p.BaseURL, secrets}, err
}
func (m *Manager) Resolve(ctx context.Context, storeID, provider string) (Runtime, error) {
	cfg, err := m.Get(ctx, storeID)
	if err != nil {
		return Runtime{}, err
	}
	s, ok := cfg.Services[provider]
	if !ok {
		return Runtime{}, fmt.Errorf("未知外部服务")
	}
	return m.Profile(ctx, storeID, provider, s.Active)
}
func (m *Manager) Current(ctx context.Context, storeID string, b Binding) bool {
	r, err := m.Resolve(ctx, storeID, b.Provider)
	return err == nil && r.Binding == b
}
func (m *Manager) Save(ctx context.Context, storeID string, u Update) (Config, error) {
	if _, err := m.Get(ctx, storeID); err != nil {
		return Config{}, err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	var cfg Config
	if err := m.db.ForStore(storeID).Get(ctx, "integrations", "default", &cfg); err != nil {
		return cfg, err
	}
	if cfg.Version != u.Version {
		return cfg, ErrConflict
	}
	s, ok := cfg.Services[u.Provider]
	if !ok {
		return cfg, fmt.Errorf("未知外部服务")
	}
	p, ok := s.Profiles[u.Environment]
	if !ok {
		return cfg, fmt.Errorf("请选择Mock或真实环境")
	}
	u.BaseURL = strings.TrimRight(strings.TrimSpace(u.BaseURL), "/")
	if u.BaseURL != "" {
		if err := ValidateURL(u.BaseURL); err != nil {
			return cfg, err
		}
	}
	if u.Activate && u.BaseURL == "" {
		return cfg, fmt.Errorf("请先填写服务地址")
	}
	secrets, err := m.decrypt(storeID, u.Provider, u.Environment, p)
	if err != nil {
		return cfg, err
	}
	allowed := map[string]bool{}
	for _, k := range fields(u.Provider) {
		allowed[k] = true
	}
	for k, v := range u.Credentials {
		if !allowed[k] {
			return cfg, fmt.Errorf("不支持的凭据字段")
		}
		if v != "" {
			secrets[k] = v
		}
	}
	for _, k := range u.Clear {
		if !allowed[k] {
			return cfg, fmt.Errorf("不支持的凭据字段")
		}
		delete(secrets, k)
	}
	p.BaseURL = u.BaseURL
	p.TestProductID = u.TestProductID
	p.Version++
	p.Ciphertext, err = m.encrypt(storeID, u.Provider, u.Environment, secrets)
	if err != nil {
		return cfg, err
	}
	s.Profiles[u.Environment] = p
	if u.Activate {
		s.Active = u.Environment
	}
	cfg.Services[u.Provider] = s
	cfg.Version++
	err = m.db.ForStore(storeID).Put(ctx, "integrations", "default", cfg)
	return cfg, err
}

// Copies the previous process-wide Seerfar settings once, without overwriting web settings.
func (m *Manager) MigrateLegacy(ctx context.Context, base, key string, mock bool) error {
	rows, err := m.db.List(ctx, "stores")
	if err != nil {
		return err
	}
	for _, row := range rows {
		var s struct {
			ID string `json:"id"`
		}
		if json.Unmarshal(row, &s) != nil {
			continue
		}
		var old Config
		err = m.db.ForStore(s.ID).Get(ctx, "integrations", "default", &old)
		if err == nil {
			continue
		}
		if !errors.Is(err, store.ErrNotFound) {
			return err
		}
		cfg, e := m.Get(ctx, s.ID)
		if e != nil {
			return e
		}
		if base != "" {
			env := "real"
			if mock {
				env = "mock"
			}
			_, e = m.Save(ctx, s.ID, Update{Provider: "seerfar", Environment: env, Version: cfg.Version, BaseURL: base, Credentials: map[string]string{"api_key": key}, Activate: true})
			if e != nil {
				return e
			}
		}
	}
	return nil
}
