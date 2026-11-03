package app

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"pricepilot/internal/engine"
	"pricepilot/internal/integrations"
	"strings"
	"testing"
	"time"
)

func setService(t *testing.T, h http.Handler, storeID, provider, base string) {
	t.Helper()
	root := "/api/stores/" + storeID
	code, v := call(t, h, "GET", root+"/integrations", nil)
	if code != 200 {
		t.Fatal(code)
	}
	var cfg integrations.Config
	json.Unmarshal(v["integrations"], &cfg)
	code, v = call(t, h, "PUT", root+"/integrations", map[string]any{"provider": provider, "environment": "mock", "version": cfg.Version, "base_url": base, "activate": true})
	if code != 200 {
		t.Fatalf("save %d %s", code, v["error"])
	}
}
func TestFXPreviewApplyAndStaleVersions(t *testing.T) {
	h, db := testServer(t)
	up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte(`{"date":"2026-10-30","base":"CNY","quote":"RUB","rate":12.0844}`))
	}))
	defer up.Close()
	setService(t, h, "default", "frankfurter", up.URL)
	var before engine.Settings
	db.ForStore("default").Get(t.Context(), "settings", "default", &before)
	code, v := call(t, h, "POST", "/api/stores/default/exchange-rates/preview", map[string]any{})
	if code != 200 {
		t.Fatalf("%d %s", code, v["error"])
	}
	var p FXPreview
	json.Unmarshal(v["preview"], &p)
	var same engine.Settings
	db.ForStore("default").Get(t.Context(), "settings", "default", &same)
	if same.FX != before.FX || same.Version != before.Version {
		t.Fatal("preview wrote settings")
	}
	if code, _ = call(t, h, "POST", "/api/stores/demo-second/exchange-rates/apply", map[string]any{"preview_id": p.ID}); code != 404 {
		t.Fatal("cross-store fx accepted", code)
	}
	code, v = call(t, h, "POST", "/api/stores/default/exchange-rates/apply", map[string]any{"preview_id": p.ID})
	var after engine.Settings
	json.Unmarshal(v["settings"], &after)
	if code != 200 || after.FX != 12.0844 || after.FXDate != "2026-10-30" || after.Stress != before.Stress || !after.Demo || after.Fees[0].Version != before.Fees[0].Version || after.FXMeta == nil || !after.FXMeta.Demo {
		t.Fatalf("%d %+v", code, after)
	}
	if code, _ = call(t, h, "POST", "/api/stores/default/exchange-rates/apply", map[string]any{"preview_id": p.ID}); code != 409 {
		t.Fatal("reapplied stale preview", code)
	}
	code, v = call(t, h, "POST", "/api/stores/default/exchange-rates/preview", map[string]any{})
	json.Unmarshal(v["preview"], &p)
	setService(t, h, "default", "frankfurter", up.URL)
	if code, _ = call(t, h, "POST", "/api/stores/default/exchange-rates/apply", map[string]any{"preview_id": p.ID}); code != 409 {
		t.Fatal("stale profile accepted", code)
	}
}

