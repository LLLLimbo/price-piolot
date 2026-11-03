// mockfx is a deterministic Frankfurter v2 protocol fixture for the demo network.
package main

import (
	"encoding/json"
	"log"
	"net/http"
	"os"
	"strings"
	"time"
)

func main() {
	if len(os.Args) > 1 && os.Args[1] == "health" {
		client := http.Client{Timeout: 2 * time.Second}
		r, err := client.Get("http://127.0.0.1:8080/health")
		if err != nil {
			os.Exit(1)
		}
		r.Body.Close()
		if r.StatusCode != 200 {
			os.Exit(1)
		}
		return
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", func(w http.ResponseWriter, _ *http.Request) { w.Write([]byte("ok")) })
	mux.HandleFunc("GET /", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		if !strings.HasSuffix(r.URL.Path, "/v2/rate/CNY/RUB") {
			http.NotFound(w, r)
			return
		}
		if strings.HasPrefix(r.URL.Path, "/rate-limit/") {
			w.WriteHeader(429)
			w.Write([]byte(`{"status":429,"message":"mock rate limit"}`))
			return
		}
		base, quote, rate := "CNY", "RUB", 12.0844
		if strings.HasPrefix(r.URL.Path, "/inverted/") {
			base, quote = "RUB", "CNY"
		}
		if strings.HasPrefix(r.URL.Path, "/invalid/") {
			rate = 0
		}
		json.NewEncoder(w).Encode(map[string]any{"date": "2026-10-30", "base": base, "quote": quote, "rate": rate})
	})
	s := http.Server{Addr: ":8080", Handler: mux, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 10 * time.Second, WriteTimeout: 10 * time.Second}
	log.Fatal(s.ListenAndServe())
}
