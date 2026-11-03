package app

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"pricepilot/internal/alibaba"
	"pricepilot/internal/engine"
	"pricepilot/internal/external"
	"pricepilot/internal/frankfurter"
	"pricepilot/internal/integrations"
	"pricepilot/internal/seerfar"
	"pricepilot/internal/store"
)

func serviceSource(b integrations.Binding, at time.Time) *engine.ServiceSource {
	return &engine.ServiceSource{Provider: b.Provider, Environment: b.Environment, ProfileVersion: b.Version, ConfigVersion: b.ConfigVersion, FetchedAt: at, Demo: b.Environment == "mock"}
}
func integrationError(c *gin.Context, err error) {
	var e *external.Error
	if errors.As(err, &e) {
		c.JSON(502, gin.H{"error": e.Message, "kind": e.Kind})
		return
	}
	if errors.Is(err, integrations.ErrConflict) {
		c.JSON(409, gin.H{"error": err.Error(), "kind": "stale_configuration"})
		return
	}
	bad(c, err)
}
func (s *Server) getIntegrations(c *gin.Context) {
	v, err := s.services.View(c.Request.Context(), s.store.Scope())
	if err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"integrations": v})
}
func (s *Server) saveIntegrations(c *gin.Context) {
	var req integrations.Update
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("服务配置格式无效"))
		return
	}
	if req.Provider == "1688" && req.TestProductID != "" {
		id, err := alibaba.ParseOfferID(req.TestProductID)
		if err != nil {
			bad(c, err)
			return
		}
		req.TestProductID = id
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, err := s.services.Save(c.Request.Context(), s.store.Scope(), req); err != nil {
		integrationError(c, err)
		return
	}
	s.getIntegrations(c)
}
func (s *Server) testIntegration(c *gin.Context) {
	var req struct {
		Version   int    `json:"version"`
		ProductID string `json:"product_id"`
	}
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("测试参数无效"))
		return
	}
	r, err := s.services.Profile(c.Request.Context(), s.store.Scope(), c.Param("provider"), c.Param("environment"))
	if err != nil {
		bad(c, err)
		return
	}
	if req.Version != r.ConfigVersion {
		integrationError(c, integrations.ErrConflict)
		return
	}
	switch r.Provider {
	case "1688":
		if req.ProductID == "" {
			cfg, _ := s.services.Get(c.Request.Context(), s.store.Scope())
			req.ProductID = cfg.Services[r.Provider].Profiles[r.Environment].TestProductID
		}
		id, e := alibaba.ParseOfferID(req.ProductID)
		if e != nil {
			bad(c, fmt.Errorf("请输入用于只读测试的商品ID，测试不会建立铺货关系"))
			return
		}
		_, err = alibabaClient(r).Detail(c.Request.Context(), id)
	case "seerfar":
		_, err = seerfarClient(r).Quota(c.Request.Context())
	case "frankfurter":
		_, err = frankfurter.New(r.BaseURL, r.Credentials["bearer_token"]).Latest(c.Request.Context())
	}
	if err != nil {
		if _, ok := err.(*seerfar.Error); ok {
			sourceError(c, err)
		} else {
			integrationError(c, err)
		}
		return
	}
	current, e := s.services.Profile(c.Request.Context(), s.store.Scope(), r.Provider, r.Environment)
	if e != nil || current.Binding != r.Binding {
		integrationError(c, integrations.ErrConflict)
		return
	}
	c.JSON(200, gin.H{"ok": true, "message": "连接及只读接口调用成功"})
}
func alibabaClient(r integrations.Runtime) *alibaba.Client {
	return alibaba.New(r.BaseURL, r.Credentials["app_key"], r.Credentials["app_secret"], r.Credentials["access_token"])
}
func seerfarClient(r integrations.Runtime) *seerfar.Client {
	c := seerfar.New(r.BaseURL, r.Credentials["api_key"])
	c.HTTP = external.Client()
	return c
}
func (s *Server) currentSeerfar(c *gin.Context) (*seerfar.Client, integrations.Runtime, error) {
	r, err := s.services.Resolve(c.Request.Context(), s.store.Scope(), "seerfar")
	return seerfarClient(r), r, err
}

