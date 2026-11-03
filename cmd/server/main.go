package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"pricepilot/internal/app"
	"pricepilot/internal/integrations"
	"pricepilot/internal/seerfar"
	"pricepilot/internal/store"
	"syscall"
	"time"
)

func env(key, fallback string) string {
	if s := os.Getenv(key); s != "" {
		return s
	}
	return fallback
}
func main() {
	if len(os.Args) > 1 && os.Args[1] == "health" {
		client := http.Client{Timeout: 2 * time.Second}
		res, err := client.Get("http://127.0.0.1:8080/api/health")
		if err != nil {
			os.Exit(1)
		}
		res.Body.Close()
		if res.StatusCode != 200 {
			os.Exit(1)
		}
		return
	}

	driver := env("DATABASE_DRIVER", "sqlite")
	dsn := env("DATABASE_DSN", "data/pricepilot.db")
	if driver == "sqlite" {
		if err := os.MkdirAll("data", 0755); err != nil {
			log.Fatal(err)
		}
	}
	db, err := store.Open(driver, dsn)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()
	if err = app.Seed(context.Background(), db); err != nil {
		log.Fatal(err)
	}
	client := seerfar.New(os.Getenv("SEERFAR_BASE_URL"), os.Getenv("SEERFAR_API_KEY"))
	key, err := integrations.LoadKey(env("INTEGRATIONS_KEY_FILE", "data/integrations.key"))
	if err != nil {
		log.Fatal(err)
	}
	services := integrations.New(db, key)
	if err = services.MigrateLegacy(context.Background(), client.BaseURL, client.Key, env("SEERFAR_MODE", "mock") == "mock"); err != nil {
		log.Fatal(err)
	}
	server := &http.Server{Addr: env("LISTEN_ADDR", ":8080"), Handler: app.New(db, client, env("STATIC_DIR", "web/dist"), env("SEERFAR_MODE", "mock") == "mock", services), ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 30 * time.Second, WriteTimeout: 60 * time.Second, IdleTimeout: 60 * time.Second}
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	go func() {
		<-ctx.Done()
		shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		server.Shutdown(shutdown)
	}()
	log.Printf("Price Pilot listening on %s (%s)", server.Addr, driver)
	if err = server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatal(err)
	}
}
