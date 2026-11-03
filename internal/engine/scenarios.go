package engine

import (
	"fmt"
	"math"
	"strings"
	"time"
)

type StoreProfile struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	SellerIDs []string  `json:"seller_ids"`
	Archived  bool      `json:"archived"`
	Demo      bool      `json:"demo"`
	CreatedAt time.Time `json:"created_at"`
}

type Discount struct {
	Kind      string   `json:"kind"`
	Value     *float64 `json:"value"`
	Currency  string   `json:"currency"`
	Confirmed bool     `json:"confirmed"`
	Source    string   `json:"source"`
}
type PriceBreakdown struct {
	ListCNY             float64 `json:"list_cny"`
	SellerCNY           float64 `json:"seller_cny"`
	BuyerRUB            float64 `json:"buyer_rub"`
	PlatformDiscountCNY float64 `json:"platform_discount_cny"`
	RetainedRevenueCNY  float64 `json:"retained_revenue_cny"`
	ComparisonBasis     string  `json:"comparison_basis"`
	Formula             string  `json:"formula"`
}
type PromotionScenario struct {
	ID               string   `json:"id"`
	StoreID          string   `json:"store_id"`
	Version          int      `json:"version"`
	Name             string   `json:"name"`
	Source           string   `json:"source"`
	StartsAt         string   `json:"starts_at"`
	EndsAt           string   `json:"ends_at"`
	MerchantDiscount Discount `json:"merchant_discount"`
	PlatformDiscount Discount `json:"platform_discount"`
	PriceCondition   string   `json:"price_condition"`
	Demo             bool     `json:"demo"`
}
type ContextResult struct {
	ID        string             `json:"id"`
	Name      string             `json:"name"`
	State     string             `json:"state"`
	Promotion *PromotionScenario `json:"promotion,omitempty"`
	Input     Input              `json:"input"`
	Result    Result             `json:"result"`
}
type SalesScenario struct {
	ID          string   `json:"id"`
	StoreID     string   `json:"store_id"`
	ProductID   string   `json:"product_id"`
	PromotionID string   `json:"promotion_id"`
	Name        string   `json:"name"`
	Price       float64  `json:"price"`
	QuoteID     string   `json:"quote_id"`
	DailyUnits  *float64 `json:"daily_units"`
	Note        string   `json:"note"`
}
type SalesEvaluation struct {
	Scenario         SalesScenario `json:"scenario"`
	Evaluation       *Evaluation   `json:"evaluation"`
	DailyProfit      *float64      `json:"daily_profit"`
	WorstDailyProfit *float64      `json:"worst_daily_profit"`
	MeetsConstraints bool          `json:"meets_constraints"`
	Status           string        `json:"status"`
	Messages         []string      `json:"messages"`
}

