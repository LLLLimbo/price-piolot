package app

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"pricepilot/internal/engine"
	"pricepilot/internal/store"
)

func (s *Server) listStores(c *gin.Context) {
	rows, err := s.root.List(c.Request.Context(), "stores")
	if err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"stores": rows})
}
func (s *Server) saveStore(c *gin.Context) {
	var p engine.StoreProfile
	if err := c.ShouldBindJSON(&p); err != nil {
		bad(c, fmt.Errorf("店铺格式无效"))
		return
	}
	if strings.TrimSpace(p.Name) == "" {
		bad(c, fmt.Errorf("请输入店铺名称"))
		return
	}
	ctx := c.Request.Context()
	fresh := c.Param("store_id") == ""
	if fresh {
		p.ID = store.ID()
		p.CreatedAt = time.Now().UTC()
		p.Demo = false
	} else {
		var old engine.StoreProfile
		if s.root.Get(ctx, "stores", c.Param("store_id"), &old) != nil {
			notFound(c)
			return
		}
		p.ID = old.ID
		p.CreatedAt = old.CreatedAt
		p.Demo = old.Demo
	}
	err := s.root.Atomic(ctx, func(tx *store.Store) error {
		if err := tx.Put(ctx, "stores", p.ID, p); err != nil {
			return err
		}
		if fresh {
			return seedEmptyStore(ctx, tx.ForStore(p.ID))
		}
		return nil
	})
	if err != nil {
		internal(c)
		return
	}
	if fresh {
		if _, err = s.services.Get(ctx, p.ID); err != nil {
			internal(c)
			return
		}
	}
	c.JSON(200, gin.H{"store": p})
}
func seedEmptyStore(ctx context.Context, db *store.Store) error {
	_, settings, _, _ := demo(time.Now().UTC())
	settings.StoreID = db.Scope()
	settings.Version = 1
	settings.Demo = false
	settings.FX = 0
	settings.FXSource = ""
	settings.FXDate = ""
	for i := range settings.Fees {
		settings.Fees[i].Rate = nil
		settings.Fees[i].Amount = nil
		settings.Fees[i].Source = ""
		settings.Fees[i].Status = "pending"
		settings.Fees[i].Version = 1
	}
	return db.Put(ctx, "settings", "default", settings)
}
func (s *Server) publicCatalog(c *gin.Context) {
	rows, err := s.root.List(c.Request.Context(), "public_quotes")
	if err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"quotes": rows})
}
func (s *Server) copyPublic(c *gin.Context) {
	var req struct {
		PublicID string `json:"public_id"`
	}
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("资料编号无效"))
		return
	}
	var q engine.Quote
	if s.root.Get(c.Request.Context(), "public_quotes", req.PublicID, &q) != nil {
		notFound(c)
		return
	}
	q.PublicID = q.ID
	q.ID = store.ID()
	q.StoreID = s.store.Scope()
	q.Version = 1
	if q.Status != "excluded" {
		q.Status = "pending"
	}
	if err := s.versioned(c, "quotes", q.ID, q.Version, q); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"quote": q})
}
func (s *Server) quoteVersions(c *gin.Context) {
	var q engine.Quote
	if s.store.Get(c.Request.Context(), "quotes", c.Param("id"), &q) != nil {
		notFound(c)
		return
	}
	raw, err := s.store.List(c.Request.Context(), "quotes_versions")
	if err != nil {
		internal(c)
		return
	}
	out := []engine.Quote{}
	for _, v := range raw {
		var item engine.Quote
		if json.Unmarshal(v, &item) == nil && item.ID == q.ID {
			out = append(out, item)
		}
	}
	c.JSON(200, gin.H{"versions": out})
}
func (s *Server) nextVersion(c *gin.Context, kind, id string) int {
	var old struct {
		Version int `json:"version"`
	}
	s.store.Get(c.Request.Context(), kind, id, &old)
	return old.Version + 1
}
func (s *Server) versioned(c *gin.Context, kind, id string, version int, value any) error {
	return s.store.Atomic(c.Request.Context(), func(tx *store.Store) error {
		if err := tx.Put(c.Request.Context(), kind+"_versions", fmt.Sprintf("%s/%d", id, version), value); err != nil {
			return err
		}
		return tx.Put(c.Request.Context(), kind, id, value)
	})
}
func (s *Server) bindProduct(c *gin.Context, p *engine.Product) error {
	if p.StoreID != "" && p.StoreID != s.store.Scope() {
		return fmt.Errorf("商品不属于当前店铺")
	}
	p.StoreID = s.store.Scope()
	if err := s.validateFeeRefs(c, p.Costs); err != nil {
		return err
	}
	for _, id := range p.RouteIDs {
		var q engine.Quote
		if s.store.Get(c.Request.Context(), "quotes", id, &q) != nil || q.Kind != "international" {
			return fmt.Errorf("商品引用了不属于当前店铺的跨境报价")
		}
	}
	if id := p.WarehouseQuoteID; id != "" && id != "self" {
		var q engine.Quote
		if s.store.Get(c.Request.Context(), "quotes", id, &q) != nil || q.Kind != "warehouse" {
			return fmt.Errorf("商品引用了不属于当前店铺的仓配报价")
		}
	}

	return nil
}
func (s *Server) ownedSellerIDs(c *gin.Context) []string {
	rows, _ := s.root.List(c.Request.Context(), "stores")
	out := []string{}
	for _, raw := range rows {
		var p engine.StoreProfile
		if json.Unmarshal(raw, &p) == nil {
			for _, id := range p.SellerIDs {
				if id != "" && !contains(out, id) {
					out = append(out, id)
				}
			}
		}
	}
	return out
}
func (s *Server) quota(c *gin.Context) {
	client, _, err := s.currentSeerfar(c)
	if err != nil {
		internal(c)
		return
	}
	v, err := client.Quota(c.Request.Context())
	if err != nil {
		sourceError(c, err)
		return
	}
	c.JSON(200, gin.H{"data": v})
}
func (s *Server) deleteProduct(c *gin.Context) {
	if err := s.store.Delete(c.Request.Context(), "products", c.Param("id")); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"ok": true})
}
func (s *Server) savePromotion(c *gin.Context) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var p engine.PromotionScenario
	if c.ShouldBindJSON(&p) != nil {
		bad(c, fmt.Errorf("活动格式无效"))
		return
	}
	if p.StoreID != "" && p.StoreID != s.store.Scope() {
		bad(c, fmt.Errorf("活动不属于当前店铺"))
		return
	}
	p.StoreID = s.store.Scope()
	if c.Param("id") != "" {
		p.ID = c.Param("id")
	} else if p.ID == "" {
		p.ID = store.ID()
	}
	if err := engine.ValidatePromotion(p); err != nil {
		bad(c, err)
		return
	}
	p.Version = s.nextVersion(c, "promotions", p.ID)
	if err := s.versioned(c, "promotions", p.ID, p.Version, p); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"promotion": p})
}
func (s *Server) deletePromotion(c *gin.Context) {
	rows, _ := s.store.List(c.Request.Context(), "sales_scenarios")
	for _, raw := range rows {
		var v engine.SalesScenario
		json.Unmarshal(raw, &v)
		if v.PromotionID == c.Param("id") {
			bad(c, fmt.Errorf("先删除或调整引用此活动的销量情景"))
			return
		}
	}
	if err := s.store.Delete(c.Request.Context(), "promotions", c.Param("id")); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"ok": true})
}
func (s *Server) saveSales(c *gin.Context) {
	var v engine.SalesScenario
	if c.ShouldBindJSON(&v) != nil {
		bad(c, fmt.Errorf("销量情景格式无效"))
		return
	}
	if v.StoreID != "" && v.StoreID != s.store.Scope() {
		bad(c, fmt.Errorf("销量情景不属于当前店铺"))
		return
	}
	v.StoreID = s.store.Scope()
	if c.Param("id") != "" {
		v.ID = c.Param("id")
	} else if v.ID == "" {
		v.ID = store.ID()
	}
	var p engine.Product
	if s.store.Get(c.Request.Context(), "products", v.ProductID, &p) != nil {
		bad(c, fmt.Errorf("请先保存当前店铺的商品档案"))
		return
	}
	var q engine.Quote
	if s.store.Get(c.Request.Context(), "quotes", v.QuoteID, &q) != nil || q.Kind != "international" {
		bad(c, fmt.Errorf("请选择当前店铺的跨境渠道"))
		return
	}
	if v.PromotionID != "" {
		var promo engine.PromotionScenario
		if s.store.Get(c.Request.Context(), "promotions", v.PromotionID, &promo) != nil {
			bad(c, fmt.Errorf("活动不属于当前店铺"))
			return
		}
	}
	if v.Price < .01 || v.Price > 1000000 || v.DailyUnits != nil && (*v.DailyUnits < 0 || *v.DailyUnits > 10000000) {
		bad(c, fmt.Errorf("情景售价或预计日销量无效"))
		return
	}
	if err := s.store.Put(c.Request.Context(), "sales_scenarios", v.ID, v); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"scenario": v})
}
func (s *Server) deleteSales(c *gin.Context) {
	if err := s.store.Delete(c.Request.Context(), "sales_scenarios", c.Param("id")); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"ok": true})
}
func (s *Server) loadDemo(c *gin.Context) {
	p, _, snap, _ := demo(time.Now().UTC())
	p.StoreID = s.store.Scope()
	snap.StoreID = s.store.Scope()
	err := s.store.Atomic(c.Request.Context(), func(tx *store.Store) error {
		if e := tx.Put(c.Request.Context(), "products", p.ID, p); e != nil {
			return e
		}
		return tx.Put(c.Request.Context(), "snapshots", snap.ID, snap)
	})
	if err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"product": p, "snapshot": snap})
}
func (s *Server) getRun(c *gin.Context) {
	var run Run
	if s.store.Get(c.Request.Context(), "runs", c.Param("id"), &run) != nil {
		notFound(c)
		return
	}
	c.JSON(200, run)
}

