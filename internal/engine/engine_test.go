package engine

import (
	"math"
	"strings"
	"testing"
	"time"
)

func basic() Input {
	now := time.Date(2026, 11, 2, 12, 0, 0, 0, time.UTC)
	p := Product{ID: "p", Name: "测试商品", Supply: "stock", PurchaseQuantity: 1, MOQ: 1, Weight: 200, Length: 10, Width: 10, Height: 10, SupplierScore: Ptr(4.5), SupplierScale: 5, SupplierSource: "1688", SupplierDays: &Days{0, 0}, DomesticDays: &Days{1, 2}, TailDays: &Days{1, 2}, Mode: "profit", Options: ModeOptions{Margin: .2, TargetScore: 80}, Costs: []Fee{{Code: "procurement", Name: "采购价", Enabled: true, Kind: "fixed", Amount: Ptr(60), Currency: "CNY", On: "shipped", Source: "商家"}}}
	s := Settings{FX: 12, Stress: .05, FXDate: "2026-11-02", FXSource: "商家确认", Region: "莫斯科", SnapshotHours: 24, Weights: Weights{50, 30, 20}, Fees: []Fee{{Code: "commission", Name: "佣金", Enabled: true, Kind: "rate", Rate: Ptr(.25), Currency: "CNY", Basis: "sale", On: "kept", Source: "商家确认"}}}
	q := Quote{ID: "route", Provider: "签约货代", Name: "陆运", Kind: "international", Status: "active", Currency: "CNY", ValueCurrency: "CNY", ValidFrom: "2026-01-01", ValidTo: "2026-12-31", Source: "商家合同", Platforms: []string{"ozon"}, Batteries: true, Liquids: true, Delivery: &Days{10, 15}, TimeScope: "buyer", Bands: []Band{{MaxWeight: 35000, Step: 1}}}
	snap := Snapshot{ID: "snap", ProductID: "p", Fingerprint: Fingerprint(p), Source: "manual", FetchedAt: now, Competitors: []Competitor{{SKU: "a", Title: "同款", PriceRUB: 1500, Include: true, Verified: true, Delivery: &Days{15, 20}, DeliverySource: "手动确认"}, {SKU: "b", PriceRUB: 1800, Include: true, Verified: true, Delivery: &Days{10, 15}, DeliverySource: "手动确认"}}}
	return Input{Product: p, Settings: s, Quotes: []Quote{q}, Snapshot: snap, AsOf: now}
}
func near(t *testing.T, got, want float64) {
	t.Helper()
	if math.Abs(got-want) > 1e-6 {
		t.Fatalf("got %.9f, want %.9f", got, want)
	}
}
func TestMinimumPriceExample(t *testing.T) {
	in := basic()
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, r.Recommended.Price, 109.10)
	if r.Recommended.WorstMargin < .2 {
		t.Fatal("margin floor violated")
	}
}
func TestDisplayedFormulaGroupsChargesBeforeConversionAndOccurrence(t *testing.T) {
	in := basic()
	in.Product.Returns = Returns{CancelRate: .1, CancelCost: Ptr(0)}
	in.Quotes[0].Currency = "RUB"
	in.Quotes[0].Bands = []Band{{MaxWeight: 35000, Fixed: 144, PerGram: .12, Step: 1}}
	in.Settings.Fees[0].Minimum = Ptr(35)
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	for _, line := range ev.Lines {
		switch line.Code {
		case "international":
			near(t, line.CNY, 12.6)
			if line.Formula != "(144 + 200 g × 0.12 RUB/g) ÷ 12 RUB/CNY × 发生比例 90%" {
				t.Fatalf("displayed formula changes operation order: %s", line.Formula)
			}
		case "commission":
			near(t, line.CNY, 31.5)
			if !strings.Contains(line.Formula, "max(100.0000 CNY × 25%, 35 CNY)") {
				t.Fatalf("minimum charge is absent from calculation: %s", line.Formula)
			}
		}
	}
}
func TestFourModesAreIndependent(t *testing.T) {
	for _, mode := range []string{"profit", "breakeven", "delivery", "target"} {
		t.Run(mode, func(t *testing.T) {
			in := basic()
			in.Product.Mode = mode
			in.Product.Options.Margin = .90
			in.Product.Options.DeliveryLoss = 15
			in.Product.Options.TargetLoss = 9
			in.Product.Options.TargetScore = 50
			r, err := Solve(in)
			if err != nil {
				t.Fatal(err)
			}
			if mode == "profit" {
				if r.Recommended != nil {
					t.Fatal("unattainable margin accepted")
				}
				return
			}
			if r.Recommended == nil {
				t.Fatalf("independent parameters lost: %+v", r)
			}
			if mode == "breakeven" {
				near(t, r.Recommended.Price, 80)
				if r.Recommended.WorstProfit < 0 {
					t.Fatal("loss in break-even")
				}
			}
		})
	}
}
func TestFXStressIncludesInteriorTierTransitions(t *testing.T) {
	in := basic()
	in.Settings.Fees = []Fee{{Code: "commission", Name: "佣金", Enabled: true, Kind: "rate", Rate: Ptr(.1), Currency: "CNY", Basis: "sale", On: "kept", TierCurrency: "RUB", Tiers: []Tier{{UpTo: Ptr(1200), Rate: .1}, {UpTo: Ptr(1250), Rate: .45}, {Rate: .1}}}}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.WorstProfit, -5)
	if len(ev.Scenarios) < 5 {
		t.Fatal("interior FX boundaries not evaluated")
	}
}
func TestFeeCapAndSettlementBasis(t *testing.T) {
	in := basic()
	in.Settings.Fees = []Fee{{Code: "commission", Name: "佣金", Enabled: true, Kind: "rate", Rate: Ptr(.1), Currency: "CNY", Basis: "sale", On: "kept", DeductSettlement: true}, {Code: "withdrawal", Name: "提现", Enabled: true, Kind: "rate", Rate: Ptr(.1), Currency: "CNY", Basis: "settlement", On: "kept", Minimum: Ptr(2), Maximum: Ptr(5)}}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 75)
}
func TestReturnRefundAndRecoveryNotDoubleCounted(t *testing.T) {
	in := basic()
	in.Product.Returns = Returns{Destination: "partner", Source: "测试协议", Rate: .1, CancelRate: .1, Reverse: Ptr(10), Disposal: Ptr(0), Recovery: Ptr(30), CancelCost: Ptr(2)}
	in.Settings.Fees[0].Rate = Ptr(.1)
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Revenue, 80)
	near(t, ev.Cost, 60.2)
	near(t, ev.Profit, 19.8)
	near(t, ev.Margin, .2475)
}
func TestIncludedTailIsNotChargedAgain(t *testing.T) {
	in := basic()
	in.Quotes[0].Includes = []string{"tail_delivery"}
	in.Product.Costs = append(in.Product.Costs, Fee{Code: "tail_delivery", Name: "尾程", Enabled: true, Kind: "fixed", Amount: Ptr(10), Currency: "CNY", On: "shipped"})
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 85)
	found := false
	for _, l := range ev.Lines {
		if l.Code == "tail_delivery" && l.Included {
			found = true
		}
	}
	if !found {
		t.Fatal("included service not explained")
	}
}
func TestUnknownCostIsNotZero(t *testing.T) {
	in := basic()
	in.Product.Costs[0].Amount = nil
	if _, err := Solve(in); err == nil {
		t.Fatal("unknown procurement accepted")
	}
}
func TestWeightAndValueBandRecalculation(t *testing.T) {
	in := basic()
	in.Quotes[0].Bands = []Band{{UpTo: Ptr(135), MaxWeight: 500, Fixed: 2.9, PerGram: .0247, Step: 1}, {UpTo: Ptr(135), MinWeight: 500.00001, MaxWeight: 35000, Fixed: 22.25, PerGram: .0164, Step: 1}, {MinWeight: 0, MaxWeight: 2000, Fixed: 15.48, PerGram: .0242, Step: 1}}
	in.Settings.Fees = nil
	in.Product.Weight = 500
	ev, _ := Evaluate(in, in.Quotes[0], 100)
	near(t, ev.Cost, 75.25)
	in.Product.Weight = 501
	ev, _ = Evaluate(in, in.Quotes[0], 100)
	near(t, ev.Cost, 90.4664)
	ev, _ = Evaluate(in, in.Quotes[0], 136)
	near(t, ev.Cost, 87.6042)
}
func TestVolumeAndBillingRounding(t *testing.T) {
	in := basic()
	in.Settings.Fees = nil
	in.Product.Length = 60
	in.Product.Width = 50
	in.Product.Height = 40
	in.Quotes[0].Bands = []Band{{MaxWeight: 35000, PerGram: .01, VolumeDivisor: 12, Step: 100}}
	in.Product.Weight = 1044
	ev, err := Evaluate(in, in.Quotes[0], 200)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 160)
}
func TestInactiveExpiredAndExcludedQuotesNeverRecommend(t *testing.T) {
	for _, status := range []string{"pending", "historical", "excluded", "expired"} {
		t.Run(status, func(t *testing.T) {
			in := basic()
			if status == "expired" {
				in.Quotes[0].ValidTo = "2026-01-01"
			} else {
				in.Quotes[0].Status = status
			}
			r, err := Solve(in)
			if err != nil {
				t.Fatal(err)
			}
			if r.Recommended != nil || len(r.Unavailable) == 0 {
				t.Fatal("unavailable route selected")
			}
		})
	}
}
func TestUnknownDeliveryProducesScoreRange(t *testing.T) {
	in := basic()
	in.Snapshot.Competitors[0].Delivery = nil
	ev, _ := Evaluate(in, in.Quotes[0], 100)
	if ev.Score.Total != nil || ev.Score.Maximum <= ev.Score.Minimum {
		t.Fatal("fabricated complete score")
	}
	in.Product.Mode = "target"
	r, _ := Solve(in)
	if r.Recommended != nil {
		t.Fatal("target solved using unknown delivery")
	}
}
func TestSupplierCeilingAndTargetUnreachable(t *testing.T) {
	in := basic()
	in.Product.Mode = "target"
	in.Product.Options.TargetScore = 99
	r, err := Solve(in)
	if err != nil {
		t.Fatal(err)
	}
	near(t, r.TheoreticalMax, 98)
	if r.Recommended != nil || r.Status != "infeasible" {
		t.Fatal("target ceiling ignored")
	}
}
func TestLossBudgetsAndProfitMaximization(t *testing.T) {
	in := basic()
	in.Product.Mode = "target"
	in.Product.Options.TargetScore = 95
	in.Product.Options.TargetLoss = 30
	in.Product.Options.TargetMaxPrice = Ptr(200)
	in.Snapshot.Competitors[0].Delivery = &Days{30, 40}
	in.Snapshot.Competitors[1].Delivery = &Days{30, 40}
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("%+v %v", r, err)
	}
	if r.Recommended.WorstProfit < -30 || r.Recommended.Score.Total == nil || *r.Recommended.Score.Total < 95 {
		t.Fatal("target/loss violation")
	}
	p := r.Recommended.Price
	next, _ := Evaluate(in, in.Quotes[0], p+.01)
	if next.Score.Total != nil && *next.Score.Total >= 95 && next.Profit > r.Recommended.Profit {
		t.Fatal("not maximum profit at target")
	}
}
func TestStaleSnapshotDoesNotProduceCurrentScore(t *testing.T) {
	in := basic()
	in.Snapshot.FetchedAt = in.AsOf.Add(-25 * time.Hour)
	ev, _ := Evaluate(in, in.Quotes[0], 100)
	if !ev.Score.Stale || ev.Score.Total != nil {
		t.Fatal("stale score presented as current")
	}
}
func TestMatchingRejectsWrongSpecificationsAndOwnStore(t *testing.T) {
	p := basic().Product
	p.CategoryID = "cups"
	p.Keywords = []string{"термос"}
	p.Attributes = map[string]string{"容量": "500ml"}
	p.OwnSellerIDs = []string{"my-store"}
	rows := []Competitor{{SKU: "ok", Title: "термос 500ml", CategoryID: "cups", PriceRUB: 1000, Attributes: map[string]string{"容量": "500ml"}}, {SKU: "wrong", Title: "термос 1000ml", CategoryID: "cups", PriceRUB: 1000, Attributes: map[string]string{"容量": "1000ml"}}, {SKU: "mine", Title: "термос 500ml", CategoryID: "cups", SellerID: "my-store", PriceRUB: 1000}, {SKU: "unknown", Title: "термос", CategoryID: "cups", PriceRUB: 1000}}
	out := Match(p, rows)
	if !out[0].Include || out[1].Include || out[2].Include || out[3].Include {
		t.Fatalf("bad matching %+v", out)
	}
}
func TestScoreTiesHalfCredit(t *testing.T) {
	in := basic()
	in.Settings.Stress = 0
	ev, _ := Evaluate(in, in.Quotes[0], 125)
	near(t, *ev.Score.Price, 75)
}
func TestInvalidWeightsAndModeRejected(t *testing.T) {
	in := basic()
	in.Settings.Weights.Price = 60
	if _, err := Solve(in); err == nil {
		t.Fatal("invalid weights accepted")
	}
	in = basic()
	in.Product.Mode = "combined"
	if _, err := Solve(in); err == nil {
		t.Fatal("combined mode accepted")
	}
}

