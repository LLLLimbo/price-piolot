import { Alert, Button, Descriptions, Spin, Table, Tag } from "antd";
import { useEffect, useState } from "react";
import { money, pct, useStoreApi } from "../lib";
import type { Product, ProfitCheckResult } from "../types";

export default function ProfitCheckPanel({ product }: { product: Product }) {
	const api = useStoreApi();
	const [result, setResult] = useState<ProfitCheckResult>();
	const [error, setError] = useState("");
	const [retry, setRetry] = useState(0);
	const sample = product.profit_check;
	const encoded = JSON.stringify(sample);
	useEffect(() => {
		let active = true;
		setResult(undefined);
		setError("");
		if (!encoded) return;
		const timer = window.setTimeout(() => {
			api<ProfitCheckResult>("/profit-checks/evaluate", {
				product_id: product.id,
				sample: JSON.parse(encoded),
			})
				.then((r) => {
					if (active) setResult(r);
				})
				.catch((e) => {
					if (active) setError((e as Error).message);
				});
		}, 200);
		return () => {
			active = false;
			window.clearTimeout(timer);
		};
	}, [api, product.id, encoded, retry]);
	if (!sample) return null;
	return (
		<section className="profit-check-panel" aria-label="订单利润校验">
			<div className="panel-title">
				<div>
					<h2>订单利润校验</h2>
					<span className="muted-text">仅核对样本所列费用 · CNY</span>
				</div>
				<Tag
					color={
						result?.status === "matched"
							? "green"
							: result?.status === "different"
								? "orange"
								: "default"
					}
				>
					{result?.status === "matched"
						? "同口径一致"
						: result?.status === "different"
							? "存在差异"
							: result
								? "资料待补充"
								: error
									? "校验失败"
									: "资料校验中"}
				</Tag>
			</div>
			<p className="muted-text">
				订单 {sample.order_ref || "未填写"} · {sample.date || "日期待补充"} ·{" "}
				{sample.fee_label || "费用方案待补充"}
			</p>
			<div className="profit-check-values">
				<div>
					<span>来源参考利润</span>
					<strong>{money(sample.reported_profit)}</strong>
				</div>
				<div>
					<span>引擎样本利润</span>
					<strong>{money(result?.profit)}</strong>
				</div>
				<div>
					<span>差额（引擎 − 来源）</span>
					<strong>{money(result?.difference)}</strong>
				</div>
			</div>
			{!result && !error && <Spin size="small" />}
			{error && (
				<Alert
					type="error"
					showIcon
					title={error}
					action={
						<Button size="small" onClick={() => setRetry((v) => v + 1)}>
							重试校验
						</Button>
					}
				/>
			)}
			{result?.formula && (
				<p className="profit-check-formula">{result.formula}</p>
			)}
			{result && (
				<Alert
					type={result.status === "matched" ? "info" : "warning"}
					showIcon
					title={
						result.status === "matched"
							? "四项费用计算一致，实际结算与完整利润仍待核对"
							: result.status === "different"
								? "样本利润存在差异，请核对费用口径"
								: "样本资料待补充"
					}
					description={
						<ul>
							{result.messages.map((m) => (
								<li key={m}>{m}</li>
							))}
						</ul>
					}
				/>
			)}
			<details className="product-source">
				<summary>样本费用、来源与校验范围</summary>
				<Descriptions
					column={1}
					size="small"
					items={[
						{
							key: "amount",
							label: "订单金额",
							children: money(sample.amount),
						},
						{
							key: "route",
							label: "承运渠道",
							children: sample.carrier || "待补充",
						},
						{
							key: "freight",
							label: "运费口径",
							children:
								sample.freight_kind === "settled"
									? "已结算运费"
									: sample.freight_kind === "estimated"
										? "预计运费"
										: "待补充",
						},
						{
							key: "fees",
							label: "费用依据",
							children:
								sample.fee_kind === "prototype"
									? "原型方案"
									: sample.fee_kind === "statement"
										? "平台扣费账单"
										: sample.fee_kind === "manual"
											? "商家手填假设"
											: "待补充",
						},
						{
							key: "rate",
							label: "样本佣金率",
							children: pct(sample.commission_rate),
						},
						{
							key: "source",
							label: "资料来源",
							children: sample.source || "待补充",
						},
					]}
				/>
				{result && (
					<Table
						rowKey="code"
						size="small"
						pagination={false}
						dataSource={result.lines}
						columns={[
							{ title: "所列费用", dataIndex: "name" },
							{ title: "金额 CNY", render: (_, l) => money(l.cny) },
							{ title: "计算式", dataIndex: "formula" },
						]}
						scroll={{ x: 430 }}
					/>
				)}
				<p className="muted-text">
					不套用商品整体的取消／拒收／退货概率，不把未列费用视为已确认零费用；样本运费不作为其他重量、价格或物流渠道的通用报价。
				</p>
			</details>
		</section>
	);
}
