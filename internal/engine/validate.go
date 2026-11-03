package engine

import (
	"fmt"
	"math"
	"strings"
	"time"
)

func validNumber(v float64) bool { return !math.IsNaN(v) && !math.IsInf(v, 0) }
func has(xs []string, s string) bool {
	for _, x := range xs {
		if x == s {
			return true
		}
	}
	return false
}
func daysValid(d *Days) bool {
	return d == nil || (validNumber(d.Min) && validNumber(d.Max) && d.Min >= 0 && d.Max >= d.Min)
}
func allFees(in Input) []Fee {
	fees := append([]Fee{}, in.Product.Costs...)
	settingsFees := in.Settings.Fees
	if fs, ok := in.Settings.CategoryFees[in.Product.CommissionCategory]; ok {
		settingsFees = fs
	}
	return append(fees, settingsFees...)
}
func feeApplies(in Input, f Fee, route string) bool {
	return (len(f.SupplyModes) == 0 || has(f.SupplyModes, in.Product.Supply)) && (len(f.RouteIDs) == 0 || has(f.RouteIDs, route))
}
func feesFor(in Input, q Quote) []Fee {
	out := []Fee{}
	for _, f := range allFees(in) {
		if feeApplies(in, f, q.ID) {
			out = append(out, f)
		}
	}
	out = append(out, q.Fees...)
	if wh, _ := warehouseQuote(in); wh != nil {
		out = append(out, wh.Fees...)
	}
	return out
}

func routeFeesAvailable(in Input, q Quote, includes []string) error {
	seen := map[string]bool{}
	for _, f := range feesFor(in, q) {
		if !f.Enabled {
			continue
		}
		if seen[f.Code] {
			return fmt.Errorf("费用编码重复：%s，请在店铺费率或报价中保留一项", f.Name)
		}
		seen[f.Code] = true
		if !has(includes, f.Code) {
			if err := feeAvailable(f, in.AsOf); err != nil {
				return err
			}
		}
	}
	check := func(required map[string]string) error {
		for code, name := range required {
			if !seen[code] {
				return fmt.Errorf("%s待补充；不适用时须明确填0并确认依据", name)
			}
		}
		return nil
	}
	if err := check(q.RequiredFees); err != nil {
		return err
	}
	if wh, _ := warehouseQuote(in); wh != nil {
		if err := check(wh.RequiredFees); err != nil {
			return err
		}
	}
	return nil
}

