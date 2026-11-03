package frankfurter

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestRatePairDateAndErrors(t *testing.T) {
	for _, tc := range []struct {
		body string
		ok   bool
	}{
		{`{"date":"2026-10-30","base":"CNY","quote":"RUB","rate":12.0844}`, true},
		{`{"date":"2026-10-30","base":"RUB","quote":"CNY","rate":12}`, false},
		{`{"date":"2026-10-30","base":"CNY","quote":"RUB","rate":0}`, false},
		{`{"date":"2026-10-30","base":"CNY","quote":"RUB","rate":-1}`, false},
		{`{"base":"CNY","quote":"RUB","rate":12}`, false},
	} {
		up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path != "/v2/rate/CNY/RUB" || r.Header.Get("Authorization") != "Bearer private-token" {
				t.Error("wrong request")
			}
			w.Write([]byte(tc.body))
		}))
		q, err := New(up.URL, "private-token").Latest(t.Context())
		up.Close()
		if (err == nil) != tc.ok {
			t.Fatalf("%s: %v", tc.body, err)
		}
		if tc.ok && (q.Date != "2026-10-30" || q.Rate != 12.0844) {
			t.Fatalf("%+v", q)
		}
	}
}
