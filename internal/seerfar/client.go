package seerfar

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"pricepilot/internal/engine"
	"strconv"
	"strings"
	"time"
)

type Error struct {
	Kind    string `json:"kind"`
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func (e *Error) Error() string { return e.Message }

type Client struct {
	BaseURL string
	Key     string
	HTTP    *http.Client
}

func New(url, key string) *Client {
	return &Client{strings.TrimRight(url, "/"), key, &http.Client{Timeout: 20 * time.Second}}
}
func (c *Client) Configured() bool { return c.BaseURL != "" && c.Key != "" }
func (c *Client) call(ctx context.Context, path string, input any, output any) error {
	if !c.Configured() {
		return &Error{Kind: "configuration", Message: "请配置Seerfar服务地址和API Key"}
	}
	var body io.Reader
	method := "GET"
	if input != nil {
		b, err := json.Marshal(input)
		if err != nil {
			return err
		}
		body = bytes.NewReader(b)
		method = "POST"
	}
	req, err := http.NewRequestWithContext(ctx, method, c.BaseURL+"/open-api/"+path, body)
	if err != nil {
		return &Error{Kind: "configuration", Message: "Seerfar服务地址无效"}
	}
	req.Header.Set("Authorization", "Bearer "+c.Key)
	req.Header.Set("Content-Type", "application/json")
	res, err := c.HTTP.Do(req)
	if err != nil {
		return &Error{Kind: "network", Message: "Seerfar连接失败或请求超时，已保存的竞品快照仍可查看"}
	}
	defer res.Body.Close()
	if res.StatusCode == 429 {
		return &Error{Kind: "rate_limit", Code: 429, Message: "Seerfar请求过于频繁，请稍后刷新"}
	}
	if res.StatusCode < 200 || res.StatusCode >= 300 {
		return &Error{Kind: "upstream", Code: res.StatusCode, Message: fmt.Sprintf("Seerfar服务返回HTTP %d", res.StatusCode)}
	}
	var envelope struct {
		Code int             `json:"code"`
		Msg  string          `json:"msg"`
		Data json.RawMessage `json:"data"`
	}
	if err = json.NewDecoder(io.LimitReader(res.Body, 4<<20)).Decode(&envelope); err != nil {
		return &Error{Kind: "response", Message: "Seerfar响应格式无效"}
	}
	if envelope.Code != 200 {
		kind := "business"
		message := fmt.Sprintf("Seerfar业务请求失败（%d）", envelope.Code)
		text := strings.ToLower(envelope.Msg)
		if envelope.Code == 402 || strings.Contains(text, "积分") || strings.Contains(text, "credit") || strings.Contains(text, "quota") {
			kind = "quota"
			message = "Seerfar积分不足，请充值后刷新"
		}
		if envelope.Code == 401 || envelope.Code == 403 {
			kind = "authentication"
			message = "Seerfar凭证无效或授权不可用"
		}
		return &Error{kind, envelope.Code, message}
	}
	if output == nil || len(envelope.Data) == 0 || string(envelope.Data) == "null" {
		return nil
	}
	if err = json.Unmarshal(envelope.Data, output); err != nil {
		return &Error{Kind: "response", Message: "Seerfar商品字段格式无效"}
	}
	return nil
}

type rawProduct struct {
	SKU         json.Number `json:"sku"`
	Title       string      `json:"title"`
	URL         string      `json:"productUrl"`
	Price       float64     `json:"price"`
	SellerID    json.Number `json:"sellerId"`
	SellerName  string      `json:"sellerName"`
	Fulfillment []string    `json:"fulfillment"`
	Category    struct {
		Category struct {
			ID string `json:"id"`
		} `json:"category"`
	} `json:"categoryInfo"`
	Characteristics []struct {
		Name       string `json:"name"`
		SourceName string `json:"sourceName"`
		Value      string `json:"value"`
	} `json:"characteristics"`
}

func (p rawProduct) competitor() engine.Competitor {
	attrs := map[string]string{}
	for _, a := range p.Characteristics {
		key := a.Name
		if key == "" {
			key = a.SourceName
		}
		attrs[key] = a.Value
	}
	url := p.URL
	if !strings.HasPrefix(url, "https://www.ozon.ru/") {
		url = "https://www.ozon.ru/product/" + p.SKU.String() + "/"
	}
	return engine.Competitor{SKU: p.SKU.String(), Title: p.Title, URL: url, PriceRUB: p.Price, SellerID: p.SellerID.String(), SellerName: p.SellerName, CategoryID: p.Category.Category.ID, Fulfillment: p.Fulfillment, Attributes: attrs}
}
func (c *Client) Discover(ctx context.Context, p engine.Product) ([]engine.Competitor, error) {
	rows := []rawProduct{}
	anchor := p.SeedSKU
	page := map[string]any{"pageNumber": 1, "pageSize": 50, "orders": []map[string]string{{"field": "revenue", "direction": "DESC"}}}
	if anchor != "" {
		var detail struct {
			Product *rawProduct `json:"product"`
		}
		if err := c.call(ctx, "product/detail/search/ozon", map[string]string{"sku": anchor}, &detail); err != nil {
			return nil, err
		}
		if detail.Product == nil {
			return []engine.Competitor{}, nil
		}
		rows = append(rows, *detail.Product)
	} else {
		if len(p.Keywords) == 0 {
			return nil, &Error{Kind: "input", Message: "请填写搜索关键词或一个Ozon商品SKU"}
		}
		input := map[string]any{"keywords": p.Keywords, "page": page, "variationsMerge": 0}
		if p.CategoryID != "" {
			input["categoryIds"] = []string{p.CategoryID}
		}
		var data struct {
			Records []rawProduct `json:"records"`
		}
		if err := c.call(ctx, "productReport/search/ozon", input, &data); err != nil {
			return nil, err
		}
		rows = append(rows, data.Records...)
		if len(rows) > 0 {
			anchor = rows[0].SKU.String()
		}
	}
	if anchor != "" {
		sku, err := strconv.ParseInt(anchor, 10, 64)
		if err != nil || sku <= 0 {
			return nil, &Error{Kind: "input", Message: "Ozon SKU必须为正整数"}
		}
		var similar []rawProduct
		if err = c.call(ctx, "product/detail/similar/ozon", map[string]any{"sku": sku}, &similar); err != nil {
			return nil, err
		}
		rows = append(rows, similar...)
		if len(p.Attributes) > 0 {
			var variants struct {
				Records []rawProduct `json:"records"`
			}
			if err = c.call(ctx, "product/detail/variations/search/ozon", map[string]any{"sku": sku, "clientLanguage": "zh-CN", "page": page}, &variants); err != nil {
				return nil, err
			}
			for _, v := range variants.Records {
				for i := range rows {
					if rows[i].SKU == v.SKU {
						rows[i].Characteristics = v.Characteristics
					}
				}
			}
		}
	}
	competitors := make([]engine.Competitor, 0, len(rows))
	for _, r := range rows {
		competitors = append(competitors, r.competitor())
	}
	return engine.Match(p, competitors), nil
}
func (c *Client) Quota(ctx context.Context) (any, error) {
	var v any
	err := c.call(ctx, "quota", nil, &v)
	return v, err
}
