package app

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"strings"

	"pricepilot/internal/engine"
)

//go:embed guoo_rates.json
var guooRatesJSON []byte

func guooQuotes() []engine.Quote {
	var rows []struct {
		Group         string  `json:"group"`
		Service       string  `json:"service"`
		Name          string  `json:"name"`
		Row           int     `json:"row"`
		MinValue      float64 `json:"min_value"`
		MaxValue      float64 `json:"max_value"`
		MinWeight     float64 `json:"min_weight"`
		MaxWeight     float64 `json:"max_weight"`
		Fixed         float64 `json:"fixed"`
		PerGram       float64 `json:"per_gram"`
		Batteries     bool    `json:"batteries"`
		Liquids       bool    `json:"liquids"`
		MaxSum        float64 `json:"max_sum"`
		MaxLength     float64 `json:"max_length"`
		VolumeDivisor float64 `json:"volume_divisor"`
		DaysMin       float64 `json:"days_min"`
		DaysMax       float64 `json:"days_max"`
		Compensation  string  `json:"compensation_rub"`
		ReturnNote    struct {
			NotLeft      string `json:"not_left_dropoff"`
			BeforeImport string `json:"before_import"`
			AfterImport  string `json:"after_import"`
		} `json:"return_note"`
	}
	if err := json.Unmarshal(guooRatesJSON, &rows); err != nil {
		panic(err)
	}
	out := make([]engine.Quote, 0, len(rows)+2)
	for _, r := range rows {
		id := "historical-GUOO-" + strings.ToLower(strings.ReplaceAll(r.Service+"-"+r.Group, " ", "-"))
		if r.Service == "Economy" && r.Group == "Extra Small" {
			id = "historical-GUOO"
		}
		fees := partnerPlatformFees()
		for i := range fees {
			fees[i].Version = 3
		}
		q := engine.Quote{ID: id, Version: 3, Provider: "GUOO", Name: r.Name, Kind: "international", Status: "historical", Currency: "CNY", ValueCurrency: "CNY", Source: partnerSource, SourceDate: "2026-07-24", ValidFrom: "2026-07-24", Platforms: []string{"ozon"}, Batteries: r.Batteries, Liquids: r.Liquids, Delivery: &engine.Days{Min: r.DaysMin, Max: r.DaysMax}, TimeScope: "sorting", Estimated: true, Includes: []string{"tail_delivery"}, Fees: fees, RequiredFees: map[string]string{"logistics_access": "物流平台电子服务", "international_contract": "国际运输组织合同签订服务"}, Bands: []engine.Band{{MinValue: r.MinValue, UpTo: engine.Ptr(r.MaxValue), MinWeight: r.MinWeight, MaxWeight: r.MaxWeight, Fixed: r.Fixed, PerGram: r.PerGram, VolumeDivisor: r.VolumeDivisor, Step: 1, MaxLength: r.MaxLength, MaxSum: r.MaxSum}}}
		q.Note = fmt.Sprintf("历史资料：Ozon官方2026-07-24版 CHINA rFBS 第%d行。人民币货值%g–%g元，%g–%gg。时效%g–%g天仅截止Ozon分拣中心。有效期、店铺开通状态及国欧线下附加费待确认，不自动推荐。仓内贴标、换袋及特殊操作另按仓库协议核实。", r.Row, r.MinValue, r.MaxValue, r.MinWeight, r.MaxWeight, r.DaysMin, r.DaysMax)
		if r.VolumeDivisor > 0 {
			q.Note += " 本档实重与长×宽×高÷12000（kg）取大；引擎以克计算，对应cm³/g除数为12。"
		} else {
			q.Note += " 本版表内本档按实重计费；线下附加费另核。"
		}
		q.Compensation = "丢失赔偿上限 " + r.Compensation + " RUB（历史表）；未离交接点：" + r.ReturnNote.NotLeft + "；进口前：" + r.ReturnNote.BeforeImport + "；进口后：" + r.ReturnNote.AfterImport + "。不自动换算为签收后退货费。"
		out = append(out, q)
	}
	for _, wh := range []struct{ id, name string }{{"heihe", "黑河仓"}, {"dongguan", "华南东莞仓"}} {
		out = append(out, engine.Quote{ID: "public-guoo-warehouse-" + wh.id, Version: 1, Provider: "GUOO", Warehouse: wh.name, Name: "国欧" + wh.name + " · 费用待确认", Kind: "warehouse", Status: "pending", Currency: "CNY", ValueCurrency: "CNY", Source: "http://www.guoocang.com/article.php?id=361", SourceDate: "2024-06-17", Platforms: []string{"ozon"}, Bands: []engine.Band{}, Note: "官网列明云仓（无货源一件代发）及备货代发服务，并指定黑河／东莞仓。2025-02-18公告说明入库贴标、箱唛、理货及出库加固、换袋、贴标等可能另收费：http://www.guoocang.com/article.php?id=374 。2025-04-01报价公告 http://www.guoocang.com/article.php?id=375 于2026-11-03读取时公开正文为空。仓配价格、耗材、计费单位及免费条件待商家协议确认，未预填零费用。国际运输与尾程费用另核，不包含在本仓配资料中。"})
	}
	return out
}
