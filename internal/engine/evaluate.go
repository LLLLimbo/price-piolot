package engine

import (
	"fmt"
	"math"
	"sort"

	"github.com/shopspring/decimal"
)

func d(v float64) decimal.Decimal   { return decimal.NewFromFloat(v) }
func num(v decimal.Decimal) float64 { return v.InexactFloat64() }
func convert(v decimal.Decimal, currency string, fx float64) decimal.Decimal {
	if currency == "RUB" {
		return v.Div(d(fx))
	}
	return v
}
func currencyValue(cny decimal.Decimal, currency string, fx float64) decimal.Decimal {
	if currency == "RUB" {
		return cny.Mul(d(fx))
	}
	return cny
}
func sale(in Input, price float64) decimal.Decimal {
	return d(price).Mul(decimal.NewFromInt(1).Sub(d(in.Product.Discount))).Sub(d(in.Product.FixedDiscount))
}

func quoteCost(in Input, q Quote, price, fx float64) (decimal.Decimal, string, error) {
	if err := quoteAvailable(in, q); err != nil {
		return decimal.Zero, "", err
	}
	p := in.Product
	value := currencyValue(sale(in, price), q.ValueCurrency, fx)
	for _, b := range q.Bands {
		if value.LessThan(d(b.MinValue)) {
			continue
		}
		if b.UpTo != nil && value.GreaterThan(d(*b.UpTo)) {
			continue
		}
		if p.Weight < b.MinWeight || p.Weight > b.MaxWeight {
			continue
		}
		if b.MaxLength > 0 && math.Max(p.Length, math.Max(p.Width, p.Height)) > b.MaxLength {
			continue
		}
		if b.MaxSum > 0 && p.Length+p.Width+p.Height > b.MaxSum {
			continue
		}
		weight := d(p.Weight)
		if b.VolumeDivisor > 0 {
			vw := d(p.Length).Mul(d(p.Width)).Mul(d(p.Height)).Div(d(b.VolumeDivisor))
			if vw.GreaterThan(weight) {
				weight = vw
			}
		}
		if d(b.MinimumWeight).GreaterThan(weight) {
			weight = d(b.MinimumWeight)
		}
		step := b.Step
		if step <= 0 {
			step = 1
		}
		weight = weight.Div(d(step)).Ceil().Mul(d(step))
		amount := d(b.Fixed).Add(weight.Mul(d(b.PerGram)))
		return convert(amount, q.Currency, fx), fmt.Sprintf("%g + %s g × %g %s/g", b.Fixed, weight.String(), b.PerGram, q.Currency), nil
	}
	return decimal.Zero, "", fmt.Errorf("%s无适用档位（货值、重量或尺寸超限）", q.Name)
}
func warehouseQuote(in Input) (*Quote, error) {
	id := in.Product.WarehouseQuoteID
	if id == "" || id == "self" {
		return nil, nil
	}
	for _, q := range in.Quotes {
		if q.ID == id && q.Kind == "warehouse" {
			return &q, nil
		}
	}
	return nil, fmt.Errorf("仓配报价不存在")
}
func fullDelivery(in Input, q Quote, warehouse *Quote) *Days {
	ds := []*Days{in.Product.SupplierDays, in.Product.DomesticDays, q.Delivery}
	if warehouse != nil {
		ds = append(ds, warehouse.Delivery)
	}
	if q.TimeScope == "sorting" {
		ds = append(ds, in.Product.TailDays)
	}
	out := Days{}
	for _, v := range ds {
		if v == nil || !daysValid(v) {
			return nil
		}
		out.Min += v.Min
		out.Max += v.Max
	}
	return &out
}
func feeRaw(in Input, f Fee, price, fx float64, international, balance decimal.Decimal) (decimal.Decimal, string) {
	p := in.Product
	amount := decimal.Zero
	formula := ""
	if f.Kind == "rate" {
		rate := *f.Rate
		v := currencyValue(sale(in, price), f.TierCurrency, fx)
		for _, tier := range f.Tiers {
			if tier.UpTo == nil || v.LessThanOrEqual(d(*tier.UpTo)) {
				rate = tier.Rate
				break
			}
		}
		base := sale(in, price)
		switch f.Basis {
		case "buyer_price":
			base = buyerPrice(in, price, fx)
		case "international":
			base = international
		case "settlement":
			base = balance
		}
		amount = currencyValue(base, f.Currency, fx).Mul(d(rate))
		formula = fmt.Sprintf("%s %s × %g%%", currencyValue(base, f.Currency, fx).StringFixed(4), f.Currency, rate*100)
	} else {
		a := *f.Amount
		if f.Code == "procurement" {
			best := 0
			for _, tier := range p.PurchaseTiers {
				if p.PurchaseQuantity >= tier.Quantity && tier.Quantity >= best {
					a = tier.Price
					best = tier.Quantity
				}
			}
		}
		amount = d(a)
		formula = fmt.Sprintf("%g %s", a, f.Currency)
		if f.Kind == "weight" {
			amount = amount.Mul(d(p.Weight))
			formula = fmt.Sprintf("%g g × %g %s/g", p.Weight, a, f.Currency)
		}
		if f.Kind == "first_weight" {
			extra := math.Max(0, p.Weight-f.FirstWeight)
			n := d(extra).Div(d(f.AdditionalWeight)).Ceil()
			amount = amount.Add(n.Mul(d(f.AdditionalAmount)))
			formula = fmt.Sprintf("%g + %s × %g %s", a, n.String(), f.AdditionalAmount, f.Currency)
		}
	}
	return amount, formula
}

