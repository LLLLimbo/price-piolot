import { createContext, useContext } from "react";
import type { Competitor, Product, Result, Snapshot } from "./types";
export async function api<T>(
	path: string,
	body?: unknown,
	method?: string,
): Promise<T> {
	const res = await fetch(`/api${path}`, {
		method: method || (body === undefined ? "GET" : "POST"),
		headers: { "Content-Type": "application/json" },
		body: body === undefined ? undefined : JSON.stringify(body),
	});
	const data = await res.json();
	if (!res.ok) throw new Error(data.error || "请求失败，请重试");
	return data;
}
export function scopedAPI(storeId: string): typeof api {
	return <T>(path: string, body?: unknown, method?: string) =>
		api<T>(`/stores/${encodeURIComponent(storeId)}${path}`, body, method);
}
export const ApiContext = createContext<typeof api>(api);
export const useStoreApi = () => useContext(ApiContext);
export const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
export const id = () =>
	`p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 9)}`;
export const money = (
	n: number | null | undefined,
	currency = "CNY",
	digits = 2,
) =>
	n == null
		? "—"
		: new Intl.NumberFormat("zh-CN", {
				style: "currency",
				currency,
				minimumFractionDigits: digits,
				maximumFractionDigits: digits,
			}).format(n);
export const pct = (n: number | null | undefined) =>
	n == null ? "—" : `${(n * 100).toFixed(2)}%`;
export const time = (s: string) =>
	new Date(s).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
	});
export const modes: Record<
	string,
	{ name: string; short: string; description: string }