type ImportPreview struct {
	ID          string               `json:"id"`
	Binding     integrations.Binding `json:"binding"`
	OfferID     string               `json:"offer_id"`
	OriginalURL string               `json:"original_url"`
	FetchedAt   time.Time            `json:"fetched_at"`
	ExpiresAt   time.Time            `json:"expires_at"`
	Status      string               `json:"status"`
	Product     *alibaba.Product     `json:"product"`
}

func (s *Server) previewProduct(c *gin.Context) {
	var req struct {
		Input string `json:"input"`
	}
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("请输入1688商品链接或ID"))
		return
	}
	id, err := alibaba.ParseOfferID(req.Input)
	if err != nil {
		bad(c, err)
		return
	}
	r, err := s.services.Resolve(c.Request.Context(), s.store.Scope(), "1688")
	if err != nil {
		internal(c)
		return
	}
	p, err := alibabaClient(r).Detail(c.Request.Context(), id)
	status := "ready"
	if e, ok := err.(*external.Error); ok && e.Kind == "relation_required" {
		status = "relation_required"
	} else if err != nil {
		integrationError(c, err)
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if !s.services.Current(c.Request.Context(), s.store.Scope(), r.Binding) {
		integrationError(c, integrations.ErrConflict)
		return
	}
	now := time.Now().UTC()
	v := ImportPreview{ID: store.ID(), Binding: r.Binding, OfferID: id, OriginalURL: "https://detail.1688.com/offer/" + id + ".html", FetchedAt: now, ExpiresAt: now.Add(15 * time.Minute), Status: status}
	if original := strings.TrimSpace(req.Input); strings.HasPrefix(original, "http://") || strings.HasPrefix(original, "https://") {
		v.OriginalURL = original
	}
	if status == "ready" {
		v.Product = &p
	}
	if err = s.store.Put(c.Request.Context(), "import_previews", v.ID, v); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"preview": v})
}
func (s *Server) readImport(c *gin.Context, id string) (ImportPreview, bool) {
	var p ImportPreview
	if s.store.Get(c.Request.Context(), "import_previews", id, &p) != nil {
		notFound(c)
		return p, false
	}
	if time.Now().After(p.ExpiresAt) {
		c.JSON(409, gin.H{"error": "商品预览已超过15分钟，请重新获取"})
		return p, false
	}
	if !s.services.Current(c.Request.Context(), s.store.Scope(), p.Binding) {
		integrationError(c, integrations.ErrConflict)
		return p, false
	}
	return p, true
}
func (s *Server) establishRelation(c *gin.Context) {
	var req struct {
		PreviewID string `json:"preview_id"`
		Confirmed bool   `json:"confirmed"`
	}
	if c.ShouldBindJSON(&req) != nil || !req.Confirmed {
		bad(c, fmt.Errorf("请先确认在当前1688账号建立商品铺货关系"))
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	p, ok := s.readImport(c, req.PreviewID)
	if !ok {
		return
	}
	if p.Status != "relation_required" {
		bad(c, fmt.Errorf("此预览无需重复建立关系，请重新查询详情"))
		return
	}
	r, err := s.services.Resolve(c.Request.Context(), s.store.Scope(), "1688")
	if err != nil {
		internal(c)
		return
	}
	// Consume the confirmation before the write; uncertain outcomes require a fresh detail query.
	p.Status = "relation_attempted"
	if err = s.store.Put(c.Request.Context(), "import_previews", p.ID, p); err != nil {
		internal(c)
		return
	}
	if err = alibabaClient(r).EstablishRelation(c.Request.Context(), p.OfferID); err != nil {
		integrationError(c, err)
		return
	}
	product, err := alibabaClient(r).Detail(c.Request.Context(), p.OfferID)
	if err != nil {
		integrationError(c, err)
		return
	}
	p.Product = &product
	p.Status = "ready"
	p.FetchedAt = time.Now().UTC()
	p.ExpiresAt = p.FetchedAt.Add(15 * time.Minute)
	if err = s.store.Put(c.Request.Context(), "import_previews", p.ID, p); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"preview": p})
}

