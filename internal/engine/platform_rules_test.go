package engine

import (
	"strings"
	"testing"
)

func TestQuoteComponentsAreRequiredAndUseShipmentCaps(t *testing.T) {
	in := basic()
	in.Settings.Stress = 0
	in.Quotes[0].RequiredFees = map[string]string{"logistics_access": "物流平台电子服务"}
	r, err := Solve(in)
	if err != nil || r.Recommended != nil || r.Status != "incomplete" {
		t.Fatalf("missing required service treated as zero: %+v %v", r, err)
	}
	in.Quotes[0].Fees = []Fee{{Code: "logistics_access", Name: "物流平台电子服务", Enabled: true, Kind: "rate", Rate: Ptr(.02), Currency: "RUB", Basis: "sale", On: "delivered", Minimum: Ptr(15), Maximum: Ptr(200), Status: "pending", Source: "OGL费用"}}
	r, err = Solve(in)
	if err != nil || r.Recommended != nil || r.Status != "incomplete" {
		t.Fatalf("pending route component generated a complete recommendation: %+v %v", r, err)
	}
	in.Quotes[0].Fees[0].Status = "confirmed"
	for _, tc := range []struct{ price, cost float64 }{{50, 73.75}, {100, 87}, {2000, 576.666666666667}} {
		ev, err := Evaluate(in, in.Quotes[0], tc.price)
		if err != nil {
			t.Fatal(err)
		}
		near(t, ev.Cost, tc.cost)
	}
	in.Settings.Fees = append(in.Settings.Fees, in.Quotes[0].Fees[0])
	if _, err := Evaluate(in, in.Quotes[0], 100); err == nil || !strings.Contains(err.Error(), "重复") {
		t.Fatal("same service in store and quote was charged twice", err)
	}
}

func TestFeeCapsApplyBeforeExpectedRetention(t *testing.T) {
	in := basic()
	in.Product.Returns = Returns{Rate: .5, Reverse: Ptr(0), Disposal: Ptr(0), Recovery: Ptr(0), Destination: "partner", Source: "商家售后协议"}
	in.Settings.Fees = []Fee{{Code: "service", Name: "留存订单服务", Enabled: true, Kind: "rate", Rate: Ptr(.02), Currency: "CNY", Basis: "retained_revenue", On: "kept", Minimum: Ptr(3)}}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 61.5)
	in.Settings.Fees[0].Minimum = nil
	in.Settings.Fees[0].Maximum = Ptr(1)
	ev, err = Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 60.5)
}

func TestAdvertisingAllocationCanIncludeCancelledOrders(t *testing.T) {
	in := basic()
	in.Product.Returns = Returns{CancelRate: .2, CancelCost: Ptr(0)}
	in.Settings.Fees = []Fee{{Code: "advertising", Name: "点击广告分摊预算", Enabled: true, Kind: "fixed", Amount: Ptr(10), Currency: "CNY", On: "ordered"}}
	if err := Validate(in); err != nil {
		t.Fatal(err)
	}
	ev, err := Evaluate(in, in.Quotes[0], 100)
	if err != nil {
		t.Fatal(err)
	}
	near(t, ev.Cost, 58)
	near(t, ev.Profit, 22)
}

func TestRealReturnsRequireDestinationAndDoNotAssumeWHDResale(t *testing.T) {
	in := basic()
	in.Product.Returns = Returns{Rate: .1, Reverse: Ptr(0), Disposal: Ptr(0), Recovery: Ptr(0)}
	if _, err := Solve(in); err == nil || !strings.Contains(err.Error(), "售后去向") {
		t.Fatal("unknown after-sales destination accepted", err)
	}
	in.Product.Returns.Destination = "whd"
	in.Product.Returns.Source = "Ozon仓库设置"
	if _, err := Solve(in); err == nil || !strings.Contains(err.Error(), "WHD") {
		t.Fatal("WHD resale generated a complete cross-border-only conclusion", err)
	}
	in.Product.Returns.Destination = "partner"
	if _, err := Solve(in); err != nil {
		t.Fatal(err)
	}
	in.Product.Returns.Destination = "disposal"
	in.Product.Returns.Recovery = Ptr(1)
	if _, err := Solve(in); err == nil {
		t.Fatal("disposed goods retained recovery value")
	}
}

func TestOzonCommissionCannotUsePlatformDiscountedBuyerPrice(t *testing.T) {
	in := basic()
	in.Settings.Fees[0].Basis = "buyer_price"
	if err := Validate(in); err == nil {
		t.Fatal("Ozon commission used buyer subsidy to reduce its basis")
	}
	in.Settings.Fees[0].Basis = "sale"
	in.Settings.Fees[0].TierCurrency = "CNY"
	in.Settings.Fees[0].Tiers = []Tier{{Rate: .25}}
	if err := Validate(in); err == nil {
		t.Fatal("CNY thresholds presented as official Ozon commission bands")
	}
}
