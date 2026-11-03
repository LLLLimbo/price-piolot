import { Form, Input, InputNumber, Select } from "antd";
import { PercentInput } from "./Fields";

export default function ProfitCheckFields() {
	return (
		<>
			<p className="muted-text">
				手工核对单笔订单的所列费用，金额均为人民币订单总额。样本随商品档案保存。
			</p>
			<Form.Item name={["profit_check", "order_ref"]} label="参考订单号">
				<Input />
			</Form.Item>
			<Form.Item name={["profit_check", "date"]} label="样本订单日期">
				<Input placeholder="YYYY-MM-DD" />
			</Form.Item>
			<Form.Item name={["profit_check", "source"]} label="样本资料来源">
				<Input.TextArea autoSize={{ minRows: 2, maxRows: 6 }} />
			</Form.Item>
			<Form.Item name={["profit_check", "carrier"]} label="样本承运渠道">
				<Input />
			</Form.Item>
			<Form.Item name={["profit_check", "freight_kind"]} label="样本运费口径">
				<Select
					options={[
						{ value: "estimated", label: "预计运费" },
						{ value: "settled", label: "已结算运费" },
					]}
				/>
			</Form.Item>
			<Form.Item name={["profit_check", "fee_label"]} label="样本费用方案">
				<Input />
			</Form.Item>
			<Form.Item name={["profit_check", "fee_kind"]} label="样本费用依据">
				<Select
					options={[
						{ value: "prototype", label: "原型方案" },
						{ value: "statement", label: "平台扣费账单" },
						{ value: "manual", label: "商家手填假设" },
					]}
				/>
			</Form.Item>
			{(
				[
					["amount", "样本订单金额"],
					["procurement", "样本采购总成本"],
					["freight", "样本运费"],
					["fixed_fee", "样本固定费用"],
				] as const
			).map(([key, label]) => (
				<Form.Item key={key} name={["profit_check", key]} label={label}>
					<InputNumber min={0} precision={2} suffix="CNY" />
				</Form.Item>
			))}
			<Form.Item name={["profit_check", "commission_rate"]} label="样本佣金率">
				<PercentInput max={100} />
			</Form.Item>
			<Form.Item
				name={["profit_check", "reported_profit"]}
				label="样本参考利润"
			>
				<InputNumber precision={2} suffix="CNY" />
			</Form.Item>
		</>
	);
}
