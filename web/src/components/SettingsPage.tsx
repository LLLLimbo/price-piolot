import {
	Alert,
	Button,
	Checkbox,
	Collapse,
	Form,
	Input,
	InputNumber,
	Select,
	Space,
	Tabs,
	Tag,
} from "antd";
import { DeleteOutlined, PlusOutlined, SaveOutlined } from "@ant-design/icons";
import { useState } from "react";
import type { Bootstrap, Settings, Quote } from "../types";
import { useStoreApi } from "../lib";
import { FeeEditor, PercentInput } from "./Fields";

export default function SettingsPage({
	data,
	onSaved,
	onError,
}: {
	data: Bootstrap;
	onSaved: () => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [form] = Form.useForm<Settings>();
	const [saving, setSaving] = useState(false);
	const [quota, setQuota] = useState<string>("");
	const [checking, setChecking] = useState(false);
	const save = async () => {
		try {
			const value = await form.validateFields();
			value.category_fees = value.category_fees || {};
			setSaving(true);
			await api("/settings", value, "PUT");
			await onSaved();
		} catch (e) {
			if (e instanceof Error) onError(e.message);
		} finally {
			setSaving(false);
		}
	};
	return (
		<div className="page-surface settings-page">
			<div className="page-heading">
				<div>
					<span className="eyebrow">PRICING PREFERENCES</span>
					<h1>费率与评分设置</h1>
					<p>为每次测算提供明确的费用、汇率和比较口径。</p>
				</div>
				<Button
					type="primary"
					icon={<SaveOutlined />}
					loading={saving}
					onClick={save}
				>
					保存设置
				</Button>
			</div>
			<Form
				name="settings-editor"
				form={form}
				layout="vertical"
				initialValues={data.settings}
				requiredMark={false}
			>
				<Alert
					className="spaced-alert"
					type={data.settings.demo ? "warning" : "info"}
					showIcon
					title={
						data.settings.demo
							? "当前使用演示费率。用于真实商品前，请逐项核对来源并关闭下方演示标识。"
							: "目标净利率和亏损额度均按当前成本假设与指定汇率区间验证。"
					}
				/>
				<Tabs
					items={[
						{
							key: "general",
							label: "汇率与权重",
							children: (
								<div className="settings-grid">
									<section>
										<h2>人民币结算与汇率保护</h2>
										<p className="muted-text">
											汇率单位为1 CNY兑换的RUB；人民币收入按合同口径计算。
											销售及佣金分档使用下单日口径；部分服务按提供日汇率收费。当前为统一汇率预算，非订单对账。
										</p>
										<div className="two-fields">
											<Form.Item
												name="fx"
												label="参考汇率 CNY → RUB"
												rules={[{ required: true }]}
											>
												<InputNumber min={0.0001} precision={6} />
											</Form.Item>
											<Form.Item
												name="fx_date"
												label="汇率日期"
												rules={[{ required: true }]}
											>
												<Input type="date" />
											</Form.Item>
										</div>
										<Form.Item
											name="fx_source"
											label="汇率来源"
											rules={[{ required: true }]}
										>
											<Input placeholder="Ozon合同口径／商家确认" />
										</Form.Item>
										<Form.Item name="stress" label="汇率上下波动幅度">
											<PercentInput max={49.99} />
										</Form.Item>
										<Form.Item
											name="region"
											label="目标地区"
											rules={[{ required: true }]}
										>
											<Input placeholder="莫斯科" />
										</Form.Item>
										<Form.Item
											name="snapshot_hours"
											label="竞品快照有效期 / 小时"
										>
											<InputNumber min={1} max={720} />
										</Form.Item>
										<Form.Item name="demo" valuePropName="checked">
											<Checkbox>当前费率为演示假设</Checkbox>
										</Form.Item>
									</section>
									<section>
										<h2>综合竞争力权重</h2>
										<p className="muted-text">
											三项合计100%；供应商可靠性与市场子分分别展示。
										</p>
										<Form.Item name={["weights", "price"]} label="价格权重">
											<InputNumber min={0} max={100} suffix="%" />
										</Form.Item>
										<Form.Item
											name={["weights", "delivery"]}
											label="物流时效权重"
										>
											<InputNumber min={0} max={100} suffix="%" />
										</Form.Item>
										<Form.Item
											name={["weights", "supplier"]}
											label="1688供应商权重"
										>
											<InputNumber min={0} max={100} suffix="%" />
										</Form.Item>
										<div className="source-status">
											<h3>竞品数据源</h3>
											<Space>
												<Tag
													color={data.seerfar.configured ? "green" : "default"}
												>
													Seerfar{" "}
													{data.seerfar.configured ? "已配置" : "未配置"}
												</Tag>
												{data.seerfar.mock && <Tag>Mock测试环境</Tag>}
											</Space>
											<p>API凭证由服务端环境配置，页面不展示密钥。</p>
											<Button
												loading={checking}
												disabled={!data.seerfar.configured}
												onClick={async () => {
													try {
														setChecking(true);
														const v = await api<{ data: unknown }>(
															"/seerfar/quota",
														);
														setQuota(
															(() => {
																const q = v.data as {
																	creditLimit?: number;
																	creditUsed?: number;
																};
																return `剩余积分 ${Math.max(0, (q.creditLimit || 0) - (q.creditUsed || 0))} · 已用 ${q.creditUsed || 0} / ${q.creditLimit || 0}`;
															})(),
														);
													} catch (e) {
														onError((e as Error).message);
													} finally {
														setChecking(false);
													}
												}}
											>
												查询接口积分
											</Button>
											{quota && <p className="quota-value">{quota}</p>}
										</div>
									</section>
								</div>
							),
						},
						{
							key: "fees",
							label: "平台与经营费用",
							children: (
								<div className="fees-settings">
									<p className="muted-text">
										默认费率用于未单独配置的类目。类目佣金按商家优惠后、积分折扣前的价格计费，分档使用下单日卢布货值。银行收单费由银行确定，独立于物流平台电子服务费。点击广告可按全部原始订单分摊；月费、推广等按已确认的发生条件预算。
									</p>
									<Form.Item name="fees" noStyle>
										<FeeEditor quotes={data.quotes} />
									</Form.Item>
									<div className="section-divider" />
									<h2>按Ozon计费类目覆盖</h2>
									<p className="muted-text">
										覆盖项为该类目的完整平台费用配置；商品采购和仓配费用仍独立计算。
									</p>
									<Form.Item name="category_fees" noStyle>
										<CategoryFees quotes={data.quotes} />
									</Form.Item>
								</div>
							),
						},
						{
							key: "delivery",
							label: "参考时效",
							children: (
								<>
									<p className="muted-text">
										Seerfar未提供实际配送天数。这里仅填写有来源的完整妥投参考区间，结果会明确标记为估算。
									</p>
									<Form.List name="delivery_references">
										{(fields, { add, remove }) => (
											<>
												{fields.map((f) => (
													<div className="reference-editor" key={f.key}>
														<div className="three-fields">
															<Form.Item
																name={[f.name, "fulfillment"]}
																label="履约类型"
															>
																<Select
																	options={[
																		"FBO",
																		"FBS",
																		"RFBS",
																		"FBP",
																		"OZON",
																	].map((value) => ({ value, label: value }))}
																/>
															</Form.Item>
															<Form.Item
																name={[f.name, "region"]}
																label="目标地区"
															>
																<Input />
															</Form.Item>
															<Button
																aria-label="删除参考时效"
																icon={<DeleteOutlined />}
																onClick={() => remove(f.name)}
															/>
														</div>
														<div className="two-fields">
															<Form.Item
																name={[f.name, "days", "min"]}
																label="最短天数"
															>
																<InputNumber min={0} />
															</Form.Item>
															<Form.Item
																name={[f.name, "days", "max"]}
																label="最长天数"
															>
																<InputNumber min={0} />
															</Form.Item>
														</div>
														<Form.Item
															name={[f.name, "source"]}
															label="区间来源"
														>
															<Input placeholder="近期样本／商品页记录／商家假设" />
														</Form.Item>
													</div>
												))}
												<Button
													aria-label="添加参考时效"
													icon={<PlusOutlined />}
													onClick={() =>
														add({
															fulfillment: "RFBS",
															region: data.settings.region,
															days: { min: 0, max: 0 },
															source: "",
														})
													}
												>
													添加参考时效
												</Button>
											</>
										)}
									</Form.List>
								</>
							),
						},
						{
							key: "mapping",
							label: "类目映射",
							children: (
								<>
									<p className="muted-text">
										1688、Seerfar与Ozon的类目ID分别维护。填写1688类目后，可自动关联检索类目和费用配置。
									</p>
									<Form.List name="category_mappings">
										{(fields, { add, remove }) => (
											<>
												{fields.map((f) => (
													<div className="reference-editor" key={f.key}>
														<Form.Item name={[f.name, "name"]} label="类目名称">
															<Input />
														</Form.Item>
														<div className="three-fields">
															<Form.Item
																name={[f.name, "alibaba"]}
																label="1688类目ID"
															>
																<Input />
															</Form.Item>
															<Form.Item
																name={[f.name, "seerfar"]}
																label="Seerfar类目ID"
															>
																<Input />
															</Form.Item>
															<Form.Item
																name={[f.name, "ozon"]}
																label="Ozon计费类目ID"
															>
																<Input />
															</Form.Item>
														</div>
														<Button
															size="small"
															danger
															icon={<DeleteOutlined />}
															onClick={() => remove(f.name)}
														>
															删除映射
														</Button>
													</div>
												))}
												<Button
													icon={<PlusOutlined />}
													onClick={() =>
														add({
															name: "",
															alibaba: "",
															seerfar: "",
															ozon: "",
														})
													}
												>
													添加类目映射
												</Button>
											</>
										)}
									</Form.List>
								</>
							),
						},
					]}
				/>
			</Form>
		</div>
	);
}
function CategoryFees({
	value = {},
	onChange,
	quotes,
}: {
	quotes: Quote[];
	value?: Record<string, Settings["fees"]>;
	onChange?: (v: Record<string, Settings["fees"]>) => void;
}) {
	const [rows, setRows] = useState(
		Object.entries(value).map(([key, fees]) => ({ key, fees })),
	);
	const update = (next: typeof rows) => {
		setRows(next);
		onChange?.(
			Object.fromEntries(
				next.filter((r) => r.key.trim()).map((r) => [r.key.trim(), r.fees]),
			),
		);
	};
	return (
		<>
			<Collapse
				items={rows.map((row, i) => ({
					key: i,
					label: row.key || "新类目费率",
					children: (
						<>
							<div className="section-toolbar">
								<Input
									value={row.key}
									placeholder="Ozon计费类目ID"
									onChange={(e) =>
										update(
											rows.map((r, j) =>
												i === j ? { ...r, key: e.target.value } : r,
											),
										)
									}
								/>
								<Button
									danger
									icon={<DeleteOutlined />}
									onClick={() => update(rows.filter((_, j) => i !== j))}
								>
									删除
								</Button>
							</div>
							<FeeEditor
								quotes={quotes}
								value={row.fees}
								onChange={(fees) =>
									update(rows.map((r, j) => (i === j ? { ...r, fees } : r)))
								}
							/>
						</>
					),
				}))}
			/>
			<Button
				className="top-gap"
				icon={<PlusOutlined />}
				onClick={() => update([...rows, { key: "", fees: [] }])}
			>
				添加类目费率
			</Button>
		</>
	);
}
