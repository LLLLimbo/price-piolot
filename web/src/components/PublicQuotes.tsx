import { Alert, Button, Table, Tag } from "antd";
import type { Quote } from "../types";
import { useStoreApi } from "../lib";
export default function PublicQuotes({
	quotes,
	onSaved,
	onError,
}: {
	quotes: Quote[];
	onSaved: () => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	return (
		<div className="page-surface">
			<div className="page-heading">
				<div>
					<span className="eyebrow">PUBLIC RATE REFERENCES</span>
					<h1>公共物流资料</h1>
					<p>保留公开来源，复制到店铺后再确认协议适用性。</p>
				</div>
			</div>
			<Alert
				className="spaced-alert"
				type="info"
				showIcon
				title="公开、历史和过期资料不会直接参与推荐。复制后仍为待确认；请核对有效期、承运条件和附加费。"
			/>
			<Table<Quote>
				rowKey="id"
				dataSource={quotes}
				scroll={{ x: 850 }}
				columns={[
					{
						title: "服务商／渠道",
						render: (_, q) => (
							<>
								<strong>{q.name}</strong>
								<small className="block muted-text">
									{q.provider} · {q.source_date || "日期待核实"}
								</small>
							</>
						),
					},
					{
						title: "公开计费示例",
						render: (_, q) =>
							q.bands[0]
								? `${q.bands[0].fixed} ＋ ${q.bands[0].per_gram} × 克重 ${q.currency}`
								: "待补充",
					},
					{
						title: "适用说明",
						width: 300,
						render: (_, q) => (
							<>
								<Tag
									color={
										q.status === "excluded"
											? "red"
											: q.status === "historical"
												? "default"
												: "gold"
									}
								>
									{q.status === "excluded"
										? "不适用Ozon"
										: q.status === "historical"
											? "历史资料"
											: "待确认"}
								</Tag>
								<p>{q.note}</p>
							</>
						),
					},
					{
						title: "来源",
						render: (_, q) => (
							<a href={q.source} target="_blank" rel="noreferrer">
								查看原始资料
							</a>
						),
					},
					{
						title: "操作",
						render: (_, q) => (
							<Button
								aria-label={`复制${q.name}到本店`}
								disabled={q.status === "excluded"}
								onClick={async () => {
									try {
										await api("/quotes/copy-public", { public_id: q.id });
										await onSaved();
									} catch (e) {
										onError((e as Error).message);
									}
								}}
							>
								复制到本店
							</Button>
						),
					},
				]}
			/>
		</div>
	);
}
