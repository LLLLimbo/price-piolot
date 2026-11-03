package app

import (
	"context"
	"encoding/json"
	"fmt"
	"pricepilot/internal/engine"
	"pricepilot/internal/store"
	"strings"
	"time"
)

const oglSource = "https://cdn.ozone.ru/s3/ozon-disk-api/Partner_Delivery/OGL_ENG_CN_24_08_26_added_Pick-up_1786697358.xlsx"
const partnerSource = "https://cdn.ozone.ru/s3/ozon-disk-api/Partner_Delivery/China_scoring_ENG_CN_24_07_26_1784197567.xlsx"

func oglPlatformFees() []engine.Fee {
	return []engine.Fee{
		{Version: 1, Status: "pending", Code: "logistics_access", Name: "物流平台电子服务（非银行收单费）", Enabled: true, Kind: "rate", Rate: engine.Ptr(.02), Currency: "RUB", Basis: "sale", On: "delivered", Minimum: engine.Ptr(15), Maximum: engine.Ptr(200), DeductSettlement: true, Source: "https://global-help.ozon.com/zh/commissions/ozon-fees/ogl-rates/"},
		{Version: 1, Status: "pending", Code: "international_contract", Name: "国际运输组织合同签订服务（适用性待确认）", Enabled: true, Kind: "rate", Rate: engine.Ptr(.0033), Currency: "CNY", Basis: "sale", On: "delivered", ValidFrom: "2026-09-01", DeductSettlement: true, Source: "https://global-help.ozon.com/zh/commissions/ozon-fees/commissions-for-other-services/"},
	}
}

func partnerPlatformFees() []engine.Fee {
	fees := oglPlatformFees()
	fees[0].Source = "https://global-help.ozon.com/zh/commissions/ozon-fees/partner-delivery-rates/"
	return fees
}

