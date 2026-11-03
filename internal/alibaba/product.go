package alibaba

import (
	"encoding/json"
	"fmt"
	"math"
	"net/url"
	"sort"
	"strconv"
	"strings"

	"pricepilot/internal/engine"
)

type SKU struct {
	Default    bool                  `json:"default"`
	ID         string                `json:"id"`
	SpecID     string                `json:"spec_id"`
	Attributes map[string]string     `json:"attributes"`
	Price      *float64              `json:"price"`
	Tiers      []engine.PurchaseTier `json:"tiers"`
	Stock      *int                  `json:"stock"`
}
type Product struct {
	ID           string                `json:"id"`
	Title        string                `json:"title"`
	Status       string                `json:"status"`
	Supplier     string                `json:"supplier"`
	CategoryID   string                `json:"category_id"`
	CategoryName string                `json:"category_name"`
	Images       []string              `json:"images"`
	Attributes   map[string]string     `json:"attributes"`
	SKUs         []SKU                 `json:"skus"`
	MOQ          int                   `json:"moq"`
	Stock        *int                  `json:"stock"`
	Unit         string                `json:"unit"`
	Currency     string                `json:"currency"`
	QuoteType    *int                  `json:"quote_type"`
	Tiers        []engine.PurchaseTier `json:"tiers"`
	Weight       float64               `json:"weight"`
	Length       float64               `json:"length"`
	Width        float64               `json:"width"`
	Height       float64               `json:"height"`
	Batch        bool                  `json:"batch"`
	Warnings     []string              `json:"warnings"`
}
type Quote struct {
	Price             *float64              `json:"price"`
	Tiers             []engine.PurchaseTier `json:"tiers"`
	KnownUnit         bool                  `json:"known_unit"`
	NeedsConfirmation bool                  `json:"needs_confirmation"`
}

