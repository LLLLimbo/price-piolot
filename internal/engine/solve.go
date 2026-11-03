package engine

import (
	"fmt"
	"math"
	"sort"
)

func percentile(values []float64, p float64) float64 {
	if len(values) == 0 {
		return 0
	}
	v := append([]float64{}, values...)
	sort.Float64s(v)
	pos := float64(len(v)-1) * p
	lo := int(math.Floor(pos))
	hi := int(math.Ceil(pos))
	return v[lo] + (v[hi]-v[lo])*(pos-float64(lo))
}
func constraint(in Input, c cash) float64 {
	switch in.Product.Mode {
	case "profit":
		return num(c.profit.Sub(c.revenue.Mul(d(in.Product.Options.Margin))))
	case "delivery":
		return num(c.profit.Add(d(in.Product.Options.DeliveryLoss)))
	case "target":
		return num(c.profit.Add(d(in.Product.Options.TargetLoss)))
	default:
		return num(c.profit)
	}
}
func feasible(in Input, e Evaluation) bool {
	if !e.Valid {
		return false
	}
	switch in.Product.Mode {
	case "profit":
		return e.WorstMargin+1e-10 >= in.Product.Options.Margin
	case "breakeven":
		return e.WorstProfit >= -1e-9
	case "delivery":
		return e.WorstProfit+in.Product.Options.DeliveryLoss >= -1e-9
	case "target":
		return e.WorstProfit+in.Product.Options.TargetLoss >= -1e-9
	}
	return false
}
func critical(in Input, q Quote, cap, market float64) []int64 {
	if cap < .01 {
		return nil
	}
	points := map[int64]bool{1: true, int64(math.Floor(cap*100 + 1e-7)): true}
	factor := 1 - in.Product.Discount
	offset := in.Product.FixedDiscount
	add := func(p float64) {
		if !validNumber(p) {
			return
		}
		for _, base := range []int64{int64(math.Floor(p * 100)), int64(math.Ceil(p * 100))} {
			for delta := int64(-1); delta <= 1; delta++ {
				c := base + delta
				if c >= 1 && float64(c)/100 <= cap+1e-9 {
					points[c] = true
				}
			}
		}
	}
	rates := []float64{in.Settings.FX, in.Settings.FX * (1 - in.Settings.Stress), in.Settings.FX * (1 + in.Settings.Stress)}
	valuePoint := func(v float64, currency string) {
		if currency == "RUB" {
			for _, r := range rates {
				add((v/r + offset) / factor)
			}
		} else {
			add((v + offset) / factor)
		}
	}
	for _, f := range feesFor(in, q) {
		if f.Enabled {
			for _, t := range f.Tiers {
				if t.UpTo != nil {
					valuePoint(*t.UpTo, f.TierCurrency)
				}
			}
		}
	}
	for _, b := range q.Bands {
		if b.UpTo != nil {
			valuePoint(*b.UpTo, q.ValueCurrency)
		}
	}
	if wh, _ := warehouseQuote(in); wh != nil {
		for _, b := range wh.Bands {
			if b.UpTo != nil {
				valuePoint(*b.UpTo, wh.ValueCurrency)
			}
		}
	}
	for _, c := range effectivePeers(in) {
		add(listForBuyer(in, c.PriceRUB, in.Settings.FX))
		add(listForBuyer(in, c.PriceRUB-.005, in.Settings.FX))
		add(listForBuyer(in, c.PriceRUB+.005, in.Settings.FX))
	}
	for _, r := range rates {
		add(listForBuyer(in, 0, r))
	}
	add(offset / factor)
	add(market)
	out := make([]int64, 0, len(points))
	for p := range points {
		out = append(out, p)
	}
	sort.Slice(out, func(i, j int) bool { return out[i] < out[j] })
	return out
}

