package engine

import (
	"encoding/json"
	"strings"
	"testing"
)

func referenceProfitCheck(t *testing.T) ProfitCheck {
	t.Helper()
	var c ProfitCheck
	if err := json.Unmarshal([]byte(`{"order_ref":"sample-order","date":"2026-10-23","source":"商家订单截图","carrier":"China Post to OZON Economy","freight_kind":"estimated","fee_label":"原型基线 A（16%佣金）","fee_kind":"prototype","amount":29,"procurement":7.7,"freight":11.1,"commission_rate":0.16,"fixed_fee":1.36,"reported_profit":4.2}`), &c); err != nil {
		t.Fatal(err)
	}
	return c
}

func TestProfitCheckMatchesSourceUsingCostEngine(t *testing.T) {
	c := referenceProfitCheck(t)
	r, err := CheckProfit(c)
	if err != nil || r.Status != "matched" || r.Profit == nil || r.Difference == nil {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, *r.Profit, 4.2)
	near(t, *r.Difference, 0)
	near(t, *r.Margin, 4.2/29)
	if len(r.Lines) != 4 || !strings.Contains(r.Formula, "4.20") || !strings.Contains(strings.Join(r.Messages, " "), "原型") {
		t.Fatalf("scope or engine fee lines missing: %+v", r)
	}
	var commission float64
	for _, line := range r.Lines {
		if line.Code == "commission" {
			commission = line.CNY
		}
	}
	near(t, commission, 4.64)
	// Changing an input must change the engine result, rather than echoing the source profit.
	c.Freight = Ptr(12.1)
	r, err = CheckProfit(c)
	if err != nil || r.Status != "different" {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, *r.Profit, 3.2)
	near(t, *r.Difference, -1)
}

func TestProfitCheckMissingCostIsNotZero(t *testing.T) {
	c := referenceProfitCheck(t)
	c.FixedFee = nil
	r, err := CheckProfit(c)
	if err != nil || r.Status != "incomplete" || r.Profit != nil {
		t.Fatalf("%+v %v", r, err)
	}
	c.FixedFee = Ptr(0)
	r, err = CheckProfit(c)
	if err != nil || r.Profit == nil {
		t.Fatalf("%+v %v", r, err)
	}
	near(t, *r.Profit, 5.56)
	c.CommissionRate = Ptr(-.1)
	if _, err = CheckProfit(c); err == nil {
		t.Fatal("negative commission accepted")
	}
}
