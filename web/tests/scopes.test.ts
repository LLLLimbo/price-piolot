import { afterEach, expect, it, vi } from "vitest";
import { scopedAPI, newProduct } from "../src/lib";
import type { Product } from "../src/types";
afterEach(() => vi.unstubAllGlobals());
it("每个请求绑定发起时的店铺，不使用可变的全局店铺", async () => {
	const fetch = vi.fn(async (_url: string, _options?: RequestInit) => ({
		ok: true,
		json: async (_url: string, _options?: RequestInit) => ({ ok: true }),
	}));
	vi.stubGlobal("fetch", fetch);
	const first = scopedAPI("shop-a"),
		second = scopedAPI("shop-b");
	await Promise.all([
		first("/bootstrap"),
		second("/calculate", { product: {} }),
	]);
	expect(fetch.mock.calls[0][0]).toBe("/api/stores/shop-a/bootstrap");
	expect(fetch.mock.calls[1][0]).toBe("/api/stores/shop-b/calculate");
});
it("新商品不继承已确认的费用和优惠", () => {
	const p = newProduct({
		costs: [{ amount: 5, rate: 0.1, status: "confirmed" }],
		options: {},
		returns: {},
		platform_discount: { kind: "fixed", value: 5 },
		sourcing: { offer_id: "1001" },
		batteries: true,
		notes: "上一商品的来源",
		profit_check: { order_ref: "上一商品的订单" },
		liquids: true,
	} as unknown as Product);
	expect(p.costs[0].amount).toBeNull();
	expect(p.costs[0].status).toBe("pending");
	expect(p.platform_discount?.kind).toBe("none");
	expect(p.returns.refusal_reverse).toBeNull();
	expect(p.returns.destination).toBe("");
	expect(p.returns.source).toBe("");
	expect(p.sourcing).toBeUndefined();
	expect(p.batteries).toBe(false);
	expect(p.liquids).toBe(false);
	expect(p.notes).toBe("");
	expect(p.profit_check).toBeUndefined();
	expect(p.returns.unclassified_rate).toBe(0);
});