// Monetary constraints are piecewise affine between tariff/price boundaries.
// Clip each FX scenario separately: clipping the worst-of-scenarios endpoints
// would miss a feasible interior when scenario slopes have opposite signs.
func routeCandidates(in Input, q Quote, cap, market float64) []Evaluation {
	points := critical(in, q, cap, market)
	cases := fxCases(in, q)
	dynamic := dynamicFX(in, q)
	count := len(cases)
	if dynamic {
		count++
	}
	cache := map[int64][]float64{}
	knots := map[int64][][]float64{}
	prices := map[int64]bool{}
	values := func(cents int64) []float64 {
		if v, ok := cache[cents]; ok {
			return v
		}
		p := float64(cents) / 100
		v := make([]float64, count)
		boundaries := make([][]float64, count)
		for i, fc := range cases {
			rate, ok := caseRate(in, fc, p)
			if !ok {
				v[i] = math.NaN()
				continue
			}
			cash, err := costAt(in, q, p, rate, false)
			if err != nil {
				v[i] = math.Inf(-1)
			} else {
				v[i] = constraint(in, cash)
				boundaries[i] = cash.knots
			}
		}
		if dynamic {
			worst := math.Inf(1)
			for _, point := range fxEvaluations(in, q, p) {
				if point.err != nil {
					worst = math.Inf(-1)
					break
				}
				worst = math.Min(worst, constraint(in, point.value))
			}
			v[len(cases)] = worst
		}
		cache[cents] = v
		knots[cents] = boundaries
		return v
	}
	var interval func(int64, int64, int)
	interval = func(lo, hi int64, depth int) {
		if lo > hi {
			return
		}
		if hi-lo <= 3 {
			for p := lo; p <= hi; p++ {
				prices[p] = true
			}
			return
		}
		left, right := values(lo), values(hi)
		mid := lo + (hi-lo)/2
		middle := values(mid)
		// Partition at fee minimums, caps and payout-zero crossings before
		// interpolating the constraints. This also covers dependent payout fees.
		for i, a := range left {
			if !validNumber(a) || !validNumber(right[i]) {
				continue
			}
			ks, ke := knots[lo][i], knots[hi][i]
			if len(ks) != len(ke) {
				continue
			}
			for j, k := range ks {
				end := ke[j]
				if (k < 0 && end > 0) || (k > 0 && end < 0) {
					root := float64(lo) - k*float64(hi-lo)/(end-k)
					cut := int64(math.Floor(root))
					if cut < lo {
						cut = lo
					}
					if cut >= hi {
						cut = hi - 1
					}
					interval(lo, cut, depth+1)
					interval(cut+1, hi, depth+1)
					return
				}
			}
		}
		split := false
		ratio := float64(mid-lo) / float64(hi-lo)
		for i, a := range left {
			b, m := right[i], middle[i]
			if math.IsNaN(a) && math.IsNaN(b) && math.IsNaN(m) {
				continue
			}
			if !validNumber(a) || !validNumber(b) || !validNumber(m) {
				if math.IsInf(a, -1) && math.IsInf(b, -1) && math.IsInf(m, -1) {
					return
				}
				split = true
				break
			}
			expected := a + (b-a)*ratio
			if math.Abs(expected-m) > 1e-6 {
				split = true
				break
			}
		}
		if split {
			if depth < 30 {
				interval(lo, mid, depth+1)
				interval(mid+1, hi, depth+1)
			}
			return
		}
		low, high := float64(lo), float64(hi)
		for i, a := range left {
			b := right[i]
			if math.IsNaN(a) && math.IsNaN(b) {
				continue
			}
			slope := (b - a) / float64(hi-lo)
			if math.Abs(slope) < 1e-14 {
				if a < -1e-9 {
					return
				}
				continue
			}
			root := float64(lo) - a/slope
			if slope > 0 {
				low = math.Max(low, root)
			} else {
				high = math.Min(high, root)
			}
		}
		if low > high+1e-7 {
			return
		}
		for _, root := range []float64{low, high} {
			base := int64(math.Floor(root))
			for offset := int64(-2); offset <= 3; offset++ {
				p := base + offset
				if p >= lo && p <= hi {
					prices[p] = true
				}
			}
		}
	}
	for i, p := range points {
		prices[p] = true
		if i+1 < len(points) {
			interval(p+1, points[i+1]-1, 0)
		}
	}
	out := make([]Evaluation, 0, len(prices))
	for cents := range prices {
		e, err := Evaluate(in, q, float64(cents)/100)
		if err == nil && feasible(in, e) {
			out = append(out, e)
		}
	}
	return out
}
func timeUpper(e Evaluation) float64 {
	if e.Delivery == nil {
		return math.Inf(1)
	}
	return e.Delivery.Max
}
func better(in Input, a, b Evaluation, market float64) bool {
	const eps = 1e-8
	switch in.Product.Mode {
	case "profit":
		if a.Score.Total != nil && b.Score.Total == nil {
			return true
		}
		if a.Score.Total == nil && b.Score.Total != nil {
			return false
		}
		if a.Score.Total != nil && math.Abs(*a.Score.Total-*b.Score.Total) > eps {
			return *a.Score.Total > *b.Score.Total
		}
		if math.Abs(a.Price-b.Price) > eps {
			return a.Price < b.Price
		}
	case "breakeven":
		if math.Abs(a.Price-b.Price) > eps {
			return a.Price < b.Price
		}
	case "delivery":
		if timeUpper(a) != timeUpper(b) {
			return timeUpper(a) < timeUpper(b)
		}
		if math.Abs(math.Abs(a.Price-market)-math.Abs(b.Price-market)) > eps {
			return math.Abs(a.Price-market) < math.Abs(b.Price-market)
		}
	case "target":
		if math.Abs(a.Profit-b.Profit) > eps {
			return a.Profit > b.Profit
		}
		if math.Abs(a.Price-b.Price) > eps {
			return a.Price < b.Price
		}
	}
	if timeUpper(a) != timeUpper(b) {
		return timeUpper(a) < timeUpper(b)
	}
	if math.Abs(a.Price-b.Price) > eps {
		return a.Price < b.Price
	}
	return a.QuoteID < b.QuoteID
}
func Solve(in Input) (Result, error) {
	r := Result{Status: "infeasible", Mode: in.Product.Mode, Alternatives: []Evaluation{}, Unavailable: []Unavailable{}, Messages: []string{}, SnapshotID: in.Snapshot.ID, CalculatedAt: in.AsOf, Demo: in.Product.Demo || in.Snapshot.Demo || in.Settings.Demo}
	if err := Validate(in); err != nil {
		return r, err
	}
	if in.Settings.Demo && !in.Product.Demo {
		return r, fmt.Errorf("请先在费率设置中确认真实成本假设，演示费率不能用于真实商品")
	}
	r.TheoreticalMax = score(in, 1, nil).TheoreticalMax
	peerPrices := []float64{}
	for _, c := range effectivePeers(in) {
		peerPrices = append(peerPrices, listForBuyer(in, c.PriceRUB, in.Settings.FX))
	}
	market := percentile(peerPrices, .5)
	cap := 1000000.0
	if in.Product.Mode == "target" {
		cap = percentile(peerPrices, .9)
		if in.Product.Options.TargetMaxPrice != nil {
			cap = *in.Product.Options.TargetMaxPrice
		}
		if cap <= 0 {
			r.Messages = append(r.Messages, "缺少有效竞品，无法确定目标评分或自动售价上限")
			return r, nil
		}
		if score(in, 1, nil).Stale {
			r.Messages = append(r.Messages, "竞品快照已过期或规格不匹配，请重新检索")
			return r, nil
		}
	}
	r.PriceCap = cap
	for _, q := range in.Quotes {
		if q.Kind != "international" || len(in.Product.RouteIDs) > 0 && !has(in.Product.RouteIDs, q.ID) {
			continue
		}
		if err := quoteAvailable(in, q); err != nil {
			r.Unavailable = append(r.Unavailable, Unavailable{q.ID, q.Name, err.Error()})
			continue
		}
		wh, err := warehouseQuote(in)
		if err != nil {
			return r, err
		}
		if wh != nil {
			if err := quoteAvailable(in, *wh); err != nil {
				r.Unavailable = append(r.Unavailable, Unavailable{q.ID, q.Name, err.Error()})
				continue
			}
		}
		if in.Product.Mode == "delivery" && fullDelivery(in, q, wh) == nil {
			r.Unavailable = append(r.Unavailable, Unavailable{q.ID, q.Name, "完整妥投时效待补充"})
			continue
		}
		missing := ""
		includes := append([]string{}, q.Includes...)
		if wh != nil {
			includes = append(includes, wh.Includes...)
		}
		if err := routeFeesAvailable(in, q, includes); err != nil {
			missing = err.Error()
		}
		if in.Product.Returns.Rate > 0 && in.Product.Returns.Reverse == nil && q.ReturnCost == nil {
			missing = "退运费用待补充"
		}
		if missing != "" {
			r.Unavailable = append(r.Unavailable, Unavailable{q.ID, q.Name, missing})
			r.Status = "incomplete"
			continue
		}
		candidates := routeCandidates(in, q, cap, market)
		var best *Evaluation
		for _, ev := range candidates {
			if ev.Score.Total != nil && (r.MaxScore == nil || *ev.Score.Total > *r.MaxScore) {
				v := *ev.Score.Total
				r.MaxScore = &v
			}
			if in.Product.Mode == "target" && (ev.Score.Total == nil || *ev.Score.Total+1e-8 < in.Product.Options.TargetScore) {
				continue
			}
			if best == nil || better(in, ev, *best, market) {
				v := ev
				best = &v
			}
		}
		if best == nil {
			r.Unavailable = append(r.Unavailable, Unavailable{q.ID, q.Name, "在该报价适用范围内无法满足模式目标、评分完整性或亏损约束"})
		} else {
			r.Alternatives = append(r.Alternatives, *best)
		}
	}
	sort.SliceStable(r.Alternatives, func(i, j int) bool { return better(in, r.Alternatives[i], r.Alternatives[j], market) })
	if len(r.Alternatives) > 0 {
		v := r.Alternatives[0]
		r.Recommended = &v
		r.Status = "ok"
		if v.Score.Total == nil {
			r.Status = "partial"
		}
		for _, alternative := range r.Alternatives {
			if alternative.Score.Total == nil {
				r.Status = "partial"
				r.Messages = append(r.Messages, "部分可行方案的评分信息不完整，无法确认其市场排名")
				break
			}
		}
		if market == 0 && in.Product.Mode == "delivery" {
			r.Messages = append(r.Messages, "缺少有效竞品市场价，采用最快路线的成本可行价")
		}
	}
	if r.Recommended == nil {
		r.Messages = append(r.Messages, "当前条件下没有可行方案，请检查报价、费用及模式目标")
		if in.Product.Mode == "target" && in.Product.Options.TargetScore > r.TheoreticalMax {
			r.Messages = append(r.Messages, fmt.Sprintf("供应商评分固定时，理论总分上限为%.2f分", r.TheoreticalMax))
		}
	}
	return r, nil
}