func demo(now time.Time) (engine.Product, engine.Settings, engine.Snapshot, []engine.Quote) {
	now = now.In(time.FixedZone("CST", 8*3600))
	fixed := func(code, name string, v float64) engine.Fee {
		return engine.Fee{Version: 1, Status: "confirmed", Code: code, Name: name, Enabled: true, Kind: "fixed", Amount: engine.Ptr(v), Currency: "CNY", On: "shipped", Source: "演示假设"}
	}
	p := engine.Product{PriceCondition: "standard", ID: "demo-bag", Name: "旅行收纳袋 · 三件套", Demo: true, Supply: "dropship", Supplier: "示例1688供货商", SupplierScore: engine.Ptr(4.7), SupplierScale: 5, SupplierSource: "演示评分（5分制）", PurchaseQuantity: 1, MOQ: 1, Weight: 200, Length: 24, Width: 18, Height: 4, SupplierDays: &engine.Days{Min: 1, Max: 2}, DomesticDays: &engine.Days{Min: 1, Max: 3}, TailDays: &engine.Days{Min: 2, Max: 4}, WarehouseQuoteID: "demo-warehouse", RouteIDs: []string{"demo-economy", "demo-express"}, Costs: []engine.Fee{fixed("procurement", "采购价", 45), fixed("domestic_shipping", "国内运费", 4), fixed("packaging", "包装耗材", 1), fixed("warehouse_operation", "打包贴标", 2), fixed("tail_delivery", "尾程配送", 0), fixed("tax", "税费分摊", 0), fixed("overhead", "经营费用分摊", 0)}, Returns: engine.Returns{Rate: .03, CancelRate: .01, Reverse: engine.Ptr(15), Disposal: engine.Ptr(0), Recovery: engine.Ptr(10), CancelCost: engine.Ptr(2)}, Keywords: []string{"органайзер"}, CategoryID: "demo-storage", Attributes: map[string]string{"套装数量": "3件", "材质": "涤纶"}, Mode: "profit", Options: engine.ModeOptions{Margin: .2, TargetScore: 80}, UpdatedAt: now}
	rate := func(code, name string, v float64, basis string, deduct bool) engine.Fee {
		return engine.Fee{Version: 1, Status: "confirmed", Code: code, Name: name, Enabled: true, Kind: "rate", Rate: engine.Ptr(v), Currency: "CNY", Basis: basis, On: "kept", DeductSettlement: deduct, Source: "演示假设，非平台官方费率"}
	}
	s := engine.Settings{FX: 12, FXSource: "演示参考汇率", FXDate: now.Format("2006-01-02"), Stress: .05, Region: "莫斯科", Weights: engine.Weights{Price: 50, Delivery: 30, Supplier: 20}, SnapshotHours: 24, Demo: true, Fees: []engine.Fee{rate("commission", "类目佣金", .12, "sale", true), rate("advertising", "广告预算", .08, "sale", false), rate("payment", "支付收款费", .02, "sale", true), rate("withdrawal", "提现手续费", .005, "settlement", false)}, CategoryFees: map[string][]engine.Fee{}, DeliveryReferences: []engine.DeliveryReference{}, CategoryMappings: []engine.CategoryMapping{}}
	snap := engine.Snapshot{ID: store.ID(), ProductID: p.ID, Fingerprint: engine.Fingerprint(p), Source: "manual_demo", Demo: true, FetchedAt: now, Competitors: []engine.Competitor{}, Warnings: []string{"此快照为演示样本，未从Seerfar获取"}}
	prices := []float64{1290, 1390, 1460, 1550, 1690, 1790, 1890, 2090}
	days := []engine.Days{{Min: 18, Max: 25}, {Min: 10, Max: 15}, {Min: 14, Max: 20}, {Min: 7, Max: 12}, {Min: 20, Max: 30}, {Min: 12, Max: 18}, {Min: 9, Max: 14}, {Min: 5, Max: 9}}
	for i, v := range prices {
		snap.Competitors = append(snap.Competitors, engine.Competitor{SKU: "DEMO-" + string(rune('A'+i)), Title: "旅行收纳袋 三件套（演示）", PriceRUB: v, SellerName: "演示店铺", CategoryID: p.CategoryID, Attributes: p.Attributes, Include: true, Verified: true, Confidence: 1, Reason: "演示同规格样本", Delivery: &days[i], DeliverySource: "演示时效假设", Estimated: true, Fulfillment: []string{"RFBS"}})
	}
	q := engine.Quote{Version: 1, ID: "demo-economy", Provider: "演示货代", Name: "示例经济陆运", Kind: "international", Status: "active", Demo: true, Currency: "CNY", ValueCurrency: "CNY", ValidFrom: now.AddDate(-1, 0, 0).Format("2006-01-02"), ValidTo: now.AddDate(1, 0, 0).Format("2006-01-02"), Source: "演示报价，非实际签约价", SourceDate: now.Format("2006-01-02"), Platforms: []string{"ozon"}, Batteries: false, Liquids: false, Bands: []engine.Band{{Fixed: 2.9, PerGram: .0247, MaxWeight: 35000, Step: 1}}, Includes: []string{"tail_delivery"}, Delivery: &engine.Days{Min: 15, Max: 20}, TimeScope: "buyer", Estimated: true, ReturnCost: engine.Ptr(15), ReturnCurrency: "CNY", Note: "仅用于演示计算"}
	fast := q
	fast.ID = "demo-express"
	fast.Name = "示例航空快线"
	fast.Bands = []engine.Band{{Fixed: 8, PerGram: .055, MaxWeight: 35000, Step: 1}}
	fast.Delivery = &engine.Days{Min: 5, Max: 8}
	wh := q
	wh.ID = "demo-warehouse"
	wh.Name = "示例国内云仓"
	wh.Warehouse = "示例东莞仓"
	wh.Kind = "warehouse"
	wh.Bands = []engine.Band{{Fixed: 2.5, MaxWeight: 35000, Step: 1}}
	wh.Includes = []string{"warehouse_operation", "packaging"}
	wh.Delivery = &engine.Days{Min: 0, Max: 1}
	wh.TimeScope = ""
	return p, s, snap, []engine.Quote{q, fast, wh}
}
func publicQuotes() []engine.Quote {
	q := engine.Quote{ID: "public-ogl", Provider: "Ozon", Name: "OGL 官方物流", Kind: "international", Status: "pending", Currency: "CNY", ValueCurrency: "CNY", ValidFrom: "2026-08-24", Source: oglSource, SourceDate: "2026-08-24", Platforms: []string{"ozon"}, Batteries: true, Liquids: true, Delivery: &engine.Days{Min: 13, Max: 17}, TimeScope: "sorting", Estimated: true, Note: "公开费率，待确认店铺适用性、有效期、平台附加服务费和尾程费用。时效截止Ozon分拣中心。"}
	q.Version = 2
	q.Fees = oglPlatformFees()
	q.RequiredFees = map[string]string{"logistics_access": "物流平台电子服务", "international_contract": "国际运输组织合同签订服务"}
	q.Includes = []string{"tail_delivery"}
	q.Note = "国际运费覆盖向买家配送，尾程费用不重复计入；13–17天仅截止Ozon分拣中心，仍需补充尾程时效。物流电子服务2%（每票15–200 RUB）与银行收单费分开；另核对0.33%国际运输组织合同服务、适用日期、扣费事件及店铺开通状态。单件按独立一票预算。"
	band := func(max *float64, minW, maxW, fixed, perGram, div float64) engine.Band {
		return engine.Band{UpTo: max, MinWeight: minW, MaxWeight: maxW, Fixed: fixed, PerGram: perGram, VolumeDivisor: div, Step: 1, MaxLength: 200}
	}
	q.Bands = []engine.Band{band(engine.Ptr(135), 1, 500, 2.9, .0247, 0), band(engine.Ptr(135), 500.000001, 35000, 22.25, .0164, 0), band(engine.Ptr(635), 1, 2000, 15.48, .0242, 0), band(engine.Ptr(635), 2000.000001, 35000, 34.82, .0164, 12), band(engine.Ptr(22525), 1, 5000, 21.28, .0242, 0), band(engine.Ptr(22525), 5000.000001, 35000, 59.97, .0222, 12)}
	out := []engine.Quote{q}
	for _, name := range []string{"CEL", "GUOO", "RETS", "Ural", "ZTO", "Xingyuan", "OYX", "ABT", "ATC"} {
		partner := q
		partner.Version, partner.Fees = 2, partnerPlatformFees()
		partner.ID = "historical-" + name
		partner.Provider = name
		partner.Name = name + " Economy Extra Small"
		partner.Status = "historical"
		partner.ValidFrom = "2026-07-24"
		partner.ValidTo = ""
		partner.SourceDate = "2026-07-24"
		partner.Source = partnerSource
		partner.Bands = []engine.Band{{UpTo: engine.Ptr(135), MinWeight: 1, MaxWeight: 500, Fixed: 3.37, PerGram: .0281, Step: 1, MaxLength: 60, MaxSum: 90}}
		partner.Delivery = &engine.Days{Min: 17, Max: 29}
		partner.Batteries = name != "CEL"
		partner.Liquids = name != "ZTO" && name != "ATC"
		partner.Compensation = "该档丢失赔偿上限1500 RUB（历史版本）"
		partner.Note = "历史参考；合作渠道停用安排需以店铺后台核实，不自动推荐。"
		out = append(out, partner)
	}
	gbs := q
	gbs.Version, gbs.Fees = 2, partnerPlatformFees()
	gbs.ID = "historical-GBS"
	gbs.Provider = "GBS"
	gbs.Name = "GBS Economy Budget"
	gbs.Status = "historical"
	gbs.Source = partnerSource
	gbs.SourceDate = "2026-07-24"
	gbs.ValidFrom = "2026-07-24"
	gbs.ValidTo = ""
	gbs.Liquids = false
	gbs.Delivery = &engine.Days{Min: 17, Max: 29}
	gbs.Bands = []engine.Band{{UpTo: engine.Ptr(135), MinWeight: 501, MaxWeight: 30000, Fixed: 25.83, PerGram: .0191, Step: 1, MaxLength: 60, MaxSum: 150}}
	gbs.Note = "历史Budget档，非轻小件报价。"
	out = append(out, gbs)
	cloud := q
	cloud.Version, cloud.Fees = 1, nil
	cloud.RequiredFees = nil
	cloud.ID = "public-yunque"
	cloud.Provider = "云雀驿站"
	cloud.Warehouse = "国内云仓"
	cloud.Name = "单SKU基础操作"
	cloud.Kind = "warehouse"
	cloud.Source = "https://www.10100.com/article/3661910"
	cloud.SourceDate = "2025-05-15"
	cloud.ValidFrom = "2025-05-15"
	cloud.ValidTo = ""
	cloud.Bands = []engine.Band{{Fixed: 2.5, MinWeight: 1, MaxWeight: 15000, MaxSum: 150, Step: 1}}
	cloud.Delivery = nil
	cloud.TimeScope = ""
	cloud.Includes = []string{"warehouse_operation"}
	cloud.Note = "2025年公示：单SKU5件以内2.5元起，多SKU3元起，E包裹另加2元；超件、大件、耗材及国际运费需另核。"
	out = append(out, cloud)
	wh := cloud
	wh.ID = "public-timegate"
	wh.Provider = "东莞时空之门仓"
	wh.Name = "基础贴单"
	wh.Source = "https://subuser.kuajing84.com/index/setting/storehouselist.html"
	wh.SourceDate = ""
	wh.ValidFrom = ""
	wh.Bands = []engine.Band{{Fixed: 1, Step: 1}}
	wh.Note = "平台公示贴单1元、简单验货加贴单3元；当前资费、限制和有效期待核实。"
	out = append(out, wh)
	post := q
	post.Version, post.Fees = 1, nil
	post.RequiredFees = nil
	post.Includes = nil
	post.ID = "historical-chinapost"
	post.Provider = "中国邮政"
	post.Name = "Ozon航空e邮宝（2023旧资费）"
	post.Status = "historical"
	post.ValidFrom = "2023-03-22"
	post.ValidTo = ""
	post.SourceDate = "2023-03-16"
	post.Source = "https://my.ems.com.cn/pcp-web/f/pcp/indexController/tonoticeindex?ntificationId=7538f9df411cfb5310d78771f20d3a6f"
	post.Bands = []engine.Band{{Fixed: 13.5, PerGram: .0855, Step: 1}}
	post.Delivery = nil
	post.Note = "仅保存旧资费事实；当前协议价及承运条件待补充，不自动推荐。"
	out = append(out, post)
	uni := q
	uni.Version, uni.Fees = 1, nil
	uni.RequiredFees = nil
	uni.ID = "excluded-uni-offline"
	uni.Provider = "UNI"
	uni.Name = "UNI OFFLINE Economy"
	uni.Status = "excluded"
	uni.ValidFrom = "2026-03-01"
	uni.ValidTo = "2026-12-31"
	uni.SourceDate = "2026-03-01"
	uni.Source = "https://storage.yandexcloud.net/unitrade-public/线下B2C公开要约协议.pdf"
	uni.Platforms = []string{}
	uni.Bands = []engine.Band{{Fixed: 15, PerGram: .035, MaxWeight: 30000, Step: 100}}
	uni.Delivery = &engine.Days{Min: 20, Max: 25}
	uni.TimeScope = "buyer"
	uni.Note = "公开条款3.3.7明确排除Ozon等平台订单，禁止启用该公开方案。"
	out = append(out, uni)
	return out
}
func Seed(ctx context.Context, db *store.Store) error {
	return db.Atomic(ctx, func(root *store.Store) error {
		for _, q := range publicQuotes() {
			var old engine.Quote
			if err := root.Get(ctx, "public_quotes", q.ID, &old); err == store.ErrNotFound {
				if q.Version == 0 {
					q.Version = 1
				}
				if err = root.Put(ctx, "public_quotes", q.ID, q); err != nil {
					return err
				}
			} else if err == nil && old.Version < q.Version {
				if err = root.Put(ctx, "public_quotes", q.ID, q); err != nil {
					return err
				}
			} else if err != nil {
				return err
			}
		}
		var marker map[string]bool
		if err := root.Get(ctx, "meta", "stores-v2", &marker); err == nil {
			return seedOzonRules(ctx, root)
		} else if err != store.ErrNotFound {
			return err
		}
		now := time.Now().In(time.FixedZone("CST", 8*3600))
		for i, id := range []string{"default", "demo-second"} {
			profile := engine.StoreProfile{ID: id, Name: "Ozon 示例一店", Demo: true, CreatedAt: now, SellerIDs: []string{"demo-owned-1"}}
			if i == 1 {
				profile.Name = "Ozon 示例二店"
				profile.SellerIDs = []string{"demo-owned-2"}
			}
			if err := root.Put(ctx, "stores", id, profile); err != nil {
				return err
			}
			scoped := root.ForStore(id)
			var existing engine.Settings
			if err := scoped.Get(ctx, "settings", "default", &existing); err == store.ErrNotFound {
				p, settings, snapshot, quotes := demo(now)
				p.StoreID = id
				p.Version = 1
				settings.StoreID = id
				settings.Version = 1
				snapshot.StoreID = id
				if i == 1 {
					settings.Fees[0].Rate = engine.Ptr(.16)
					settings.FX = 11.8
					p.Costs[0].Amount = engine.Ptr(52)
				}
				for j := range p.Costs {
					p.Costs[j].Status = "confirmed"
					p.Costs[j].Version = 1
				}
				for j := range settings.Fees {
					settings.Fees[j].Status = "confirmed"
					settings.Fees[j].Version = 1
				}
				for _, q := range quotes {
					q.StoreID = id
					q.Version = 1
					if err := scoped.Put(ctx, "quotes", q.ID, q); err != nil {
						return err
					}
					if err := scoped.Put(ctx, "quotes_versions", q.ID+"/1", q); err != nil {
						return err
					}
				}
				if err := scoped.Put(ctx, "products", p.ID, p); err != nil {
					return err
				}
				if err := scoped.Put(ctx, "snapshots", snapshot.ID, snapshot); err != nil {
					return err
				}
				if err := scoped.Put(ctx, "settings", "default", settings); err != nil {
					return err
				}
			} else if err != nil {
				return err
			}
			// Add ownership and explicit versions to migrated resources without changing their economics.
			for _, kind := range []string{"settings", "products", "quotes", "snapshots"} {
				rows, err := scoped.List(ctx, kind)
				if err != nil {
					return err
				}
				for _, raw := range rows {
					var v map[string]any
					if err = json.Unmarshal(raw, &v); err != nil {
						return err
					}
					v["store_id"] = id
					if kind == "quotes" {
						var ref engine.Quote
						key, _ := v["id"].(string)
						if root.Get(ctx, "public_quotes", key, &ref) == nil {
							v["public_id"] = key
						}
					}
					key, _ := v["id"].(string)
					if kind == "settings" {
						key = "default"
					}
					if kind != "snapshots" {
						if version, _ := v["version"].(float64); version < 1 {
							v["version"] = 1
						}
					}
					if kind == "products" || kind == "settings" {
						for _, field := range []string{"fees", "costs"} {
							if fees, ok := v[field].([]any); ok {
								for _, item := range fees {
									fee, _ := item.(map[string]any)
									if fee["status"] == nil || fee["status"] == "" {
										fee["status"] = "confirmed"
									}
									if fee["version"] == nil {
										fee["version"] = 1
									}
								}
							}
						}
					}
					if err = scoped.Put(ctx, kind, key, v); err != nil {
						return err
					}
					if kind != "snapshots" {
						if err = scoped.Put(ctx, kind+"_versions", fmt.Sprintf("%s/%v", key, v["version"]), v); err != nil {
							return err
						}
					}
				}
			}
			promo := engine.PromotionScenario{ID: "demo-promo", StoreID: id, Version: 1, Name: "周末活动 · 演示", Source: "演示活动假设", StartsAt: now.AddDate(0, 0, -1).Format("2006-01-02"), EndsAt: now.AddDate(0, 0, 30).Format("2006-01-02"), MerchantDiscount: engine.Discount{Kind: "percent", Value: engine.Ptr(.15)}, PlatformDiscount: engine.Discount{Kind: "fixed", Value: engine.Ptr(5), Currency: "CNY", Confirmed: true, Source: "演示平台全额承担"}, Demo: true}
			if err := scoped.Put(ctx, "promotions", promo.ID, promo); err != nil {
				return err
			}
			sale := engine.SalesScenario{ID: "demo-sales", StoreID: id, ProductID: "demo-bag", Name: "日常销量 · 商家假设", Price: 125, QuoteID: "demo-economy", DailyUnits: engine.Ptr(2), Note: "演示预期，非销量预测"}
			if err := scoped.Put(ctx, "sales_scenarios", sale.ID, sale); err != nil {
				return err
			}
		}
		if err := root.Put(ctx, "meta", "stores-v2", map[string]bool{"complete": true}); err != nil {
			return err
		}
		return seedOzonRules(ctx, root)
	})
}