func TestSellerDiscountIsAppliedBeforeFeesAndScore(t *testing.T) {
	in := basic()
	in.Product.Discount = .2
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("%v %+v", err, r)
	}
	near(t, r.Recommended.Price, 136.37)
	if r.Recommended.WorstMargin < .2 {
		t.Fatal("discount broke the margin floor")
	}
}

func TestFeesFollowRouteAndSupplyApplicability(t *testing.T) {
	in := basic()
	extra := Fee{Code: "label", Name: "代发贴标", Enabled: true, Kind: "fixed", Amount: Ptr(10), Currency: "CNY", On: "shipped", SupplyModes: []string{"dropship"}, RouteIDs: []string{"route"}}
	in.Product.Costs = append(in.Product.Costs, extra)
	ev, _ := Evaluate(in, in.Quotes[0], 100)
	near(t, ev.Cost, 85)
	in.Product.Supply = "dropship"
	ev, _ = Evaluate(in, in.Quotes[0], 100)
	near(t, ev.Cost, 95)
	in.Product.Costs[1].RouteIDs = []string{"another"}
	ev, _ = Evaluate(in, in.Quotes[0], 100)
	near(t, ev.Cost, 85)
}

func TestLossBudgetCanHaveAnIsolatedInteriorSolution(t *testing.T) {
	in := basic()
	in.Product.Mode = "target"
	in.Product.Options.TargetScore = 0
	in.Product.Options.TargetMaxPrice = Ptr(20)
	in.Product.Options.TargetLoss = 10
	in.Product.Costs[0].Amount = Ptr(0)
	in.Settings.Fees = []Fee{{Code: "a", Name: "封顶费", Enabled: true, Kind: "rate", Rate: Ptr(1), Currency: "CNY", Basis: "sale", On: "kept", Maximum: Ptr(8)}, {Code: "b", Name: "最低费", Enabled: true, Kind: "rate", Rate: Ptr(1), Currency: "CNY", Basis: "sale", On: "kept", Minimum: Ptr(10)}, {Code: "c", Name: "比例费", Enabled: true, Kind: "rate", Rate: Ptr(.2), Currency: "CNY", Basis: "sale", On: "kept"}}
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("interior solution missed %v %+v", err, r)
	}
	near(t, r.Recommended.Price, 10)
	near(t, r.Recommended.WorstProfit, -10)
}

