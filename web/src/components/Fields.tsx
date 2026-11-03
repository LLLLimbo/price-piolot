import {
	Button,
	Checkbox,
	Collapse,
	Input,
	InputNumber,
	Select,
	Space,
	Switch,
	Tooltip,
} from "antd";
import { PlusOutlined, DeleteOutlined } from "@ant-design/icons";
import { useState } from "react";
import type { Days, Fee, Quote } from "../types";
import { attributes, id } from "../lib";

export function PercentInput({
	value,
	onChange,
	id: inputId,
	max = 100,
}: {
	value?: number | null;
	onChange?: (v: number | null) => void;
	id?: string;
	max?: number;
}) {
	return (
		<InputNumber
			id={inputId}
			value={value == null ? null : Math.round(value * 1000000) / 10000}
			min={0}
			max={max}
			precision={3}
			suffix="%"
			onChange={(v) => onChange?.(v == null ? null : v / 100)}
			style={{ width: "100%" }}
		/>
	);
}
export function DaysInput({
	value,
	onChange,
	id: inputId,
}: {
	value?: Days | null;
	onChange?: (v: Days | null) => void;
	id?: string;
}) {
	return (
		<div className="days-input">
			<InputNumber
				id={inputId}
				aria-label="最短天数"
				min={0}
				value={value?.min}
				placeholder="最短"
				onChange={(v) =>
					onChange?.(
						v == null ? null : { min: v, max: Math.max(v, value?.max ?? v) },
					)
				}
			/>
			<span>—</span>
			<InputNumber
				aria-label="最长天数"
				min={0}
				value={value?.max}
				placeholder="最长"
				onChange={(v) =>
					onChange?.(
						v == null ? null : { min: Math.min(value?.min ?? v, v), max: v },
					)
				}
			/>
			<span>天</span>
		</div>
	);
}
export function AttributesInput({
	value,
	onChange,
}: {
	value?: Record<string, string> | null;
	onChange?: (v: Record<string, string>) => void;
}) {
	const [text, setText] = useState(
		Object.entries(value || {})
			.map(([k, v]) => `${k}：${v}`)
			.join("\n"),
	);
	return (
		<Input.TextArea
			value={text}
			rows={3}
			placeholder={"容量：500ml\n材质：304不锈钢\n套装数量：1件"}
			onChange={(e) => {
				setText(e.target.value);
				onChange?.(attributes(e.target.value));
			}}
		/>
	);
}
export function FeeEditor({
	value = [],
	onChange,
	compact = false,
	quotes = [],
	attached = false,
}: {
	value?: Fee[];
	onChange?: (fees: Fee[]) => void;
	compact?: boolean;
	quotes?: Quote[];
	attached?: boolean;
}) {
	const update = (i: number, patch: Partial<Fee>) =>
		onChange?.(value.map((f, j) => (i === j ? { ...f, ...patch } : f)));
	return (
		<div className="fee-editor">
			{value.map((f, i) => (
				<div
					className={`fee-row ${f.enabled ? "" : "muted"}`}
					key={`${i}-${f.code}`}
				>
					<div className="fee-heading">
						<Switch
							size="small"
							aria-label={`启用${f.name}`}
							checked={f.enabled}
							onChange={(enabled) => update(i, { enabled })}
						/>
						<Input
							aria-label="费用名称"
							variant="borderless"
							value={f.name}
							onChange={(e) => update(i, { name: e.target.value })}
						/>
						<Tooltip title="删除费用">
							<Button
								type="text"
								size="small"
								aria-label={`删除${f.name}`}
								icon={<DeleteOutlined />}
								onClick={() => onChange?.(value.filter((_, j) => i !== j))}
							/>
						</Tooltip>
					</div>
					{f.enabled && (
						<>
							<div className="fee-amount">
								<Select
									aria-label={`${f.name}计费方式`}
									value={f.kind}
									onChange={(kind) =>
										update(i, {
											kind,
											basis: kind === "rate" ? f.basis || "sale" : f.basis,
											rate: f.rate ?? null,
										})
									}
									options={[
										{ value: "fixed", label: "固定金额" },
										{ value: "rate", label: "按比例" },
										{ value: "weight", label: "每克计费" },
										{ value: "first_weight", label: "首重＋续重" },
									]}
								/>
								{f.kind === "rate" ? (
									<PercentInput
										value={f.rate}
										onChange={(rate) => update(i, { rate })}
									/>
								) : (
									<InputNumber
										aria-label={`${f.name}金额`}
										value={f.amount}
										min={0}
										precision={4}
										placeholder="待补充"
										onChange={(amount) => update(i, { amount })}
									/>
								)}
								<Select
									aria-label={`${f.name}币种`}
									value={f.currency}
									onChange={(currency) => update(i, { currency })}
									options={[
										{ value: "CNY", label: "CNY" },
										{ value: "RUB", label: "RUB" },
									]}
								/>
							</div>
							{!compact && (
								<Input
									size="small"
									aria-label={`${f.name}费用编码`}
									prefix="编码"
									value={f.code}
									onChange={(e) => update(i, { code: e.target.value })}
								/>
							)}
							<Collapse
								ghost
								size="small"
								items={[
									{
										key: "details",
										label: "计费条件与来源",
										children: (
											<div className="field-stack">
												<Checkbox
													checked={f.status === "confirmed"}
													onChange={(e) =>
														update(i, {
															status: e.target.checked
																? "confirmed"
																: "pending",
														})
													}
												>
													来源和适用性已确认
													{f.version ? ` · v${f.version}` : ""}
												</Checkbox>
												<div className="two-fields">
													<label>
														适用开始
														<Input
															type="date"
															value={f.valid_from || ""}
															onChange={(e) =>
																update(i, { valid_from: e.target.value })
															}
														/>
													</label>
													<label>
														适用结束
														<Input
															type="date"
															value={f.valid_to || ""}
															onChange={(e) =>
																update(i, { valid_to: e.target.value })
															}
														/>
													</label>
												</div>
												{f.kind === "rate" && (
													<label>
														计费基数
														<Select
															value={f.basis}
															onChange={(basis) => update(i, { basis })}
															options={
																f.code === "commission"
																	? [
																			{
																				value: "sale",
																				label: "商家折后成交价",
																			},
																			{
																				value: "retained_revenue",
																				label:
																					"留存订单成交价（单笔限额后折算）",
																			},
																		]
																	: [
																			{
																				value: "sale",
																				label: "商家折后成交价",
																			},
																			{
																				value: "buyer_price",
																				label:
																					"买家商品参考价（需明确合同依据）",
																			},
																			{
																				value: "retained_revenue",
																				label:
																					"留存订单成交价（单笔限额后折算）",
																			},
																			{
																				value: "international",
																				label: "国际运费",
																			},
																			{
																				value: "settlement",
																				label: "净结算额",
																			},
																		]
															}
														/>
													</label>
												)}
												<label>
													发生条件
													<Select
														value={f.on || "shipped"}
														onChange={(on) => update(i, { on })}
														options={
															f.code === "commission"
																? [
																		{
																			value: "kept",
																			label: "最终留存订单（退货冲回佣金）",
																		},
																	]
																: [
																		{
																			value: "ordered",
																			label: "全部原始订单（含发货前取消）",
																		},
																		{
																			value: "shipped",
																			label: "已发货订单（含退货）",
																		},
																		{ value: "kept", label: "最终留存订单" },
																		{
																			value: "delivered",
																			label: "已签收订单（含签收后退货）",
																		},
																	]
														}
													/>
												</label>
												<div className="two-fields">
													<label>
														最低收费
														<InputNumber
															min={0}
															value={f.minimum}
															placeholder="不限"
															onChange={(minimum) => update(i, { minimum })}
														/>
													</label>
													<label>
														收费封顶
														<InputNumber
															min={0}
															value={f.maximum}
															placeholder="不限"
															onChange={(maximum) => update(i, { maximum })}
														/>
													</label>
												</div>
												{f.kind === "first_weight" && (
													<>
														<div className="two-fields">
															<label>
																首重 / g
																<InputNumber
																	min={1}
																	value={f.first_weight}
																	onChange={(v) =>
																		update(i, { first_weight: v ?? 1 })
																	}
																/>
															</label>
															<label>
																续重 / g
																<InputNumber
																	min={1}
																	value={f.additional_weight}
																	onChange={(v) =>
																		update(i, { additional_weight: v ?? 1 })
																	}
																/>
															</label>
														</div>
														<label>
															每续重金额
															<InputNumber
																min={0}
																value={f.additional_amount}
																onChange={(v) =>
																	update(i, { additional_amount: v ?? 0 })
																}
															/>
														</label>
													</>
												)}
												<Checkbox
													checked={f.deduct_settlement}
													onChange={(e) =>
														update(i, { deduct_settlement: e.target.checked })
													}
												>
													从提现前结算额扣除
												</Checkbox>
												{!attached && (
													<>
														<label>
															适用供货方式
															<Select
																mode="multiple"
																value={f.supply_modes || []}
																placeholder="不限"
																onChange={(supply_modes) =>
																	update(i, { supply_modes })
																}
																options={[
																	{ value: "stock", label: "自备货" },
																	{ value: "dropship", label: "一件代发" },
																]}
															/>
														</label>
														<label>
															适用跨境渠道
															<Select
																mode="multiple"
																value={f.route_ids || []}
																placeholder="不限"
																onChange={(route_ids) =>
																	update(i, { route_ids })
																}
																options={quotes
																	.filter((q) => q.kind === "international")
																	.map((q) => ({ value: q.id, label: q.name }))}
															/>
														</label>
													</>
												)}
												<label>
													费用来源
													<Input
														value={f.source}
														placeholder="店铺合同／账单／商家假设"
														onChange={(e) =>
															update(i, { source: e.target.value })
														}
													/>
												</label>
												{f.kind === "rate" && (
													<>
														<div className="inline-heading">
															<span>按货值分档</span>
															<Select
																size="small"
																value={
																	f.tier_currency ||
																	(f.code === "commission" ? "RUB" : "CNY")
																}
																onChange={(tier_currency) =>
																	update(i, { tier_currency })
																}
																options={
																	f.code === "commission"
																		? [
																				{
																					value: "RUB",
																					label: "下单日卢布货值",
																				},
																			]
																		: [
																				{ value: "CNY", label: "人民币货值" },
																				{ value: "RUB", label: "卢布货值" },
																			]
																}
															/>
														</div>
														{(f.tiers || []).map((t, j) => (
															<Space.Compact key={j} className="tier-line">
																<InputNumber
																	aria-label="阶梯货值上限"
																	value={t.up_to}
																	placeholder="末档留空"
																	min={0}
																	onChange={(v) =>
																		update(i, {
																			tiers: (f.tiers || []).map((x, k) =>
																				j === k ? { ...x, up_to: v } : x,
																			),
																		})
																	}
																/>
																<PercentInput
																	value={t.rate}
																	onChange={(v) =>
																		update(i, {
																			tiers: (f.tiers || []).map((x, k) =>
																				j === k ? { ...x, rate: v || 0 } : x,
																			),
																		})
																	}
																/>
																<Button
																	aria-label="删除费率阶梯"
																	icon={<DeleteOutlined />}
																	onClick={() =>
																		update(i, {
																			tiers: (f.tiers || []).filter(
																				(_, k) => j !== k,
																			),
																		})
																	}
																/>
															</Space.Compact>
														))}
														<Button
															size="small"
															icon={<PlusOutlined />}
															onClick={() =>
																update(i, {
																	tier_currency:
																		f.tier_currency ||
																		(f.code === "commission" ? "RUB" : "CNY"),
																	tiers: [
																		...(f.tiers || []),
																		{ up_to: null, rate: f.rate || 0 },
																	],
																})
															}
														>
															添加费率档
														</Button>
													</>
												)}
											</div>
										),
									},
								]}
							/>
						</>
					)}
				</div>
			))}
			<Button
				block
				type="dashed"
				icon={<PlusOutlined />}
				onClick={() =>
					onChange?.([
						...value,
						{
							code: `other_${id()}`,
							name: "其他费用",
							enabled: true,
							kind: "fixed",
							amount: null,
							rate: null,
							currency: "CNY",
							basis: "sale",
							on: "shipped",
							tiers: [],
							tier_currency: "CNY",
							minimum: null,
							maximum: null,
							first_weight: 500,
							additional_weight: 100,
							additional_amount: 0,
							deduct_settlement: false,
							source: "商家录入",
							status: "pending",
							version: 0,
							valid_from: "",
							valid_to: "",
						},
					])
				}
			>
				添加费用
			</Button>
		</div>
	);
}
