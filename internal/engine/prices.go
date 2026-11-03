package engine

import (
	"fmt"
	"github.com/shopspring/decimal"
	"sort"
	"time"
)

func buyerPrice(in Input, price, fx float64) decimal.Decimal {
	value := sale(in, price)
	v := in.Product.PlatformDiscount
	if v.Value == nil {
		return value
	}
	switch v.Kind {
	case "percent":
		return value.Mul(d(1).Sub(d(*v.Value)))
	case "fixed":
		return value.Sub(convert(d(*v.Value), v.Currency, fx))
	}
	return value
}
func listForBuyer(in Input, rub, fx float64) float64 {
	v := in.Product.PlatformDiscount
	buyer := d(rub).Div(d(fx))
	if v.Value != nil {
		switch v.Kind {
		case "percent":
			buyer = buyer.Div(d(1).Sub(d(*v.Value)))
		case "fixed":
			buyer = buyer.Add(convert(d(*v.Value), v.Currency, fx))
		}
	}
	return num(buyer.Add(d(in.Product.FixedDiscount)).Div(d(1).Sub(d(in.Product.Discount))))
}
func feeAvailable(f Fee, asOf time.Time) error {
	if err := ValidateFee(f); err != nil {
		return err
	}
	if f.Version > 0 && f.Source == "" {
		return fmt.Errorf("%s来源待补充", f.Name)
	}
	if f.Status == "pending" {
		return fmt.Errorf("%s来源与适用性待确认", f.Name)
	}
	day := asOf.In(time.FixedZone("CST", 8*3600)).Format("2006-01-02")
	if f.ValidFrom != "" && day < f.ValidFrom {
		return fmt.Errorf("%s尚未生效", f.Name)
	}
	if f.ValidTo != "" && day > f.ValidTo {
		return fmt.Errorf("%s已过期", f.Name)
	}
	return nil
}

// Currency-dependent minimums, caps and settlement clamps can have extrema
// inside the FX range. Between their knots, costs are affine in 1/fx.
type fxCash struct {
	rate  float64
	label string
	value cash
	err   error
}

func dynamicFX(in Input, q Quote) bool {
	foreign := q.Currency == "RUB"
	if wh, _ := warehouseQuote(in); wh != nil && wh.Currency == "RUB" {
		foreign = true
	}
	bounded := false
	for _, f := range feesFor(in, q) {
		if !f.Enabled {
			continue
		}
		if f.Currency == "RUB" {
			foreign = true
		}
		if f.Minimum != nil || f.Maximum != nil || f.Basis == "settlement" {
			bounded = true
		}
		if f.Basis == "buyer_price" && in.Product.PlatformDiscount.Kind == "fixed" && in.Product.PlatformDiscount.Currency == "RUB" {
			foreign = true
		}
	}
	return foreign && bounded && in.Settings.Stress > 0
}
func fxEvaluations(in Input, q Quote, price float64) []fxCash {
	values := map[float64]fxCash{}
	add := func(rate float64, label string) fxCash {
		if v, ok := values[rate]; ok {
			return v
		}
		c, e := costAt(in, q, price, rate, false)
		v := fxCash{rate: rate, label: label, value: c, err: e}
		values[rate] = v
		return v
	}
	for _, fc := range fxCases(in, q) {
		if rate, ok := caseRate(in, fc, price); ok {
			add(rate, fc.label)
		}
	}
	rates := make([]float64, 0, len(values))
	for rate := range values {
		rates = append(rates, rate)
	}
	sort.Float64s(rates)
	if dynamicFX(in, q) {
		var split func(float64, float64, int)
		split = func(lo, hi float64, depth int) {
			if depth > 24 || hi-lo < 1e-11 {
				return
			}
			a, b := values[lo], values[hi]
			if a.err != nil || b.err != nil {
				return
			}
			for i, k := range a.value.knots {
				if i >= len(b.value.knots) {
					break
				}
				end := b.value.knots[i]
				if k*end >= 0 {
					continue
				}
				u := 1/lo + (-k)/(end-k)*(1/hi-1/lo)
				root := 1 / u
				if root <= lo+1e-11 || root >= hi-1e-11 {
					continue
				}
				add(root, "费用计费临界值")
				split(lo, root, depth+1)
				split(root, hi, depth+1)
				return
			}
		}
		for i := 1; i < len(rates); i++ {
			split(rates[i-1], rates[i], 0)
		}
	}
	out := make([]fxCash, 0, len(values))
	for _, v := range values {
		out = append(out, v)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].rate < out[j].rate })
	return out
}