func TestAutoPriceCapBelowOneCentCannotBeExceeded(t *testing.T) {
	in := basic()
	in.Product.Mode = "target"
	in.Product.Options.TargetScore = 0
	in.Product.Costs[0].Amount = Ptr(0)
	for i := range in.Snapshot.Competitors {
		in.Snapshot.Competitors[i].PriceRUB = .01
	}
	r, err := Solve(in)
	if err != nil {
		t.Fatal(err)
	}
	if r.Recommended != nil {
		t.Fatal("automatic P90 ceiling below the minimum price was exceeded")
	}
}

func TestMissingTailCostAllowedOnlyWhenIncluded(t *testing.T) {
	in := basic()
	in.Product.Costs = append(in.Product.Costs, Fee{Code: "tail_delivery", Name: "尾程", Enabled: true, Kind: "fixed", Currency: "CNY", On: "shipped"})
	in.Quotes[0].Includes = []string{"tail_delivery"}
	r, err := Solve(in)
	if err != nil || r.Recommended == nil {
		t.Fatalf("included fee blocked price: %v %+v", err, r)
	}
	in.Quotes[0].Includes = nil
	r, err = Solve(in)
	if err == nil && r.Recommended != nil {
		t.Fatal("unknown fee became zero")
	}
}

func TestOptimizerAgainstCentByCentReference(t *testing.T) {
	for _, target := range []float64{40, 70, 95} {
		in := basic()
		in.Product.Mode = "target"
		in.Product.Options.TargetScore = target
		in.Product.Options.TargetMaxPrice = Ptr(20)
		in.Product.Options.TargetLoss = 2
		in.Product.Costs[0].Amount = Ptr(5)
		in.Settings.Fees = []Fee{{Code: "commission", Name: "阶梯佣金", Enabled: true, Kind: "rate", Rate: Ptr(.1), Currency: "CNY", Basis: "sale", On: "kept", TierCurrency: "RUB", Tiers: []Tier{{UpTo: Ptr(140), Rate: .1}, {UpTo: Ptr(180), Rate: .45}, {Rate: .15}}}, {Code: "service", Name: "服务费", Enabled: true, Kind: "rate", Rate: Ptr(.2), Currency: "RUB", Basis: "sale", On: "kept", Minimum: Ptr(10), Maximum: Ptr(30)}}
		in.Snapshot.Competitors[0].PriceRUB = 150
		in.Snapshot.Competitors[1].PriceRUB = 210
		for i := range in.Snapshot.Competitors {
			in.Snapshot.Competitors[i].Delivery = &Days{30, 40}
		}
		var reference *Evaluation
		for cents := 1; cents <= 2000; cents++ {
			e, err := Evaluate(in, in.Quotes[0], float64(cents)/100)
			if err == nil && feasible(in, e) && e.Score.Total != nil && *e.Score.Total >= target && (reference == nil || better(in, e, *reference, 0)) {
				v := e
				reference = &v
			}
		}
		r, err := Solve(in)
		if err != nil {
			t.Fatal(err)
		}
		if (reference == nil) != (r.Recommended == nil) {
			t.Fatalf("target%v brute and solver disagree", target)
		}
		if reference != nil {
			near(t, r.Recommended.Price, reference.Price)
		}
	}
}
