import {
	Alert,
	Button,
	Drawer,
	Form,
	Input,
	InputNumber,
	Popconfirm,
	Select,
	Space,
	Table,
	Tag,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import { useState } from "react";
import type {
	Bootstrap,
	Product,
	SalesScenario,
	SalesEvaluation,
	Snapshot,
} from "../types";
import { money, useStoreApi } from "../lib";
export default function SalesPanel({
	data,
	product,
	snapshot,
	results,
	onSaved,
	onResults,
	onError,
}: {
	data: Bootstrap;
	product: Product;
	snapshot?: Snapshot;
	results: SalesEvaluation[];
	onSaved: () => Promise<void>;
	onResults: (r: SalesEvaluation[]) => void;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [editing, setEditing] = useState<SalesScenario | null | undefined>();
	const [saving, setSaving] = useState(false);
	const [evaluating, setEvaluating] = useState(false);
	const [form] = Form.useForm<SalesScenario>();
	const rows = data.sales_scenarios.filter((s) => s.product_id === product.id);
	const routes = data.quotes.filter((q) => q.kind === "international");
	const open = (s?: SalesScenario) => {
		setEditing(s ?? null);
		form.resetFields();
		form.setFieldsValue(
			s ?? {
				name: "",
				product_id: product.id,
				promotion_id: "",
				price: undefined,
				daily_units: null,
				quote_id:
					product.route_ids?.[0] ||
					routes.find((q) => q.status === "active")?.id,
				note: "",
			},
		);
	};
	const save = async () => {
		try {
			const s = await form.validateFields();
			setSaving(true);
			await api(
				`/sales-scenarios${editing ? `/${editing.id}` : ""}`,
				{ ...s, product_id: product.id, daily_units: s.daily_units ?? null },
				editing ? "PUT" : "POST",
			);
			await onSaved();
			setEditing(undefined);
		} catch (e) {
			if (e instanceof Error) onError(e.message);
		} finally {
			setSaving(false);
		}
	};
	const evaluate = async () => {
		try {
			setEvaluating(true);
			const v = await api<{ evaluations: SalesEvaluation[] }>(
				"/sales-scenarios/evaluate",
				{ product, snapshot_id: snapshot?.id || "" },
			);
			onResults(v.evaluations);
		} catch (e) {
			onError((e as Error).message);
		} finally {
			setEvaluating(false);
		}
	};
	return (
		<section className="sales-panel">
			<div className="section-toolbar">
				<div>
					<h2>销量情景比较</h2>
					<p>填写你预计的销量，查看不同价格下的日利润。</p>
				</div>
				<Space wrap>
					<Button
						aria-label="新增销量情景"
						icon={<PlusOutlined />}
						onClick={() => open()}
					>
						新增情景
					</Button>
					<Button
						aria-label="评估销量情景"
						loading={evaluating}
						disabled={!rows.length}
						onClick={evaluate}
					>
						评估情景
					</Button>
				</Space>
			</div>
			<Alert
				type="info"
				showIcon
				className="spaced-alert"
				title="商家假设：不预测销量，不改变四种模式的建议；日利润已包含单件费用中的广告及经营成本。"
			/>
			<Table<SalesScenario>
				rowKey="id"
				dataSource={rows}
				pagination={false}
				scroll={{ x: 950 }}
				locale={{ emptyText: "可选：录入不同售价及预计日销量后比较收益" }}
				columns={[
					{
						title: "情景／活动",
						render: (_, s) => (
							<>
								<strong>{s.name || "销量假设"}</strong>
								<small className="block muted-text">
									{data.promotions.find((p) => p.id === s.promotion_id)?.name ||
										"日常"}{" "}
									· {routes.find((q) => q.id === s.quote_id)?.name}
								</small>
							</>
						),
					},
					{ title: "标价", render: (_, s) => money(s.price) },
					{ title: "预计日销量", render: (_, s) => s.daily_units ?? "未填写" },
					{
						title: "单件预计利润",
						render: (_, s) =>
							money(
								results.find((r) => r.scenario.id === s.id)?.evaluation?.profit,
							),
					},
					{
						title: "预计日利润",
						render: (_, s) =>
							money(results.find((r) => r.scenario.id === s.id)?.daily_profit),
					},
					{
						title: "压力日利润",
						render: (_, s) =>
							money(
								results.find((r) => r.scenario.id === s.id)?.worst_daily_profit,
							),
					},
					{
						title: "约束与状态",
						render: (_, s) => {
							const r = results.find((r) => r.scenario.id === s.id);
							return r ? (
								<>
									<Tag color={r.meets_constraints ? "green" : "gold"}>
										{r.meets_constraints ? "符合约束" : "仅供比较"}
									</Tag>
									<small className="block muted-text">
										{r.messages
											.filter((m) => !m.startsWith("商家假设"))
											.join("；")}
									</small>
								</>
							) : (
								"待评估"
							);
						},
					},
					{
						title: "操作",
						render: (_, s) => (
							<Space>
								<Button size="small" onClick={() => open(s)}>
									编辑
								</Button>
								<Popconfirm
									title="删除此销量假设？"
									onConfirm={async () => {
										try {
											await api(
												`/sales-scenarios/${s.id}`,
												undefined,
												"DELETE",
											);
											await onSaved();
										} catch (e) {
											onError((e as Error).message);
										}
									}}
								>
									<Button size="small" danger>
										删除
									</Button>
								</Popconfirm>
							</Space>
						),
					},
				]}
			/>
			<Drawer
				title={editing ? "编辑销量情景" : "新增销量情景"}
				open={editing !== undefined}
				onClose={() => setEditing(undefined)}
				width={500}
				extra={
					<Button
						aria-label="保存销量情景"
						type="primary"
						loading={saving}
						onClick={save}
					>
						保存销量情景
					</Button>
				}
			>
				<Form name="sales-editor" form={form} layout="vertical">
					<Form.Item name="name" label="情景名称" rules={[{ required: true }]}>
						<Input />
					</Form.Item>
					<Form.Item name="promotion_id" label="定价情景">
						<Select
							options={[
								{ value: "", label: "日常" },
								...data.promotions.map((p) => ({ value: p.id, label: p.name })),
							]}
						/>
					</Form.Item>
					<Form.Item
						name="price"
						label="假设标价 CNY"
						rules={[{ required: true }]}
					>
						<InputNumber min={0.01} max={1000000} precision={2} />
					</Form.Item>
					<Form.Item
						name="quote_id"
						label="物流渠道"
						rules={[{ required: true }]}
					>
						<Select
							options={routes.map((q) => ({
								value: q.id,
								label: `${q.name}${q.status !== "active" ? " · 不可用" : ""}`,
							}))}
						/>
					</Form.Item>
					<Form.Item
						name="daily_units"
						label="预计日销量"
						extra="可留空或填0；小数表示平均预期销量。"
					>
						<InputNumber
							min={0}
							max={10000000}
							precision={3}
							placeholder="未填写时不生成日利润"
						/>
					</Form.Item>
					<Form.Item name="note" label="假设依据">
						<Input.TextArea
							rows={3}
							placeholder="商家经验／演示假设，非销量预测"
						/>
					</Form.Item>
				</Form>
			</Drawer>
		</section>
	);
}
