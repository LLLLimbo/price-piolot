import {
	Alert,
	Button,
	Collapse,
	Empty,
	Progress,
	Table,
	Tabs,
	Tag,
} from "antd";
import {
	ArrowRightOutlined,
	CheckCircleFilled,
	DownloadOutlined,
	InfoCircleOutlined,
} from "@ant-design/icons";
import type { Result, Evaluation, Product, Snapshot } from "../types";
import { money, pct, modes, time, exportCSV, download } from "../lib";

export default function Results({
	result,
	product,
	snapshot,
	dirty,
}: {
	result?: Result;
	product: Product;
	snapshot?: Snapshot;
	dirty: boolean;
}) {
	const e = result?.recommended;
	if (!result)
		return (
			<div className="result-empty">
				<Empty description="补充商品成本后，计算售价与物流方案" />
			</div>
		);
	if (!e)
		return (
			<div className="infeasible">
				<div className="inline-heading">
					<h2>
						{["invalid", "incomplete"].includes(result.status)
							? "资料待补充，暂无法完整计算"
							: "当前目标暂无可行方案"}
					</h2>
					<Tag>约束未放宽</Tag>
				</div>
				{result.messages.map((m) => (
					<p key={m}>{m}</p>
				))}
				<div className="limits">
					<span>
						预算内最高可达{" "}
						<strong>
							{result.max_score?.toFixed(1) ?? "待补充数据"}
							{result.max_score != null && "分"}
						</strong>
					</span>
					<span>
						供应商固定时理论上限{" "}
						<strong>{result.theoretical_max.toFixed(1)}分</strong>
					</span>
				</div>
				<div className="attention-links">
					<a href="/settings">补充费率</a>
					<a href="/quotes">检查报价</a>
					<a href="/promotions">检查活动</a>
				</div>
				<Unavailable result={result} />
			</div>
		);
	const exportResult = () =>
		download(
			exportCSV(product, result, snapshot),
			`PricePilot-${product.name}-${new Date().toISOString().slice(0, 10)}.csv`,
		);
	return (
		<div className="results" aria-live="polite">
			{dirty && (
				<Alert
					type="warning"
					title="参数已更改，下方为上一次计算结果。请重新计算。"
					showIcon
					className="spaced-alert"
				/>
			)}
			<div className="recommendation">
				<div className="recommendation-main">
					<div className="result-overline">
						<CheckCircleFilled />
						<span>{modes[result.mode]?.name} · 推荐方案</span>
						{result.demo && <Tag bordered={false}>演示测算</Tag>}
					</div>
					<div className="price-line">
						<span className="price-value">{money(e.price)}</span>
						<span className="price-caption">建议标价 / 件</span>
					</div>
					<div className="rub-price">
						{money(e.price_rub, "RUB")}
						<span>
							买家商品参考价 · 1 CNY ≈{" "}
							{(
								e.scenarios.find((s) => s.label === "当前汇率")?.fx || 0
							).toFixed(4)}{" "}
							RUB
						</span>
					</div>
					{e.prices && e.prices.list_cny > 0 && (
						<div className="price-breakdown">
							<span>
								商家折后成交价 <strong>{money(e.sale_price)}</strong>
							</span>
							<span>
								预计留存销售收入 <strong>{money(e.revenue)}</strong>
							</span>
							{e.prices.platform_discount_cny > 0 && (
								<span>
									平台等额补偿优惠{" "}
									<strong>{money(e.prices.platform_discount_cny)}</strong>
								</span>
							)}
						</div>
					)}
					<div className="route-line">
						<span>{e.warehouse}</span>
						<ArrowRightOutlined />
						<strong>{e.route}</strong>
					</div>
					<div className="profit-line">
						<div>
							<span>单件预计净利润</span>
							<strong
								className={
									e.profit < 0 ? "negative numeric" : "positive numeric"
								}
							>
								{money(e.profit)}
							</strong>
						</div>
						<div>
							<span>预计净利率</span>
							<strong className="numeric">{pct(e.margin)}</strong>
						</div>
						<div>
							<span>完整妥投时效</span>
							<strong className="numeric">
								{e.delivery
									? `${e.delivery.min}–${e.delivery.max} 天`
									: "待补充"}
							</strong>
						</div>
					</div>
				</div>
				<div className="score-panel">
					<div className="inline-heading">
						<h3>综合竞争力</h3>
						<Tag bordered={false}>
							{e.score.estimated ? "含估算" : "当前快照"}
						</Tag>
					</div>
					<div className="score-main">
						<strong>
							{e.score.total == null
								? `${e.score.minimum.toFixed(0)}–${e.score.maximum.toFixed(0)}`
								: e.score.total.toFixed(1)}
						</strong>
						<span>/100</span>
					</div>
					<div className="score-bars">
						{[
							["价格", e.score.price],
							["时效", e.score.delivery],
							["供应商", e.score.supplier],
						].map(([name, value]) => (
							<div className="score-bar" key={String(name)}>
								<span>{name}</span>
								<Progress
									percent={typeof value === "number" ? value : 0}
									showInfo={false}
									strokeColor={name === "供应商" ? "#608b76" : "#0067c0"}
									size="small"
								/>
								<strong>
									{value == null ? "待补充" : Number(value).toFixed(0)}
								</strong>
							</div>
						))}
					</div>
					<p>
						市场子分 {e.score.market?.toFixed(1) ?? "待补充"} / 100
						<br />
						{e.score.samples} 个有效样本 · 内部经营评分
					</p>
				</div>
			</div>
			<div className="stress-strip">
				<InfoCircleOutlined />
				<span>汇率压力情景</span>
				<span>
					最低预计利润{" "}
					<strong className={e.worst_profit < 0 ? "negative" : ""}>
						{money(e.worst_profit)}
					</strong>
				</span>
				<span>
					最低净利率 <strong>{pct(e.worst_margin)}</strong>
				</span>
				<Button
					aria-label="导出测算"
					type="text"
					size="small"
					icon={<DownloadOutlined />}
					onClick={exportResult}
					disabled={dirty}
				>
					导出测算
				</Button>
			</div>
			<div className="section-toolbar slim">
				<div>
					<h3>物流方案对比</h3>
					<p>同一商品、成本口径与竞品快照</p>
				</div>
				<span className="muted-text">{time(result.calculated_at)}</span>
			</div>
			<Table<Evaluation>
				className="alternatives"
				size="middle"
				rowKey="quote_id"
				dataSource={result.alternatives}
				pagination={false}
				scroll={{ x: 650 }}
				rowClassName={(row) =>
					row.quote_id === e.quote_id ? "recommended-row" : ""
				}
				columns={[
					{
						title: "配送方案",
						width: 205,
						render: (_, row) => (
							<>
								<strong>{row.route}</strong>
								{row.quote_id === e.quote_id && (
									<Tag color="blue" bordered={false}>
										推荐
									</Tag>
								)}
								<small className="block muted-text">
									{row.delivery
										? `${row.delivery.min}–${row.delivery.max}天 · 全链路`
										: "妥投时效待补充"}
								</small>
							</>
						),
					},
					{
						title: "建议售价",
						render: (_, row) => (
							<span className="numeric">{money(row.price)}</span>
						),
					},
					{
						title: "预计利润",
						render: (_, row) => (
							<span
								className={`${row.profit < 0 ? "negative" : "positive"} numeric`}
							>
								{money(row.profit)}
							</span>
						),
					},
					{
						title: "最低净利率",
						render: (_, row) => (
							<span className="numeric">{pct(row.worst_margin)}</span>
						),
					},
					{
						title: "竞争力",
						render: (_, row) => (
							<strong className="numeric">
								{row.score.total?.toFixed(1) ??
									`${row.score.minimum.toFixed(0)}–${row.score.maximum.toFixed(0)}`}
							</strong>
						),
					},
				]}
			/>
			<Tabs
				className="detail-tabs"
				items={[
					{
						key: "costs",
						label: "费用与公式",
						children: (
							<>
								<div className="calculation-formula">
									<span>按实际数值计算</span>
									{e.prices?.formula && <p>{e.prices.formula}</p>}
									<p>{e.formula}</p>
									<small>
										取消、拒收与签收后退货分别计算；费用按发生条件折算为每次下单的预期成本。商品金额不包含买家另付运费。
									</small>
									<small className="block">
										建议标价是本工具的优惠前试算基价，不是Ozon划线价。买家参考价未计平台区域及推广加价；平台补偿按等额抵扣计入经济测算，不重复增加现金收入。
									</small>
								</div>
								<Table
									size="small"
									pagination={false}
									rowKey="code"
									dataSource={e.lines}
									scroll={{ x: 720 }}
									columns={[
										{ title: "费用项", dataIndex: "name", width: 160 },
										{
											title: "预期成本 CNY",
											render: (_, l) => (
												<span className="numeric">
													{l.included ? (
														<Tag bordered={false}>报价已含</Tag>
													) : (
														money(l.cny, "CNY", 4)
													)}
												</span>
											),
											width: 150,
										},
										{ title: "计算式", dataIndex: "formula" },
										{
											title: "来源",
											dataIndex: "source",
											render: (
												value: string,
												line: Evaluation["lines"][number],
											) => (
												<>
													{value}
													<small className="block muted-text">
														{line.version ? `v${line.version}` : "历史未标注"} ·{" "}
														{line.valid_from || "不限"} ～{" "}
														{line.valid_to || "不限"}
													</small>
												</>
											),
											ellipsis: true,
										},
									]}
								/>
							</>
						),
					},
					{
						key: "stress",
						label: "汇率压力",
						children: (
							<>
								<p className="muted-text">
									人民币收入按合同口径计算。区间端点与内部费率临界点分别重算，以下均为同一建议标价。
								</p>
								<Table
									size="small"
									rowKey={(_, i) => String(i)}
									pagination={false}
									scroll={{ x: 540 }}
									dataSource={e.scenarios}
									columns={[
										{ title: "情景", dataIndex: "label" },
										{
											title: "CNY → RUB",
											render: (_, s) => (
												<span className="numeric">{s.fx.toFixed(7)}</span>
											),
										},
										{
											title: "预计利润",
											render: (_, s) =>
												s.valid ? (
													<span
														className={
															s.profit < 0 ? "negative numeric" : "numeric"
														}
													>
														{money(s.profit, "CNY", 4)}
													</span>
												) : (
													s.reason
												),
										},
										{
											title: "净利率",
											render: (_, s) => (s.valid ? pct(s.margin) : "不可用"),
										},
									]}
								/>
							</>
						),
					},
					{
						key: "notes",
						label: "数据与适用条件",
						children: (
							<div className="data-notes">
								<p>
									竞品快照：{snapshot?.id || result.snapshot_id || "暂无"}
									{snapshot && ` · ${time(snapshot.fetched_at)}`}
								</p>
								<p>
									价格口径：Ozon商品展示价，未含买家运费。评分综合市场比较和1688供应商可靠性，不代表成交概率。
								</p>
								<p>
									供应商固定时理论总分上限：{e.score.theoretical_max.toFixed(2)}
									分。
								</p>
								{e.warnings.map((w) => (
									<p key={w}>• {w}</p>
								))}
								{result.messages.map((m) => (
									<p key={m}>{m}</p>
								))}
							</div>
						),
					},
				]}
			/>
			<Unavailable result={result} />
		</div>
	);
}
function Unavailable({ result }: { result: Result }) {
	return result.unavailable.length ? (
		<Collapse
			ghost
			items={[
				{
					key: "unavailable",
					label: `${result.unavailable.length} 个未采用方案及限制`,
					children: (
						<ul className="unavailable-list">
							{result.unavailable.map((q) => (
								<li key={q.quote_id}>
									<strong>{q.name}</strong>
									<span>{q.reason}</span>
								</li>
							))}
						</ul>
					),
				},
			]}
		/>
	) : null;
}
