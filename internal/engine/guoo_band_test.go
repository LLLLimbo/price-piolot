package engine

import (
	"encoding/json"
	"testing"
)

func TestBandMinimumValueAndReferenceWeight(t *testing.T) {
	in := basic()
	q := in.Quotes[0]
	var band Band
	json.Unmarshal([]byte(`{"min_value":135.01,"up_to":635,"min_weight":1,"max_weight":2000,"fixed":17.97,"per_gram":0.0281,"step":1}`), &band)
	q.Bands = []Band{band}
	if _, _, err := quoteCost(in, q, 135, in.Settings.FX); err == nil {
		t.Fatal("lower value limit ignored")
	}
	if _, _, err := quoteCost(in, q, 135.01, in.Settings.FX); err != nil {
		t.Fatal(err)
	}
	q.Bands = []Band{{UpTo: Ptr(135), MinWeight: 1, MaxWeight: 500, Fixed: 3.37, PerGram: .0281, Step: 1, MaxLength: 60, MaxSum: 90}}
	for _, tt := range []struct{ weight, want float64 }{{275, 11.0975}, {290, 11.519}} {
		in.Product.Weight = tt.weight
		cost, _, err := quoteCost(in, q, 29, in.Settings.FX)
		if err != nil {
			t.Fatal(err)
		}
		near(t, num(cost), tt.want)
	}
}