type loadProductRequest struct {
	PreviewID      string   `json:"preview_id"`
	SKUID          string   `json:"sku_id"`
	Quantity       int      `json:"quantity"`
	QuoteConfirmed bool     `json:"quote_confirmed"`
	Unit           string   `json:"unit"`
	UnitPrice      *float64 `json:"unit_price"`
	MOQ            int      `json:"moq"`
}

func (s *Server) loadProduct(c *gin.Context) {
	var req loadProductRequest
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("商品规格和数量格式无效"))
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	p, ok := s.readImport(c, req.PreviewID)
	if !ok {
		return
	}
	if p.Status != "ready" || p.Product == nil {
		bad(c, fmt.Errorf("请先获取可用商品详情"))
		return
	}
	q, err := p.Product.Quote(req.SKUID, req.Quantity)
	if err != nil {
		bad(c, err)
		return
	}
	unit, moq := p.Product.Unit, p.Product.MOQ
	req.Unit = strings.TrimSpace(req.Unit)
	if q.NeedsConfirmation {
		if !req.QuoteConfirmed || req.Unit == "" || req.UnitPrice == nil || *req.UnitPrice <= 0 || *req.UnitPrice > 1000000 || req.MOQ < 1 || req.MOQ < p.Product.MOQ || req.Quantity < req.MOQ {
			bad(c, fmt.Errorf("报价或计价单位不明确，请确认每个销售单位的人民币采购价、单位及起订量"))
			return
		}
		q.Price = req.UnitPrice
		q.Tiers = nil
		unit = req.Unit
		moq = req.MOQ
	}
	if q.Price == nil {
		bad(c, fmt.Errorf("采购价待补充"))
		return
	}
	product := blankProduct(s.store.Scope())
	product.Name = p.Product.Title
	product.Demo = p.Binding.Environment == "mock"
	product.Supplier = p.Product.Supplier
	product.PurchaseQuantity = req.Quantity
	product.MOQ = moq
	product.PurchaseTiers = q.Tiers
	product.Weight = p.Product.Weight
	product.Length = p.Product.Length
	product.Width = p.Product.Width
	product.Height = p.Product.Height
	product.AlibabaCategory = p.Product.CategoryID
	product.Attributes = p.Product.Attributes
	if product.Attributes == nil {
		product.Attributes = map[string]string{}
	}
	var specID string
	sourceSKUID := req.SKUID
	for _, sku := range p.Product.SKUs {
		if sku.ID == req.SKUID {
			if sku.Default {
				sourceSKUID = ""
			}
			specID = sku.SpecID
			for k, v := range sku.Attributes {
				product.Attributes[k] = v
			}
		}
	}
	product.Keywords = []string{p.Product.Title}
	data, _ := json.Marshal(p.Product)
	product.Sourcing = &engine.ProcurementSource{ServiceSource: *serviceSource(p.Binding, p.FetchedAt), OfferID: p.OfferID, SKUID: sourceSKUID, SpecID: specID, OriginalURL: p.OriginalURL, Unit: unit, Quantity: req.Quantity, Price: q.Price, Confirmed: req.QuoteConfirmed, Data: data}
	for i := range product.Costs {
		if product.Costs[i].Code == "procurement" {
			product.Costs[i].Amount = q.Price
			product.Costs[i].Status = "confirmed"
			product.Costs[i].Source = "1688商品详情 · " + p.OfferID
			if sourceSKUID != "" {
				product.Costs[i].Source += " · SKU " + sourceSKUID
			}
		}
	}
	var settings engine.Settings
	if s.store.Get(c.Request.Context(), "settings", "default", &settings) == nil {
		for _, mapping := range settings.CategoryMappings {
			if mapping.Alibaba == product.AlibabaCategory {
				product.CategoryID = mapping.Seerfar
				product.CommissionCategory = mapping.Ozon
				break
			}
		}
	}
	c.JSON(200, gin.H{"product": product})
}
func blankProduct(storeID string) engine.Product {
	p, _, _, _ := demo(time.Now().UTC())
	p.ID = store.ID()
	p.StoreID = storeID
	p.Version = 0
	p.Name = ""
	p.Demo = false
	p.Supply = "stock"
	p.Supplier = ""
	p.SupplierScore = nil
	p.SupplierSource = ""
	p.SupplierScale = 5
	p.PurchaseQuantity = 1
	p.MOQ = 1
	p.PurchaseTiers = nil
	p.Weight = 0
	p.Length = 0
	p.Width = 0
	p.Height = 0
	p.Batteries = false
	p.Liquids = false
	p.SupplierDays = nil
	p.DomesticDays = nil
	p.TailDays = nil
	p.WarehouseQuoteID = "self"
	p.RouteIDs = nil
	p.Discount = 0
	p.FixedDiscount = 0
	p.PlatformDiscount = engine.Discount{Kind: "none", Currency: "CNY"}
	p.PriceCondition = "standard"
	p.Keywords = nil
	p.CategoryID = ""
	p.AlibabaCategory = ""
	p.CommissionCategory = ""
	p.Attributes = map[string]string{}
	p.OwnSellerIDs = nil
	p.SeedSKU = ""
	p.Mode = "profit"
	p.Options = engine.ModeOptions{Margin: .2, TargetScore: 80}
	p.Returns = engine.Returns{}
	for i := range p.Costs {
		f := &p.Costs[i]
		*f = engine.Fee{Code: f.Code, Name: f.Name, Enabled: true, Kind: "fixed", Currency: "CNY", Basis: "sale", On: "shipped", Status: "pending", Source: "商家录入"}
	}
	return p
}

