import { Alert, Button, Descriptions, Space, Tag } from "antd";
import { useEffect, useRef, useState } from "react";
import type { FXPreview, Settings } from "../types";
import { time, useStoreApi } from "../lib";

export default function ExchangeRatePreview({
	settings,
	onApplied,
	onError,
}: {
	settings: Settings;
	onApplied: (s: Settings) => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [preview, setPreview] = useState<FXPreview>(),
		[busy, setBusy] = useState(false);
	const request = useRef(0);
	useEffect(
		() => () => {
			request.current++;
		},
		[],
	);
	const fetch = async () => {
		const seq = ++request.current;
		setBusy(true);
		setPreview(undefined);
		try {
			const v = await api<{ preview: FXPreview }>(
				"/exchange-rates/preview",
				{},
			);
			if (seq === request.current) setPreview(v.preview);
		} catch (e) {
			if (seq === request.current) onError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	const apply = async () => {
		if (!preview) return;
		const seq = ++request.current;
		setBusy(true);
		try {
			const v = await api<{ settings: Settings }>("/exchange-rates/apply", {
				preview_id: preview.id,
			});
			if (seq === request.current) {
				setPreview(undefined);
				await onApplied(v.settings);
			}
		} catch (e) {
			if (seq === request.current) onError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	return (
		<div className="exchange-rate-tool">
			<Space wrap>
				<Button loading={busy} onClick={fetch}>
					获取参考汇率
				</Button>
				<span className="muted-text">Frankfurter · CNY → RUB</span>
			</Space>
			{settings.fx_meta && (
				<p className="muted-text">
					{settings.fx_meta.demo ? "Mock 演示汇率" : "接口参考汇率"} · 获取于{" "}
					{time(settings.fx_meta.fetched_at)}
				</p>
			)}
			{preview && (
				<div className="exchange-rate-preview">
					<Space wrap>
						<strong>1 CNY = {preview.rate.rate} RUB</strong>
						<Tag
							color={preview.binding.environment === "mock" ? "gold" : "blue"}
						>
							{preview.binding.environment === "mock"
								? "Mock 演示"
								: "Frankfurter 综合参考"}
						</Tag>
					</Space>
					<Descriptions
						size="small"
						column={1}
						items={[
							{ key: "date", label: "数据日期", children: preview.rate.date },
							{
								key: "time",
								label: "获取时间",
								children: time(preview.fetched_at),
							},
							{
								key: "current",
								label: "当前设置",
								children: `1 CNY = ${settings.fx || "待补充"} RUB`,
							},
						]}
					/>
					<Alert
						type="info"
						showIcon
						title="参考汇率，非Ozon合同汇率"
						description="上游可能返回最近工作日数据。应用只更新本店汇率，已有计算记录保持原值。"
					/>
					<Button type="primary" loading={busy} onClick={apply}>
						应用到本店
					</Button>
				</div>
			)}
		</div>
	);
}