func text(b json.RawMessage) string {
	var s string
	if json.Unmarshal(b, &s) == nil {
		return s
	}
	var n json.Number
	if json.Unmarshal(b, &n) == nil {
		return n.String()
	}
	return ""
}
func number(b json.RawMessage) *float64 {
	s := text(b)
	if s == "" {
		return nil
	}
	n, err := strconv.ParseFloat(s, 64)
	if err != nil || math.IsNaN(n) || math.IsInf(n, 0) || n < 0 {
		return nil
	}
	return &n
}
func integer(b json.RawMessage) *int {
	n := number(b)
	if n == nil || *n != math.Trunc(*n) || *n > float64(1<<31-1) {
		return nil
	}
	v := int(*n)
	return &v
}
func positive(b json.RawMessage) float64 {
	if n := number(b); n != nil && *n > 0 {
		return *n
	}
	return 0
}
func object(b json.RawMessage) map[string]json.RawMessage {
	var m map[string]json.RawMessage
	json.Unmarshal(b, &m)
	return m
}
func array(b json.RawMessage) []map[string]json.RawMessage {
	var a []map[string]json.RawMessage
	json.Unmarshal(b, &a)
	return a
}
func tiers(b json.RawMessage) []engine.PurchaseTier {
	out := []engine.PurchaseTier{}
	for _, m := range array(b) {
		q, p := integer(m["startQuantity"]), number(m["price"])
		if q != nil && *q > 0 && p != nil && *p > 0 {
			out = append(out, engine.PurchaseTier{Quantity: *q, Price: *p})
		} else {
			return nil
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Quantity < out[j].Quantity })
	for i := 1; i < len(out); i++ {
		if out[i].Quantity == out[i-1].Quantity {
			return nil
		}
	}
	return out
}
func Normalize(raw json.RawMessage, id string) (Product, error) {
	m := object(raw)
	p := Product{ID: text(m["productID"]), Title: text(m["subject"]), Status: text(m["status"]), Supplier: text(m["supplierLoginId"]), CategoryID: text(m["categoryID"]), CategoryName: text(m["categoryName"]), Attributes: map[string]string{}, Images: []string{}, SKUs: []SKU{}, Warnings: []string{}}
	if p.ID != id || p.Title == "" {
		return Product{}, &Error{Kind: "response", Message: "1688返回的商品ID或标题不匹配"}
	}
	if p.Status != "published" && p.Status != "" {
		return Product{}, &Error{Kind: "unavailable", Message: "该1688商品当前未在售"}
	}
	if p.Supplier == "" {
		p.Supplier = text(m["supplierUserId"])
	}
	for _, a := range array(m["attributes"]) {
		if k, v := text(a["attributeName"]), text(a["value"]); k != "" && v != "" {
			p.Attributes[k] = v
		}
	}
	image := object(m["image"])
	// The local mock exposes this documented compatible alias.
	if len(image) == 0 {
		image = object(m["productImage"])
	}
	var images []string
	json.Unmarshal(image["images"], &images)
	for _, src := range images {
		if strings.HasPrefix(src, "//") {
			src = "https:" + src
		} else if !strings.Contains(src, "://") {
			src = "https://cbu01.alicdn.com/" + strings.TrimLeft(src, "/")
		}
		u, err := url.Parse(src)
		if err == nil && (u.Scheme == "http" || u.Scheme == "https") && u.Host != "" {
			p.Images = append(p.Images, src)
		}
	}
	sale := object(m["saleInfo"])
	p.QuoteType = integer(sale["quoteType"])
	p.Stock = integer(sale["amountOnSale"])
	p.Unit = strings.TrimSpace(text(sale["unit"]))
	p.Currency = text(sale["currency"])
	if p.Currency == "" {
		p.Currency = "CNY"
	}
	if q := integer(sale["minOrderQuantity"]); q != nil {
		p.MOQ = *q
	} else if q = integer(sale["beginAmount"]); q != nil {
		p.MOQ = *q
	}
	p.Tiers = tiers(sale["priceRanges"])
	p.Batch = text(sale["saleType"]) == "batch" || positive(sale["batchNumber"]) > 1
	ship := object(m["shippingInfo"])
	p.Weight = positive(ship["unitWeight"]) * 1000
	p.Length = positive(ship["length"])
	p.Width = positive(ship["width"])
	p.Height = positive(ship["height"])
	for _, s := range array(m["skuInfos"]) {
		sku := SKU{ID: text(s["skuId"]), SpecID: text(s["specId"]), Price: number(s["price"]), Tiers: tiers(s["priceRange"]), Stock: integer(s["amountOnSale"]), Attributes: map[string]string{}}
		if sku.ID == "" || sku.ID == "0" {
			sku.ID = sku.SpecID
		}
		if sku.ID == "" {
			continue
		}
		for _, a := range array(s["attributes"]) {
			k, v := text(a["attributeDisplayName"]), text(a["attributeValue"])
			if v == "" {
				v = text(a["customValueName"])
			}
			if k != "" && v != "" {
				sku.Attributes[k] = v
			}
		}
		p.SKUs = append(p.SKUs, sku)
	}
	if len(p.SKUs) == 0 {
		p.SKUs = append(p.SKUs, SKU{ID: "default", Default: true, Attributes: map[string]string{}, Stock: p.Stock})
	}
	if p.MOQ == 0 {
		p.Warnings = append(p.Warnings, "起订量未返回，请人工确认")
	}
	if p.Unit == "" || p.Batch || p.QuoteType == nil {
		p.Warnings = append(p.Warnings, "报价类型或计价单位需人工确认，不能将整批价格直接当作单件采购价")
	}
	if p.Weight > 0 || p.Length > 0 {
		p.Warnings = append(p.Warnings, "包装重量尺寸来自商品接口，请核实是否适用于所选SKU及实际包装")
	}
	p.Warnings = append(p.Warnings, "供应商评分、备货时间、国内运费及其他缺失费用需另行补充")
	return p, nil
}
func (p Product) Quote(skuID string, quantity int) (Quote, error) {
	q := Quote{Tiers: []engine.PurchaseTier{}, KnownUnit: p.Unit != "" && !p.Batch}
	if quantity < 1 || quantity > 1<<31-1 {
		return q, fmt.Errorf("采购数量须为正整数")
	}
	if p.MOQ > 0 && quantity < p.MOQ {
		return q, fmt.Errorf("采购数量不能低于起订量%d", p.MOQ)
	}
	if p.Currency != "CNY" && p.Currency != "RMB" {
		return q, fmt.Errorf("当前仅支持人民币采购报价，请人工核实币种")
	}
	var sku *SKU
	for i := range p.SKUs {
		if p.SKUs[i].ID == skuID {
			sku = &p.SKUs[i]
			break
		}
	}
	if sku == nil {
		return q, fmt.Errorf("请选择当前商品的有效SKU")
	}
	for _, stock := range []*int{p.Stock, sku.Stock} {
		if stock != nil && quantity > *stock {
			return q, fmt.Errorf("采购数量超过接口返回的可用库存")
		}
	}
	t := -1
	if p.QuoteType != nil {
		t = *p.QuoteType
	}
	switch t {
	case 0, 2:
		q.Tiers = p.Tiers
	case 1:
		q.Price = sku.Price
	case 3:
		q.Tiers = sku.Tiers
	default:
		q.Price = sku.Price
		if q.Price == nil {
			q.Tiers = p.Tiers
		}
	}
	for _, tier := range q.Tiers {
		if tier.Quantity <= quantity {
			q.Price = engine.Ptr(tier.Price)
		}
	}
	if q.Price != nil && *q.Price <= 0 {
		q.Price = nil
	}
	q.NeedsConfirmation = !q.KnownUnit || p.MOQ == 0 || t < 0 || t > 3 || q.Price == nil
	return q, nil
}
