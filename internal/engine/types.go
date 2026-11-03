package engine

import "time"

type Days struct {
	Min float64 `json:"min"`
	Max float64 `json:"max"`
}
type Tier struct {
	UpTo *float64 `json:"up_to"`
	Rate float64  `json:"rate"`
}
type Fee struct {
	Version          int      `json:"version"`
	Status           string   `json:"status"`
	ValidFrom        string   `json:"valid_from"`
	ValidTo          string   `json:"valid_to"`
	Code             string   `json:"code"`
	Name             string   `json:"name"`
	Enabled          bool     `json:"enabled"`
	Kind             string   `json:"kind"`
	Amount           *float64 `json:"amount"`
	Rate             *float64 `json:"rate"`
	Currency         string   `json:"currency"`
	Basis            string   `json:"basis"`
	On               string   `json:"on"`
	Tiers            []Tier   `json:"tiers"`
	TierCurrency     string   `json:"tier_currency"`
	Minimum          *float64 `json:"minimum"`
	Maximum          *float64 `json:"maximum"`
	FirstWeight      float64  `json:"first_weight"`
	AdditionalWeight float64  `json:"additional_weight"`
	AdditionalAmount float64  `json:"additional_amount"`
	DeductSettlement bool     `json:"deduct_settlement"`
	Source           string   `json:"source"`
	RouteIDs         []string `json:"route_ids"`
	SupplyModes      []string `json:"supply_modes"`
}
type Band struct {
	UpTo          *float64 `json:"up_to"`
	MinWeight     float64  `json:"min_weight"`
	MaxWeight     float64  `json:"max_weight"`
	Fixed         float64  `json:"fixed"`
	PerGram       float64  `json:"per_gram"`
	VolumeDivisor float64  `json:"volume_divisor"`
	Step          float64  `json:"step"`
	MinimumWeight float64  `json:"minimum_weight"`
	MaxLength     float64  `json:"max_length"`
	MaxSum        float64  `json:"max_sum"`
}
type Quote struct {
	RequiredFees   map[string]string `json:"required_fees,omitempty"`
	Fees           []Fee             `json:"fees,omitempty"`
	StoreID        string            `json:"store_id"`
	Version        int               `json:"version"`
	PublicID       string            `json:"public_id,omitempty"`
	ID             string            `json:"id"`
	Provider       string            `json:"provider"`
	Warehouse      string            `json:"warehouse"`
	Name           string            `json:"name"`
	Kind           string            `json:"kind"`
	Status         string            `json:"status"`
	Demo           bool              `json:"demo"`
	Currency       string            `json:"currency"`
	ValueCurrency  string            `json:"value_currency"`
	ValidFrom      string            `json:"valid_from"`
	ValidTo        string            `json:"valid_to"`
	Source         string            `json:"source"`
	SourceDate     string            `json:"source_date"`
	Note           string            `json:"note"`
	Platforms      []string          `json:"platforms"`
	Batteries      bool              `json:"batteries"`
	Liquids        bool              `json:"liquids"`
	Bands          []Band            `json:"bands"`
	Includes       []string          `json:"includes"`
	Delivery       *Days             `json:"delivery"`
	TimeScope      string            `json:"time_scope"`
	Estimated      bool              `json:"estimated"`
	ReturnCost     *float64          `json:"return_cost"`
	ReturnCurrency string            `json:"return_currency"`
	Compensation   string            `json:"compensation"`
}
type Returns struct {
	Destination     string   `json:"destination"`
	Source          string   `json:"source"`
	RefusalRate     float64  `json:"refusal_rate"`
	RefusalReverse  *float64 `json:"refusal_reverse"`
	RefusalDisposal *float64 `json:"refusal_disposal"`
	RefusalRecovery *float64 `json:"refusal_recovery"`
	Rate            float64  `json:"rate"`
	CancelRate      float64  `json:"cancel_rate"`
	Reverse         *float64 `json:"reverse"`
	Disposal        *float64 `json:"disposal"`
	Recovery        *float64 `json:"recovery"`
	CancelCost      *float64 `json:"cancel_cost"`
}
type ModeOptions struct {
	Margin         float64  `json:"margin"`
	DeliveryLoss   float64  `json:"delivery_loss"`
	TargetScore    float64  `json:"target_score"`
	TargetLoss     float64  `json:"target_loss"`
	TargetMaxPrice *float64 `json:"target_max_price"`
}
type Product struct {
	StoreID            string            `json:"store_id"`
	Version            int               `json:"version"`
	FixedDiscount      float64           `json:"fixed_discount"`
	PlatformDiscount   Discount          `json:"platform_discount"`
	PriceCondition     string            `json:"price_condition"`
	ID                 string            `json:"id"`
	Name               string            `json:"name"`
	Demo               bool              `json:"demo"`
	Supply             string            `json:"supply"`
	Supplier           string            `json:"supplier"`
	SupplierScore      *float64          `json:"supplier_score"`
	SupplierScale      float64           `json:"supplier_scale"`
	SupplierSource     string            `json:"supplier_source"`
	PurchaseQuantity   int               `json:"purchase_quantity"`
	MOQ                int               `json:"moq"`
	PurchaseTiers      []PurchaseTier    `json:"purchase_tiers"`
	Weight             float64           `json:"weight"`
	Length             float64           `json:"length"`
	Width              float64           `json:"width"`
	Height             float64           `json:"height"`
	Batteries          bool              `json:"batteries"`
	Liquids            bool              `json:"liquids"`
	SupplierDays       *Days             `json:"supplier_days"`
	DomesticDays       *Days             `json:"domestic_days"`
	TailDays           *Days             `json:"tail_days"`
	WarehouseQuoteID   string            `json:"warehouse_quote_id"`
	RouteIDs           []string          `json:"route_ids"`
	Costs              []Fee             `json:"costs"`
	Returns            Returns           `json:"returns"`
	Discount           float64           `json:"discount"`
	Keywords           []string          `json:"keywords"`
	CategoryID         string            `json:"category_id"`
	AlibabaCategory    string            `json:"alibaba_category"`
	CommissionCategory string            `json:"commission_category"`
	Attributes         map[string]string `json:"attributes"`
	OwnSellerIDs       []string          `json:"own_seller_ids"`
	SeedSKU            string            `json:"seed_sku"`
	Mode               string            `json:"mode"`
	Options            ModeOptions       `json:"options"`
	UpdatedAt          time.Time         `json:"updated_at"`
}
type PurchaseTier struct {
	Quantity int     `json:"quantity"`
	Price    float64 `json:"price"`
}
type DeliveryReference struct {
	Fulfillment string `json:"fulfillment"`
	Region      string `json:"region"`
	Days        Days   `json:"days"`
	Source      string `json:"source"`
}
type CategoryMapping struct {
	Alibaba string `json:"alibaba"`
	Seerfar string `json:"seerfar"`
	Ozon    string `json:"ozon"`
	Name    string `json:"name"`
}
type Settings struct {
	StoreID            string              `json:"store_id"`
	Version            int                 `json:"version"`
	OwnSellerIDs       []string            `json:"own_seller_ids"`
	FX                 float64             `json:"fx"`
	FXSource           string              `json:"fx_source"`
	FXDate             string              `json:"fx_date"`
	Stress             float64             `json:"stress"`
	Region             string              `json:"region"`
	Weights            Weights             `json:"weights"`
	Fees               []Fee               `json:"fees"`
	CategoryFees       map[string][]Fee    `json:"category_fees"`
	DeliveryReferences []DeliveryReference `json:"delivery_references"`
	CategoryMappings   []CategoryMapping   `json:"category_mappings"`
	SnapshotHours      int                 `json:"snapshot_hours"`
	Demo               bool                `json:"demo"`
}
type Weights struct {
	Price    float64 `json:"price"`
	Delivery float64 `json:"delivery"`
	Supplier float64 `json:"supplier"`
}
type Competitor struct {
	PriceCondition string            `json:"price_condition"`
	PriceSource    string            `json:"price_source"`
	SKU            string            `json:"sku"`
	Title          string            `json:"title"`
	URL            string            `json:"url"`
	PriceRUB       float64           `json:"price_rub"`
	SellerID       string            `json:"seller_id"`
	SellerName     string            `json:"seller_name"`
	CategoryID     string            `json:"category_id"`
	Fulfillment    []string          `json:"fulfillment"`
	Attributes     map[string]string `json:"attributes"`
	Include        bool              `json:"include"`
	Pinned         bool              `json:"pinned"`
	Verified       bool              `json:"verified"`
	Confidence     float64           `json:"confidence"`
	Reason         string            `json:"reason"`
	Delivery       *Days             `json:"delivery"`
	DeliverySource string            `json:"delivery_source"`
	Estimated      bool              `json:"estimated"`
}
type Snapshot struct {
	StoreID     string       `json:"store_id"`
	ID          string       `json:"id"`
	ProductID   string       `json:"product_id"`
	Fingerprint string       `json:"fingerprint"`
	Source      string       `json:"source"`
	Demo        bool         `json:"demo"`
	FetchedAt   time.Time    `json:"fetched_at"`
	Competitors []Competitor `json:"competitors"`
	Warnings    []string     `json:"warnings"`
}
type Score struct {
	Weights        Weights  `json:"weights"`
	Total          *float64 `json:"total"`
	Minimum        float64  `json:"minimum"`
	Maximum        float64  `json:"maximum"`
	Price          *float64 `json:"price"`
	Delivery       *float64 `json:"delivery"`
	Supplier       *float64 `json:"supplier"`
	Market         *float64 `json:"market"`
	Samples        int      `json:"samples"`
	Estimated      bool     `json:"estimated"`
	Stale          bool     `json:"stale"`
	TheoreticalMax float64  `json:"theoretical_max"`
}
type Line struct {
	Version      int     `json:"version"`
	ValidFrom    string  `json:"valid_from"`
	ValidTo      string  `json:"valid_to"`
	Confirmation string  `json:"confirmation"`
	Code         string  `json:"code"`
	Name         string  `json:"name"`
	Amount       float64 `json:"amount"`
	Currency     string  `json:"currency"`
	CNY          float64 `json:"cny"`
	Formula      string  `json:"formula"`
	Source       string  `json:"source"`
	Included     bool    `json:"included"`
}
type Scenario struct {
	FX      float64 `json:"fx"`
	Label   string  `json:"label"`
	Profit  float64 `json:"profit"`
	Margin  float64 `json:"margin"`
	Revenue float64 `json:"revenue"`
	Valid   bool    `json:"valid"`
	Reason  string  `json:"reason,omitempty"`
}
type Evaluation struct {
	Prices      PriceBreakdown `json:"prices"`
	QuoteID     string         `json:"quote_id"`
	Route       string         `json:"route"`
	Warehouse   string         `json:"warehouse"`
	Price       float64        `json:"price"`
	SalePrice   float64        `json:"sale_price"`
	PriceRUB    float64        `json:"price_rub"`
	Revenue     float64        `json:"revenue"`
	Profit      float64        `json:"profit"`
	Margin      float64        `json:"margin"`
	Cost        float64        `json:"cost"`
	WorstProfit float64        `json:"worst_profit"`
	WorstMargin float64        `json:"worst_margin"`
	Delivery    *Days          `json:"delivery"`
	Score       Score          `json:"score"`
	Lines       []Line         `json:"lines"`
	Scenarios   []Scenario     `json:"scenarios"`
	Formula     string         `json:"formula"`
	Warnings    []string       `json:"warnings"`
	Valid       bool           `json:"valid"`
	Reason      string         `json:"reason,omitempty"`
}
type Input struct {
	Product  Product   `json:"product"`
	Settings Settings  `json:"settings"`
	Quotes   []Quote   `json:"quotes"`
	Snapshot Snapshot  `json:"snapshot"`
	AsOf     time.Time `json:"as_of"`
}
type Result struct {
	Status         string        `json:"status"`
	Mode           string        `json:"mode"`
	Recommended    *Evaluation   `json:"recommended"`
	Alternatives   []Evaluation  `json:"alternatives"`
	Unavailable    []Unavailable `json:"unavailable"`
	MaxScore       *float64      `json:"max_score"`
	TheoreticalMax float64       `json:"theoretical_max"`
	Messages       []string      `json:"messages"`
	SnapshotID     string        `json:"snapshot_id"`
	CalculatedAt   time.Time     `json:"calculated_at"`
	Demo           bool          `json:"demo"`
	PriceCap       float64       `json:"price_cap"`
}
type Unavailable struct {
	QuoteID string `json:"quote_id"`
	Name    string `json:"name"`
	Reason  string `json:"reason"`
}

func Ptr(v float64) *float64 { return &v }
