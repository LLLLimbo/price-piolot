package seerfar

import (
	"context"
	"net/http"
	"net/http/httptest"
	"pricepilot/internal/engine"
	"strings"
	"testing"
)

func TestDiscoverUsesDocumentedPathsAndBearer(t *testing.T) {
	paths := []string{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer test-key" {
			t.Error("missing bearer")
		}
		paths = append(paths, r.URL.Path)
		w.Header().Set("Content-Type", "application/json")
		switch r.URL.Path {
		case "/open-api/productReport/search/ozon":
			w.Write([]byte(`{"code":200,"data":{"records":[{"sku":123,"title":"термос 500ml","price":1500,"categoryInfo":{"category":{"id":"cups"}},"characteristics":[{"name":"容量","value":"500ml"}]}]}}`))
		case "/open-api/product/detail/similar/ozon":
			w.Write([]byte(`{"code":200,"data":[{"sku":123,"title":"термос 500ml","price":1500,"categoryInfo":{"category":{"id":"cups"}},"characteristics":[{"name":"容量","value":"500ml"}]}]}`))
		default:
			w.Write([]byte(`{"code":200,"data":{"records":[]}}`))
		}
	}))
	defer server.Close()
	c := New(server.URL, "test-key")
	p := engine.Product{Name: "保温杯", CategoryID: "cups", Keywords: []string{"термос"}, Attributes: map[string]string{"容量": "500ml"}}
	rows, err := c.Discover(context.Background(), p)
	if err != nil || len(rows) != 1 || !rows[0].Include {
		t.Fatalf("%+v %v", rows, err)
	}
	if rows[0].Delivery != nil {
		t.Fatal("invented actual delivery")
	}
	if !strings.Contains(strings.Join(paths, ","), "productReport/search/ozon") {
		t.Fatal("wrong endpoint")
	}
}
func TestBusinessErrorAtHTTP200(t *testing.T) {
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(`{"code":402,"msg":"积分不足"}`)) }))
	defer s.Close()
	_, err := New(s.URL, "key").Discover(context.Background(), engine.Product{Keywords: []string{"x"}})
	if err == nil {
		t.Fatal("business error became empty competitors")
	}
	if e, ok := err.(*Error); !ok || e.Kind != "quota" {
		t.Fatalf("%v", err)
	}
}
func TestRateLimitAndMalformedUpstream(t *testing.T) {
	for _, status := range []int{429, 502} {
		s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.WriteHeader(status); w.Write([]byte(`upstream`)) }))
		_, err := New(s.URL, "secret-key").Discover(context.Background(), engine.Product{Keywords: []string{"x"}})
		s.Close()
		if err == nil || strings.Contains(err.Error(), "secret-key") {
			t.Fatal("missing or unsafe error")
		}
	}
}
func TestUnknownProductIsEmpty(t *testing.T) {
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(`{"code":200,"data":null}`)) }))
	defer s.Close()
	rows, err := New(s.URL, "key").Discover(context.Background(), engine.Product{SeedSKU: "123"})
	if err != nil || len(rows) != 0 {
		t.Fatalf("%v %+v", err, rows)
	}
}
