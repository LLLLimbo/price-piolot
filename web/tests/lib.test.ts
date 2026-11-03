import { describe, expect, it } from "vitest";
import { attributes, importedCompetitors, exportCSV } from "../src/lib";
import type { Product, Result } from "../src/types";
describe("商家数据输入与导出", () => {
	it("保留中文规格并支持常用分隔符", () => {
		expect(attributes("容量：500ml\n型号=A-1\n材质:304钢")).toEqual({
			容量: "500ml",
			型号: "A-1",
			材质: "304钢",
		});
	});
	it("拒绝缺失价格的导入记录，不把它当作零价竞品", () => {
		expect(() => importedCompetitors('[{"sku":"1"}]')).toThrow();
	});
	it("人工导入没有时效时保持缺失", () => {
		expect(
			importedCompetitors('[{"sku":"123","price_rub":1500}]')[0].delivery,
		).toBeNull();
	});
	it("CSV说明数据口径，转义不可信商品名称", () => {
		const result = {
			mode: "profit",
			calculated_at: "2026-11-02",
			demo: true,
			snapshot_id: "snap",
			recommended: {
				price: 109.1,
				sale_price: 109.1,
				price_rub: 1309.2,
				route: "陆运",
				warehouse: "自发",
				profit: 21.825,
				margin: 0.200046,
				worst_profit: 21.825,
				worst_margin: 0.200046,
				score: { total: null, minimum: 60, maximum: 90 },
				formula: "109.1 - 87.275 = 21.825",
				lines: [],
				scenarios: [],
			},
		} as unknown as Result;
		const csv = exportCSV({ name: "=malicious()" } as Product, result);
		expect(csv).toContain("'=malicious()");
		expect(csv).toContain("演示数据");
		expect(csv).toContain("60–90");
	});
});
