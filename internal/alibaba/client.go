package alibaba

import (
	"context"
	"crypto/hmac"
	"crypto/sha1"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"pricepilot/internal/external"
)

type Error = external.Error

var digits = regexp.MustCompile(`^[1-9][0-9]*$`)
var offerPath = regexp.MustCompile(`^/offer/([1-9][0-9]*)\.html$`)

func ParseOfferID(input string) (string, error) {
	s := strings.TrimSpace(input)
	if !digits.MatchString(s) {
		u, err := url.Parse(s)
		if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host != "detail.1688.com" || u.User != nil || strings.ContainsAny(s, " \r\n\t") || strings.Contains(u.RawQuery, "https://") || strings.Contains(u.RawQuery, "http://") {
			return "", fmt.Errorf("请输入单个1688商品详情链接或数字商品ID")
		}
		m := offerPath.FindStringSubmatch(u.Path)
		if len(m) != 2 {
			return "", fmt.Errorf("无法从链接路径识别1688商品ID")
		}
		s = m[1]
	}
	if _, err := strconv.ParseInt(s, 10, 64); err != nil {
		return "", fmt.Errorf("商品ID超出有效范围")
	}
	return s, nil
}
func Sign(path string, p url.Values, secret string) string {
	parts := []string{}
	for k, v := range p {
		if k != "_aop_signature" {
			parts = append(parts, k+v[0])
		}
	}
	sort.Strings(parts)
	h := hmac.New(sha1.New, []byte(secret))
	h.Write([]byte(path + strings.Join(parts, "")))
	return strings.ToUpper(hex.EncodeToString(h.Sum(nil)))
}

type Client struct {
	BaseURL, AppKey, Secret, Token string
	HTTP                           *http.Client
}

func New(base, key, secret, token string) *Client {
	return &Client{strings.TrimRight(base, "/"), key, secret, token, external.Client()}
}
func (c *Client) call(ctx context.Context, namespace, name string, p url.Values) (map[string]json.RawMessage, error) {
	if external.ValidateURL(c.BaseURL) != nil || c.AppKey == "" || c.Secret == "" || c.Token == "" {
		return nil, &Error{Kind: "configuration", Message: "请先配置1688服务地址、AppKey、AppSecret及AccessToken"}
	}
	path := "param2/1/" + namespace + "/" + name + "/" + url.PathEscape(c.AppKey)
	p.Set("access_token", c.Token)
	p.Set("_aop_signature", Sign(path, p, c.Secret))
	r, err := http.NewRequestWithContext(ctx, "POST", c.BaseURL+"/openapi/"+path, strings.NewReader(p.Encode()))
	if err != nil {
		return nil, &Error{Kind: "configuration", Message: "1688服务地址无效"}
	}
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded; charset=utf-8")
	res, err := c.HTTP.Do(r)
	if err != nil {
		return nil, &Error{Kind: "network", Message: "1688连接失败或请求超时；如刚确认建立关系，请先重新查询详情"}
	}
	defer res.Body.Close()
	if res.StatusCode == 429 {
		return nil, &Error{Kind: "rate_limit", Message: "1688调用过于频繁，请稍后重试"}
	}
	if res.StatusCode == 401 || res.StatusCode == 403 {
		return nil, &Error{Kind: "authentication", Message: "1688授权无效或应用权限不足，请检查凭据"}
	}
	if res.StatusCode < 200 || res.StatusCode >= 300 {
		return nil, &Error{Kind: "upstream", Message: "1688服务暂不可用"}
	}
	var out map[string]json.RawMessage
	if err = external.JSON(res.Body, &out); err != nil {
		return nil, err
	}
	if err = businessError(out); err != nil {
		return nil, err
	}
	return out, nil
}
func businessError(m map[string]json.RawMessage) error {
	code := text(m["error_code"])
	if code == "" {
		code = text(m["errorCode"])
	}
	message := text(m["error_message"])
	if message == "" {
		message = text(m["errorMessage"])
	}
	if message == "" {
		message = text(m["message"])
	}
	failed := string(m["success"]) == "false" || code != "" && code != "0" && code != "200"
	if !failed {
		return nil
	}
	s := strings.ToLower(code + " " + message)
	switch {
	case strings.Contains(s, "没有该商品的查询权限"):
		return &Error{Kind: "relation_required", Message: "该1688账号尚未建立此商品的铺货关系"}
	case strings.Contains(s, "token") || strings.Contains(s, "signature") || strings.Contains(s, "sign_") || strings.Contains(s, "appkey_not"):
		return &Error{Kind: "authentication", Message: "1688凭据失效或签名校验失败，请检查配置"}
	case strings.Contains(s, "access_denied") || strings.Contains(s, "isvusers") || strings.Contains(s, "appkey_isv") || strings.Contains(s, "permission"):
		return &Error{Kind: "permission", Message: "1688应用缺少接口权限或跨境铺货配置，请在1688授权后台核实"}
	case strings.Contains(s, "不存在") || strings.Contains(s, "not_exist"):
		return &Error{Kind: "not_found", Message: "该1688商品不存在、已下架或不可访问"}
	default:
		return &Error{Kind: "business", Message: "1688未完成请求，请核实商品状态及应用授权"}
	}
}
func (c *Client) Detail(ctx context.Context, id string) (Product, error) {
	if _, err := ParseOfferID(id); err != nil {
		return Product{}, err
	}
	out, err := c.call(ctx, "com.alibaba.product", "alibaba.cross.productInfo", url.Values{"productId": {id}})
	if err != nil {
		return Product{}, err
	}
	if len(out["productInfo"]) == 0 || string(out["productInfo"]) == "null" {
		return Product{}, &Error{Kind: "response", Message: "1688未返回商品详情"}
	}
	return Normalize(out["productInfo"], id)
}
func (c *Client) EstablishRelation(ctx context.Context, id string) error {
	if _, err := ParseOfferID(id); err != nil {
		return err
	}
	out, err := c.call(ctx, "com.alibaba.product.push", "alibaba.cross.syncProductListPushed", url.Values{"productIdList": {"[" + id + "]"}})
	if err != nil {
		return err
	}
	if len(out["result"]) > 0 {
		var result map[string]json.RawMessage
		if json.Unmarshal(out["result"], &result) != nil {
			return &Error{Kind: "response", Message: "1688铺货关系响应格式无效"}
		}
		if err = businessError(result); err != nil {
			return err
		}
		if string(result["success"]) != "true" {
			return &Error{Kind: "business", Message: "1688未确认建立铺货关系，请重新查询详情"}
		}
	} else if string(out["success"]) != "true" {
		return &Error{Kind: "business", Message: "1688未确认建立铺货关系，请重新查询详情"}
	}
	return nil
}
