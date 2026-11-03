package alibaba

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"testing"
)

func TestOfficialSignatureVector(t *testing.T) {
	p := url.Values{"access_token": {"test-token"}, "productIdList": {"[1002]"}, "_aop_signature": {"ignored"}}
	if got := Sign("param2/1/com.alibaba.product.push/alibaba.cross.syncProductListPushed/test-app", p, "test-secret"); got != "A1D0359CB9BDDCE628F6BF8F32D04A4DAE43D511" {
		t.Fatalf("%s", got)
	}
}

func TestParseOfferID(t *testing.T) {
	for input, want := range map[string]string{
		"https://detail.1688.com/offer/1049760039236.html":                                  "1049760039236",
		"https://detail.1688.com/offer/688418910404.html?spm=x&spm-auction=1049760039236#x": "688418910404",
		"9223372036854775807": "9223372036854775807",
		" 1001 ":              "1001",
	} {
		got, err := ParseOfferID(input)
		if err != nil || got != want {
			t.Fatalf("%q: %q %v", input, got, err)
		}
	}
	for _, input := range []string{"0", "-12", "9223372036854775808", "https://detail.1688.com.evil.test/offer/1001.html", "https://user@detail.1688.com/offer/1001.html", "https://detail.1688.com/offer/1001.html或者https://detail.1688.com/offer/1002.html", "1001 1002", "https://1688.com/offer/1001.html"} {
		if _, err := ParseOfferID(input); err == nil {
			t.Fatalf("accepted %q", input)
		}
	}
}

const fixture = `{"success":true,"productInfo":{"productID":9007199254740993,"subject":"陶瓷杯","status":"published","categoryID":12,"supplierLoginId":"supplier","qualityLevel":5,"saleInfo":{"quoteType":3,"unit":"个","minOrderQuantity":2},"shippingInfo":{"unitWeight":"0.25","length":12,"width":10,"height":8,"suttleWeight":0.1},"skuInfos":[{"skuId":9007199254740994,"specId":"white","attributes":[{"attributeDisplayName":"颜色","attributeValue":"白色"}],"price":99,"priceRange":[{"startQuantity":2,"price":"12.50"},{"startQuantity":10,"price":11}],"amountOnSale":50}]}}`

func TestDetailSignsAndNormalizesContract(t *testing.T) {
	up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		r.ParseForm()
		if r.URL.Path != "/openapi/param2/1/com.alibaba.product/alibaba.cross.productInfo/test-app" || r.Form.Get("productId") != "9007199254740993" || r.Form.Get("access_token") != "test-token" {
			t.Errorf("bad request %s %v", r.URL.Path, r.Form)
		}
		if r.Form.Get("_aop_signature") != Sign("param2/1/com.alibaba.product/alibaba.cross.productInfo/test-app", r.Form, "test-secret") {
			t.Error("invalid signature")
		}
		w.Write([]byte(fixture))
	}))
	defer up.Close()
	p, err := New(up.URL, "test-app", "test-secret", "test-token").Detail(t.Context(), "9007199254740993")
	if err != nil {
		t.Fatal(err)
	}
	if p.ID != "9007199254740993" || p.SKUs[0].ID != "9007199254740994" || p.Weight != 250 || p.Width != 10 {
		t.Fatalf("bad conversion %+v", p)
	}
	q, err := p.Quote(p.SKUs[0].ID, 10)
	if err != nil || (q.Price == nil || *q.Price != 11) || len(q.Tiers) != 2 || !q.KnownUnit {
		t.Fatalf("wrong quote %+v %v", q, err)
	}
	if _, err := p.Quote(p.SKUs[0].ID, 1); err == nil {
		t.Fatal("MOQ ignored")
	}
	if _, err := p.Quote(p.SKUs[0].ID, 51); err == nil {
		t.Fatal("stock ignored")
	}
	if _, err := p.Quote("foreign", 10); err == nil {
		t.Fatal("foreign sku")
	}
}

func TestDetailErrorKindsAndExplicitRelation(t *testing.T) {
	for _, tc := range []struct{ message, kind string }{{"没有该商品的查询权限", "relation_required"}, {"invalid access token", "authentication"}, {"ACCESS_DENIED", "permission"}} {
		t.Run(tc.kind, func(t *testing.T) {
			up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				json.NewEncoder(w).Encode(map[string]any{"error_code": tc.message, "error_message": tc.message})
			}))
			defer up.Close()
			_, err := New(up.URL, "key", "secret", "token").Detail(t.Context(), "1002")
			if e, ok := err.(*Error); !ok || e.Kind != tc.kind {
				t.Fatalf("%v", err)
			}
		})
	}
	up := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		r.ParseForm()
		if r.Form.Get("productIdList") != "[1002]" {
			t.Error("wrong relation payload")
		}
		w.Write([]byte(`{"result":{"success":false,"errorCode":"ACCESS_DENIED"}}`))
	}))
	defer up.Close()
	if err := New(up.URL, "key", "secret", "token").EstablishRelation(t.Context(), "1002"); err == nil {
		t.Fatal("nested business failure accepted")
	}
}

func TestMissingPriceAndUnitAreNotFabricated(t *testing.T) {
	p, err := Normalize(json.RawMessage(`{"productID":1001,"subject":"x","status":"published","saleInfo":{"quoteType":1},"skuInfos":[{"skuId":1,"consignPrice":1,"retailPrice":2}],"shippingInfo":{"suttleWeight":2}}`), "1001")
	if err != nil {
		t.Fatal(err)
	}
	q, err := p.Quote("1", 1)
	if err != nil || q.Price != nil || q.KnownUnit || p.Weight != 0 {
		t.Fatalf("invented data %+v %+v %v", p, q, err)
	}
}

func TestFixedOfferWithoutSKUDoesNotInventSKUID(t *testing.T) {
	p, err := Normalize(json.RawMessage(`{"productID":1001,"subject":"固定报价","saleInfo":{"quoteType":0,"unit":"件","minOrderQuantity":2,"priceRanges":[{"startQuantity":2,"price":20}]}}`), "1001")
	if err != nil || len(p.SKUs) != 1 || !p.SKUs[0].Default || p.SKUs[0].ID != "default" {
		t.Fatalf("%+v %v", p, err)
	}
	q, err := p.Quote("default", 2)
	if err != nil || q.Price == nil || *q.Price != 20 || q.NeedsConfirmation {
		t.Fatalf("%+v %v", q, err)
	}
}
