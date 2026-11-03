import { expect, it } from "vitest";
import { alibabaPriceHint } from "../src/lib";
import type { AlibabaProduct } from "../src/types";

it("报价预览不会用SKU单价替代商品阶梯价，也不采用尚未达到数量的档位", () => {
	const p = {
		quote_type: 2,
		tiers: [
			{ quantity: 2, price: 20 },
			{ quantity: 10, price: 18 },
		],
		skus: [
			{
				id: "selected",
				price: 99,
				tiers: [
					{ quantity: 2, price: 12 },
					{ quantity: 10, price: 11 },
				],
			},
		],
	} as AlibabaProduct;
	expect(alibabaPriceHint(p, "selected", 2)).toBe(20);
	expect(alibabaPriceHint(p, "selected", 10)).toBe(18);
	expect(alibabaPriceHint(p, "selected", 1)).toBeNull();
	expect(alibabaPriceHint({ ...p, quote_type: 1 }, "selected", 10)).toBe(99);
	expect(alibabaPriceHint({ ...p, quote_type: 3 }, "selected", 10)).toBe(11);
	expect(alibabaPriceHint(p, "other", 10)).toBeNull();
});
