import {
	Button,
	Checkbox,
	Drawer,
	Form,
	Input,
	Select,
	Space,
	Table,
	Tag,
} from "antd";
import { PlusOutlined, EditOutlined, ShopOutlined } from "@ant-design/icons";
import { useState } from "react";
import type { Store } from "../types";
import { api as rootAPI } from "../lib";
export default function StoresPage({
	stores,
	onSaved,
	onSelect,
	onError,
}: {
	stores: Store[];
	onSaved: () => Promise<void>;
	onSelect: (id: string) => void;
	onError: (s: string) => void;
}) {
	const [editing, setEditing] = useState<Store | null | undefined>();
	const [saving, setSaving] = useState(false);
	const [form] = Form.useForm<Store>();
	const open = (p?: Store) => {
		setEditing(p ?? null);
		form.resetFields();
		form.setFieldsValue(p ?? { name: "", seller_ids: [], archived: false });
	};
	const save = async () => {
		try {
			const p = await form.validateFields();
			setSaving(true);
			const v = await rootAPI<{ store: Store }>(
				`/stores${editing ? `/${editing.id}` : ""}`,
				p,
				editing ? "PUT" : "POST",
			);
			await onSaved();
			setEditing(undefined);
			if (!editing) onSelect(v.store.id);
		} catch (e) {
			if (e instanceof Error) onError(e.message);
		} finally {
			setSaving(false);
		}
	};
	return (
		<div className="page-surface">
			<div className="page-heading">
				<div>
					<span className="eyebrow">STORE PROFILES</span>
					<h1>店铺管理</h1>
					<p>每家店铺独立保存商品、报价与测算资料。</p>
				</div>
				<Button
					aria-label="新增店铺"
					type="primary"
					icon={<PlusOutlined />}
					onClick={() => open()}
				>
					新增店铺
				</Button>
			</div>
			<div className="plain-note">
				<ShopOutlined /> 本地店铺档案不连接卖家账号。卖家ID仅用于排除自有竞品。
			</div>
			<Table<Store>
				rowKey="id"
				dataSource={stores}
				pagination={false}
				scroll={{ x: 620 }}
				columns={[
					{
						title: "店铺",
						render: (_, p) => (
							<>
								<strong>{p.name}</strong>
								{p.demo && <Tag>演示</Tag>}
								<small className="block muted-text">
									{p.seller_ids?.join("、") || "卖家ID待补充"}
								</small>
							</>
						),
					},
					{
						title: "状态",
						render: (_, p) => (
							<Tag color={p.archived ? "default" : "green"}>
								{p.archived ? "已归档" : "使用中"}
							</Tag>
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
								<Button onClick={() => onSelect(p.id)}>切换店铺</Button>
							</Space>
						),
					},
				]}
			/>
			<Drawer
				title={editing ? "编辑店铺" : "新增店铺"}
				open={editing !== undefined}
				onClose={() => setEditing(undefined)}
				width={480}
				extra={
					<Button
						aria-label="保存店铺"
						type="primary"
						loading={saving}
						onClick={save}
					>
						保存店铺
					</Button>
				}
			>
				<Form name="store-editor" form={form} layout="vertical">
					<Form.Item name="name" label="店铺名称" rules={[{ required: true }]}>
						<Input maxLength={100} />
					</Form.Item>
					<Form.Item
						name="seller_ids"
						label="Ozon卖家ID"
						extra="可填写多个关联卖家ID；所有已配置自有店铺商品均从竞品中排除。"
					>
						<Select
							mode="tags"
							tokenSeparators={[",", "，", " "]}
							placeholder="输入卖家ID后按回车"
						/>
					</Form.Item>
					<Form.Item name="archived" valuePropName="checked">
						<Checkbox>归档店铺，保留记录并暂停编辑和测算</Checkbox>
					</Form.Item>
				</Form>
				<p className="muted-text">
					新店铺关键成本为待补充状态，请在费率与评分页面录入后进行测算。
				</p>
			</Drawer>
		</div>
	);
}
