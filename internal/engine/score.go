package engine

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"math"
	"strings"
	"time"
	"unicode"
)

func Fingerprint(p Product) string {
	v := struct {
		Name, Category, Seed string
		Keywords, Own        []string
		Attributes           map[string]string
	}{p.Name, p.CategoryID, p.SeedSKU, p.Keywords, p.OwnSellerIDs, p.Attributes}
	b, _ := json.Marshal(v)
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}
func normalized(s string) string {
	return strings.Map(func(r rune) rune {
		if unicode.IsLetter(r) || unicode.IsNumber(r) {
			return unicode.ToLower(r)
		}
		return -1
	}, s)
}
func Match(p Product, rows []Competitor) []Competitor {
	out := make([]Competitor, 0, len(rows))
	seen := map[string]bool{}
	for _, c := range rows {
		if seen[c.SKU] || c.SKU == "" {
			continue
		}
		seen[c.SKU] = true
		c.Include = false
		c.Verified = false
		c.Confidence = 0
		if has(p.OwnSellerIDs, c.SellerID) {
			c.Reason = "本店商品"
			out = append(out, c)
			continue
		}
		if c.PriceRUB <= 0 || !validNumber(c.PriceRUB) {
			c.Reason = "价格缺失"
			out = append(out, c)
			continue
		}
		if p.CategoryID != "" && c.CategoryID != "" && p.CategoryID != c.CategoryID {
			c.Reason = "类目不一致"
			out = append(out, c)
			continue
		}
		matchedKeyword := len(p.Keywords) == 0
		for _, k := range p.Keywords {
			if k != "" && strings.Contains(normalized(c.Title), normalized(k)) {
				matchedKeyword = true
			}
		}
		missing, mismatch := false, ""
		for key, value := range p.Attributes {
			if strings.TrimSpace(value) == "" {
				continue
			}
			actual := c.Attributes[key]
			if actual != "" {
				if normalized(actual) != normalized(value) {
					mismatch = key + "不一致"
					break
				}
			} else if !strings.Contains(normalized(c.Title), normalized(value)) {
				missing = true
			}
		}
		switch {
		case mismatch != "":
			c.Reason = mismatch
		case missing:
			c.Confidence = .5
			c.Reason = "关键规格待核对"
		case !matchedKeyword:
			c.Confidence = .4
			c.Reason = "用途或关键词待核对"
		case p.CategoryID == "" && len(p.Attributes) == 0:
			c.Confidence = .6
			c.Reason = "请补充类目或关键规格"
		default:
			c.Confidence = .9
			c.Include = true
			c.Reason = "类目与关键规格匹配"
		}
		out = append(out, c)
	}
	return out
}
func effectivePeers(in Input) []Competitor {
	if in.Snapshot.Fingerprint != "" && in.Snapshot.Fingerprint != Fingerprint(in.Product) {
		return nil
	}
	seen := map[string]bool{}
	out := []Competitor{}
	for _, c := range in.Snapshot.Competitors {
		condition := in.Product.PriceCondition
		if condition == "" {
			condition = "standard"
		}
		if c.PriceCondition != "" && c.PriceCondition != condition {
			continue
		}
		if !c.Include || c.PriceRUB <= 0 || !validNumber(c.PriceRUB) || has(in.Product.OwnSellerIDs, c.SellerID) || has(in.Settings.OwnSellerIDs, c.SellerID) || seen[c.SKU] {
			continue
		}
		seen[c.SKU] = true
		if !c.Verified && c.Confidence < .85 {
			continue
		}
		if !daysValid(c.Delivery) || c.DeliverySource == "" {
			c.Delivery = nil
		}
		if c.Delivery == nil {
			for _, r := range in.Settings.DeliveryReferences {
				if r.Region == in.Settings.Region && has(c.Fulfillment, r.Fulfillment) && r.Source != "" {
					d := r.Days
					c.Delivery = &d
					c.DeliverySource = r.Source
					c.Estimated = true
					break
				}
			}
		}
		out = append(out, c)
	}
	return out
}
func supplierScore(p Product) *float64 {
	if p.SupplierScore == nil || p.SupplierScale <= 0 || p.SupplierSource == "" {
		return nil
	}
	v := *p.SupplierScore / p.SupplierScale * 100
	return &v
}
func score(in Input, priceRUB float64, delivery *Days) Score {
	s := Score{TheoreticalMax: 100, Weights: in.Settings.Weights}
	peers := effectivePeers(in)
	s.Samples = len(peers)
	s.Supplier = supplierScore(in.Product)
	if s.Supplier != nil {
		s.TheoreticalMax = 100 - in.Settings.Weights.Supplier*(100-*s.Supplier)/100
	}
	s.Stale = in.Snapshot.FetchedAt.IsZero() || in.AsOf.Sub(in.Snapshot.FetchedAt) > time.Duration(in.Settings.SnapshotHours)*time.Hour || in.Snapshot.FetchedAt.After(in.AsOf.Add(time.Minute))
	if in.Snapshot.Fingerprint != "" && in.Snapshot.Fingerprint != Fingerprint(in.Product) {
		s.Stale = true
	}
	priceMin, priceMax, timeMin, timeMax := 0.0, 100.0, 0.0, 100.0
	if len(peers) > 0 && !s.Stale {
		p, t, unknown := 0.0, 0.0, 0
		for _, c := range peers {
			delta := math.Round(c.PriceRUB*100) - math.Round(priceRUB*100)
			if delta > 0 {
				p++
			} else if delta == 0 {
				p += .5
			}
			if delivery == nil || c.Delivery == nil {
				unknown++
				continue
			}
			if c.Delivery.Max > delivery.Max {
				t++
			} else if c.Delivery.Max == delivery.Max {
				t += .5
			}
			s.Estimated = s.Estimated || c.Estimated
		}
		v := p / float64(len(peers)) * 100
		s.Price = &v
		priceMin = v
		priceMax = v
		timeMin = t / float64(len(peers)) * 100
		timeMax = timeMin + float64(unknown)/float64(len(peers))*100
		if unknown == 0 {
			v := timeMin
			s.Delivery = &v
		}
	}
	uMin, uMax := 0.0, 100.0
	if s.Supplier != nil {
		uMin = *s.Supplier
		uMax = uMin
	}
	w := in.Settings.Weights
	s.Minimum = (priceMin*w.Price + timeMin*w.Delivery + uMin*w.Supplier) / 100
	s.Maximum = (priceMax*w.Price + timeMax*w.Delivery + uMax*w.Supplier) / 100
	if math.Abs(s.Minimum-s.Maximum) < 1e-8 {
		v := s.Minimum
		s.Total = &v
	}
	if s.Price != nil && s.Delivery != nil && w.Price+w.Delivery > 0 {
		v := (*s.Price*w.Price + *s.Delivery*w.Delivery) / (w.Price + w.Delivery)
		s.Market = &v
	}
	return s
}