func returnsAvailable(p Product) error {
	r := p.Returns
	if r.UnclassifiedRate > 0 {
		return fmt.Errorf("有待分类的发货后售后异常（原始订单口径%g%%）；请区分拒收／未签收与签收后退货并补全费用，再将待分类项归零", r.UnclassifiedRate*100)
	}
	if r.Rate == 0 && r.RefusalRate == 0 {
		return nil
	}
	if p.Demo && r.Destination == "" {
		return nil
	}
	if !has([]string{"partner", "disposal", "manual", "whd"}, r.Destination) || strings.TrimSpace(r.Source) == "" {
		return fmt.Errorf("售后去向及费用依据待确认，请核对店铺的WHD、合作伙伴或销毁设置")
	}
	if r.Destination == "whd" {
		return fmt.Errorf("WHD退货转售涉及逆向物流、仓储、二次销售及税费，当前跨境直发模型不能形成完整利润结论")
	}
	if r.Destination == "disposal" && ((r.Rate > 0 && r.Recovery != nil && *r.Recovery != 0) || (r.RefusalRate > 0 && r.RefusalRecovery != nil && *r.RefusalRecovery != 0)) {
		return fmt.Errorf("销毁方案的商品回收价值须为0")
	}
	return nil
}
func Validate(in Input) error {
	p, s := in.Product, in.Settings
	if strings.TrimSpace(p.Name) == "" {
		return fmt.Errorf("请输入商品名称")
	}
	if !has([]string{"profit", "breakeven", "delivery", "target"}, p.Mode) {
		return fmt.Errorf("请选择一种定价模式")
	}
	if !has([]string{"stock", "dropship"}, p.Supply) {
		return fmt.Errorf("请选择供货方式")
	}
	for _, v := range []float64{p.Weight, p.Length, p.Width, p.Height} {
		if !validNumber(v) || v <= 0 {
			return fmt.Errorf("包装后重量及长宽高必须大于0")
		}
	}
	if p.Weight > 1000000 {
		return fmt.Errorf("包装重量超出支持范围")
	}
	if p.PurchaseQuantity < 1 || p.MOQ < 1 || p.PurchaseQuantity < p.MOQ {
		return fmt.Errorf("采购数量须达到最低起订量")
	}
	if !validNumber(p.Discount) || p.Discount < 0 || p.Discount >= 1 {
		return fmt.Errorf("商家折扣须介于0和100%%之间")
	}
	if !validNumber(p.FixedDiscount) || p.FixedDiscount < 0 || p.Discount > 0 && p.FixedDiscount > 0 {
		return fmt.Errorf("商家优惠请选择百分比或固定金额")
	}
	if err := validateDiscount(p.PlatformDiscount, true); err != nil {
		return err
	}
	if !daysValid(p.SupplierDays) || !daysValid(p.DomesticDays) || !daysValid(p.TailDays) {
		return fmt.Errorf("时效区间无效")
	}
	if p.SupplierScore != nil && (!validNumber(*p.SupplierScore) || p.SupplierScale <= 0 || *p.SupplierScore < 0 || *p.SupplierScore > p.SupplierScale || strings.TrimSpace(p.SupplierSource) == "") {
		return fmt.Errorf("请填写供应商评分的有效量表和来源")
	}
	if !validNumber(s.FX) || s.FX <= 0 || !validNumber(s.Stress) || s.Stress < 0 || s.Stress >= .5 || s.FXSource == "" {
		return fmt.Errorf("请确认汇率、来源及0–50%%以内的压力幅度")
	}
	if _, err := time.Parse("2006-01-02", s.FXDate); err != nil {
		return fmt.Errorf("请填写汇率日期")
	}
	w := s.Weights
	if !validNumber(w.Price) || !validNumber(w.Delivery) || !validNumber(w.Supplier) || w.Price < 0 || w.Delivery < 0 || w.Supplier < 0 || math.Abs(w.Price+w.Delivery+w.Supplier-100) > 1e-8 {
		return fmt.Errorf("评分权重必须非负且合计100%%")
	}
	if s.SnapshotHours < 1 || s.SnapshotHours > 720 {
		return fmt.Errorf("竞品快照有效期须介于1和720小时之间")
	}
	if p.Options.Margin < 0 || p.Options.Margin >= 1 || !validNumber(p.Options.Margin) || p.Options.TargetScore < 0 || p.Options.TargetScore > 100 || !validNumber(p.Options.TargetScore) || p.Options.TargetLoss < 0 || p.Options.DeliveryLoss < 0 || !validNumber(p.Options.TargetLoss) || !validNumber(p.Options.DeliveryLoss) {
		return fmt.Errorf("模式目标或亏损额度无效")
	}
	if p.Options.TargetMaxPrice != nil && (!validNumber(*p.Options.TargetMaxPrice) || *p.Options.TargetMaxPrice < .01 || *p.Options.TargetMaxPrice > 1000000) {
		return fmt.Errorf("最高售价须介于0.01和1,000,000元之间")
	}
	r := p.Returns
	if err := returnsAvailable(p); err != nil {
		return err
	}
	if !validNumber(r.Rate) || !validNumber(r.CancelRate) || !validNumber(r.RefusalRate) || !validNumber(r.UnclassifiedRate) || r.UnclassifiedRate < 0 || r.RefusalRate < 0 || r.Rate < 0 || r.CancelRate < 0 || r.Rate+r.CancelRate+r.RefusalRate+r.UnclassifiedRate >= 1 {
		return fmt.Errorf("退货、拒收、取消及待分类比例须非负且合计小于100%%")
	}
	if r.RefusalRate > 0 {
		for _, v := range []*float64{r.RefusalReverse, r.RefusalDisposal, r.RefusalRecovery} {
			if v == nil || !validNumber(*v) || *v < 0 {
				return fmt.Errorf("拒收退运、处置和回收价值待补充；零费用请明确填0")
			}
		}
	}
	if r.Rate > 0 {
		for _, v := range []*float64{r.Disposal, r.Recovery} {
			if v == nil || !validNumber(*v) || *v < 0 {
				return fmt.Errorf("请补充退货处置费和可回收价值，零费用请明确填0")
			}
		}
	}
	if r.Reverse != nil && (!validNumber(*r.Reverse) || *r.Reverse < 0) {
		return fmt.Errorf("退运费无效")
	}
	if r.CancelRate > 0 && (r.CancelCost == nil || !validNumber(*r.CancelCost) || *r.CancelCost < 0) {
		return fmt.Errorf("请补充取消订单的不可回收成本")
	}
	seen := map[string]bool{}
	for _, f := range allFees(in) {
		if !f.Enabled {
			continue
		}
		if len(f.SupplyModes) > 0 && !has(f.SupplyModes, p.Supply) {
			continue
		}
		if len(f.RouteIDs) > 0 && len(p.RouteIDs) > 0 {
			matches := false
			for _, route := range p.RouteIDs {
				if has(f.RouteIDs, route) {
					matches = true
				}
			}
			if !matches {
				continue
			}
		}
		if seen[f.Code] {
			return fmt.Errorf("费用编码重复：%s", f.Name)
		}
		seen[f.Code] = true
		check := f
		if f.Code != "procurement" {
			if check.Amount == nil {
				check.Amount = Ptr(0)
			}
			if check.Rate == nil {
				check.Rate = Ptr(0)
			}
		}
		if err := ValidateFee(check); err != nil {
			return err
		}
	}
	if !seen["procurement"] {
		return fmt.Errorf("请添加采购成本，零采购成本请明确填0")
	}
	for _, t := range p.PurchaseTiers {
		if t.Quantity < 1 || t.Price < 0 || !validNumber(t.Price) {
			return fmt.Errorf("采购阶梯价格无效")
		}
	}
	for _, ref := range s.DeliveryReferences {
		if ref.Source == "" || ref.Region == "" || ref.Fulfillment == "" || !daysValid(&ref.Days) {
			return fmt.Errorf("参考时效需要有效区间、地区、履约类型和来源")
		}
	}
	return nil
}
func ValidateFee(f Fee) error {
	if f.Status != "" && !has([]string{"confirmed", "pending"}, f.Status) {
		return fmt.Errorf("%s确认状态无效", f.Name)
	}
	for _, v := range []string{f.ValidFrom, f.ValidTo} {
		if v != "" {
			if _, e := time.Parse("2006-01-02", v); e != nil {
				return fmt.Errorf("%s适用日期无效", f.Name)
			}
		}
	}
	if f.ValidFrom != "" && f.ValidTo != "" && f.ValidFrom > f.ValidTo {
		return fmt.Errorf("%s适用结束日期早于开始日期", f.Name)
	}
	for _, mode := range f.SupplyModes {
		if !has([]string{"stock", "dropship"}, mode) {
			return fmt.Errorf("%s供货适用条件无效", f.Name)
		}
	}
	if f.Code == "" || f.Name == "" || !has([]string{"CNY", "RUB"}, f.Currency) {
		return fmt.Errorf("费用名称、编码或币种无效")
	}
	if !has([]string{"fixed", "rate", "weight", "first_weight"}, f.Kind) {
		return fmt.Errorf("%s计费方式无效", f.Name)
	}
	if f.On != "" && !has([]string{"kept", "shipped", "delivered", "ordered"}, f.On) {
		return fmt.Errorf("%s发生条件无效", f.Name)
	}
	if f.Kind == "rate" && !has([]string{"sale", "retained_revenue", "international", "settlement", "buyer_price"}, f.Basis) {
		return fmt.Errorf("%s计费基数无效", f.Name)
	}
	if f.Code == "commission" && (f.Kind != "rate" || !has([]string{"sale", "retained_revenue"}, f.Basis) || f.On != "kept") {
		return fmt.Errorf("Ozon类目佣金须按商家折后成交价计算并在退货时冲回，请使用最终留存订单口径")
	}
	if f.Code == "commission" && len(f.Tiers) > 0 && f.TierCurrency != "RUB" {
		return fmt.Errorf("Ozon类目佣金分档须按下单日卢布货值判断，不可使用人民币档位")
	}
	if f.Kind == "rate" && (f.Rate == nil || !validNumber(*f.Rate) || *f.Rate < 0 || *f.Rate > 1) {
		return fmt.Errorf("%s费率待补充或无效", f.Name)
	}
	if f.Kind != "rate" && (f.Amount == nil || !validNumber(*f.Amount) || *f.Amount < 0) {
		return fmt.Errorf("%s金额待补充；零费用请明确填0", f.Name)
	}
	if f.Kind == "first_weight" && (f.FirstWeight <= 0 || f.AdditionalWeight <= 0 || f.AdditionalAmount < 0) {
		return fmt.Errorf("%s首续重规则无效", f.Name)
	}
	for _, v := range []*float64{f.Minimum, f.Maximum} {
		if v != nil && (!validNumber(*v) || *v < 0) {
			return fmt.Errorf("%s最低收费或封顶无效", f.Name)
		}
	}
	if f.Minimum != nil && f.Maximum != nil && *f.Minimum > *f.Maximum {
		return fmt.Errorf("%s最低收费高于封顶", f.Name)
	}
	if len(f.Tiers) > 0 && !has([]string{"CNY", "RUB"}, f.TierCurrency) {
		return fmt.Errorf("%s阶梯判断币种无效", f.Name)
	}
	prev := float64(-1)
	for i, t := range f.Tiers {
		if !validNumber(t.Rate) || t.Rate < 0 || t.Rate > 1 {
			return fmt.Errorf("%s阶梯费率无效", f.Name)
		}
		if t.UpTo == nil {
			if i != len(f.Tiers)-1 {
				return fmt.Errorf("%s无限上限必须放在最后一档", f.Name)
			}
		} else {
			if !validNumber(*t.UpTo) || *t.UpTo <= prev {
				return fmt.Errorf("%s阶梯上限须递增", f.Name)
			}
			prev = *t.UpTo
		}
	}
	if len(f.Tiers) > 0 && f.Tiers[len(f.Tiers)-1].UpTo != nil {
		return fmt.Errorf("%s需要覆盖末档费率", f.Name)
	}
	return nil
}
func ValidateQuote(q Quote) error {
	for code, name := range q.RequiredFees {
		if strings.TrimSpace(code) == "" || strings.TrimSpace(name) == "" {
			return fmt.Errorf("必需费用的编码和名称待补充")
		}
	}
	for _, f := range q.Fees {
		if len(f.RouteIDs) > 0 || len(f.SupplyModes) > 0 {
			return fmt.Errorf("报价附加费用继承该报价的适用范围，无需另设渠道或供货方式")
		}
		check := f
		if !f.Enabled || f.Status == "pending" {
			if check.Amount == nil {
				check.Amount = Ptr(0)
			}
			if check.Rate == nil {
				check.Rate = Ptr(0)
			}
		}
		if err := ValidateFee(check); err != nil {
			return err
		}
	}
	if q.ReturnCost != nil && (!validNumber(*q.ReturnCost) || *q.ReturnCost < 0 || !has([]string{"CNY", "RUB"}, q.ReturnCurrency)) {
		return fmt.Errorf("退货费用金额或币种无效")
	}
	if q.Name == "" || q.Provider == "" || q.Source == "" {
		return fmt.Errorf("报价需要名称、服务商及来源")
	}
	if !has([]string{"international", "warehouse"}, q.Kind) || !has([]string{"active", "pending", "historical", "excluded"}, q.Status) {
		return fmt.Errorf("报价类型或状态无效")
	}
	if !has([]string{"CNY", "RUB"}, q.Currency) || !has([]string{"CNY", "RUB"}, q.ValueCurrency) {
		return fmt.Errorf("报价币种无效")
	}
	if !daysValid(q.Delivery) {
		return fmt.Errorf("报价时效区间无效")
	}
	if q.Kind == "international" && !has([]string{"sorting", "buyer"}, q.TimeScope) {
		return fmt.Errorf("请指定物流时效截止环节")
	}
	if q.Status == "active" {
		start, e1 := time.Parse("2006-01-02", q.ValidFrom)
		end, e2 := time.Parse("2006-01-02", q.ValidTo)
		if e1 != nil || e2 != nil || end.Before(start) || !has(q.Platforms, "ozon") {
			return fmt.Errorf("启用报价需要适用Ozon和有效起止日期")
		}
	}
	if len(q.Bands) == 0 && q.Status == "active" {
		return fmt.Errorf("请至少填写一个报价档位")
	}
	for _, b := range q.Bands {
		for _, v := range []float64{b.MinValue, b.MinWeight, b.MaxWeight, b.Fixed, b.PerGram, b.VolumeDivisor, b.Step, b.MinimumWeight, b.MaxLength, b.MaxSum} {
			if !validNumber(v) || v < 0 {
				return fmt.Errorf("报价档位含无效数值")
			}
		}
		if (q.Status == "active" && b.MaxWeight <= 0) || (b.MaxWeight > 0 && b.MinWeight > b.MaxWeight) || b.Step <= 0 {
			return fmt.Errorf("请确认档位重量上限和计费进位")
		}
		if b.UpTo != nil && (!validNumber(*b.UpTo) || *b.UpTo <= 0 || b.MinValue > *b.UpTo) {
			return fmt.Errorf("货值上限无效")
		}
	}
	for _, code := range q.Includes {
		if !has([]string{"domestic_shipping", "packaging", "warehouse_operation", "tail_delivery"}, code) {
			return fmt.Errorf("不能将%s列为物流已含服务", code)
		}
	}
	return nil
}
func quoteAvailable(in Input, q Quote) error {
	if q.Status != "active" {
		return fmt.Errorf("%s报价未确认适用或已停用", q.Name)
	}
	if q.Demo && !in.Product.Demo {
		return fmt.Errorf("演示报价不能用于真实商品")
	}
	if !has(q.Platforms, "ozon") {
		return fmt.Errorf("报价不适用于Ozon订单")
	}
	today := in.AsOf.In(time.FixedZone("Asia/Shanghai", 8*60*60)).Format("2006-01-02")
	if q.ValidFrom == "" || q.ValidTo == "" || today < q.ValidFrom || today > q.ValidTo {
		return fmt.Errorf("报价尚未生效或已过期")
	}
	if in.Product.Batteries && !q.Batteries {
		return fmt.Errorf("该方案不承运带电商品")
	}
	if in.Product.Liquids && !q.Liquids {
		return fmt.Errorf("该方案不承运液体商品")
	}
	return nil
}
