import { Checkbox, Input, InputNumber, Select } from "antd";
import type { Discount } from "../types";
import { PercentInput } from "./Fields";
export const noDiscount = (): Discount => ({
	kind: "none",
	value: null,
	currency: "CNY",
	confirmed: false,
	source: "",
});
export default function DiscountEditor({
	value,
	onChange,
	merchant = false,
	label = "优惠",
}: {
	value?: Discount;
	onChange?: (v: Discount) => void;
	merchant?: boolean;
	label?: string;
}) {
	const v = { ...noDiscount(), ...value };
	const set = (patch: Partial<Discount>) => onChange?.({ ...v, ...patch });
	return (
		<div className="discount-editor">
			<div className="discount-fields">
				<Select
					aria-label={`${label}方式`}
					value={v.kind || "none"}
					onChange={(kind) =>
						set({
							kind,
							value: kind === "none" ? null : 0,
							currency: "CNY",
							confirmed: false,
						})
					}
					options={[
						{ value: "none", label: "无优惠" },
						{ value: "percent", label: "按比例优惠" },
						{ value: "fixed", label: "固定金额优惠" },
					]}
				/>
				{v.kind === "percent" ? (
					<PercentInput
						value={v.value}
						max={99.99}
						onChange={(value) => set({ value })}
					/>
				) : v.kind === "fixed" ? (
					<InputNumber
						aria-label={`${label}金额`}
						min={0}
						value={v.value}
						placeholder="待补充"
						onChange={(value) => set({ value })}
					/>
				) : null}
				{v.kind === "fixed" && (
					<Select
						aria-label={`${label}币种`}
						value={v.currency}
						onChange={(currency) => set({ currency })}
						options={(merchant ? ["CNY"] : ["CNY", "RUB"]).map((x) => ({
							label: x,
							value: x,
						}))}
					/>
				)}
			</div>
			{!merchant && v.kind !== "none" && v.kind !== "" && (
				<div className="field-stack">
					<Input
						aria-label={`${label}来源`}
						placeholder="平台承担的依据／演示假设来源"
						value={v.source}
						onChange={(e) => set({ source: e.target.value })}
					/>
					<Checkbox
						checked={v.confirmed}
						onChange={(e) => set({ confirmed: e.target.checked })}
					>
						已确认平台等额补偿，净回款不减少
					</Checkbox>
					<small className="muted-text">
						Ozon折扣可能通过积分抵扣佣金、余额奖金补偿，不能当作额外现金收入。未确认时不生成完整利润结论。
					</small>
				</div>
			)}
		</div>
	);
}
