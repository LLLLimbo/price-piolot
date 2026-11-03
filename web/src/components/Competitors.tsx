import {
	Alert,
	Button,
	Checkbox,
	Drawer,
	Empty,
	Form,
	Input,
	InputNumber,
	Select,
	Space,
	Table,
	Tag,
	Tooltip,
	Upload,
} from "antd";
import {
	EditOutlined,
	LinkOutlined,
	PlusOutlined,
	PushpinFilled,
	PushpinOutlined,
	ReloadOutlined,
	UploadOutlined,
} from "@ant-design/icons";
import { useState } from "react";
import type { Competitor, Product, Snapshot } from "../types";
import { DaysInput, AttributesInput } from "./Fields";
import { importedCompetitors, money, time } from "../lib";
type Props = {
	product: Product;
	snapshot?: Snapshot;
	loading: boolean;
	configured: boolean;
	onDiscover: () => void;
	onSave: (rows: Competitor[]) => Promise<void>;
	onError: (s: string) => void;
};
export default function Competitors({
	product,
	snapshot,
	loading,
	configured,
	onDiscover,
	onSave,
	onError,
}: Props) {
	const [editing, setEditing] = useState<Competitor | null | undefined>(
		undefined,
	);
	const [saving, setSaving] = useState(false);
	const [form] = Form.useForm<Competitor>();
	const rows = snapshot?.competitors || [];
	const edit = (c?: Competitor) => {
		setEditing(c || null);
		form.resetFields();
		form.setFieldsValue(
			c || {
				sku: "",
				title: "",
				price_rub: undefined,
				url: "",
				category_id: product.category_id,
				attributes: product.attributes || {},
				include: true,
				pinned: false,
				verified: true,
				confidence: 1,
				reason: "人工确认",
				fulfillment: [],
				delivery: null,
				delivery_source: "",
				estimated: false,
				seller_id: "",
				seller_name: "",
			},
		);
	};
	const persist = async (next: Competitor[]) => {
		try {
			setSaving(true);
			await onSave(next);
		} catch (e) {
			onError((e as Error).message);
		} finally {
			setSaving(false);
		}
	};
	const save = async () => {
		try {
			const values = await form.validateFields();
			const c = {
				...values,
				verified: true,
				confidence: 1,
				reason: "人工校对",
				delivery: values.delivery || null,
			};
			const list = editing
				? rows.map((row) => (row.sku === editing.sku ? c : row))
				: [...rows, c];
			setSaving(true);
			await onSave(list);
			setEditing(undefined);
		} catch (e) {
			if (e instanceof Error) onError(e.message);
		} finally {
			setSaving(false);
		}
	};
	return (
		<div className="competitor-section">
			<div className="section-toolbar">
				<div>
					<h3>
						竞品候选{" "}
						<span>
							{rows.filter((c) => c.include).length} / {rows.length}
						</span>
					</h3>
					<p>
						{snapshot
							? `${snapshot.source === "seerfar" ? "Seerfar" : snapshot.demo ? "演示快照" : "人工快照"} · ${time(snapshot.fetched_at)} · 商品展示价，未含买家运费`
							: "获取同类商品，再校对规格与时效"}
					</p>
				</div>
				<Space wrap>
					<Upload
						accept=".json"
						showUploadList={false}
						beforeUpload={async (file) => {
							try {
								await persist([
									...rows,
									...importedCompetitors(await file.text()),
								]);
							} catch (e) {
								onError((e as Error).message);
							}
							return false;
						}}
					>
						<Button icon={<UploadOutlined />}>导入</Button>
					</Upload>
					<Button icon={<PlusOutlined />} onClick={() => edit()}>
						手动添加
					</Button>
					<Button
						aria-label="获取竞品"
						icon={<ReloadOutlined />}
						loading={loading}
						onClick={onDiscover}
						disabled={!configured}
					>
						获取竞品
					</Button>
				</Space>
			</div>
			{!configured && (
				<Alert
					type="info"
					title="Seerfar尚未配置，可先手动添加竞品。"
					showIcon
				/>
			)}
			{(snapshot?.warnings || []).map((w) => (
				<Alert
					key={w}
					type="info"
					title={w}
					showIcon
					className="spaced-alert"
				/>
			))}
			<Table<Competitor>
				size="middle"
				rowKey="sku"
				dataSource={rows}
				pagination={{
					pageSize: 6,
					hideOnSinglePage: true,
					showSizeChanger: false,
				}}
				scroll={{ x: 800 }}
				locale={{
					emptyText: (
						<Empty description="还没有竞品。填写关键词或SKU后获取，也可以手动添加。" />
					),
				}}
				columns={[
					{
						title: "采用",
						width: 62,
						render: (_, c) => (
							<Checkbox
								aria-label={`采用竞品${c.sku}`}
								disabled={saving}
								checked={c.include}
								onChange={(e) =>
									persist(
										rows.map((row) =>
											row.sku === c.sku
												? {
														...row,
														include: e.target.checked,
														verified: true,
														reason: "人工确认",
													}
												: row,
										),
									)
								}
							/>
						),
					},
					{
						title: "商品 / 规格",
						width: 270,
						render: (_, c) => (
							<div className="competitor-name">
								<span>
									{c.title || c.sku}
									{c.url && (
										<a
											href={c.url.startsWith("https://") ? c.url : undefined}
											target="_blank"
											rel="noreferrer"
											aria-label={`查看${c.title || c.sku}`}
										>
											<LinkOutlined />
										</a>
									)}
								</span>
								<small>
									{c.seller_name || c.seller_id || "店铺未提供"} · {c.sku}
								</small>
								<div className="specs">
									{Object.entries(c.attributes || {}).map(([k, v]) => (
										<span key={k}>
											{k} {v}
										</span>
									))}
								</div>
							</div>
						),
					},
					{
						title: "商品价",
						width: 120,
						render: (_, c) => (
							<>
								<strong className="numeric">{money(c.price_rub, "RUB")}</strong>
								<small className="block muted-text">
									{c.price_condition === "standard"
										? "普通买家"
										: c.price_condition === "member"
											? "会员条件"
											: c.price_condition === "ozon_card"
												? "指定支付条件"
												: "优惠条件未知"}
								</small>
								<small className="block muted-text">
									{c.price_source || "商品展示价"}
								</small>
							</>
						),
					},
					{
						title: "预计妥投",
						width: 145,
						render: (_, c) => (
							<>
								{c.delivery ? (
									<>
										<span className="numeric">
											{c.delivery.min}–{c.delivery.max} 天
										</span>
										<small className="block muted-text">
											{c.estimated ? "估算 · " : ""}
											{c.delivery_source}
										</small>
									</>
								) : (
									<Tag color="gold">时效待补充</Tag>
								)}
							</>
						),
					},
					{
						title: "匹配状态",
						width: 170,
						render: (_, c) => (
							<Tooltip title={c.reason}>
								<Tag
									bordered={false}
									color={
										c.verified
											? "blue"
											: c.confidence >= 0.85
												? "green"
												: "default"
									}
								>
									{c.verified
										? "人工确认"
										: c.confidence >= 0.85
											? "高置信匹配"
											: "待核对"}
								</Tag>
								<small className="block muted-text">
									{(c.fulfillment || []).join(" / ") || c.reason}
								</small>
							</Tooltip>
						),
					},
					{
						title: "",
						width: 85,
						render: (_, c) => (
							<Space size={0}>
								<Tooltip title={c.pinned ? "取消固定" : "固定竞品"}>
									<Button
										type="text"
										aria-label={`固定竞品${c.sku}`}
										icon={c.pinned ? <PushpinFilled /> : <PushpinOutlined />}
										onClick={() =>
											persist(
												rows.map((row) =>
													row.sku === c.sku
														? { ...row, pinned: !row.pinned }
														: row,
												),
											)
										}
									/>
								</Tooltip>
								<Button
									type="text"
									aria-label={`校对竞品${c.sku}`}
									icon={<EditOutlined />}
									onClick={() => edit(c)}
								/>
							</Space>
						),
					},
				]}
			/>
			<Drawer
				title={editing ? "校对竞品" : "手动添加竞品"}
				open={editing !== undefined}
				onClose={() => setEditing(undefined)}
				width={480}
				extra={
					<Button type="primary" loading={saving} onClick={save}>
						保存竞品
					</Button>
				}
				destroyOnHidden
			>
				<Form name="competitor-editor" form={form} layout="vertical">
					<Form.Item
						name="sku"
						label="Ozon SKU / 自定义编号"
						rules={[{ required: true }]}
					>
						<Input disabled={!!editing} />
					</Form.Item>
					<Form.Item name="title" label="商品名称" rules={[{ required: true }]}>
						<Input />
					</Form.Item>
					<Form.Item
						name="price_rub"
						label="商品展示价（RUB）"
						rules={[{ required: true }]}
					>
						<InputNumber min={0.01} precision={2} />
					</Form.Item>
					<Form.Item name="url" label="商品链接">
						<Input placeholder="https://www.ozon.ru/product/…" />
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="seller_id" label="店铺ID">
							<Input />
						</Form.Item>
						<Form.Item name="seller_name" label="店铺名称">
							<Input />
						</Form.Item>
					</div>
					<Form.Item name="category_id" label="Seerfar类目ID">
						<Input />
					</Form.Item>
					<Form.Item name="attributes" label="商品规格">
						<AttributesInput key={editing?.sku || "new"} />
					</Form.Item>
					<Form.Item name="fulfillment" label="履约类型">
						<Select
							mode="multiple"
							options={["FBO", "FBS", "RFBS", "FBP", "OZON"].map((v) => ({
								value: v,
								label: v,
							}))}
						/>
					</Form.Item>
					<Form.Item name="delivery" label="完整妥投时效">
						<DaysInput />
					</Form.Item>
					<Form.Item name="price_source" label="价格来源">
						<Input placeholder="Seerfar／人工录入，保留采集口径" />
					</Form.Item>
					<Form.Item name="price_condition" label="已知优惠条件">
						<Select
							allowClear
							options={[
								{ value: "standard", label: "普通买家" },
								{ value: "member", label: "会员" },
								{ value: "ozon_card", label: "指定支付方式" },
							]}
							placeholder="未知，按展示价比较并标注限制"
						/>
					</Form.Item>
					<Form.Item name="delivery_source" label="时效来源">
						<Input placeholder="商品页承诺／商家参考区间" />
					</Form.Item>
					<Form.Item name="estimated" valuePropName="checked">
						<Checkbox>此时效为估算</Checkbox>
					</Form.Item>
					<Space>
						<Form.Item name="include" valuePropName="checked" noStyle>
							<Checkbox>参与评分</Checkbox>
						</Form.Item>
						<Form.Item name="pinned" valuePropName="checked" noStyle>
							<Checkbox>固定此竞品</Checkbox>
						</Form.Item>
					</Space>
				</Form>
			</Drawer>
		</div>
	);
}
