import {
	Alert,
	App,
	Button,
	Checkbox,
	Descriptions,
	Drawer,
	Form,
	Image,
	Input,
	InputNumber,
	Select,
	Space,
	Tag,
} from "antd";
import { ImportOutlined, SearchOutlined } from "@ant-design/icons";
import { useEffect, useRef, useState } from "react";
import { history } from "@umijs/max";
import type { ImportPreview, Product } from "../types";
import { alibabaPriceHint, money, time, useStoreApi } from "../lib";

const mockImage =
	"data:image/svg+xml," +
	encodeURIComponent(
		'<svg xmlns="http://www.w3.org/2000/svg" width="160" height="120"><rect width="160" height="120" rx="8" fill="#eaf2fa"/><text x="80" y="66" text-anchor="middle" fill="#37638a" font-size="17">1688 演示商品</text></svg>',
	);
const unavailableImage =
	"data:image/svg+xml," +
	encodeURIComponent(
		'<svg xmlns="http://www.w3.org/2000/svg" width="160" height="120"><rect width="160" height="120" rx="8" fill="#eef1f5"/><text x="80" y="66" text-anchor="middle" fill="#627185" font-size="17">暂无商品图片</text></svg>',
	);

export default function AlibabaImport({
	open,
	dirty,
	onClose,
	onLoaded,
}: {
	open: boolean;
	dirty: boolean;
	onClose: () => void;
	onLoaded: (p: Product) => void;
}) {
	const api = useStoreApi(),
		{ modal } = App.useApp();
	const [input, setInput] = useState("");
	const [preview, setPreview] = useState<ImportPreview>();
	const [skuID, setSKUID] = useState(""),
		[quantity, setQuantity] = useState(1),
		[unit, setUnit] = useState(""),
		[price, setPrice] = useState<number | null>(null),
		[moq, setMOQ] = useState(1),
		[confirmed, setConfirmed] = useState(false);
	const [busy, setBusy] = useState(false),
		[error, setError] = useState("");
	const request = useRef(0);
	const confirmation = useRef<ReturnType<typeof modal.confirm> | null>(null);
	useEffect(
		() => () => {
			request.current++;
			confirmation.current?.destroy();
		},
		[],
	);
	const product = preview?.product,
		sku = product?.skus.find((s) => s.id === skuID);
	const quoteType = product?.quote_type;
	const quoteTiers =
		(quoteType === 3
			? sku?.tiers
			: quoteType === 0 || quoteType === 2
				? product?.tiers
				: []) || [];
	const candidatePrice = product
		? alibabaPriceHint(product, skuID, quantity)
		: null;
	useEffect(() => {
		setPrice(candidatePrice);
		setConfirmed(false);
	}, [candidatePrice, skuID, preview?.id]);
	const needsConfirmation =
		!!product &&
		(candidatePrice == null ||
			candidatePrice <= 0 ||
			!product.unit ||
			product.batch ||
			!product.moq ||
			quoteType == null ||
			quoteType < 0 ||
			quoteType > 3 ||
			(quoteType === 1
				? sku?.price == null
				: quoteType === 0 || quoteType === 2 || quoteType === 3
					? !quoteTiers.some((t) => t.quantity <= quantity)
					: true));
	const stock = sku?.stock ?? product?.stock;
	const invalidQuantity =
		quantity < (product?.moq || 1) || (stock != null && quantity > stock);
	const accept = (p: ImportPreview) => {
		setPreview(p);
		setConfirmed(false);
		if (p.product) {
			const first = p.product.skus[0];
			setSKUID(first?.id || "");
			setQuantity(p.product.moq || 1);
			setMOQ(p.product.moq || 1);
			setUnit(p.product.unit || "");
		}
	};
	const fetch = async () => {
		const seq = ++request.current;
		setBusy(true);
		setError("");
		setPreview(undefined);
		try {
			const v = await api<{ preview: ImportPreview }>(
				"/sourcing/1688/preview",
				{ input },
			);
			if (seq === request.current) accept(v.preview);
		} catch (e) {
			if (seq === request.current) setError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	const establish = async () => {
		if (!preview) return;
		const seq = ++request.current;
		setBusy(true);
		setError("");
		setPreview({ ...preview, status: "relation_attempted" });
		try {
			const v = await api<{ preview: ImportPreview }>(
				"/sourcing/1688/establish-relation",
				{ preview_id: preview.id, confirmed: true },
			);
			if (seq === request.current) accept(v.preview);
		} catch (e) {
			if (seq === request.current) setError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	const load = async () => {
		if (!preview) return;
		const seq = ++request.current;
		setBusy(true);
		setError("");
		try {
			const v = await api<{ product: Product }>("/sourcing/1688/load", {
				preview_id: preview.id,
				sku_id: skuID,
				quantity,
				quote_confirmed: confirmed,
				unit,
				unit_price: price,
				moq,
			});
			if (seq === request.current) {
				onLoaded(v.product);
				onClose();
			}
		} catch (e) {
			if (seq === request.current) setError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	const replace = () => {
		if (dirty) {
			confirmation.current = modal.confirm({
				title: "载入新的商品草稿？",
				content:
					"当前商品尚有未保存或未重算的修改。载入后将替换工作台内容，已保存商品保持不变。",
				okText: "载入新草稿",
				cancelText: "保留当前内容",
				onOk: load,
			});
		} else void load();
	};
	const relation = () => {
		confirmation.current = modal.confirm({
			title: "确认建立1688商品铺货关系",
			content: `将在当前店铺的${preview?.binding.environment === "mock" ? "Mock" : "真实"}1688授权账号中，为商品 ${preview?.offer_id} 建立铺货关系，并重新获取详情。`,
			okText: "确认建立关系",
			cancelText: "取消",
			onOk: establish,
		});
	};
	return (
		<Drawer
			title="载入1688商品"
			open={open}
			onClose={() => {
				request.current++;
				onClose();
			}}
			width={680}
			className="alibaba-import"
			destroyOnHidden
			footer={
				<Space wrap>
					<Button onClick={onClose}>取消</Button>
					<Button
						aria-label="载入新草稿"
						type="primary"
						icon={<ImportOutlined />}
						loading={busy}
						disabled={
							!product ||
							invalidQuantity ||
							(needsConfirmation &&
								(!confirmed || !unit || !price || quantity < moq))
						}
						onClick={replace}
					>
						载入新草稿
					</Button>
				</Space>
			}
		>
			<Form component={false} layout="vertical">
				<Form.Item
					label="1688商品链接或ID"
					extra="支持 detail.1688.com/offer/商品ID.html；Mock 可使用1001或1002。"
				>
					<Input.TextArea
						aria-label="1688商品链接或ID"
						value={input}
						onChange={(e) => {
							request.current++;
							setBusy(false);
							setInput(e.target.value);
							setPreview(undefined);
							setError("");
						}}
						autoSize={{ minRows: 2, maxRows: 4 }}
						placeholder="https://detail.1688.com/offer/… .html"
					/>
				</Form.Item>
			</Form>
			<Space wrap>
				<Button
					aria-label="获取商品详情"
					type="primary"
					icon={<SearchOutlined />}
					loading={busy}
					disabled={!input.trim()}
					onClick={fetch}
				>
					获取商品详情
				</Button>
				<Button
					type="link"
					onClick={() => {
						onClose();
						history.push("/settings?tab=integrations");
					}}
				>
					配置外部服务
				</Button>
			</Space>
			{error && (
				<Alert
					className="spaced-alert"
					type="error"
					showIcon
					title={error}
					description="原商品内容保持不变。可检查配置或重新获取详情。"
				/>
			)}
			{preview && (
				<div className="import-preview">
					<Space wrap>
						<Tag
							color={preview.binding.environment === "mock" ? "gold" : "blue"}
						>
							{preview.binding.environment === "mock"
								? "Mock 演示数据"
								: "1688商品接口"}
						</Tag>
						<span className="muted-text">
							商品 {preview.offer_id} · {time(preview.fetched_at)}
						</span>
					</Space>
					{preview.status === "relation_required" && (
						<Alert
							type="warning"
							showIcon
							title="该账号尚未建立此商品的铺货关系"
							description="确认建立关系后才能获取此接口的商品详情。"
							action={
								<Button disabled={busy} onClick={relation}>
									确认建立关系
								</Button>
							}
						/>
					)}
					{preview.status === "relation_attempted" && (
						<Alert
							type="info"
							showIcon
							title="已提交建立关系请求"
							description="结果不明确时请重新获取详情，避免重复建立关系。"
						/>
					)}
					{product && (
						<>
							<div className="import-product-heading">
								<Image
									width={110}
									height={90}
									style={{ objectFit: "contain" }}
									src={
										preview.binding.environment === "mock"
											? mockImage
											: product.images[0] || unavailableImage
									}
									fallback={unavailableImage}
									preview={false}
								/>
								<div>
									<h2>{product.title}</h2>
									<p className="muted-text">
										供应商：{product.supplier || "待补充"}
									</p>
									<p className="muted-text">
										1688类目：
										{product.category_name || product.category_id || "待补充"}
									</p>
								</div>
							</div>
							<Form component={false} layout="vertical">
								<Form.Item label="选择商品规格">
									<Select
										aria-label="选择商品规格"
										value={skuID}
										onChange={(value) => {
											setSKUID(value);
											setConfirmed(false);
										}}
										options={product.skus.map((s) => ({
											value: s.id,
											label: `${
												Object.entries(s.attributes)
													.map(([k, v]) => `${k}：${v}`)
													.join(" / ") || "默认规格"
											}${s.default ? "" : ` · SKU ${s.id}`}`,
										}))}
									/>
								</Form.Item>
								<div className="two-fields">
									<Form.Item label="采购数量">
										<InputNumber
											aria-label="导入采购数量"
											min={1}
											max={2147483647}
											precision={0}
											value={quantity}
											onChange={(v) => {
												setQuantity(v || 1);
												setConfirmed(false);
											}}
										/>
									</Form.Item>
									<Form.Item label="可用库存">
										<Input
											readOnly
											value={stock == null ? "未返回" : String(stock)}
										/>
									</Form.Item>
								</div>
								{invalidQuantity && (
									<Alert
										type="warning"
										showIcon
										title={`采购数量须达到起订量${product.moq || 1}，且不超过已知库存`}
									/>
								)}
								<Descriptions
									size="small"
									column={1}
									items={[
										{
											key: "price",
											label: "接口参考单价",
											children: money(candidatePrice),
										},
										{
											key: "unit",
											label: "计价单位",
											children: product.unit || "待确认",
										},
										{
											key: "moq",
											label: "最低起订量",
											children: product.moq || "待确认",
										},
										{
											key: "weight",
											label: "包装毛重",
											children: product.weight
												? `${product.weight} g（需核实所选SKU）`
												: "待补充",
										},
										{
											key: "size",
											label: "包装尺寸",
											children: [
												product.length,
												product.width,
												product.height,
											].every(Boolean)
												? `${product.length} × ${product.width} × ${product.height} cm`
												: "待补充",
										},
									]}
								/>
								{quoteTiers.length > 0 && (
									<p className="muted-text">
										采购阶梯：
										{quoteTiers
											.map((t) => `≥${t.quantity}：${money(t.price)}`)
											.join("；")}
									</p>
								)}
								{needsConfirmation && (
									<div className="import-quote-confirm">
										<Alert
											type="warning"
											showIcon
											title="请确认采购报价及计价单位"
											description="将所选规格的一个销售单位作为计算单位；批量或混批条件需自行核实。"
										/>
										<div className="two-fields">
											<Form.Item label="确认单价 CNY">
												<InputNumber
													aria-label="确认采购单价"
													min={0.000001}
													max={1000000}
													value={price}
													onChange={(v) => {
														setPrice(v);
														setConfirmed(false);
													}}
												/>
											</Form.Item>
											<Form.Item label="确认计价单位">
												<Input
													aria-label="确认计价单位"
													placeholder="例如：个、件、套"
													value={unit}
													onChange={(e) => {
														setUnit(e.target.value);
														setConfirmed(false);
													}}
												/>
											</Form.Item>
										</div>
										<Form.Item label="确认最低起订量">
											<InputNumber
												aria-label="确认最低起订量"
												min={product.moq || 1}
												precision={0}
												value={moq}
												onChange={(v) => {
													setMOQ(v || 1);
													setConfirmed(false);
												}}
											/>
										</Form.Item>
										<Checkbox
											checked={confirmed}
											onChange={(e) => setConfirmed(e.target.checked)}
										>
											已核对所选规格的单价、销售单位及起订条件
										</Checkbox>
									</div>
								)}
							</Form>
							<Alert
								type="info"
								showIcon
								title="载入后继续补全成本"
								description={
									<ul className="import-warnings">
										{product.warnings.map((w) => (
											<li key={w}>{w}</li>
										))}
									</ul>
								}
							/>
						</>
					)}
				</div>
			)}
		</Drawer>
	);
}
