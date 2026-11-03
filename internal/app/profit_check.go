package app

import (
	"fmt"
	"pricepilot/internal/engine"

	"github.com/gin-gonic/gin"
)

func (s *Server) checkProfit(c *gin.Context) {
	var req struct {
		ProductID string             `json:"product_id"`
		Sample    engine.ProfitCheck `json:"sample"`
	}
	if c.ShouldBindJSON(&req) != nil {
		bad(c, fmt.Errorf("校验样本格式无效"))
		return
	}
	var p engine.Product
	if s.store.Get(c.Request.Context(), "products", req.ProductID, &p) != nil {
		notFound(c)
		return
	}
	r, err := engine.CheckProfit(req.Sample)
	if err != nil {
		bad(c, err)
		return
	}
	c.JSON(200, r)
}
