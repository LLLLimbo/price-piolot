package app

import (
	"pricepilot/internal/engine"
	"testing"
)

func TestGUOOPublicCatalogHasSourcedRatesAndUnpricedWarehouses(t *testing.T) {
	quotes := publicQuotes()
	rates, warehouses := 0, 0
	for _, q := range quotes {
		if q.Provider != "GUOO" {
			continue
		}
		if err := engine.ValidateQuote(q); err != nil {
			t.Fatalf("%s: %v", q.ID, err)
		}
		if q.Kind == "warehouse" {
			warehouses++
			if len(q.Bands) != 0 || q.Status != "pending" {
				t.Fatal("unknown warehouse fee became a usable zero price")
			}
		} else {
			rates++
			if q.Status != "historical" || q.Demo {
				t.Fatal("unverified historical GUOO rates recommended as current")
			}
		}
	}
	if rates != 15 || warehouses != 2 {
		t.Fatalf("rates=%d warehouses=%d", rates, warehouses)
	}
}
