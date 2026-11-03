package frankfurter

import (
	"context"
	"math"
	"net/http"
	"pricepilot/internal/external"
	"strings"
	"time"
)

type Rate struct {
	Date  string  `json:"date"`
	Base  string  `json:"base"`
	Quote string  `json:"quote"`
	Rate  float64 `json:"rate"`
}
type Client struct {
	BaseURL, Token string
	HTTP           *http.Client
}

func New(base, token string) *Client {
	return &Client{strings.TrimRight(base, "/"), token, external.Client()}
}
func (c *Client) Latest(ctx context.Context) (Rate, error) {
	var q Rate
	if external.ValidateURL(c.BaseURL) != nil {
		return q, &external.Error{Kind: "configuration", Message: "请先配置Frankfurter服务地址"}
	}
	r, err := http.NewRequestWithContext(ctx, "GET", c.BaseURL+"/v2/rate/CNY/RUB", nil)
	if err != nil {
		return q, &external.Error{Kind: "configuration", Message: "Frankfurter服务地址无效"}
	}
	if c.Token != "" {
		r.Header.Set("Authorization", "Bearer "+c.Token)
	}
	res, err := c.HTTP.Do(r)
	if err != nil {
		return q, &external.Error{Kind: "network", Message: "汇率服务连接失败或请求超时，原汇率保持不变"}
	}
	defer res.Body.Close()
	if res.StatusCode == 429 {
		return q, &external.Error{Kind: "rate_limit", Message: "汇率服务请求过于频繁，请稍后重试"}
	}
	if res.StatusCode < 200 || res.StatusCode >= 300 {
		return q, &external.Error{Kind: "upstream", Message: "汇率服务暂不可用或没有该币种数据"}
	}
	if err = external.JSON(res.Body, &q); err != nil {
		return q, err
	}
	d, e := time.Parse("2006-01-02", q.Date)
	if e != nil || d.After(time.Now().UTC().Add(24*time.Hour)) || q.Base != "CNY" || q.Quote != "RUB" || q.Rate <= 0 || math.IsNaN(q.Rate) || math.IsInf(q.Rate, 0) {
		return Rate{}, &external.Error{Kind: "response", Message: "汇率币种、数值或数据日期无效，原汇率保持不变"}
	}
	return q, nil
}