type cash struct {
	profit, revenue, cost decimal.Decimal
	lines                 []Line
	knots                 []float64
}

func costAt(in Input, q Quote, price, fx float64, withLines bool) (cash, error) {
	c := cash{}
	p := in.Product
	if err := returnsAvailable(p); err != nil {
		return c, err
	}
	if !sale(in, price).IsPositive() || !buyerPrice(in, price, fx).IsPositive() {
		return c, fmt.Errorf("优惠后商品价格须大于0")
	}
	keep := d(1).Sub(d(p.Returns.Rate)).Sub(d(p.Returns.CancelRate)).Sub(d(p.Returns.RefusalRate))
	shipped := d(1).Sub(d(p.Returns.CancelRate))
	c.revenue = sale(in, price).Mul(keep)
	intl, formula, err := quoteCost(in, q, price, fx)
	if err != nil {
		return c, err
	}
	wh, err := warehouseQuote(in)
	if err != nil {
		return c, err
	}
	includes := append([]string{}, q.Includes...)
	includedSources := map[string]string{}
	includedVersions := map[string]Quote{}
	for _, code := range q.Includes {
		includedSources[code] = q.Source
		includedVersions[code] = q
	}
	add := func(code, name string, raw decimal.Decimal, currency, formula, source string, factor decimal.Decimal, included bool) {
		expected := convert(raw, currency, fx).Mul(factor)
		if included {
			expected = decimal.Zero
		}
		c.cost = c.cost.Add(expected)
		if withLines {
			if !included && (currency == "RUB" || !factor.Equal(decimal.NewFromInt(1))) {
				formula = "(" + formula + ")"
			}
			if !included && currency == "RUB" {
				formula += fmt.Sprintf(" ÷ %g RUB/CNY", fx)
			}
			if !included && !factor.Equal(decimal.NewFromInt(1)) {
				formula += fmt.Sprintf(" × 发生比例 %s%%", factor.Mul(decimal.NewFromInt(100)).String())
			}
			c.lines = append(c.lines, Line{Code: code, Name: name, Amount: num(raw), Currency: currency, CNY: num(expected), Formula: formula, Source: source, Included: included})
		}
	}
	add("international", "国际物流", currencyValue(intl, q.Currency, fx), q.Currency, formula, q.Source, shipped, false)
	if withLines {
		c.lines[len(c.lines)-1].Version = q.Version
		c.lines[len(c.lines)-1].ValidFrom = q.ValidFrom
		c.lines[len(c.lines)-1].ValidTo = q.ValidTo
		c.lines[len(c.lines)-1].Confirmation = q.Status
	}
	if wh != nil {
		amount, f, e := quoteCost(in, *wh, price, fx)
		if e != nil {
			return c, e
		}
		add("warehouse_quote", "仓配服务", currencyValue(amount, wh.Currency, fx), wh.Currency, f, wh.Source, shipped, false)
		if withLines {
			c.lines[len(c.lines)-1].Version = wh.Version
			c.lines[len(c.lines)-1].ValidFrom = wh.ValidFrom
			c.lines[len(c.lines)-1].ValidTo = wh.ValidTo
			c.lines[len(c.lines)-1].Confirmation = wh.Status
		}
		includes = append(includes, wh.Includes...)
		for _, code := range wh.Includes {
			includedSources[code] = wh.Source
			includedVersions[code] = *wh
		}
	}
	fees := feesFor(in, q)
	if err := routeFeesAvailable(in, q, includes); err != nil {
		return c, err
	}
	balance := sale(in, price).Sub(intl)
	c.knots = append(c.knots, num(balance))
	if balance.IsNegative() {
		balance = decimal.Zero
	}
	process := func(f Fee) error {
		if !f.Enabled {
			return nil
		}
		if has(includes, f.Code) {
			add(f.Code, f.Name, decimal.Zero, f.Currency, "已包含在所选报价中", includedSources[f.Code], decimal.NewFromInt(1), true)
			if withLines {
				line := &c.lines[len(c.lines)-1]
				origin := includedVersions[f.Code]
				line.Version = origin.Version
				line.ValidFrom = origin.ValidFrom
				line.ValidTo = origin.ValidTo
				line.Confirmation = origin.Status
			}
			return nil
		}
		if err := feeAvailable(f, in.AsOf); err != nil {
			return err
		}
		raw, expr := feeRaw(in, f, price, fx, intl, balance)
		if f.Minimum != nil {
			c.knots = append(c.knots, num(convert(raw.Sub(d(*f.Minimum)), f.Currency, fx)))
		}
		if f.Maximum != nil {
			c.knots = append(c.knots, num(convert(raw.Sub(d(*f.Maximum)), f.Currency, fx)))
		}
		if f.Minimum != nil && raw.LessThan(d(*f.Minimum)) {
			raw = d(*f.Minimum)
			expr = fmt.Sprintf("max(%s, %g %s)", expr, *f.Minimum, f.Currency)
		}
		if f.Maximum != nil && raw.GreaterThan(d(*f.Maximum)) {
			raw = d(*f.Maximum)
			expr = fmt.Sprintf("min(%s, %g %s)", expr, *f.Maximum, f.Currency)
		}
		included := has(includes, f.Code)
		factor := shipped
		if f.On == "ordered" {
			factor = decimal.NewFromInt(1)
		}
		if f.On == "delivered" {
			factor = shipped.Sub(d(p.Returns.RefusalRate))
		}
		if f.On == "kept" || f.Basis == "retained_revenue" {
			factor = keep
		}
		add(f.Code, f.Name, raw, f.Currency, expr, f.Source, factor, included)
		if withLines {
			line := &c.lines[len(c.lines)-1]
			line.Version = f.Version
			line.ValidFrom = f.ValidFrom
			line.ValidTo = f.ValidTo
			line.Confirmation = f.Status
		}
		if f.DeductSettlement && !included {
			balance = balance.Sub(convert(raw, f.Currency, fx))
			c.knots = append(c.knots, num(balance))
			if balance.IsNegative() {
				balance = decimal.Zero
			}
		}
		return nil
	}
	for _, f := range fees {
		if f.Basis != "settlement" {
			if e := process(f); e != nil {
				return c, e
			}
		}
	}
	for _, f := range fees {
		if f.Basis == "settlement" {
			if e := process(f); e != nil {
				return c, e
			}
		}
	}
	if p.Returns.Rate > 0 {
		reverse := p.Returns.Reverse
		cur := "CNY"
		if reverse == nil {
			reverse = q.ReturnCost
			cur = q.ReturnCurrency
		}
		if reverse == nil {
			return c, fmt.Errorf("退运费用待补充")
		}
		add("return_shipping", "预计退运费用", d(*reverse), cur, fmt.Sprintf("%g %s", *reverse, cur), "售后假设", d(p.Returns.Rate), false)
		add("disposal", "预计处置费用", d(*p.Returns.Disposal), "CNY", fmt.Sprintf("%g CNY", *p.Returns.Disposal), "售后假设", d(p.Returns.Rate), false)
		add("recovery", "退货回收价值抵减", d(*p.Returns.Recovery).Neg(), "CNY", fmt.Sprintf("−%g CNY", *p.Returns.Recovery), "售后假设", d(p.Returns.Rate), false)
	}
	if p.Returns.RefusalRate > 0 {
		for _, v := range []*float64{p.Returns.RefusalReverse, p.Returns.RefusalDisposal, p.Returns.RefusalRecovery} {
			if v == nil {
				return c, fmt.Errorf("拒收退运、处置或回收价值待补充")
			}
		}
		add("refusal_shipping", "拒收预计退运", d(*p.Returns.RefusalReverse), "CNY", fmt.Sprintf("%g CNY", *p.Returns.RefusalReverse), "拒收假设", d(p.Returns.RefusalRate), false)
		add("refusal_disposal", "拒收预计处置", d(*p.Returns.RefusalDisposal), "CNY", fmt.Sprintf("%g CNY", *p.Returns.RefusalDisposal), "拒收假设", d(p.Returns.RefusalRate), false)
		add("refusal_recovery", "拒收回收价值抵减", d(*p.Returns.RefusalRecovery).Neg(), "CNY", fmt.Sprintf("−%g CNY", *p.Returns.RefusalRecovery), "拒收假设", d(p.Returns.RefusalRate), false)
	}
	if p.Returns.CancelRate > 0 {
		add("cancellation", "取消订单不可回收成本", d(*p.Returns.CancelCost), "CNY", fmt.Sprintf("%g CNY", *p.Returns.CancelCost), "售后假设", d(p.Returns.CancelRate), false)
	}
	c.profit = c.revenue.Sub(c.cost)
	return c, nil
}