func TestFXSettingsEditAndPreviewExpiry(t *testing.T) {
	h, db := testServer(t)
	up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte(`{"date":"2026-10-30","base":"CNY","quote":"RUB","rate":12.0844}`))
	}))
	defer up.Close()
	setService(t, h, "default", "frankfurter", up.URL)
	_, v := call(t, h, "POST", "/api/stores/default/exchange-rates/preview", map[string]any{})
	var p FXPreview
	json.Unmarshal(v["preview"], &p)
	var settings engine.Settings
	db.ForStore("default").Get(t.Context(), "settings", "default", &settings)
	settings.Weights = engine.Weights{Price: 40, Delivery: 40, Supplier: 20}
	code, out := call(t, h, "PUT", "/api/stores/default/settings", settings)
	if code != 200 {
		t.Fatalf("%d %s", code, out)
	}
	if code, _ = call(t, h, "POST", "/api/stores/default/exchange-rates/apply", map[string]any{"preview_id": p.ID}); code != 409 {
		t.Fatal("concurrent settings overwritten")
	}
	if code, _ = call(t, h, "PUT", "/api/stores/default/settings", settings); code != 409 {
		t.Fatal("stale settings overwrite accepted")
	}
	_, v = call(t, h, "POST", "/api/stores/default/exchange-rates/preview", map[string]any{})
	json.Unmarshal(v["preview"], &p)
	p.ExpiresAt = time.Now().Add(-time.Minute)
	db.ForStore("default").Put(t.Context(), "fx_previews", p.ID, p)
	if code, _ = call(t, h, "POST", "/api/stores/default/exchange-rates/apply", map[string]any{"preview_id": p.ID}); code != 409 {
		t.Fatal("expired fx applied")
	}
}
func TestImportRelationDraftAndStoreBinding(t *testing.T) {
	h, db := testServer(t)
	related := false
	writes := 0
	up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		r.ParseForm()
		if strings.Contains(r.URL.Path, "syncProductListPushed") {
			related = true
			writes++
			w.Write([]byte(`{"result":{"success":true}}`))
			return
		}
		if !related {
			w.Write([]byte(`{"error_code":"NO_PERMISSION","error_message":"没有该商品的查询权限"}`))
			return
		}
		w.Write([]byte(`{"success":true,"productInfo":{"productID":1002,"subject":"陶瓷杯","status":"published","supplierLoginId":"seller","saleInfo":{"quoteType":1,"unit":"个","minOrderQuantity":2,"amountOnSale":50},"skuInfos":[{"skuId":1002,"specId":"sku-1002","price":"12.50","amountOnSale":50}],"shippingInfo":{"unitWeight":0.3}}}`))
	}))
	defer up.Close()
	setService(t, h, "default", "1688", up.URL)
	code, v := call(t, h, "POST", "/api/stores/default/sourcing/1688/preview", map[string]any{"input": "https://detail.1688.com/offer/1002.html?spm-auction=1001"})
	var p ImportPreview
	json.Unmarshal(v["preview"], &p)
	if code != 200 || p.Status != "relation_required" || writes != 0 {
		t.Fatalf("%d %s", code, v)
	}
	if code, _ = call(t, h, "POST", "/api/stores/default/sourcing/1688/establish-relation", map[string]any{"preview_id": p.ID, "confirmed": false}); code != 400 || writes != 0 {
		t.Fatal("unconfirmed write")
	}
	if code, _ = call(t, h, "POST", "/api/stores/demo-second/sourcing/1688/establish-relation", map[string]any{"preview_id": p.ID, "confirmed": true}); code != 404 || writes != 0 {
		t.Fatal("cross store relation")
	}
	code, v = call(t, h, "POST", "/api/stores/default/sourcing/1688/establish-relation", map[string]any{"preview_id": p.ID, "confirmed": true})
	json.Unmarshal(v["preview"], &p)
	if code != 200 || writes != 1 || p.Product == nil {
		t.Fatalf("%d %s", code, v)
	}
	before, _ := db.ForStore("default").List(t.Context(), "products")
	code, v = call(t, h, "POST", "/api/stores/default/sourcing/1688/load", map[string]any{"preview_id": p.ID, "sku_id": "1002", "quantity": 2})
	var product engine.Product
	json.Unmarshal(v["product"], &product)
	if code != 200 || product.Sourcing == nil || product.Sourcing.OfferID != "1002" || product.Weight != 300 || product.SupplierScore != nil || !product.Demo || product.Costs[0].Amount == nil || *product.Costs[0].Amount != 12.5 {
		t.Fatalf("%d %+v %s", code, product, v["error"])
	}
	if !strings.Contains(product.Sourcing.OriginalURL, "spm-auction=1001") {
		t.Fatal("original link not retained")
	}
	after, _ := db.ForStore("default").List(t.Context(), "products")
	if len(before) != len(after) {
		t.Fatal("import autosaved")
	}
	for _, f := range product.Costs {
		if f.Code != "procurement" && (f.Amount != nil || f.Rate != nil || f.Status != "pending") {
			t.Fatal("old costs inherited")
		}
	}
	setService(t, h, "default", "1688", up.URL)
	if code, _ = call(t, h, "POST", "/api/stores/default/sourcing/1688/load", map[string]any{"preview_id": p.ID, "sku_id": "1002", "quantity": 2}); code != 409 {
		t.Fatal("old config preview accepted", code)
	}
}