func validateDiscount(v Discount, platform bool) error {
	if v.Kind == "" || v.Kind == "none" {
		return nil
	}
	if !has([]string{"percent", "fixed"}, v.Kind) || v.Value == nil || !validNumber(*v.Value) || *v.Value < 0 {
		return fmt.Errorf("优惠金额或比例待补充")
	}
	if v.Kind == "percent" && *v.Value >= 1 {
		return fmt.Errorf("优惠比例须小于100%%")
	}
	if v.Kind == "fixed" && !has([]string{"CNY", "RUB"}, v.Currency) {
		return fmt.Errorf("优惠币种须为人民币或卢布")
	}
	if !platform && v.Kind == "fixed" && v.Currency != "CNY" {
		return fmt.Errorf("商家固定优惠须为人民币")
	}
	if platform && (!v.Confirmed || strings.TrimSpace(v.Source) == "") {
		return fmt.Errorf("平台优惠承担方式待确认：请确认不减少商家收入并填写来源")
	}
	return nil
}
func ValidatePromotion(p PromotionScenario) error {
	if strings.TrimSpace(p.Name) == "" || strings.TrimSpace(p.Source) == "" {
		return fmt.Errorf("活动需要名称和来源")
	}
	for _, value := range []string{p.StartsAt, p.EndsAt} {
		if value != "" {
			if _, e := time.Parse("2006-01-02", value); e != nil {
				return fmt.Errorf("活动日期无效")
			}
		}
	}
	if p.StartsAt != "" && p.EndsAt != "" && p.StartsAt > p.EndsAt {
		return fmt.Errorf("活动结束日期早于开始日期")
	}
	// An unconfirmed platform discount is saved as a draft, but cannot produce a full result.
	if err := validateDiscount(p.MerchantDiscount, false); err != nil {
		return err
	}
	if p.PlatformDiscount.Kind != "" && p.PlatformDiscount.Kind != "none" {
		draft := p.PlatformDiscount
		draft.Confirmed = true
		draft.Source = "草稿结构校验"
		if err := validateDiscount(draft, true); err != nil {
			return err
		}
	}
	return nil
}
func ApplyPromotion(in Input, p PromotionScenario) (Input, error) {
	if err := ValidatePromotion(p); err != nil {
		return in, err
	}
	in.Product.Discount = 0
	in.Product.FixedDiscount = 0
	switch p.MerchantDiscount.Kind {
	case "percent":
		in.Product.Discount = *p.MerchantDiscount.Value
	case "fixed":
		in.Product.FixedDiscount = *p.MerchantDiscount.Value
	}
	in.Product.PlatformDiscount = p.PlatformDiscount
	in.Product.PriceCondition = p.PriceCondition
	return in, nil
}
func CalculateContexts(in Input, promos []PromotionScenario) []ContextResult {
	out := make([]ContextResult, 0, len(promos)+1)
	run := func(id, name, state string, input Input, p *PromotionScenario, err error) {
		var result Result
		if err == nil {
			result, err = Solve(input)
		}
		if err != nil {
			result = Result{Status: "invalid", Mode: input.Product.Mode, SnapshotID: input.Snapshot.ID, CalculatedAt: input.AsOf, Demo: input.Product.Demo || input.Settings.Demo || input.Snapshot.Demo, Messages: []string{err.Error()}, Alternatives: []Evaluation{}, Unavailable: []Unavailable{}}
		}
		out = append(out, ContextResult{ID: id, Name: name, State: state, Promotion: p, Input: input, Result: result})
	}
	run("daily", "日常定价", "日常", in, nil, nil)
	for _, p := range promos {
		next, err := ApplyPromotion(in, p)
		state := "活动中"
		day := in.AsOf.In(time.FixedZone("CST", 8*3600)).Format("2006-01-02")
		if p.StartsAt != "" && day < p.StartsAt {
			state = "未开始"
		}
		if p.EndsAt != "" && day > p.EndsAt {
			state = "已结束"
		}
		run(p.ID, p.Name, state, next, &p, err)
	}
	return out
}
func EvaluateSales(in Input, s SalesScenario) SalesEvaluation {
	r := SalesEvaluation{Scenario: s, Status: "invalid", Messages: []string{"商家假设；固定销量不代表销量预测"}}
	fail := func(err error) SalesEvaluation { r.Messages = append(r.Messages, err.Error()); return r }
	if !validNumber(s.Price) || s.Price < .01 || s.Price > 1000000 || math.Abs(s.Price*100-math.Round(s.Price*100)) > 1e-6 {
		return fail(fmt.Errorf("情景标价须为0.01–1,000,000元，最多两位小数"))
	}
	if s.DailyUnits != nil && (!validNumber(*s.DailyUnits) || *s.DailyUnits < 0 || *s.DailyUnits > 10000000) {
		return fail(fmt.Errorf("预计日销量须非负且不超过一千万件"))
	}
	if err := Validate(in); err != nil {
		return fail(err)
	}
	if in.Settings.Demo && !in.Product.Demo {
		return fail(fmt.Errorf("演示费率不能用于真实商品"))
	}
	for _, q := range in.Quotes {
		if q.ID == s.QuoteID && q.Kind == "international" {
			if len(in.Product.RouteIDs) > 0 && !has(in.Product.RouteIDs, q.ID) {
				return fail(fmt.Errorf("渠道不在该商品选定的方案中"))
			}
			e, err := Evaluate(in, q, s.Price)
			if err != nil {
				return fail(err)
			}
			r.Evaluation = &e
			r.MeetsConstraints = feasible(in, e)
			if in.Product.Mode == "target" && (e.Score.Total == nil || *e.Score.Total < in.Product.Options.TargetScore) {
				r.MeetsConstraints = false
			}
			r.Status = "ok"
			if !e.Valid {
				r.Status = "invalid"
			}
			if !r.MeetsConstraints {
				r.Messages = append(r.Messages, "未满足当前利润、亏损或评分约束，仅供情景比较")
			}
			if s.DailyUnits != nil && e.Valid {
				v := num(d(e.Profit).Mul(d(*s.DailyUnits)))
				w := num(d(e.WorstProfit).Mul(d(*s.DailyUnits)))
				r.DailyProfit = &v
				r.WorstDailyProfit = &w
			} else if s.DailyUnits == nil {
				r.Status = "partial"
				r.Messages = append(r.Messages, "未填写预计日销量，不生成日利润")
			}
			return r
		}
	}
	return fail(fmt.Errorf("情景物流渠道不存在"))
}
