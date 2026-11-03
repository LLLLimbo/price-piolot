package engine

import (
	"testing"
)

func TestPlatformDiscountDoesNotReduceOrDuplicateSellerRevenue(t *testing.T) {
	in := basic()
	in.Product.Discount = .2
	in.Product.PlatformDiscount = Discount{Kind: "fixed", Value: Ptr(120), Currency: "RUB", Confirmed: true, Source: "演示平台承担"}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.SalePrice, 80)
	near(t, ev.PriceRUB, 840)
	near(t, ev.Revenue, 80)
	near(t, ev.Cost, 80)
	near(t, ev.Profit, 0)
	near(t, ev.Prices.PlatformDiscountCNY, 10)
	near(t, ev.Prices.RetainedRevenueCNY, 80)
}

func TestFixedMerchantDiscountAndP90UseTheSamePriceMapping(t *testing.T) {
	in := basic()
	in.Product.FixedDiscount = 10
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, r.Recommended.Price, 119.1)
	in.Product.Mode = "target"
	in.Product.PlatformDiscount = Discount{Kind: "percent", Value: Ptr(.1), Confirmed: true, Source: "平台承担"}
	in.Product.Options.TargetScore = 0
	r, err = Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, r.PriceCap, 147.5/0.9+10) // peer P90 is 1770 RUB = 147.5 CNY
	if r.Recommended.PriceRUB > 1770.001 {
		t.Fatal("automatic P90 cap escaped buyer price mapping")
	}
}

func TestPromotionsReplaceDailyDiscountAndFailIndependently(t *testing.T) {
	in := basic()
	in.Product.Discount = .1
	promos := []PromotionScenario{
		{ID: "campaign", Name: "活动", Source: "演示", MerchantDiscount: Discount{Kind: "percent", Value: Ptr(.2)}, PlatformDiscount: Discount{Kind: "fixed", Value: Ptr(5), Currency: "CNY", Confirmed: true, Source: "平台承担"}},
		{ID: "unknown", Name: "承担方未确认", Source: "演示", PlatformDiscount: Discount{Kind: "percent", Value: Ptr(.1)}},
	}
	rows := CalculateContexts(in, promos)
	if len(rows) != 3 || rows[0].Result.Recommended == nil || rows[1].Result.Recommended == nil {
		t.Fatalf("%+v", rows)
	}
	near(t, rows[1].Result.Recommended.SalePrice, rows[1].Result.Recommended.Price*.8)
	if rows[2].Result.Status != "invalid" || rows[2].Result.Recommended != nil {
		t.Fatal("unconfirmed subsidy generated complete conclusion")
	}
	for _, row := range rows {
		if row.Result.SnapshotID != in.Snapshot.ID {
			t.Fatal("context changed competitor snapshot")
		}
	}
}

func TestRefusedAndReturnedOrdersHaveSeparateLosses(t *testing.T) {
	in := basic()
	in.Settings.Fees = nil
	in.Product.Returns = Returns{Destination: "partner", Source: "测试协议", CancelRate: .1, RefusalRate: .2, Rate: .1, CancelCost: Ptr(2), RefusalReverse: Ptr(5), RefusalDisposal: Ptr(1), RefusalRecovery: Ptr(20), Reverse: Ptr(10), Disposal: Ptr(2), Recovery: Ptr(30)}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Revenue, 60)
	// Procurement 54; cancellation .2; refusal 1+.2-4; return 1+.2-3.
	near(t, ev.Cost, 49.6)
	near(t, ev.Profit, 10.4)
	in.Product.Returns.RefusalReverse = nil
	if _, err := Solve(in); err == nil {
		t.Fatal("unknown refusal loss treated as zero")
	}
}

func TestSalesScenariosAreAssumptionsAndKeepConstraintFailuresVisible(t *testing.T) {
	in := basic()
	s := SalesScenario{ID: "s", Price: 100, QuoteID: "route", DailyUnits: Ptr(.5)}
	r := EvaluateSales(in, s)
	if r.Evaluation == nil || r.DailyProfit == nil {
		t.Fatalf("%+v", r)
	}
	near(t, *r.DailyProfit, 7.5)
	if r.MeetsConstraints {
		t.Fatal("under-margin scenario marked feasible")
	}
	s.DailyUnits = nil
	r = EvaluateSales(in, s)
	if r.DailyProfit != nil || r.WorstDailyProfit != nil {
		t.Fatal("missing quantity generated a prediction")
	}
	s.DailyUnits = Ptr(0)
	r = EvaluateSales(in, s)
	if r.DailyProfit == nil {
		t.Fatal("explicit zero quantity lost")
	}
}

func TestFeeVersionsAndKnownPriceConditions(t *testing.T) {
	in := basic()
	in.Settings.Fees[0].Status = "pending"
	r, _ := Solve(in)
	if r.Recommended != nil {
		t.Fatal("pending fee used")
	}
	in.Settings.Fees[0].Status = "confirmed"
	in.Settings.Fees[0].ValidTo = "2026-01-01"
	r, _ = Solve(in)
	if r.Recommended != nil {
		t.Fatal("expired fee used")
	}
	in.Settings.Fees[0].ValidTo = ""
	in.Product.PriceCondition = "standard"
	in.Snapshot.Competitors[0].PriceCondition = "member"
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	if ev.Score.Samples != 1 {
		t.Fatal("known incompatible price condition included")
	}
}

func TestFXStressIncludesInteriorFeeCap(t *testing.T) {
	in := basic()
	in.Product.Costs[0].Amount = Ptr(40)
	in.Product.PlatformDiscount = Discount{Kind: "fixed", Value: Ptr(120), Currency: "RUB", Confirmed: true, Source: "平台承担"}
	in.Quotes[0].Currency = "RUB"
	in.Quotes[0].Bands[0].Fixed = 30
	in.Settings.Fees = []Fee{{Code: "payment", Name: "买家基数费用", Enabled: true, Kind: "rate", Rate: Ptr(.5), Currency: "CNY", Basis: "buyer_price", On: "kept", Maximum: Ptr(44.9)}}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.WorstProfit, 12.55)
	in.Product.Options.Margin = .1258
	result, err := Solve(in)
	if err != nil || result.Recommended == nil {
		t.Fatalf("%+v %v", result, err)
	}
	want := 0.0
	for cents := 9900; cents <= 10200; cents++ {
		v, e := Evaluate(in, in.Quotes[0], float64(cents)/100)
		if e == nil && v.Valid && v.WorstMargin >= .1258-1e-10 {
			want = float64(cents) / 100
			break
		}
	}
	if want == 0 {
		t.Fatal("enumeration found no feasible price")
	}
	near(t, result.Recommended.Price, want)
}