func seedOzonRules(ctx context.Context, root *store.Store) error {
	const key = "ozon-rules-2026-11-03"
	var marker map[string]bool
	if err := root.Get(ctx, "meta", key, &marker); err == nil {
		return nil
	} else if err != store.ErrNotFound {
		return err
	}
	stores, err := root.List(ctx, "stores")
	if err != nil {
		return err
	}
	for _, raw := range stores {
		var profile engine.StoreProfile
		if err := json.Unmarshal(raw, &profile); err != nil {
			return err
		}
		scoped := root.ForStore(profile.ID)
		rows, err := scoped.List(ctx, "quotes")
		if err != nil {
			return err
		}
		for _, raw := range rows {
			var q engine.Quote
			if err := json.Unmarshal(raw, &q); err != nil {
				return err
			}
			ogl := (q.ID == "public-ogl" || q.PublicID == "public-ogl" || q.Source == oglSource) && len(q.Fees) == 0
			partner := q.Status == "historical" && q.Source == partnerSource && q.ValidTo == "2026-10-31"
			if !ogl && !partner {
				continue
			}
			// Preserve the signed carrier price and its old version; require confirmation of the separate services.
			if err := scoped.Put(ctx, "quotes_versions", fmt.Sprintf("%s/%d", q.ID, q.Version), q); err != nil {
				return err
			}
			q.Version++
			if ogl {
				q.Fees = oglPlatformFees()
			}
			if partner {
				q.ValidTo = ""
				if len(q.Fees) == 0 {
					q.Fees = partnerPlatformFees()
				}
			}
			q.RequiredFees = publicQuotes()[0].RequiredFees
			for i := range q.Fees {
				q.Fees[i].Version = q.Version
			}
			if !contains(q.Includes, "tail_delivery") {
				q.Includes = append(q.Includes, "tail_delivery")
			}
			if q.Status == "active" {
				q.Status = "pending"
			}
			q.Note = strings.TrimSpace(q.Note + " 国际运费含向买家配送；物流电子服务和国际运输合同服务另列，适用性与扣费事件待确认。公开时效仅到分拣中心，当前有效期待店铺后台确认。")
			if err := scoped.Put(ctx, "quotes_versions", fmt.Sprintf("%s/%d", q.ID, q.Version), q); err != nil {
				return err
			}
			if err := scoped.Put(ctx, "quotes", q.ID, q); err != nil {
				return err
			}
		}
	}
	return root.Put(ctx, "meta", key, map[string]bool{"complete": true})
}
