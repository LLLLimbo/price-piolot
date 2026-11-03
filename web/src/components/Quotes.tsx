import {
	Alert,
	Button,
	Checkbox,
	Drawer,
	Form,
	Input,
	InputNumber,
	Select,
	Space,
	Table,
	Tag,
	Tooltip,
} from "antd";
import {
	CopyOutlined,
	DeleteOutlined,
	EditOutlined,
	LinkOutlined,
	PlusOutlined,
	SearchOutlined,
} from "@ant-design/icons";
import { useState } from "react";
import type { Quote } from "../types";
import { useStoreApi, clone } from "../lib";
import { DaysInput, FeeEditor } from "./Fields";

const statuses: Record<string, { name: string; color: string }> = {
	active: { name: "已启用", color: "green" },
	pending: { name: "待确认", color: "gold" },
	historical: { name: "历史参考", color: "default" },
	excluded: { name: "不适用Ozon", color: "red" },
};
export default function Quotes({
	quotes,
	onSaved,
	onError,
}: {
	quotes: Quote[];
	onSaved: () => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [editing, setEditing] = useState<Quote | null | undefined>();
	const [base, setBase] = useState<Quote>();
	const [versions, setVersions] = useState<Quote[] | undefined>();
	const [saving, setSaving] = useState(false);
	const [search, setSearch] = useState("");
	const [filter, setFilter] = useState("all");
	const [form] = Form.useForm<Quote>();
	const kind = Form.useWatch("kind", form);
	const open = (q?: Quote, copy = false) => {
		const value = q
			? clone(q)
			: ({
					id: "",
					name: "",
					provider: "",
					warehouse: "",
					kind: "international",
					status: "pending",
					demo: false,
					currency: "CNY",
					value_currency: "CNY",
					valid_from: "",
					valid_to: "",
					source: "",
					source_date: "",
					platforms: [],
					batteries: false,
					liquids: false,
					bands: [
						{
							up_to: null,
							min_weight: 1,
							max_weight: 0,
							fixed: 0,
							per_gram: 0,
							step: 1,
							volume_divisor: 0,
							minimum_weight: 0,
							max_length: 0,
							max_sum: 0,
						},
					],
					includes: [],
					fees: [],
					delivery: null,
					time_scope: "buyer",
					estimated: true,
					return_cost: null,
					return_currency: "CNY",
					note: "",
					compensation: "",
				} as Quote);
		if (copy) {
			value.id = "";
			value.name = `${value.name} · 我的协议`;
			value.demo = false;
			value.source = "";
			value.valid_from = "";
			value.valid_to = "";
			value.status = "pending";
		}
		setBase(value);
		setEditing(copy || !q ? null : q);
		form.resetFields();
		form.setFieldsValue(value);
	};
	const save = async () => {
		try {
			const q = { ...base, ...(await form.validateFields()) };
			q.id = editing?.id || "";
			q.bands = q.bands.map((b) => ({
				...b,
				up_to: b.up_to ?? null,
				min_weight: b.min_weight ?? 0,
				volume_divisor: b.volume_divisor ?? 0,
				minimum_weight: b.minimum_weight ?? 0,
				max_length: b.max_length ?? 0,
				max_sum: b.max_sum ?? 0,
			}));
			if (!q.platforms) q.platforms = [];
			if (!q.includes) q.includes = [];
			setSaving(true);
			await api(`/quotes${q.id ? `/${q.id}` : ""}`, q, q.id ? "PUT" : "POST");
			await onSaved();
			setEditing(undefined);
		} catch (e) {
			if (e instanceof Error) onError(e.message);
		} finally {
			setSaving(false);
		}
	};
	const data = quotes.filter(
		(q) =>
			(filter === "all" || q.status === filter || q.kind === filter) &&
			`${q.provider}${q.name}${q.warehouse}`
				.toLowerCase()
				.includes(search.toLowerCase()),
	);
	return (
		<div className="page-surface">
			<div className="page-heading">
				<div>
					<span className="eyebrow">LOGISTICS & RATES</span>
					<h1>物流与报价</h1>
					<p>企业、仓库与运输渠道分别记录。仅使用已确认且在有效期内的方案。</p>
				</div>
				<Button
					aria-label="新增协议报价"
					type="primary"
					icon={<PlusOutlined />}
					onClick={() => open()}
				>
					新增协议报价
				</Button>
			</div>
			<Alert
				showIcon
				type="info"
				title="公开资料仅作参考。确认适用平台、有效期、承运限制和附加费后，再启用为协议报价。"
				className="spaced-alert"
			/>
			<div className="section-toolbar">
				<Input
					prefix={<SearchOutlined />}
					placeholder="搜索服务商、仓库或渠道"
					value={search}
					onChange={(e) => setSearch(e.target.value)}
					style={{ maxWidth: 320 }}
					allowClear
				/>
				<Select
					aria-label="筛选报价"
					value={filter}
					onChange={setFilter}
					style={{ width: 150 }}
					options={[
						{ value: "all", label: "全部报价" },
						{ value: "active", label: "已启用" },
						{ value: "pending", label: "待确认" },
						{ value: "historical", label: "历史参考" },
						{ value: "excluded", label: "不适用Ozon" },
						{ value: "warehouse", label: "国内仓配" },
					]}
				/>
			</div>
			<Table<Quote>
				rowKey="id"
				dataSource={data}
				pagination={{ pageSize: 12, showSizeChanger: false }}
				scroll={{ x: 1000 }}
				columns={[
					{
						title: "版本",
						width: 80,
						render: (_, q) => (
							<Button
								size="small"
								type="link"
								onClick={async () => {
									try {
										const v = await api<{ versions: Quote[] }>(
											`/quotes/${q.id}/versions`,
										);
										setVersions(v.versions);
									} catch (e) {
										onError((e as Error).message);
									}
								}}
							>
								v{q.version || 1}
							</Button>
						),
					},
					{
						title: "服务商 / 渠道",
						width: 230,
						render: (_, q) => (
							<>
								<strong>{q.name}</strong>
								<small className="block muted-text">
									{q.provider}
									{q.warehouse && ` · ${q.warehouse}`}
								</small>
								{q.demo && <Tag bordered={false}>演示</Tag>}
							</>
						),
					},
					{
						title: "服务类型",
						width: 110,
						render: (_, q) =>
							q.kind === "warehouse" ? "国内仓配" : "跨境运输",
					},
					{
						title: "基础费率",
						width: 215,
						render: (_, q) => (
							<>
								<span className="numeric">
									{q.bands?.[0]
										? `${q.bands[0].fixed} + ${q.bands[0].per_gram} × g`
										: "待补充"}
								</span>
								<small className="block muted-text">
									{q.currency} · {q.bands?.length || 0} 个档位
								</small>
							</>
						),
					},
					{
						title: "有效期",
						width: 185,
						render: (_, q) => (
							<span className="muted-text">
								{q.valid_from || "待确认"}
								<br />
								{q.valid_to ? `至 ${q.valid_to}` : "到期日待确认"}
							</span>
						),
					},
					{
						title: "状态",
						width: 125,
						render: (_, q) => (
							<>
								<Tag color={statuses[q.status]?.color}>
									{statuses[q.status]?.name}
								</Tag>
								{q.valid_to &&
									q.valid_to < new Date().toISOString().slice(0, 10) && (
										<Tag>已过期</Tag>
									)}
							</>
						),
					},
					{ title: "适用说明", dataIndex: "note", width: 250, ellipsis: true },
					{
						title: "操作",
						fixed: "right",
						width: 120,
						render: (_, q) => (
							<Space size={0}>
								<Tooltip title="编辑报价">
									<Button
										type="text"
										aria-label={`编辑${q.name}`}
										icon={<EditOutlined />}
										onClick={() => open(q)}
									/>
								</Tooltip>
								<Tooltip title="另建我的协议">
									<Button
										type="text"
										aria-label={`复制${q.name}`}
										icon={<CopyOutlined />}
										onClick={() => open(q, true)}
									/>
								</Tooltip>
								{/^https?:\/\//.test(q.source) && (
									<Tooltip title="公开来源">
										<Button
											type="text"
											href={q.source}
											target="_blank"
											rel="noreferrer"
											aria-label={`查看${q.name}来源`}
											icon={<LinkOutlined />}
										/>
									</Tooltip>
								)}
							</Space>
						),
					},
				]}
			/>
			<Drawer
				title="报价版本"
				open={versions !== undefined}
				onClose={() => setVersions(undefined)}
				width={620}
			>
				<Table<Quote>
					rowKey="version"
					dataSource={versions}
					pagination={false}
					columns={[
						{ title: "版本", dataIndex: "version" },
						{ title: "来源", dataIndex: "source" },
						{ title: "起始", dataIndex: "valid_from" },
						{ title: "截止", dataIndex: "valid_to" },
						{ title: "状态", render: (_, q) => statuses[q.status]?.name },
					]}
				/>
			</Drawer>
			<Drawer
				open={editing !== undefined}
				title={editing ? "编辑报价与适用条件" : "新增协议报价"}
				onClose={() => setEditing(undefined)}
				width={720}
				destroyOnHidden
				extra={
					<Button
						aria-label="保存报价"
						type="primary"
						loading={saving}
						onClick={save}
					>
						保存报价
					</Button>
				}
			>
				<Form
					name="quote-editor"
					form={form}
					layout="vertical"
					requiredMark={false}
				>
					<div className="two-fields">
						<Form.Item
							name="provider"
							label="企业 / 服务商"
							rules={[{ required: true }]}
						>
							<Input />
						</Form.Item>
						<Form.Item name="warehouse" label="仓库 / 网点">
							<Input />
						</Form.Item>
					</div>
					<Form.Item name="name" label="方案名称" rules={[{ required: true }]}>
						<Input />
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="kind" label="服务类型">
							<Select
								options={[
									{ value: "international", label: "跨境运输" },
									{ value: "warehouse", label: "国内仓配" },
								]}
							/>
						</Form.Item>
						<Form.Item name="status" label="适用状态">
							<Select
								options={Object.entries(statuses).map(([value, s]) => ({
									value,
									label: s.name,
									disabled:
										editing?.status === "excluded" && value !== "excluded",
								}))}
							/>
						</Form.Item>
					</div>
					<Form.Item name="demo" valuePropName="checked">
						<Checkbox>演示报价（仅用于演示商品）</Checkbox>
					</Form.Item>
					<Form.Item name="platforms" label="适用平台">
						<Checkbox.Group options={[{ label: "Ozon", value: "ozon" }]} />
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="valid_from" label="生效日期">
							<Input type="date" />
						</Form.Item>
						<Form.Item name="valid_to" label="失效日期">
							<Input type="date" />
						</Form.Item>
					</div>
					<Form.Item
						name="source"
						label="报价来源"
						rules={[{ required: true }]}
					>
						<Input placeholder="合同编号、账单或公开来源链接" />
					</Form.Item>
					<Form.Item name="source_date" label="来源日期">
						<Input type="date" />
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="currency" label="收费币种">
							<Select
								options={["CNY", "RUB"].map((value) => ({
									value,
									label: value,
								}))}
							/>
						</Form.Item>
						<Form.Item name="value_currency" label="货值分档币种">
							<Select
								options={["CNY", "RUB"].map((value) => ({
									value,
									label: value,
								}))}
							/>
						</Form.Item>
					</div>
					<Form.Item label="承运属性">
						<Space>
							<Form.Item name="batteries" valuePropName="checked" noStyle>
								<Checkbox>允许带电</Checkbox>
							</Form.Item>
							<Form.Item name="liquids" valuePropName="checked" noStyle>
								<Checkbox>允许液体</Checkbox>
							</Form.Item>
						</Space>
					</Form.Item>
					<Form.Item name="includes" label="报价已包含的费用">
						<Checkbox.Group
							options={[
								{ value: "domestic_shipping", label: "国内运费" },
								{ value: "packaging", label: "包装" },
								{ value: "warehouse_operation", label: "仓内操作" },
								{ value: "tail_delivery", label: "尾程配送" },
							]}
						/>
					</Form.Item>
					<h3>另行收取的渠道及平台费用</h3>
					{base?.required_fees && (
						<p className="muted-text">
							必须确认：{Object.values(base.required_fees).join("、")}
							。不适用时请明确填0并保留依据，删除或关闭项目仍会提示待补充。
						</p>
					)}
					<p className="muted-text">
						与国际运费、银行收单费分别核对。每票最低及封顶按单件独立一票预算；未确认的费用会阻止完整推荐。同一费用请在此处或店铺费率中保留一项。
					</p>
					<Form.Item name="fees" noStyle>
						<FeeEditor attached />
					</Form.Item>
					<div className="inline-heading">
						<h3>货值、重量与计费档位</h3>
					</div>
					<p className="muted-text">
						按顺序匹配；上限包含边界。货值上限留空表示不限，尺寸限制填0表示未设置。
					</p>
					<Form.List name="bands">
						{(fields, { add, remove }) => (
							<>
								{fields.map((f, i) => (
									<div className="band-editor" key={f.key}>
										<div className="inline-heading">
											<strong>档位 {i + 1}</strong>
											<Button
												type="text"
												aria-label="删除报价档位"
												icon={<DeleteOutlined />}
												onClick={() => remove(f.name)}
											/>
										</div>
										<div className="three-fields">
											<Form.Item name={[f.name, "up_to"]} label="货值上限">
												<InputNumber min={0} placeholder="不限" />
											</Form.Item>
											<Form.Item
												name={[f.name, "min_weight"]}
												label="最小实重 / g"
											>
												<InputNumber min={0} />
											</Form.Item>
											<Form.Item
												name={[f.name, "max_weight"]}
												label="最大实重 / g"
												rules={[{ required: true }]}
											>
												<InputNumber min={0} placeholder="待确认" />
											</Form.Item>
										</div>
										<div className="three-fields">
											<Form.Item
												name={[f.name, "fixed"]}
												label="每票固定费"
												rules={[{ required: true }]}
											>
												<InputNumber min={0} precision={4} />
											</Form.Item>
											<Form.Item
												name={[f.name, "per_gram"]}
												label="每克费用"
												rules={[{ required: true }]}
											>
												<InputNumber min={0} precision={6} />
											</Form.Item>
											<Form.Item name={[f.name, "step"]} label="进位单位 / g">
												<InputNumber min={1} />
											</Form.Item>
										</div>
										<div className="three-fields">
											<Form.Item
												name={[f.name, "volume_divisor"]}
												label="体积重除数（cm³/g）"
											>
												<InputNumber min={0} placeholder="0为不计抛" />
											</Form.Item>
											<Form.Item
												name={[f.name, "minimum_weight"]}
												label="最低计费重 / g"
											>
												<InputNumber min={0} />
											</Form.Item>
											<Form.Item
												name={[f.name, "max_length"]}
												label="最长边上限 / cm"
											>
												<InputNumber min={0} />
											</Form.Item>
										</div>
										<Form.Item
											name={[f.name, "max_sum"]}
											label="三边和上限 / cm"
										>
											<InputNumber min={0} />
										</Form.Item>
									</div>
								))}
								<Button
									block
									type="dashed"
									icon={<PlusOutlined />}
									onClick={() =>
										add({
											up_to: null,
											min_weight: 0,
											max_weight: 0,
											fixed: 0,
											per_gram: 0,
											step: 1,
											volume_divisor: 0,
											minimum_weight: 0,
											max_length: 0,
											max_sum: 0,
										})
									}
								>
									添加报价档位
								</Button>
							</>
						)}
					</Form.List>
					<div className="section-divider" />
					<Form.Item name="delivery" label="该服务时效区间">
						<DaysInput />
					</Form.Item>
					{kind === "international" && (
						<Form.Item name="time_scope" label="时效截止环节">
							<Select
								options={[
									{
										value: "sorting",
										label: "到Ozon分拣中心（需另加尾程时效）",
									},
									{ value: "buyer", label: "到买家（已含尾程时效）" },
								]}
							/>
						</Form.Item>
					)}
					<Form.Item name="estimated" valuePropName="checked">
						<Checkbox>以上时效为估算区间</Checkbox>
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="return_cost" label="每次退货固定运费">
							<InputNumber min={0} placeholder="待补充" />
						</Form.Item>
						<Form.Item name="return_currency" label="退货运费币种">
							<Select
								options={["CNY", "RUB"].map((value) => ({
									value,
									label: value,
								}))}
							/>
						</Form.Item>
					</div>
					<Form.Item name="compensation" label="赔付条件">
						<Input.TextArea rows={2} />
					</Form.Item>
					<Form.Item name="note" label="适用说明与附加费用">
						<Input.TextArea rows={3} />
					</Form.Item>
				</Form>
			</Drawer>
		</div>
	);
}
