package external

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"
)

type Error struct {
	Kind    string `json:"kind"`
	Message string `json:"message"`
}

func (e *Error) Error() string { return e.Message }

func Client() *http.Client {
	return &http.Client{Timeout: 20 * time.Second, CheckRedirect: func(_ *http.Request, _ []*http.Request) error { return fmt.Errorf("redirects disabled") }}
}
func ValidateURL(s string) error {
	u, err := url.Parse(s)
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Hostname() == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || u.Opaque != "" {
		return fmt.Errorf("请输入不含凭据或查询参数的HTTP(S)服务地址")
	}
	return nil
}
func JSON(r io.Reader, v any) error {
	b, err := io.ReadAll(io.LimitReader(r, (4<<20)+1))
	if err != nil || len(b) > 4<<20 {
		return &Error{Kind: "response", Message: "外部服务响应过大或读取失败"}
	}
	if json.Unmarshal(b, v) != nil {
		return &Error{Kind: "response", Message: "外部服务响应格式无效"}
	}
	return nil
}