type fxCase struct {
	rate     float64
	boundary float64
	label    string
}

func boundaries(in Input, q Quote) []float64 {
	out := []float64{}
	for _, f := range feesFor(in, q) {
		if f.Enabled && f.TierCurrency == "RUB" {
			for _, t := range f.Tiers {
				if t.UpTo != nil {
					out = append(out, *t.UpTo)
				}
			}
		}
	}
	if q.ValueCurrency == "RUB" {
		for _, b := range q.Bands {
			if b.MinValue > 0 {
				out = append(out, b.MinValue)
			}
			if b.UpTo != nil {
				out = append(out, *b.UpTo)
			}
		}
	}
	if wh, _ := warehouseQuote(in); wh != nil && wh.ValueCurrency == "RUB" {
		for _, b := range wh.Bands {
			if b.MinValue > 0 {
				out = append(out, b.MinValue)
			}
			if b.UpTo != nil {
				out = append(out, *b.UpTo)
			}
		}
	}
	return out
}
func fxCases(in Input, q Quote) []fxCase {
	s := in.Settings
	out := []fxCase{{rate: s.FX, label: "当前汇率"}}
	if s.Stress == 0 {
		return out
	}
	out = append(out, fxCase{rate: s.FX * (1 - s.Stress), label: "汇率下界"}, fxCase{rate: s.FX * (1 + s.Stress), label: "汇率上界"})
	for _, b := range boundaries(in, q) {
		out = append(out, fxCase{boundary: b - 1e-7, label: "档位临界下侧"}, fxCase{boundary: b, label: "档位临界值"}, fxCase{boundary: b + 1e-7, label: "档位临界上侧"})
	}
	return out
}
func caseRate(in Input, c fxCase, price float64) (float64, bool) {
	rate := c.rate
	if c.boundary > 0 {
		rate = c.boundary / num(sale(in, price))
	}
	lo, hi := in.Settings.FX*(1-in.Settings.Stress), in.Settings.FX*(1+in.Settings.Stress)
	return rate, rate >= lo-1e-10 && rate <= hi+1e-10
}
func Evaluate(in Input, q Quote, price float64) (Evaluation, error) {
	ev := Evaluation{QuoteID: q.ID, Route: q.Name, Warehouse: "自行处理", Price: price, Valid: true, Warnings: []string{}, Lines: []Line{}, Scenarios: []Scenario{}}
	if !validNumber(price) || price <= 0 {
		return ev, fmt.Errorf("售价必须大于0")
	}
	wh, err := warehouseQuote(in)
	if err != nil {
		return ev, err
	}
	if wh != nil {
		ev.Warehouse = wh.Name
	}
	ev.Delivery = fullDelivery(in, q, wh)
	cur, err := costAt(in, q, price, in.Settings.FX, true)
	if err != nil {
		return ev, err
	}
	ev.Lines = cur.lines
	ev.Revenue = num(cur.revenue)
	ev.Cost = num(cur.cost)
	ev.Profit = num(cur.profit)
	ev.Margin = num(cur.profit.Div(cur.revenue))
	ev.SalePrice = num(sale(in, price))
	ev.PriceRUB = num(buyerPrice(in, price, in.Settings.FX).Mul(d(in.Settings.FX)).Round(2))
	ev.Prices = PriceBreakdown{ListCNY: price, SellerCNY: ev.SalePrice, BuyerRUB: ev.PriceRUB, PlatformDiscountCNY: num(sale(in, price).Sub(buyerPrice(in, price, in.Settings.FX))), RetainedRevenueCNY: ev.Revenue, ComparisonBasis: "商品展示价，未含买家运费", Formula: fmt.Sprintf("标价 %.4f × (1 − 商家折扣 %.4f%%) − 商家固定优惠 %.4f = 商家成交价 %.4f CNY；(商家成交价 %.4f − 平台等额补偿优惠 %.4f CNY) × 汇率 %g = 买家参考价 %.2f RUB；商家成交价 %.4f × 留存比例 %.4f%% = 留存销售收入 %.4f CNY", price, in.Product.Discount*100, in.Product.FixedDiscount, ev.SalePrice, ev.SalePrice, num(sale(in, price).Sub(buyerPrice(in, price, in.Settings.FX))), in.Settings.FX, ev.PriceRUB, ev.SalePrice, (1-in.Product.Returns.Rate-in.Product.Returns.CancelRate-in.Product.Returns.RefusalRate)*100, ev.Revenue)}
	ev.WorstProfit = ev.Profit
	ev.WorstMargin = ev.Margin
	for _, fc := range fxEvaluations(in, q, price) {
		c, e := fc.value, fc.err
		s := Scenario{FX: fc.rate, Label: fc.label, Valid: e == nil}
		if e != nil {
			ev.Valid = false
			s.Reason = e.Error()
			ev.Reason = s.Reason
		} else {
			s.Profit = num(c.profit)
			s.Margin = num(c.profit.Div(c.revenue))
			s.Revenue = num(c.revenue)
			ev.WorstProfit = math.Min(ev.WorstProfit, s.Profit)
			ev.WorstMargin = math.Min(ev.WorstMargin, s.Margin)
		}
		ev.Scenarios = append(ev.Scenarios, s)
	}
	sort.SliceStable(ev.Scenarios, func(i, j int) bool { return ev.Scenarios[i].FX < ev.Scenarios[j].FX })
	ev.Score = score(in, ev.PriceRUB, ev.Delivery)
	ev.Score.Estimated = ev.Score.Estimated || q.Estimated || (wh != nil && wh.Estimated)
	ev.Formula = fmt.Sprintf("留存收入 %.4f − 全部预期成本 %.4f = 单件预计净利润 %.4f CNY；净利率 = %.4f ÷ %.4f × 100%% = %.4f%%", ev.Revenue, ev.Cost, ev.Profit, ev.Profit, ev.Revenue, ev.Margin*100)
	if ev.Score.Stale {
		ev.Warnings = append(ev.Warnings, "竞品快照缺失、过期或商品规格已改变，评分暂缺")
	}
	if ev.Score.Total == nil {
		ev.Warnings = append(ev.Warnings, "评分信息不完整，显示可达区间")
	}
	if ev.Score.Estimated {
		ev.Warnings = append(ev.Warnings, "时效含估算区间")
	}
	for _, p := range effectivePeers(in) {
		if p.PriceCondition == "" {
			ev.Warnings = append(ev.Warnings, "部分竞品优惠条件未知，当前仅比较商品展示价")
			break
		}
	}
	if in.Snapshot.Demo || in.Product.Demo || q.Demo {
		ev.Warnings = append(ev.Warnings, "演示数据，费用与时效均为示例假设")
	}
	if in.Settings.FXMeta != nil && in.Settings.FXMeta.Demo {
		ev.Warnings = append(ev.Warnings, "使用Mock演示汇率，不代表实际合同汇率")
	}
	return ev, nil
}
