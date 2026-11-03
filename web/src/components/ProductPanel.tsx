import {
	Button,
	Checkbox,
	Collapse,
	Form,
	Input,
	InputNumber,
	Select,
	Space,
	Tag,
} from "antd";
import { SaveOutlined, PlusOutlined, DeleteOutlined } from "@ant-design/icons";
import { useEffect, useState } from "react";
import type { Product, Quote, Settings } from "../types";
import DiscountEditor, { noDiscount } from "./DiscountEditor";
import { AttributesInput, DaysInput, FeeEditor, PercentInput } from "./Fields";

export default function ProductPanel({
	product,
	quotes,
	settings,
	onChange,
	onSave,
	saving,
}: {
	product: Product;
	quotes: Quote[];
	settings: Settings;
	onChange: (p: Product) => void;
	onSave: () => void;
	saving: boolean;
}) {
	const [form] = Form.useForm<Product>();
	const [merchantKind, setMerchantKind] = useState(
		(product.fixed_discount ?? 0) > 0
			? "fixed"
			: product.discount > 0
				? "percent"
				: "none",
	);
	useEffect(() => {
		if ((product.fixed_discount ?? 0) > 0) setMerchantKind("fixed");
		else if (product.discount > 0) setMerchantKind("percent");
	}, [product.discount, product.fixed_discount]);
	useEffect(() => {
		form.setFieldsValue(product);
	}, [product, form]);
	const changed = () => {
		const p = { ...product, ...form.getFieldsValue(true) };
		const mapping = settings.category_mappings.find(
			(m) => m.alibaba && m.alibaba === p.alibaba_category,
		);
		if (mapping && p.alibaba_category !== product.alibaba_category) {
			p.category_id = mapping.seerfar;
			p.commission_category = mapping.ozon;
		}
		onChange(p);
	};
	return (
		<aside className="input-panel">
			<div className="panel-title">
				<div>
					<span className="section-index">01</span>
					<h2>商品与成本</h2>
				</div>
				<Tag bordered={false}>{product.demo ? "演示商品" : "我的商品"}</Tag>
			</div>
			<Form
				name="product-editor"
				form={form}
				layout="vertical"
				initialValues={product}
				onValuesChange={changed}
				requiredMark={false}
				size="middle"
			>
				<div className="form-section">
					<Form.Item name="name" label="商品名称" rules={[{ required: true }]}>
						<Input placeholder="例如：不锈钢保温杯 500ml" maxLength={150} />
					</Form.Item>
					<Form.Item name="supply" label="供货方式">
						<Select
							options={[
								{ value: "stock", label: "自备货直发" },
								{ value: "dropship", label: "1688一件代发" },
							]}
						/>
					</Form.Item>
					<Form.Item name="warehouse_quote_id" label="国内仓配服务">
						<Select
							options={[
								{ value: "self", label: "自行打包贴标" },
								...quotes
									.filter((q) => q.kind === "warehouse")
									.map((q) => ({
										value: q.id,
										label: `${q.name}${q.demo ? " · 演示" : ""}${q.status !== "active" ? " · 待确认" : ""}`,
									})),
							]}
						/>
					</Form.Item>
					<Form.Item
						name="route_ids"
						label="参与比较的跨境渠道"
						extra="留空则比较所有适用的已启用渠道"
					>
						<Select
							mode="multiple"
							allowClear
							placeholder="选择渠道"
							maxTagCount="responsive"
							options={quotes
								.filter(
									(q) => q.kind === "international" && q.status === "active",
								)
								.map((q) => ({
									value: q.id,
									label: `${q.name}${q.demo ? " · 演示" : ""}`,
								}))}
						/>
					</Form.Item>
					<div className="two-fields">
						<Form.Item name="purchase_quantity" label="本次采购数量">
							<InputNumber min={1} precision={0} suffix="件" />
						</Form.Item>
						<Form.Item name="moq" label="最低起订量">
							<InputNumber min={1} precision={0} suffix="件" />
						</Form.Item>
					</div>
					<Form.Item name="weight" label="包装后重量">
						<InputNumber min={1} suffix="g" />
					</Form.Item>
					<div className="dimensions">
						<Form.Item name="length" label="长">
							<InputNumber min={0.1} suffix="cm" />
						</Form.Item>
						<Form.Item name="width" label="宽">
							<InputNumber min={0.1} suffix="cm" />
						</Form.Item>
						<Form.Item name="height" label="高">
							<InputNumber min={0.1} suffix="cm" />
						</Form.Item>
					</div>
					<Space>
						<Form.Item name="batteries" valuePropName="checked" noStyle>
							<Checkbox>带电</Checkbox>
						</Form.Item>
						<Form.Item name="liquids" valuePropName="checked" noStyle>
							<Checkbox>含液体</Checkbox>
						</Form.Item>
					</Space>
				</div>
				<Collapse
					ghost
					defaultActiveKey={window.innerWidth > 760 ? ["costs"] : []}
					items={[
						{
							key: "costs",
							label: "单件费用",
							children: (
								<Form.Item name="costs" noStyle>
									<FeeEditor compact quotes={quotes} />
								</Form.Item>
							),
						},
						{
							key: "supplier",
							label: "供货与完整时效",
							children: (
								<>
									<Form.Item name="supplier" label="1688供货商">
										<Input />
									</Form.Item>
									<div className="two-fields">
										<Form.Item name="supplier_score" label="供应商原始评分">
											<InputNumber
												min={0}
												max={product.supplier_scale}
												placeholder="待补充"
											/>
										</Form.Item>
										<Form.Item name="supplier_scale" label="评分满分">
											<InputNumber min={1} max={100} />
										</Form.Item>
									</div>
									<Form.Item name="supplier_source" label="评分来源">
										<Input placeholder="1688综合体验分／商家评估" />
									</Form.Item>
									<Form.Item name="supplier_days" label="供应商备货时效">
										<DaysInput />
									</Form.Item>
									<Form.Item name="domestic_days" label="国内运输时效">
										<DaysInput />
									</Form.Item>
									<Form.Item
										name="tail_days"
										label="分拣中心至买家时效"
										extra="跨境报价已包含妥投时效时，不重复相加"
									>
										<DaysInput />
									</Form.Item>
								</>
							),
						},
						{
							key: "returns",
							label: "折扣与售后损失",
							children: (
								<>
									<Form.Item label="日常商家优惠">
										<DiscountEditor
											merchant
											label="日常商家优惠"
											value={{
												...noDiscount(),
												kind: merchantKind,
												value:
													(product.fixed_discount ?? 0) > 0
														? (product.fixed_discount ?? null)
														: product.discount,
												currency: "CNY",
											}}
											onChange={(v) => {
												setMerchantKind(v.kind);
												form.setFieldsValue({
													discount: v.kind === "percent" ? (v.value ?? 0) : 0,
													fixed_discount:
														v.kind === "fixed" ? (v.value ?? 0) : 0,
												});
												changed();
											}}
										/>
									</Form.Item>
									<Form.Item name="platform_discount" label="日常平台优惠">
										<DiscountEditor label="日常平台优惠" />
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
									<Form.Item
										name={["returns", "destination"]}
										label="售后去向"
										extra="俄罗斯订单可能默认通过WHD退货转售；请核对后台实际设置。WHD二次销售不在本模型范围内。"
									>
										<Select
											aria-label="售后去向"
											placeholder="待确认"
											allowClear
											options={[
												{
													value: "partner",
													label: "合作伙伴接收／退回（已核对费用）",
												},
												{ value: "disposal", label: "销毁（回收价值为0）" },
												{
													value: "manual",
													label: "各类去向分别确认并汇总为预算",
												},
												{
													value: "whd",
													label: "WHD退货转售（当前不支持完整测算）",
												},
											]}
										/>
									</Form.Item>
									<Form.Item
										name={["returns", "source"]}
										label="售后设置及费用依据"
									>
										<Input placeholder="后台退货设置／服务商协议／预算依据" />
									</Form.Item>
									<Form.Item
										name={["returns", "refusal_rate"]}
										label="拒收／未签收率"
									>
										<PercentInput max={99.99} />
									</Form.Item>
									<div className="three-fields">
										<Form.Item
											name={["returns", "refusal_reverse"]}
											label="拒收退运费"
										>
											<InputNumber min={0} placeholder="待补充" />
										</Form.Item>
										<Form.Item
											name={["returns", "refusal_disposal"]}
											label="拒收处置费"
										>
											<InputNumber min={0} placeholder="待补充" />
										</Form.Item>
										<Form.Item
											name={["returns", "refusal_recovery"]}
											label="拒收回收价值"
										>
											<InputNumber min={0} placeholder="待补充" />
										</Form.Item>
									</div>
									<p className="muted-text">
										取消、拒收及签收后退货均按原始订单计算，三者互斥。
									</p>
									<div className="two-fields">
										<Form.Item name={["returns", "rate"]} label="签收后退货率">
											<PercentInput max={99.99} />
										</Form.Item>
										<Form.Item
											name={["returns", "cancel_rate"]}
											label="发货前取消率"
										>
											<PercentInput max={99.99} />
										</Form.Item>
									</div>
									<Form.Item
										name={["returns", "reverse"]}
										label="每次退货运费（CNY）"
										extra="留空时使用渠道已确认的退货费用"
									>
										<InputNumber min={0} placeholder="待补充" />
									</Form.Item>
									<Form.Item
										name={["returns", "disposal"]}
										label="每次退货处置费（CNY）"
									>
										<InputNumber min={0} placeholder="待补充" />
									</Form.Item>
									<Form.Item
										name={["returns", "recovery"]}
										label="每件退货可回收价值（CNY）"
									>
										<InputNumber min={0} placeholder="待补充" />
									</Form.Item>
									<Form.Item
										name={["returns", "cancel_cost"]}
										label="每次取消不可回收成本（CNY）"
									>
										<InputNumber min={0} placeholder="待补充" />
									</Form.Item>
								</>
							),
						},
						{
							key: "search",
							label: "竞品检索条件",
							children: (
								<>
									<Form.Item name="seed_sku" label="已知Ozon商品SKU">
										<Input placeholder="可选，例如149222760" />
									</Form.Item>
									<Form.Item
										name="keywords"
										label="搜索关键词"
										extra="支持俄语；输入后按回车添加"
									>
										<Select
											mode="tags"
											tokenSeparators={[",", "，", ";"]}
											placeholder="例如 термос"
										/>
									</Form.Item>
									<Form.Item name="alibaba_category" label="1688类目ID">
										<Input placeholder="可从类目映射自动关联" />
									</Form.Item>
									<Form.Item name="category_id" label="Seerfar类目ID">
										<Input placeholder="可选，用于竞品过滤" />
									</Form.Item>
									<Form.Item name="commission_category" label="Ozon计费类目ID">
										<Input placeholder="对应费率设置中的类目" />
									</Form.Item>
									<Form.Item
										name="attributes"
										label="关键规格"
										extra="每行填写一项；缺少关键规格的候选需要人工确认"
									>
										<AttributesInput />
									</Form.Item>
									<Form.Item name="own_seller_ids" label="我的Ozon店铺ID">
										<Select
											mode="tags"
											tokenSeparators={[",", "，"]}
											placeholder="排除本店商品"
										/>
									</Form.Item>
								</>
							),
						},
						{
							key: "tiers",
							label: "采购阶梯价",
							children: (
								<Form.List name="purchase_tiers">
									{(fields, { add, remove }) => (
										<>
											{fields.map((field) => (
												<div className="purchase-tier" key={field.key}>
													<Form.Item
														name={[field.name, "quantity"]}
														label="起订数量"
													>
														<InputNumber min={1} precision={0} />
													</Form.Item>
													<Form.Item
														name={[field.name, "price"]}
														label="采购单价CNY"
													>
														<InputNumber min={0} />
													</Form.Item>
													<Button
														aria-label="删除采购阶梯"
														icon={<DeleteOutlined />}
														onClick={() => remove(field.name)}
													/>
												</div>
											))}
											<Button
												size="small"
												icon={<PlusOutlined />}
												onClick={() => add({ quantity: 1, price: 0 })}
											>
												添加阶梯
											</Button>
										</>
									)}
								</Form.List>
							),
						},
					]}
				/>
				<div className="panel-bottom">
					<Button
						block
						icon={<SaveOutlined />}
						loading={saving}
						onClick={onSave}
					>
						保存商品档案
					</Button>
					<span>费用为0时请明确填0，留空表示待补充。</span>
				</div>
			</Form>
		</aside>
	);
}
