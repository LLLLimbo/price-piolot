package app

import (
	"encoding/json"
	"fmt"
	"github.com/gin-gonic/gin"
	"net/http"
	"os"
	"path/filepath"
	"pricepilot/internal/engine"
	"pricepilot/internal/seerfar"
	"pricepilot/internal/store"
	"strings"
	"sync"
	"time"
)

type Server struct {
	store   *store.Store
	root    *store.Store
	seerfar *seerfar.Client
	mock    bool
	mu      *sync.Mutex
}
type Run struct {
	ID             string                     `json:"id"`
	SchemaVersion  int                        `json:"schema_version"`
	StoreID        string                     `json:"store_id"`
	Store          engine.StoreProfile        `json:"store"`
	Contexts       []engine.ContextResult     `json:"contexts"`
	Promotions     []engine.PromotionScenario `json:"promotions"`
	SalesScenarios []engine.SalesScenario     `json:"sales_scenarios"`
	SalesResults   []engine.SalesEvaluation   `json:"sales_results"`
	Input          engine.Input               `json:"input"`
	Result         engine.Result              `json:"result"`
}

func New(db *store.Store, client *seerfar.Client, static string, mock bool) http.Handler {
	gin.SetMode(gin.ReleaseMode)
	r := gin.New()
	r.Use(gin.Recovery())
	r.Use(func(c *gin.Context) {
		c.Request.Body = http.MaxBytesReader(c.Writer, c.Request.Body, 2<<20)
		c.Header("X-Content-Type-Options", "nosniff")
		c.Next()
	})
	s := &Server{store: db, root: db, seerfar: client, mock: mock, mu: &sync.Mutex{}}
	r.GET("/api/health", func(c *gin.Context) {
		if err := db.Ping(c.Request.Context()); err != nil {
			c.JSON(503, gin.H{"error": "数据库不可用"})
			return
		}
		c.JSON(200, gin.H{"status": "ok"})
	})
	r.GET("/api/stores", s.listStores)
	r.POST("/api/stores", s.saveStore)
	r.PUT("/api/stores/:store_id", s.saveStore)
	r.GET("/api/public-quotes", s.publicCatalog)
	g := r.Group("/api/stores/:store_id")
	g.Use(func(c *gin.Context) {
		var profile engine.StoreProfile
		if err := db.Get(c.Request.Context(), "stores", c.Param("store_id"), &profile); err != nil {
			notFound(c)
			c.Abort()
			return
		}
		if profile.Archived && c.Request.Method != "GET" {
			c.JSON(409, gin.H{"error": "店铺已归档，请恢复后编辑或测算"})
			c.Abort()
			return
		}
		c.Set("profile", profile)
	})
	wrap := func(h func(*Server, *gin.Context)) gin.HandlerFunc {
		return func(c *gin.Context) { scoped := *s; scoped.store = db.ForStore(c.Param("store_id")); h(&scoped, c) }
	}
	g.GET("/bootstrap", wrap((*Server).bootstrap))
	g.GET("/seerfar/quota", wrap((*Server).quota))
	g.PUT("/settings", wrap((*Server).settings))
	g.POST("/products", wrap((*Server).saveProduct))
	g.PUT("/products/:id", wrap((*Server).saveProduct))
	g.DELETE("/products/:id", wrap((*Server).deleteProduct))
	g.POST("/quotes", wrap((*Server).saveQuote))
	g.PUT("/quotes/:id", wrap((*Server).saveQuote))
	g.POST("/quotes/copy-public", wrap((*Server).copyPublic))
	g.GET("/quotes/:id/versions", wrap((*Server).quoteVersions))
	g.POST("/promotions", wrap((*Server).savePromotion))
	g.PUT("/promotions/:id", wrap((*Server).savePromotion))
	g.DELETE("/promotions/:id", wrap((*Server).deletePromotion))
	g.POST("/sales-scenarios", wrap((*Server).saveSales))
	g.PUT("/sales-scenarios/:id", wrap((*Server).saveSales))
	g.DELETE("/sales-scenarios/:id", wrap((*Server).deleteSales))
	g.POST("/sales-scenarios/evaluate", wrap((*Server).evaluateSales))
	g.POST("/competitors/discover", wrap((*Server).discover))
	g.POST("/snapshots", wrap((*Server).newSnapshot))
	g.PUT("/snapshots/:id", wrap((*Server).updateSnapshot))
	g.POST("/calculate", wrap((*Server).calculate))
	g.POST("/demo", wrap((*Server).loadDemo))
	g.GET("/runs/:id", wrap((*Server).getRun))

	if static != "" {
		r.NoRoute(func(c *gin.Context) {
			if strings.HasPrefix(c.Request.URL.Path, "/api/") {
				notFound(c)
				return
			}
			if c.Request.Method != "GET" {
				notFound(c)
				return
			}
			path := filepath.Join(static, filepath.Clean("/"+c.Request.URL.Path))
			if info, err := os.Stat(path); err == nil && !info.IsDir() {
				c.File(path)
				return
			}
			c.File(filepath.Join(static, "index.html"))
		})
	} else {
		r.NoRoute(notFound)
	}
	return r
}
func internal(c *gin.Context)       { c.JSON(500, gin.H{"error": "数据保存或读取失败，请重试"}) }
func notFound(c *gin.Context)       { c.JSON(404, gin.H{"error": "记录不存在"}) }
func bad(c *gin.Context, err error) { c.JSON(400, gin.H{"error": err.Error()}) }
func sourceError(c *gin.Context, err error) {
	if e, ok := err.(*seerfar.Error); ok {
		c.JSON(502, gin.H{"error": e.Message, "kind": e.Kind, "code": e.Code})
		return
	}
	c.JSON(502, gin.H{"error": "竞品数据获取失败"})
}
func (s *Server) bootstrap(c *gin.Context) {
	ctx := c.Request.Context()
	var settings engine.Settings
	if err := s.store.Get(ctx, "settings", "default", &settings); err != nil {
		internal(c)
		return
	}
	out := gin.H{"settings": settings, "seerfar": gin.H{"configured": s.seerfar.Configured(), "mock": s.mock}}
	template, _, _, _ := demo(time.Now().UTC())
	template.StoreID = s.store.Scope()
	out["template"] = template
	out["store"] = c.MustGet("profile")
	out["public_quotes"], _ = s.root.List(ctx, "public_quotes")
	for _, kind := range []string{"products", "quotes", "snapshots", "promotions", "sales_scenarios"} {
		rows, err := s.store.List(ctx, kind)
		if err != nil {
			internal(c)
			return
		}
		out[kind] = rows
	}
	runs, err := s.store.List(ctx, "runs")
	if err != nil {
		internal(c)
		return
	}
	summaries := []gin.H{}
	for _, b := range runs {
		var v Run
		if json.Unmarshal(b, &v) == nil {
			summaries = append(summaries, gin.H{"id": v.ID, "product": v.Input.Product.Name, "product_id": v.Input.Product.ID, "mode": v.Result.Mode, "calculated_at": v.Result.CalculatedAt, "recommended": v.Result.Recommended, "status": v.Result.Status, "demo": v.Result.Demo, "legacy": v.SchemaVersion < 2, "context_count": len(v.Contexts)})
		}
		if len(summaries) >= 50 {
			break
		}
	}
	out["runs"] = summaries
	c.JSON(200, out)
}
func (s *Server) settings(c *gin.Context) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var settings engine.Settings
	if err := c.ShouldBindJSON(&settings); err != nil {
		bad(c, fmt.Errorf("设置格式无效"))
		return
	}
	if settings.StoreID != "" && settings.StoreID != s.store.Scope() {
		bad(c, fmt.Errorf("设置不属于当前店铺"))
		return
	}
	settings.StoreID = s.store.Scope()
	settings.Version = s.nextVersion(c, "settings", "default")
	for i := range settings.Fees {
		settings.Fees[i].Version = settings.Version
		if settings.Fees[i].Status == "" {
			settings.Fees[i].Status = "pending"
		}
	}
	for cat, fees := range settings.CategoryFees {
		for i := range fees {
			fees[i].Version = settings.Version
			if fees[i].Status == "" {
				fees[i].Status = "pending"
			}
		}
		settings.CategoryFees[cat] = fees
	}
	for _, fs := range append([][]engine.Fee{settings.Fees}, feeGroups(settings.CategoryFees)...) {
		if err := s.validateFeeRefs(c, fs); err != nil {
			bad(c, err)
			return
		}
	}
	p, _, snap, qs := demo(time.Now().UTC())
	if err := engine.Validate(engine.Input{Product: p, Settings: settings, Snapshot: snap, Quotes: qs, AsOf: time.Now().UTC()}); err != nil {
		bad(c, err)
		return
	}
	for _, fees := range settings.CategoryFees {
		for _, f := range fees {
			if f.Enabled {
				check := f
				if check.Amount == nil {
					check.Amount = engine.Ptr(0)
				}
				if check.Rate == nil {
					check.Rate = engine.Ptr(0)
				}
				if err := engine.ValidateFee(check); err != nil {
					bad(c, err)
					return
				}
			}
		}
	}
	if err := s.versioned(c, "settings", "default", settings.Version, settings); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"settings": settings})
}
func (s *Server) saveProduct(c *gin.Context) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var p engine.Product
	if err := c.ShouldBindJSON(&p); err != nil {
		bad(c, fmt.Errorf("商品格式无效"))
		return
	}
	if c.Param("id") != "" {
		p.ID = c.Param("id")
	}
	if p.ID == "" {
		p.ID = store.ID()
	}
	if err := s.bindProduct(c, &p); err != nil {
		bad(c, err)
		return
	}
	if strings.TrimSpace(p.Name) == "" {
		bad(c, fmt.Errorf("请输入商品名称"))
		return
	}
	p.Version = s.nextVersion(c, "products", p.ID)
	for i := range p.Costs {
		if p.Costs[i].Status == "" {
			p.Costs[i].Status = "pending"
		}
		p.Costs[i].Version = p.Version
	}

	p.UpdatedAt = time.Now().UTC()
	if err := s.versioned(c, "products", p.ID, p.Version, p); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"product": p})
}
func (s *Server) saveQuote(c *gin.Context) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var q engine.Quote
	if err := c.ShouldBindJSON(&q); err != nil {
		bad(c, fmt.Errorf("报价格式无效"))
		return
	}
	if c.Param("id") != "" {
		q.ID = c.Param("id")
	}
	if q.ID == "" {
		q.ID = store.ID()
	}
	if q.StoreID != "" && q.StoreID != s.store.Scope() {
		bad(c, fmt.Errorf("报价不属于当前店铺"))
		return
	}
	q.StoreID = s.store.Scope()
	q.Version = s.nextVersion(c, "quotes", q.ID)
	for i := range q.Fees {
		q.Fees[i].Version = q.Version
		if q.Fees[i].Status == "" {
			q.Fees[i].Status = "pending"
		}
	}
	if q.PublicID != "" {
		var ref engine.Quote
		if s.root.Get(c.Request.Context(), "public_quotes", q.PublicID, &ref) != nil {
			bad(c, fmt.Errorf("公共报价资料不存在"))
			return
		}
		if ref.Status == "excluded" && q.Status == "active" {
			bad(c, fmt.Errorf("该公开资料明确排除Ozon，不能启用"))
			return
		}
		q.RequiredFees = ref.RequiredFees
	}
	var old engine.Quote
	if err := s.store.Get(c.Request.Context(), "quotes", q.ID, &old); err == nil && old.Status == "excluded" && q.Status != "excluded" {
		bad(c, fmt.Errorf("该公开方案明确排除Ozon订单，不能启用；如有另行签署的Ozon合同，请新建协议报价"))
		return
	}
	if err := engine.ValidateQuote(q); err != nil {
		bad(c, err)
		return
	}
	if err := s.versioned(c, "quotes", q.ID, q.Version, q); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"quote": q})
}
func (s *Server) discover(c *gin.Context) {
	var req struct {
		Product    engine.Product `json:"product"`
		PreviousID string         `json:"previous_id"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		bad(c, fmt.Errorf("商品查询格式无效"))
		return
	}
	if err := s.bindProduct(c, &req.Product); err != nil {
		bad(c, err)
		return
	}
	if req.PreviousID != "" {
		var previous engine.Snapshot
		if s.store.Get(c.Request.Context(), "snapshots", req.PreviousID, &previous) != nil {
			notFound(c)
			return
		}
	}
	queryProduct := req.Product
	queryProduct.OwnSellerIDs = append(append([]string{}, req.Product.OwnSellerIDs...), s.ownedSellerIDs(c)...)
	rows, err := s.seerfar.Discover(c.Request.Context(), queryProduct)
	if err != nil {
		sourceError(c, err)
		return
	}
	if req.PreviousID != "" {
		var prev engine.Snapshot
		if s.store.Get(c.Request.Context(), "snapshots", req.PreviousID, &prev) == nil && prev.Fingerprint == engine.Fingerprint(req.Product) {
			for i, row := range rows {
				for _, old := range prev.Competitors {
					if old.SKU == row.SKU && old.Verified && row.PriceRUB > 0 && !strings.Contains(row.Reason, "不一致") && !contains(queryProduct.OwnSellerIDs, row.SellerID) {
						rows[i].Include = old.Include
						rows[i].Verified = true
						rows[i].Pinned = old.Pinned
						rows[i].Delivery = old.Delivery
						rows[i].DeliverySource = old.DeliverySource
						rows[i].Estimated = old.Estimated
					}
				}
			}
			for _, old := range prev.Competitors {
				if !old.Pinned {
					continue
				}
				found := false
				for _, row := range rows {
					if row.SKU == old.SKU {
						found = true
						break
					}
				}
				if !found {
					old.Include = false
					old.Verified = false
					old.Confidence = 0
					old.Reason = "固定竞品本次未返回，保留旧记录待核实"
					rows = append(rows, old)
				}
			}
		}
	}
	for i := range rows {
		if rows[i].PriceSource == "" {
			rows[i].PriceSource = "Seerfar商品展示价"
		}
	}
	snap := engine.Snapshot{StoreID: s.store.Scope(), ID: store.ID(), ProductID: req.Product.ID, Fingerprint: engine.Fingerprint(req.Product), Source: "seerfar", Demo: s.mock, FetchedAt: time.Now().UTC(), Competitors: rows, Warnings: []string{}}
	if len(rows) == 0 {
		snap.Warnings = append(snap.Warnings, "Seerfar未返回匹配商品，可修改关键词、SKU或手动添加竞品")
	}
	if err = s.store.Put(c.Request.Context(), "snapshots", snap.ID, snap); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"snapshot": snap})
}
func contains(v []string, s string) bool {
	for _, x := range v {
		if x == s {
			return true
		}
	}
	return false
}
func validateSnapshot(snap engine.Snapshot) error {
	seen := map[string]bool{}
	for _, c := range snap.Competitors {
		if c.SKU == "" || seen[c.SKU] || c.PriceRUB <= 0 || c.PriceRUB > 1e10 {
			return fmt.Errorf("竞品SKU不可为空或重复，价格须大于0")
		}
		seen[c.SKU] = true
		if c.Delivery != nil && (c.Delivery.Min < 0 || c.Delivery.Max < c.Delivery.Min || c.DeliverySource == "") {
			return fmt.Errorf("竞品时效需要有效区间和来源")
		}
	}
	return nil
}
func (s *Server) newSnapshot(c *gin.Context) {
	var req struct {
		Product     engine.Product      `json:"product"`
		Competitors []engine.Competitor `json:"competitors"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		bad(c, fmt.Errorf("竞品格式无效"))
		return
	}
	if err := s.bindProduct(c, &req.Product); err != nil {
		bad(c, err)
		return
	}
	snap := engine.Snapshot{StoreID: s.store.Scope(), ID: store.ID(), ProductID: req.Product.ID, Fingerprint: engine.Fingerprint(req.Product), Source: "manual", Demo: req.Product.Demo, FetchedAt: time.Now().UTC(), Competitors: req.Competitors, Warnings: []string{}}
	if err := validateSnapshot(snap); err != nil {
		bad(c, err)
		return
	}
	if err := s.store.Put(c.Request.Context(), "snapshots", snap.ID, snap); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"snapshot": snap})
}
func (s *Server) updateSnapshot(c *gin.Context) {
	var old, snap engine.Snapshot
	if err := s.store.Get(c.Request.Context(), "snapshots", c.Param("id"), &old); err != nil {
		notFound(c)
		return
	}
	if err := c.ShouldBindJSON(&snap); err != nil {
		bad(c, fmt.Errorf("快照格式无效"))
		return
	}
	if snap.StoreID != "" && snap.StoreID != s.store.Scope() {
		bad(c, fmt.Errorf("快照不属于当前店铺"))
		return
	}
	snap.StoreID = s.store.Scope()
	snap.ID = old.ID
	snap.ProductID = old.ProductID
	snap.FetchedAt = old.FetchedAt
	snap.Fingerprint = old.Fingerprint
	snap.Source = old.Source
	snap.Demo = old.Demo
	if err := validateSnapshot(snap); err != nil {
		bad(c, err)
		return
	}
	if err := s.store.Put(c.Request.Context(), "snapshots", snap.ID, snap); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"snapshot": snap})
}