type calculationRequest struct {
	Product      engine.Product         `json:"product"`
	SnapshotID   string                 `json:"snapshot_id"`
	PromotionIDs []string               `json:"promotion_ids"`
	Scenarios    []engine.SalesScenario `json:"scenarios"`
}

func (s *Server) input(c *gin.Context, req *calculationRequest) (engine.Input, error) {
	in := engine.Input{Product: req.Product, AsOf: time.Now().UTC()}
	if err := s.bindProduct(c, &in.Product); err != nil {
		return in, err
	}
	ctx := c.Request.Context()
	if err := s.store.Get(ctx, "settings", "default", &in.Settings); err != nil {
		return in, err
	}
	in.Settings.OwnSellerIDs = append(in.Settings.OwnSellerIDs, s.ownedSellerIDs(c)...)
	if req.SnapshotID != "" {
		if err := s.store.Get(ctx, "snapshots", req.SnapshotID, &in.Snapshot); err != nil {
			return in, err
		}
		if in.Snapshot.ProductID != "" && in.Snapshot.ProductID != in.Product.ID {
			return in, fmt.Errorf("竞品快照不属于当前商品")
		}
		if in.Snapshot.Source == "seerfar" && in.Snapshot.ServiceSource == nil {
			return in, fmt.Errorf("旧Seerfar快照缺少连接版本，请重新获取竞品；历史记录仍可查看")
		}
		if source := in.Snapshot.ServiceSource; source != nil && source.Provider == "seerfar" {
			active, err := s.services.Resolve(ctx, s.store.Scope(), "seerfar")
			if err != nil {
				return in, err
			}
			if source.Environment != active.Environment || source.ProfileVersion != active.Version {
				return in, fmt.Errorf("Seerfar配置已变化，旧快照仅供查看，请重新获取竞品")
			}
		}
	}
	rows, err := s.store.List(ctx, "quotes")
	if err != nil {
		return in, err
	}
	for _, raw := range rows {
		var q engine.Quote
		if err := json.Unmarshal(raw, &q); err != nil {
			return in, err
		}
		in.Quotes = append(in.Quotes, q)
	}
	return in, nil
}
func (s *Server) buildRun(c *gin.Context, req calculationRequest) (Run, error) {
	in, err := s.input(c, &req)
	if err != nil {
		return Run{}, err
	}
	ctx := c.Request.Context()
	all := []engine.PromotionScenario{}
	rows, err := s.store.List(ctx, "promotions")
	if err != nil {
		return Run{}, err
	}
	for _, raw := range rows {
		var p engine.PromotionScenario
		if err := json.Unmarshal(raw, &p); err != nil {
			return Run{}, err
		}
		all = append(all, p)
	}
	chosen := []engine.PromotionScenario{}
	for _, id := range req.PromotionIDs {
		found := false
		for _, p := range all {
			if p.ID == id {
				if !containsPromotion(chosen, id) {
					chosen = append(chosen, p)
				}
				found = true
				break
			}
		}
		if !found {
			return Run{}, store.ErrNotFound
		}
	}
	sales := req.Scenarios
	if sales == nil {
		sales = []engine.SalesScenario{}
		rows, err = s.store.List(ctx, "sales_scenarios")
		if err != nil {
			return Run{}, err
		}
		for _, raw := range rows {
			var v engine.SalesScenario
			json.Unmarshal(raw, &v)
			if v.ProductID == in.Product.ID {
				sales = append(sales, v)
			}
		}
	}
	results := []engine.SalesEvaluation{}
	for _, v := range sales {
		if v.StoreID != "" && v.StoreID != s.store.Scope() || v.ProductID != in.Product.ID {
			return Run{}, fmt.Errorf("销量情景不属于当前店铺或商品")
		}
		var quote engine.Quote
		if s.store.Get(ctx, "quotes", v.QuoteID, &quote) != nil {
			return Run{}, store.ErrNotFound
		}
		next := in
		if v.PromotionID != "" {
			found := false
			for _, p := range all {
				if p.ID == v.PromotionID {
					next, err = engine.ApplyPromotion(in, p)
					found = true
					break
				}
			}
			if !found {
				return Run{}, store.ErrNotFound
			}
			if err != nil {
				return Run{}, err
			}
		}
		results = append(results, engine.EvaluateSales(next, v))
	}
	contexts := engine.CalculateContexts(in, chosen)
	run := Run{ID: store.ID(), SchemaVersion: 2, StoreID: s.store.Scope(), Store: c.MustGet("profile").(engine.StoreProfile), Input: in, Result: contexts[0].Result, Contexts: contexts, Promotions: all, SalesScenarios: sales, SalesResults: results}
	if err = s.store.Put(ctx, "runs", run.ID, run); err != nil {
		return Run{}, err
	}
	return run, nil
}
func containsPromotion(rows []engine.PromotionScenario, id string) bool {
	for _, p := range rows {
		if p.ID == id {
			return true
		}
	}
	return false
}
func (s *Server) calculate(c *gin.Context) {
	var req calculationRequest
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("定价请求格式无效"))
		return
	}
	run, err := s.buildRun(c, req)
	if err != nil {
		if err == store.ErrNotFound {
			notFound(c)
		} else {
			bad(c, err)
		}
		return
	}
	c.JSON(http.StatusOK, gin.H{"id": run.ID, "result": run.Result, "contexts": run.Contexts, "sales_results": run.SalesResults})
}
func (s *Server) evaluateSales(c *gin.Context) {
	var req calculationRequest
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("情景请求格式无效"))
		return
	}
	run, err := s.buildRun(c, req)
	if err != nil {
		if err == store.ErrNotFound {
			notFound(c)
		} else {
			bad(c, err)
		}
		return
	}
	c.JSON(200, gin.H{"id": run.ID, "evaluations": run.SalesResults})
}

func feeGroups(m map[string][]engine.Fee) [][]engine.Fee {
	out := [][]engine.Fee{}
	for _, v := range m {
		out = append(out, v)
	}
	return out
}
func (s *Server) validateFeeRefs(c *gin.Context, fees []engine.Fee) error {
	for _, f := range fees {
		for _, id := range f.RouteIDs {
			var q engine.Quote
			if s.store.Get(c.Request.Context(), "quotes", id, &q) != nil {
				return fmt.Errorf("费用引用了不属于当前店铺的渠道")
			}
		}
	}
	return nil
}