type FXPreview struct {
	ID              string               `json:"id"`
	Binding         integrations.Binding `json:"binding"`
	SettingsVersion int                  `json:"settings_version"`
	FetchedAt       time.Time            `json:"fetched_at"`
	ExpiresAt       time.Time            `json:"expires_at"`
	Rate            frankfurter.Rate     `json:"rate"`
}

func (s *Server) previewFX(c *gin.Context) {
	r, err := s.services.Resolve(c.Request.Context(), s.store.Scope(), "frankfurter")
	if err != nil {
		internal(c)
		return
	}
	var settings engine.Settings
	if s.store.Get(c.Request.Context(), "settings", "default", &settings) != nil {
		internal(c)
		return
	}
	rate, err := frankfurter.New(r.BaseURL, r.Credentials["bearer_token"]).Latest(c.Request.Context())
	if err != nil {
		integrationError(c, err)
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if !s.services.Current(c.Request.Context(), s.store.Scope(), r.Binding) {
		integrationError(c, integrations.ErrConflict)
		return
	}
	now := time.Now().UTC()
	p := FXPreview{ID: store.ID(), Binding: r.Binding, SettingsVersion: settings.Version, FetchedAt: now, ExpiresAt: now.Add(15 * time.Minute), Rate: rate}
	if s.store.Put(c.Request.Context(), "fx_previews", p.ID, p) != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"preview": p})
}
func (s *Server) applyFX(c *gin.Context) {
	var req struct {
		PreviewID string `json:"preview_id"`
	}
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("汇率预览编号无效"))
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	var p FXPreview
	if s.store.Get(c.Request.Context(), "fx_previews", req.PreviewID, &p) != nil {
		notFound(c)
		return
	}
	if time.Now().After(p.ExpiresAt) || !s.services.Current(c.Request.Context(), s.store.Scope(), p.Binding) {
		integrationError(c, integrations.ErrConflict)
		return
	}
	var settings engine.Settings
	if s.store.Get(c.Request.Context(), "settings", "default", &settings) != nil {
		internal(c)
		return
	}
	if settings.Version != p.SettingsVersion {
		c.JSON(409, gin.H{"error": "店铺设置已变化，请重新获取汇率后应用"})
		return
	}
	settings.FX = p.Rate.Rate
	settings.FXDate = p.Rate.Date
	settings.FXSource = "Frankfurter综合参考汇率（非Ozon合同汇率）"
	settings.FXMeta = serviceSource(p.Binding, p.FetchedAt)
	settings.Version++
	if err := s.versioned(c, "settings", "default", settings.Version, settings); err != nil {
		internal(c)
		return
	}
	c.JSON(200, gin.H{"settings": settings})
}