> = {
	profit: {
		name: "利润优先",
		short: "守住净利率",
		description: "满足利润底线，争取最高竞争力",
	},
	breakeven: {
		name: "保本",
		short: "最低不亏价",
		description: "完整成本与汇率压力下不亏损",
	},
	delivery: {
		name: "配送时效优先",
		short: "更快到达",
		description: "先选最快路线，再参考市场价格",
	},
	target: {
		name: "竞争力目标优先",
		short: "按分数反推",
		description: "达到目标分数，尽量保留单件利润",
	},
};
export function attributes(text: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const line of text.split("\n")) {
		const match = line.match(/^\s*([^=:：]+)[=:：](.+)$/);
		if (match) out[match[1].trim()] = match[2].trim();
	}
	return out;
}
export function newProduct(template: Product): Product {
	const p = clone(template);
	p.id = id();
	p.name = "";
	p.demo = false;
	p.supplier = "";
	p.supplier_score = null;
	p.supplier_source = "";
	p.warehouse_quote_id = "self";
	p.route_ids = [];
	p.keywords = [];
	p.category_id = "";
	p.attributes = {};
	p.seed_sku = "";
	p.purchase_tiers = [];
	p.discount = 0;
	p.fixed_discount = 0;
	p.platform_discount = {
		kind: "none",
		value: null,
		currency: "CNY",
		confirmed: false,
		source: "",
	};
	p.price_condition = "standard";
	p.version = 0;
	p.alibaba_category = "";
	p.commission_category = "";
	p.supplier_scale = 5;
	p.purchase_quantity = 1;
	p.moq = 1;
	p.mode = "profit";
	p.options = {
		margin: 0.2,
		delivery_loss: 0,
		target_score: 80,
		target_loss: 0,
		target_max_price: null,
	};
	p.weight = 0;
	p.length = 0;
	p.width = 0;
	p.height = 0;
	p.supplier_days = null;
	p.domestic_days = null;
	p.tail_days = null;
	p.costs = p.costs.map((f) => ({
		...f,
		amount: null,
		rate: null,
		status: "pending",
		version: 0,
		valid_from: "",
		valid_to: "",
		source: "商家录入",
	}));
	p.returns = {
		destination: "",
		source: "",
		rate: 0,
		refusal_rate: 0,
		refusal_reverse: null,
		refusal_disposal: null,
		refusal_recovery: null,
		cancel_rate: 0,
		reverse: null,
		recovery: null,
		disposal: null,
		cancel_cost: null,
	};
	return p;
}
function csvCell(value: unknown): string {
	let text = String(value ?? "");
	if (/^[=+@\t\r]/.test(text)) text = `'${text}`;
	return `"${text.replaceAll('"', '""')}"`;
}
export function exportCSV(
	product: Product,
	result: Result,
	snapshot?: Snapshot,
): string {
	const e = result.recommended;
	if (!e) throw new Error("没有可导出的推荐方案");
	const rows: unknown[][] = [
		["商品", product.name],
		["模式", modes[result.mode]?.name],
		["数据标识", result.demo ? "演示数据" : "商家测算"],
		["计算时间", result.calculated_at],
		["竞品快照", snapshot?.id || result.snapshot_id],
		["竞品时间", snapshot?.fetched_at],
		["建议标价 CNY", e.price],
		["商家折后成交价 CNY", e.sale_price],
		["商家预计留存销售收入 CNY", e.revenue],
		["平台等额补偿优惠 CNY", e.prices?.platform_discount_cny ?? "历史未标注"],
		["比较口径", e.prices?.comparison_basis ?? "历史商品展示价"],
		["卢布参考价 RUB", e.price_rub],
		["物流", e.route],
		["仓配", e.warehouse],
		["单件预计利润 CNY", e.profit],
		["净利率", pct(e.margin)],
		["压力情景最低利润 CNY", e.worst_profit],
		["压力情景最低净利率", pct(e.worst_margin)],
		["评分", e.score.total ?? `${e.score.minimum}–${e.score.maximum}`],
		["公式", e.formula],
		[],
		[
			"费用",
			"预期金额 CNY",
			"原币金额",
			"币种",
			"计算式",
			"来源",
			"已包含",
			"版本",
			"有效期",
		],
	];
	for (const l of e.lines)
		rows.push([
			l.name,
			l.cny,
			l.amount,
			l.currency,
			l.formula,
			l.source,
			l.included ? "是" : "否",
			l.version || "历史未标注",
			`${l.valid_from || "不限"} ~ ${l.valid_to || "不限"}`,
		]);
	rows.push([], ["汇率 CNY→RUB", "利润 CNY", "净利率", "情景"]);
	for (const s of e.scenarios)
		rows.push([s.fx, s.profit, pct(s.margin), s.label]);
	return "\ufeff" + rows.map((row) => row.map(csvCell).join(",")).join("\r\n");
}
export function download(
	text: string,
	name: string,
	type = "text/csv;charset=utf-8",
) {
	const url = URL.createObjectURL(new Blob([text], { type }));
	const a = document.createElement("a");
	a.href = url;
	a.download = name;
	a.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function importedCompetitors(text: string): Competitor[] {
	const value: unknown = JSON.parse(text);
	if (!Array.isArray(value)) throw new Error("导入文件须为竞品列表");
	return value.map((row: Partial<Competitor>) => {
		if (
			!row.sku ||
			!Number.isFinite(Number(row.price_rub)) ||
			Number(row.price_rub) <= 0
		)
			throw new Error("每项竞品需要SKU和有效卢布价格");
		return {
			title: "",
			url: "",
			seller_id: "",
			seller_name: "",
			category_id: "",
			fulfillment: [],
			attributes: {},
			include: true,
			pinned: false,
			verified: true,
			confidence: 1,
			reason: "人工导入",
			delivery: null,
			delivery_source: "",
			estimated: false,
			...row,
			sku: String(row.sku),
			price_rub: Number(row.price_rub),
		} as Competitor;
	});
}

export function exportBundleCSV(
	storeName: string,
	product: Product,
	contexts: import("./types").ContextResult[],
	sales: import("./types").SalesEvaluation[],
): string {
	const heading = (row: unknown[]) => row.map(csvCell).join(",");
	const chunks = [
		heading(["店铺", storeName]),
		heading(["商品", product.name]),
		heading(["用途", "自动定价与竞争力分析 Demo；不执行平台改价"]),
	];
	for (const c of contexts) {
		chunks.push(
			"",
			heading(["定价情景", c.name, c.state]),
			heading(["情景状态", c.result.status]),
		);
		if (c.result.recommended) {
			chunks.push(
				exportCSV(c.input.product, c.result, c.input.snapshot).replace(
					/^\ufeff/,
					"",
				),
			);
		} else {
			chunks.push(heading(["待补充或限制", ...(c.result.messages || [])]));
		}
	}
	chunks.push(
		"",
		heading([
			"销量情景",
			"所属活动",
			"标价 CNY",
			"预计日销量",
			"单件预计利润",
			"预计日利润",
			"压力日利润",
			"满足约束",
			"假设依据",
		]),
	);
	for (const r of sales)
		chunks.push(
			heading([
				r.scenario.name,
				r.scenario.promotion_id || "日常",
				r.scenario.price,
				r.scenario.daily_units,
				r.evaluation?.profit,
				r.daily_profit,
				r.worst_daily_profit,
				r.meets_constraints ? "是" : "否",
				r.scenario.note,
			]),
		);
	chunks.push(
		heading(["销量口径", "商家假设；不预测销量，未填写数量不生成日利润"]),
	);
	return "\ufeff" + chunks.join("\r\n");
}
