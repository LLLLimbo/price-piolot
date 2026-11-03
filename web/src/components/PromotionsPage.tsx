import {
	Alert,
	Button,
	Drawer,
	Form,
	Input,
	Popconfirm,
	Select,
	Space,
	Table,
	Tag,
} from "antd";
import { PlusOutlined, EditOutlined } from "@ant-design/icons";
import { useState } from "react";
import type { PromotionScenario } from "../types";
import { useStoreApi } from "../lib";
import DiscountEditor, { noDiscount } from "./DiscountEditor";
export default function PromotionsPage({
	promotions,
	onSaved,
	onError,
}: {
	promotions: PromotionScenario[];
	onSaved: () => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [editing, setEditing] = useState<
		PromotionScenario | null | undefined
	>();
	const [saving, setSaving] = useState(false);
	const [form] = Form.useForm<PromotionScenario>();
	const open = (p?: PromotionScenario) => {
		setEditing(p ?? null);
		form.resetFields();
		form.setFieldsValue(
			p ?? {
				name: "",
				source: "商家录入",
				starts_at: "",
				ends_at: "",
				merchant_discount: noDiscount(),
				platform_discount: noDiscount(),
				price_condition: "standard",
				demo: false,
			},
		);
	};
	const save = async () => {
		try {
			const p = await form.validateFields();
			setSaving(true);
			await api(
				`/promotions${editing ? `/${editing.id}` : ""}`,
				{ ...p, demo: editing?.demo ?? false },
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
	const describe = (p: PromotionScenario) => {
		const d = p.merchant_discount;
		return d.kind === "percent"
			? `商家优惠 ${((d.value ?? 0) * 100).toFixed(1)}%`
			: d.kind === "fixed"
				? `商家优惠 ¥${d.value ?? "待补充"}`
				: "无商家优惠";
	};
	return (
		<div className="page-surface">
			<div className="page-heading">
				<div>
					<span className="eyebrow">PROMOTION SCENARIOS</span>
					<h1>促销方案</h1>
					<p>与日常定价分别试算，各自遵守利润及亏损约束。</p>
				</div>
				<Button
					aria-label="新增促销方案"
					type="primary"
					icon={<PlusOutlined />}
					onClick={() => open()}
				>
					新增促销方案
				</Button>
			</div>
			<Alert
				className="spaced-alert"
				type="info"
				showIcon
				title="请按后台最终叠加结果填写活动商家总优惠，替代日常优惠。Ozon可能叠加促销码、满额折扣；此处分别试算各情景，不校验报名资格或价格锁定，也不会参加或退出平台活动。"
			/>
			<Table<PromotionScenario>
				rowKey="id"
				dataSource={promotions}
				scroll={{ x: 760 }}
				columns={[
					{
						title: "活动",
						render: (_, p) => (
							<>
								<strong>{p.name}</strong>
								{p.demo && <Tag>演示</Tag>}
								<small className="block muted-text">
									{p.source} · v{p.version || 1}
								</small>
							</>
						),
					},
					{
						title: "活动时间",
						render: (_, p) =>
							`${p.starts_at || "未限定"} — ${p.ends_at || "未限定"}`,
					},
					{ title: "商家优惠", render: (_, p) => describe(p) },
					{
						title: "平台优惠",
						render: (_, p) =>
							p.platform_discount.kind &&
							p.platform_discount.kind !== "none" ? (
								<Tag color={p.platform_discount.confirmed ? "green" : "gold"}>
									{p.platform_discount.confirmed
										? "已确认等额补偿"
										: "承担方待确认"}
								</Tag>
							) : (
								"无"
							),
					},
					{
						title: "操作",
						render: (_, p) => (
							<Space>
								<Button
									aria-label={`编辑${p.name}`}
									icon={<EditOutlined />}
									onClick={() => open(p)}
								>
									编辑
								</Button>
								<Popconfirm
									title="删除该促销方案？"
									onConfirm={async () => {
										try {
											await api(`/promotions/${p.id}`, undefined, "DELETE");
											await onSaved();
										} catch (e) {
											onError((e as Error).message);
										}
									}}
								>
									<Button danger>删除</Button>
								</Popconfirm>
							</Space>
						),
					},
				]}
			/>
			<Drawer
				width={560}
				title={editing ? "编辑促销方案" : "新增促销方案"}
				open={editing !== undefined}
				onClose={() => setEditing(undefined)}
				extra={
					<Button
						aria-label="保存促销方案"
						type="primary"
						loading={saving}
						onClick={save}
					>
						保存促销方案
					</Button>
				}
			>
				<Form name="promotion-editor" form={form} layout="vertical">
					<Form.Item name="name" label="活动名称" rules={[{ required: true }]}>
						<Input />
					</Form.Item>
					<Form.Item
						name="source"
						label="活动来源"
						rules={[{ required: true }]}
					>
						<Input placeholder="活动条款／演示假设" />
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="starts_at" label="开始日期">
							<Input type="date" />
						</Form.Item>
						<Form.Item name="ends_at" label="结束日期">
							<Input type="date" />
						</Form.Item>
					</div>
					<Form.Item name="merchant_discount" label="活动商家总优惠">
						<DiscountEditor merchant label="活动商家优惠" />
					</Form.Item>
					<Form.Item name="platform_discount" label="平台承担优惠">
						<DiscountEditor label="活动平台优惠" />
					</Form.Item>
					<Form.Item name="price_condition" label="买家优惠条件">
						<Select
							options={[
								{ value: "standard", label: "普通买家" },
								{ value: "member", label: "会员" },
								{ value: "ozon_card", label: "指定支付方式" },
							]}
						/>
					</Form.Item>
				</Form>
			</Drawer>
		</div>
	);
}
