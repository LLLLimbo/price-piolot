package engine

import (
	"fmt"
	"strings"
	"time"
)

// ProfitCheck records a manually supplied order example. Amounts are CNY totals
// for that order, independent of the product's prospective pricing assumptions.
type ProfitCheck struct {
	OrderRef       string   `json:"order_ref"`
	Date           string   `json:"date"`
	Source         string   `json:"source"`
	Carrier        string   `json:"carrier"`
	FreightKind    string   `json:"freight_kind"`
	FeeLabel       string   `json:"fee_label"`
	FeeKind        string   `json:"fee_kind"`
	Amount         *float64 `json:"amount"`
	Procurement    *float64 `json:"procurement"`
	Freight        *float64 `json:"freight"`
	CommissionRate *float64 `json:"commission_rate"`
	FixedFee       *float64 `json:"fixed_fee"`
	ReportedProfit *float64 `json:"reported_profit"`
}

type ProfitCheckResult struct {
	Status     string   `json:"status"`
	Profit     *float64 `json:"profit"`
	Margin     *float64 `json:"margin"`
	Difference *float64 `json:"difference"`
	Formula    string   `json:"formula"`
	Lines      []Line   `json:"lines"`
	Messages   []string `json:"messages"`
}

func ValidateProfitCheck(c ProfitCheck) error {
	for _, v := range []*float64{c.Amount, c.Procurement, c.Freight, c.FixedFee, c.CommissionRate} {
		if v != nil && (!validNumber(*v) || *v < 0 || *v > 10000000) {
			return fmt.Errorf("校验样本金额及佣金率须为有效非负数")
		}
	}
	if c.Amount != nil && *c.Amount <= 0 || c.CommissionRate != nil && *c.CommissionRate > 1 {
		return fmt.Errorf("校验样本订单金额须大于0，佣金率须介于0和100%%之间")
	}
	if c.ReportedProfit != nil && (!validNumber(*c.ReportedProfit) || *c.ReportedProfit < -10000000 || *c.ReportedProfit > 10000000) {
		return fmt.Errorf("校验样本参考利润无效")
	}
	if c.FreightKind != "" && !has([]string{"estimated", "settled"}, c.FreightKind) || c.FeeKind != "" && !has([]string{"prototype", "statement", "manual"}, c.FeeKind) {
		return fmt.Errorf("请明确校验样本的运费和费用方案来源")
	}
	if c.Date != "" {
		if _, err := time.Parse("2006-01-02", c.Date); err != nil {
			return fmt.Errorf("校验样本日期无效")
		}
	}
	return nil
}

func CheckProfit(c ProfitCheck) (ProfitCheckResult, error) {
	r := ProfitCheckResult{Status: "incomplete", Lines: []Line{}, Messages: []string{}}
	if err := ValidateProfitCheck(c); err != nil {
		return r, err
	}
	for _, field := range []struct {
		name  string
		value *float64
	}{
		{"订单金额", c.Amount}, {"采购总成本", c.Procurement}, {"运费", c.Freight}, {"佣金率", c.CommissionRate}, {"固定费用", c.FixedFee},
	} {
		if field.value == nil {
			r.Messages = append(r.Messages, field.name+"待补充；不按0计算")
		}
	}
	for _, field := range []struct{ name, value string }{
		{"订单日期", c.Date}, {"样本来源", c.Source}, {"运费口径", c.FreightKind}, {"费用方案名称", c.FeeLabel}, {"费用方案口径", c.FeeKind},
	} {
		if strings.TrimSpace(field.value) == "" {
			r.Messages = append(r.Messages, field.name+"待补充")
		}
	}
	if len(r.Messages) > 0 {
		return r, nil
	}
	day, _ := time.Parse("2006-01-02", c.Date)
	source := c.Source + "；" + c.FeeLabel
	// The private flat shipment represents this order's quoted charge only. It is
	// never saved as a reusable route and has no inferred weight or transit time.
	q := Quote{ID: "sample-shipment", Name: c.Carrier, Kind: "international", Status: "active", Currency: "CNY", ValueCurrency: "CNY", ValidFrom: c.Date, ValidTo: c.Date, Source: c.Source, Platforms: []string{"ozon"}, TimeScope: "buyer", Bands: []Band{{MaxWeight: 1000000, Step: 1, Fixed: *c.Freight}}}
	in := Input{AsOf: day, Settings: Settings{FX: 1}, Product: Product{WarehouseQuoteID: "self", Costs: []Fee{
		{Code: "procurement", Name: "订单采购成本", Enabled: true, Kind: "fixed", Amount: c.Procurement, Currency: "CNY", On: "ordered", Status: "confirmed", Source: c.Source},
		{Code: "commission", Name: "样本佣金", Enabled: true, Kind: "rate", Rate: c.CommissionRate, Currency: "CNY", Basis: "sale", On: "kept", Status: "confirmed", Source: source},
		{Code: "sample_fixed", Name: "样本固定费用", Enabled: true, Kind: "fixed", Amount: c.FixedFee, Currency: "CNY", On: "ordered", Status: "confirmed", Source: source},
	}}}
	cash, err := costAt(in, q, *c.Amount, 1, true)
	if err != nil {
		return r, err
	}
	r.Profit = Ptr(num(cash.profit))
	r.Margin = Ptr(num(cash.profit.Div(d(*c.Amount))))
	r.Lines = cash.lines
	for i := range r.Lines {
		if r.Lines[i].Code == "international" {
			r.Lines[i].Name = "样本运费"
		}
	}
	r.Formula = fmt.Sprintf("%.2f − %.2f − %.2f − %.2f × %g%% − %.2f = %.2f CNY", *c.Amount, *c.Procurement, *c.Freight, *c.Amount, *c.CommissionRate*100, *c.FixedFee, *r.Profit)
	r.Messages = append(r.Messages, "仅校验所列采购、运费、佣金和固定费用。未列费用及商品整体售后仍待核实；不构成完整净利润或定价推荐。")
	if c.FreightKind == "estimated" {
		r.Messages = append(r.Messages, "运费为来源中的预计值，尚未核对实际物流结算。")
	}
	if c.FeeKind == "prototype" {
		r.Messages = append(r.Messages, "费用采用原型方案，尚未核对Ozon实际佣金和扣费账单。")
	}
	if c.ReportedProfit == nil {
		r.Status = "reference_missing"
		r.Messages = append(r.Messages, "参考利润待补充，已计算样本口径利润。")
		return r, nil
	}
	difference := cash.profit.Round(2).Sub(d(*c.ReportedProfit).Round(2))
	r.Difference = Ptr(num(difference))
	r.Status = "different"
	if difference.IsZero() {
		r.Status = "matched"
	}
	return r, nil
}
