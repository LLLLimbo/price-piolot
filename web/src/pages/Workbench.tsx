import {
	App,
	Alert,
	Button,
	ConfigProvider,
	Dropdown,
	InputNumber,
	Popconfirm,
	Select,
	Slider,
	Space,
	Spin,
	Table,
	Tag,
	Tooltip,
	Tabs,
	theme,
} from "antd";
import zhCN from "antd/locale/zh_CN";
import { ProLayout } from "@ant-design/pro-components";
import {
	AppstoreOutlined,
	ArrowRightOutlined,
	CalculatorOutlined,
	CheckOutlined,
	DatabaseOutlined,
	FileTextOutlined,
	HistoryOutlined,
	PlusOutlined,
	ReloadOutlined,
	SettingOutlined,
	SlidersOutlined,
	ThunderboltOutlined,
	TrophyOutlined,
	BulbOutlined,
	DeleteOutlined,
	MenuOutlined,
} from "@ant-design/icons";
import { history, useLocation } from "@umijs/max";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	Bootstrap,
	Product,
	Result,
	RunSummary,
	Snapshot,
	Store,
	ContextResult,
	SalesEvaluation,
	Calculation,
	FrozenRun,
} from "../types";
import {
	useStoreApi,
	api as rootAPI,
	ApiContext,
	scopedAPI,
	exportBundleCSV,
	download,
	clone,
	modes,
	money,
	newProduct,
	time,
} from "../lib";
import ProductPanel from "../components/ProductPanel";
import Results from "../components/Results";
import Competitors from "../components/Competitors";
import Quotes from "../components/Quotes";
import StoresPage from "../components/StoresPage";
import PromotionsPage from "../components/PromotionsPage";
import SalesPanel from "../components/SalesPanel";
import PublicQuotes from "../components/PublicQuotes";
import SettingsPage from "../components/SettingsPage";
import AlibabaImport from "../components/AlibabaImport";
import ProfitCheckPanel from "../components/ProfitCheckPanel";
import "../style.css";

const iconModes = {
	profit: <TrophyOutlined />,
	breakeven: <CheckOutlined />,
	delivery: <ThunderboltOutlined />,
	target: <SlidersOutlined />,
};
function Brand() {
	return (
		<svg width="32" height="32" viewBox="0 0 40 40" aria-hidden="true">
			<rect width="40" height="40" rx="9" fill="#0067c0" />
			<path
				d="M12 29V11h8a6 6 0 0 1 0 12h-4v6m0-10h4a2 2 0 0 0 0-4h-4"
				fill="#fafcff"
			/>
			<circle cx="29" cy="28" r="3" fill="#b9ddff" />
		</svg>
	);
}
export default function Root() {
	useEffect(() => {
		document.documentElement.lang = "zh-CN";
	}, []);
	const [dark, setDark] = useState(
		() => localStorage.getItem("price-pilot-theme") === "dark",
	);
	useEffect(() => {
		document.documentElement.dataset.theme = dark ? "dark" : "light";
	}, [dark]);
	return (
		<ConfigProvider
			locale={zhCN}
			theme={{
				algorithm: dark ? theme.darkAlgorithm : theme.defaultAlgorithm,
				token: {
					colorPrimary: "#0067c0",
					colorSuccess: "#247052",
					colorInfo: "#0067c0",
					colorBgLayout: dark ? "#171d25" : "#f1f3f7",
					colorBgContainer: dark ? "#202833" : "#fdfefe",
					colorText: dark ? "#e9edf2" : "#1c2938",
					colorTextSecondary: dark ? "#a6b2c3" : "#627185",
					colorBorder: dark ? "#3b4551" : "#dce2e9",
					borderRadius: 7,
					fontFamily:
						'"Segoe UI Variable", "Segoe UI", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif',
					fontSize: 13,
					controlHeight: 35,
				},
				components: {
					Table: {
						headerBg: dark ? "#252f3c" : "#f7f9fb",
						cellPaddingBlock: 13,
					},
					Button: { primaryShadow: "none" },
					Tabs: { horizontalItemGutter: 26 },
				},
			}}
		>
			<App>
				<StoreHost
					dark={dark}
					toggleTheme={() => {
						setDark(!dark);
						localStorage.setItem("price-pilot-theme", dark ? "light" : "dark");
					}}
				/>
			</App>
		</ConfigProvider>
	);
}
function StoreHost({
	dark,
	toggleTheme,
}: {
	dark: boolean;
	toggleTheme: () => void;
}) {
	const [stores, setStores] = useState<Store[]>([]);
	const [storeId, setStoreId] = useState(
		() => localStorage.getItem("price-pilot-store") || "default",
	);
	const [failure, setFailure] = useState("");
	const load = useCallback(async () => {
		try {
			const v = await rootAPI<{ stores: Store[] }>("/stores");
			setStores(v.stores);
			setFailure("");
			if (!v.stores.some((p) => p.id === storeId)) {
				setStoreId(
					v.stores.find((p) => !p.archived)?.id || v.stores[0]?.id || "default",
				);
			}
		} catch (e) {
			setFailure((e as Error).message);
		}
	}, [storeId]);
	useEffect(() => {
		void load();
	}, [load]);
	const select = (id: string) => {
		setStoreId(id);
		localStorage.setItem("price-pilot-store", id);
		history.push("/");
	};
	const scoped = useMemo(() => scopedAPI(storeId), [storeId]);
	if (!stores.length)
		return (
			<div className="boot-state">
				{failure ? (
					<>
						<Alert type="error" title={failure} />
						<Button onClick={load}>重试连接</Button>
					</>
				) : (
					<Spin />
				)}
			</div>
		);
	return (
		<ApiContext.Provider value={scoped}>
			<Workbench
				key={storeId}
				storeId={storeId}
				stores={stores}
				onStoresChanged={load}
				onStoreSelect={select}
				dark={dark}
				toggleTheme={toggleTheme}
			/>
		</ApiContext.Provider>
	);
}
function Workbench({
	storeId,
	stores,
	onStoresChanged,
	onStoreSelect,
	dark,
	toggleTheme,
}: {
	storeId: string;
	stores: Store[];
	onStoresChanged: () => Promise<void>;
	onStoreSelect: (id: string) => void;
	dark: boolean;
	toggleTheme: () => void;
}) {
	const api = useStoreApi();
	const { message } = App.useApp();
	const location = useLocation();
	const path = location.pathname;
	const [data, setData] = useState<Bootstrap>();
	const [product, setProduct] = useState<Product>();
	const [snapshot, setSnapshot] = useState<Snapshot>();
	const [result, setResult] = useState<Result>();
	const [contexts, setContexts] = useState<ContextResult[]>([]);
	const [activeContext, setActiveContext] = useState("daily");
	const [salesResults, setSalesResults] = useState<SalesEvaluation[]>([]);
	const [selectedPromotions, setSelectedPromotions] = useState<string[]>([]);
	const promotionIds = useRef<string[]>([]);
	const promotionsInitialized = useRef(false);
	const [historicalRun, setHistoricalRun] = useState<FrozenRun>();
	const [busy, setBusy] = useState(false);
	const [saving, setSaving] = useState(false);
	const [discovering, setDiscovering] = useState(false);
	const [dirty, setDirty] = useState(false);
	const [bootError, setBootError] = useState("");
	const [collapsed, setCollapsed] = useState(false);
	const [importOpen, setImportOpen] = useState(false);
	const revision = useRef(0);
	const sequence = useRef(0);
	const error = useCallback(
		(s: string) => {
			message.error(s, 6);
		},
		[message],
	);
	const calculate = useCallback(
		async (p: Product, snap?: Snapshot) => {
			const seq = ++sequence.current;
			const rev = revision.current;
			setBusy(true);
			try {
				const v = await api<Calculation>("/calculate", {
					product: p,
					snapshot_id: snap?.id || "",
					promotion_ids: promotionIds.current,
				});
				if (seq === sequence.current && rev === revision.current) {
					setResult(v.result);
					setContexts(v.contexts);
					setSalesResults(v.sales_results);
					setActiveContext("daily");
					setHistoricalRun(undefined);
					setDirty(false);
				}
			} catch (e) {
				if (seq === sequence.current) error((e as Error).message);
			} finally {
				if (seq === sequence.current) setBusy(false);
			}
		},
		[error, api],
	);
	const bootstrap = useCallback(async () => {
		try {
			const v = await api<Bootstrap>("/bootstrap");
			setData(v);
			if (!promotionsInitialized.current) {
				promotionIds.current = v.promotions.map((p) => p.id);
				setSelectedPromotions(promotionIds.current);
				promotionsInitialized.current = true;
			} else {
				promotionIds.current = promotionIds.current.filter((id) =>
					v.promotions.some((p) => p.id === id),
				);
				setSelectedPromotions(promotionIds.current);
			}
			setBootError("");
			return v;
		} catch (e) {
			setBootError((e as Error).message);
			return undefined;
		}
	}, [api]);
	useEffect(() => {
		let active = true;
		(async () => {
			const v = await bootstrap();
			if (!active || !v) return;
			const preferred = localStorage.getItem(`price-pilot-product:${storeId}`);
			const p =
				v.products.find((p) => p.id === preferred) ||
				v.products[0] ||
				newProduct(v.template);
			if (p) {
				setProduct(clone(p));
				const snap = v.snapshots.find((s) => s.product_id === p.id);
				setSnapshot(snap);
				if (p.name && !v.store.archived) calculate(p, snap);
			}
		})();
		return () => {
			active = false;
			sequence.current++;
			revision.current++;
		};
	}, [bootstrap, calculate, storeId]);
	const change = (p: Product) => {
		setSalesResults([]);
		revision.current++;
		setProduct(p);
		setDirty(true);
	};
	const choose = (p: Product) => {
		setHistoricalRun(undefined);
		setContexts([]);
		setSalesResults([]);
		revision.current++;
		const next = clone(p);
		setProduct(next);
		localStorage.setItem(`price-pilot-product:${storeId}`, next.id);
		const snap = data?.snapshots.find((s) => s.product_id === next.id);
		setSnapshot(snap);
		setResult(undefined);
		setDirty(false);
		history.push("/");
		calculate(next, snap);
	};
	const refresh = async () => {
		setSalesResults([]);
		revision.current++;
		await bootstrap();
		setDirty(true);
		message.success("已保存");
	};
	const saveProduct = async () => {
		if (!product) return;
		try {
			setSaving(true);
			const v = await api<{ product: Product }>(
				`/products${data?.products.some((p) => p.id === product.id) ? `/${product.id}` : ""}`,
				product,
				data?.products.some((p) => p.id === product.id) ? "PUT" : "POST",
			);
			setProduct(v.product);
			localStorage.setItem(`price-pilot-product:${storeId}`, v.product.id);
			await bootstrap();
			message.success("商品档案已保存");
		} catch (e) {
			error((e as Error).message);
		} finally {
			setSaving(false);
		}
	};
	const savePeers = async (rows: Snapshot["competitors"]) => {
		if (!product) return;
		const v = snapshot
			? await api<{ snapshot: Snapshot }>(
					`/snapshots/${snapshot.id}`,
					{ ...snapshot, competitors: rows },
					"PUT",
				)
			: await api<{ snapshot: Snapshot }>("/snapshots", {
					product,
					competitors: rows,
				});
		setSnapshot(v.snapshot);
		await bootstrap();
		setDirty(true);
		revision.current++;
		await calculate(product, v.snapshot);
	};
	const discover = async () => {
		if (!product) return;
		try {
			setDiscovering(true);
			const v = await api<{ snapshot: Snapshot }>("/competitors/discover", {
				product,
				previous_id: snapshot?.id,
			});
			setSnapshot(v.snapshot);
			await bootstrap();
			revision.current++;
			setDirty(true);
			await calculate(product, v.snapshot);
			message.success(
				v.snapshot.competitors.length
					? "竞品快照已更新，低置信候选请核对"
					: "本次未找到匹配竞品",
			);
		} catch (e) {
			error((e as Error).message);
		} finally {
			setDiscovering(false);
		}
	};
	const loadDemo = async () => {
		setHistoricalRun(undefined);
		setContexts([]);
		setSalesResults([]);
		try {
			const v = await api<{ product: Product; snapshot: Snapshot }>(
				"/demo",
				{},
			);
			revision.current++;
			setProduct(v.product);
			setSnapshot(v.snapshot);
			setResult(undefined);
			await bootstrap();
			history.push("/");
			await calculate(v.product, v.snapshot);
		} catch (e) {
			error((e as Error).message);
		}
	};
	const mockPaper = () => {
		setHistoricalRun(undefined);
		setContexts([]);
		setSalesResults([]);
		if (!product) return;
		const p = clone(product);
		p.id = "seerfar-paper";
		p.name = "打印纸 A4 · 5包（Mock联调）";
		p.demo = true;
		p.seed_sku = "149222760";
		p.keywords = ["Бумага"];
		p.category_id = "17027492_17029017_91400";
		p.attributes = {};
		p.weight = 12520;
		p.length = 30.5;
		p.width = 28;
		p.height = 22;
		p.own_seller_ids = [];
		revision.current++;
		setProduct(p);
		setSnapshot(undefined);
		setResult(undefined);
		setDirty(true);
		history.push("/");
		message.info(
			"已载入Seerfar测试SKU，点击“获取竞品”读取Mock数据。成本仍为演示假设。",
			6,
		);
	};
	const create = () => {
		setHistoricalRun(undefined);
		setContexts([]);
		setSalesResults([]);
		if (!product) return;
		revision.current++;
		setProduct(newProduct(product));
		setSnapshot(undefined);
		setResult(undefined);
		setDirty(true);
		history.push("/");
	};
	const loadImported = (p: Product) => {
		revision.current++;
		sequence.current++;
		setProduct(p);
		setSnapshot(undefined);
		setResult(undefined);
		setContexts([]);
		setSalesResults([]);
		setHistoricalRun(undefined);
		setBusy(false);
		setDirty(true);
		history.push("/");
		message.success("已载入1688商品草稿，请补全成本后保存和测算");
	};
	const openRun = async (run: RunSummary) => {
		try {
			const v = await api<FrozenRun>(`/runs/${run.id}`);
			revision.current++;
			sequence.current++;
			setProduct(v.input.product);
			setSnapshot(v.input.snapshot);
			setResult(v.result);
			setContexts(v.contexts || []);
			setSalesResults(v.sales_results || []);
			setActiveContext("daily");
			setHistoricalRun(v);
			setDirty(false);
			history.push("/");
		} catch (e) {
			error((e as Error).message);
		}
	};
	const nav = [
		{ path: "/", name: "定价工作台", icon: <CalculatorOutlined /> },
		{ path: "/products", name: "商品档案", icon: <AppstoreOutlined /> },
		{ path: "/quotes", name: "物流与报价", icon: <DatabaseOutlined /> },
		{ path: "/history", name: "计算记录", icon: <HistoryOutlined /> },
		{ path: "/settings", name: "费率与评分", icon: <SettingOutlined /> },
		{ path: "/promotions", name: "促销方案", icon: <SlidersOutlined /> },
		{ path: "/stores", name: "店铺管理", icon: <AppstoreOutlined /> },
	];
	return (
		<div className={`pilot-app ${dark ? "dark" : ""}`}>
			<ProLayout
				title="Price Pilot"
				logo={<Brand />}
				layout="side"
				headerRender={false}
				fixSiderbar
				siderWidth={208}
				collapsed={collapsed}
				onCollapse={setCollapsed}
				collapsedButtonRender={false}
				location={{ pathname: path }}
				route={{ routes: nav }}
				menuItemRender={(item, dom) => (
					<a
						aria-label={String(item.name || "")}
						href={item.path}
						onClick={(e) => {
							e.preventDefault();
							history.push(item.path || "/");
						}}
					>
						{dom}
					</a>
				)}
				menuFooterRender={() =>
					!collapsed && (
						<div className="sidebar-foot">
							<span className="connection-dot" />
							人民币结算 · 俄罗斯市场<small>1688 → Ozon</small>
						</div>
					)
				}
				token={{
					pageContainer: {
						paddingInlinePageContainerContent: 0,
						paddingBlockPageContainerContent: 0,
					},
					sider: {
						colorMenuBackground: dark ? "#1c2430" : "#f8fafc",
						colorTextMenuSelected: "#0067c0",
						colorBgMenuItemSelected: dark ? "#12344f" : "#e8f1fa",
					},
				}}
			>
				<header className="workspace-bar">
					<div>
						<Button
							aria-label="折叠菜单"
							type="text"
							icon={<MenuOutlined />}
							onClick={() => setCollapsed(!collapsed)}
						/>
						<Select
							aria-label="选择店铺"
							className="store-select"
							value={storeId}
							onChange={onStoreSelect}
							options={stores.map((p) => ({
								value: p.id,
								label: p.name + (p.archived ? " · 已归档" : ""),
							}))}
						/>
						<span className="bar-separator">/</span>
						<strong>
							{nav.find((n) => n.path === path)?.name || "定价工作台"}
						</strong>
					</div>
					<Space>
						<span className="connection-label">
							<span
								className={`connection-dot ${data?.seerfar.configured ? "" : "offline"}`}
							/>
							{data?.seerfar.configured
								? `Seerfar${data.seerfar.mock ? " Mock" : ""}`
								: "数据源未配置"}
						</span>
						<Tooltip title="切换明暗主题">
							<Button
								type="text"
								aria-label="切换主题"
								icon={<BulbOutlined />}
								onClick={toggleTheme}
							/>
						</Tooltip>
					</Space>
				</header>
				{!data || !product ? (
					<div className="boot-state">
						{bootError ? (
							<>
								<Alert type="error" title={bootError} showIcon />
								<Button
									onClick={async () => {
										const v = await bootstrap();
										if (v) {
											const p = v.products[0] || newProduct(v.template);
											setProduct(p);
											const snap = v.snapshots.find(
												(s) => s.product_id === p.id,
											);
											setSnapshot(snap);
											if (p.name && !v.store.archived) calculate(p, snap);
										}
									}}
									icon={<ReloadOutlined />}
								>
									重试连接
								</Button>
							</>
						) : (
							<Spin tip="正在载入工作空间">
								<div style={{ height: 120 }} />
							</Spin>
						)}
					</div>
				) : (
					<main>
						{data.store.archived && (
							<Alert
								type="warning"
								showIcon
								className="spaced-alert"
								title="此店铺已归档，保留资料与历史记录。恢复后可继续编辑和测算。"
							/>
						)}
						<ConfigProvider
							componentDisabled={
								data.store.archived && !["/stores", "/history"].includes(path)
							}
						>
							{path === "/quotes" ? (
								<Tabs
									items={[
										{
											key: "store",
											label: "店铺协议报价",
											children: (
												<Quotes
													quotes={data.quotes}
													onSaved={refresh}
													onError={error}
												/>
											),
										},
										{
											key: "public",
											label: "公共物流资料",
											children: (
												<PublicQuotes
													quotes={data.public_quotes}
													onSaved={refresh}
													onError={error}
												/>
											),
										},
									]}
								/>
							) : path === "/settings" ? (
								<SettingsPage data={data} onSaved={refresh} onError={error} />
							) : path === "/stores" ? (
								<ConfigProvider componentDisabled={false}>
									<StoresPage
										stores={stores}
										onSaved={async () => {
											await onStoresChanged();
											await bootstrap();
										}}
										onSelect={onStoreSelect}
										onError={error}
									/>
								</ConfigProvider>
							) : path === "/promotions" ? (
								<PromotionsPage
									promotions={data.promotions}
									onSaved={refresh}
									onError={error}
								/>
							) : path === "/products" ? (
								<div className="page-surface">
									<div className="page-heading">
										<div>
											<span className="eyebrow">PRODUCT LIBRARY</span>
											<h1>商品档案</h1>
											<p>保存采购成本、商品规格和独立的定价目标。</p>
										</div>
										<Button
											type="primary"
											icon={<PlusOutlined />}
											onClick={create}
										>
											新建商品
										</Button>
									</div>
									<Table<Product>
										rowKey="id"
										dataSource={data.products}
										scroll={{ x: 650 }}
										columns={[
											{
												title: "商品",
												render: (_, p) => (
													<>
														<strong>{p.name}</strong>
														{p.demo && <Tag>演示</Tag>}
														<small className="block muted-text">
															{p.supplier || "供应商待补充"}
														</small>
													</>
												),
											},
											{
												title: "供货",
												render: (_, p) =>
													p.supply === "dropship" ? "一件代发" : "自备货",
											},
											{ title: "包装重", render: (_, p) => `${p.weight}g` },
											{
												title: "当前模式",
												render: (_, p) => modes[p.mode]?.name,
											},
											{
												title: "操作",
												render: (_, p) => (
													<Space>
														<Button size="small" onClick={() => choose(p)}>
															打开测算
														</Button>
														<Popconfirm
															title="删除该商品档案？"
															description="已保存的计算记录仍会保留。"
															onConfirm={async () => {
																try {
																	await api(
																		`/products/${p.id}`,
																		undefined,
																		"DELETE",
																	);
																	await bootstrap();
																} catch (e) {
																	error((e as Error).message);
																}
															}}
														>
															<Button
																aria-label={`删除${p.name}`}
																type="text"
																icon={<DeleteOutlined />}
															/>
														</Popconfirm>
													</Space>
												),
											},
										]}
									/>
								</div>
							) : path === "/history" ? (
								<div className="page-surface">
									<div className="page-heading">
										<div>
											<span className="eyebrow">CALCULATION RECORDS</span>
											<h1>计算记录</h1>
											<p>每条记录保存当时的费用、报价版本、汇率与竞品快照。</p>
										</div>
										<Button icon={<ReloadOutlined />} onClick={bootstrap}>
											刷新
										</Button>
									</div>
									<Table<RunSummary>
										rowKey="id"
										dataSource={data.runs}
										scroll={{ x: 800 }}
										locale={{ emptyText: "计算方案后会自动保存记录" }}
										columns={[
											{
												title: "商品 / 模式",
												render: (_, r) => (
													<>
														<strong>{r.product}</strong>
														<small className="block muted-text">
															{modes[r.mode]?.name}
															{r.demo && " · 演示"}
															{r.legacy && " · 旧口径只读"}
															{!r.legacy && ` · ${r.context_count || 1}个情景`}
														</small>
													</>
												),
											},
											{
												title: "计算时间",
												render: (_, r) => time(r.calculated_at),
											},
											{
												title: "建议售价",
												render: (_, r) => money(r.recommended?.price),
											},
											{
												title: "预计利润",
												render: (_, r) => money(r.recommended?.profit),
											},
											{
												title: "结果",
												render: (_, r) => (
													<Tag
														color={
															r.status === "ok"
																? "green"
																: r.status === "partial"
																	? "gold"
																	: "default"
														}
													>
														{r.status === "ok"
															? "有可行方案"
															: r.status === "partial"
																? "评分待补充"
																: "无可行方案"}
													</Tag>
												),
											},
											{
												title: "",
												render: (_, r) => (
													<Button
														aria-label="查看记录"
														onClick={() => openRun(r)}
													>
														查看记录
													</Button>
												),
											},
										]}
									/>
								</div>
							) : (
								<>
									<div className="workbench-heading">
										<div>
											<span className="eyebrow">PRICING WORKBENCH</span>
											<h1>商品定价工作台</h1>
											<p>比较完整成本、物流时效与Ozon市场价格。</p>
										</div>
										<Space wrap>
											<Select
												aria-label="选择商品"
												className="product-select"
												value={
													data.products.some((p) => p.id === product.id)
														? product.id
														: undefined
												}
												placeholder="未保存商品"
												style={{ width: 220 }}
												options={data.products.map((p) => ({
													value: p.id,
													label: p.name,
												}))}
												onChange={(value) => {
													const p = data.products.find((p) => p.id === value);
													if (p) choose(p);
												}}
											/>
											<Dropdown
												trigger={["click"]}
												placement="bottomRight"
												menu={{
													items: [
														{
															key: "new",
															label: "新建真实商品",
															icon: <PlusOutlined />,
														},
														{ key: "demo", label: "载入演示商品" },
														{ key: "1688", label: "载入1688商品" },
														{ key: "paper", label: "载入Seerfar测试SKU" },
													],
													onClick: ({ key }) =>
														key === "new"
															? create()
															: key === "demo"
																? loadDemo()
																: key === "1688"
																	? setImportOpen(true)
																	: mockPaper(),
												}}
											>
												<Button aria-label="商品操作" icon={<PlusOutlined />}>
													商品操作
												</Button>
											</Dropdown>
										</Space>
									</div>
									{product.demo && (
										<div className="demo-notice">
											<FileTextOutlined />
											<span>演示工作空间</span>
											<p>
												费用、汇率与供货评分为示例假设；可切换至你的真实商品。
											</p>
											<Button type="link" size="small" onClick={create}>
												新建我的商品 <ArrowRightOutlined />
											</Button>
										</div>
									)}
									{historicalRun && (
										<Alert
											type="info"
											showIcon
											className="spaced-alert"
											title={
												historicalRun.schema_version < 2
													? "历史记录只读：保留原有公式与结果，未用新口径重算。"
													: "历史记录只读：以下为当时冻结的情景、费用版本和销量假设。"
											}
											action={
												<Button
													onClick={() =>
														choose(
															data.products.find((p) => p.id === product.id) ||
																data.products[0] ||
																newProduct(data.template),
														)
													}
												>
													返回当前参数
												</Button>
											}
										/>
									)}
									<ConfigProvider
										componentDisabled={
											Boolean(historicalRun) || data.store.archived
										}
									>
										<div className="workbench-grid">
											<ProductPanel
												key={product.id}
												product={product}
												quotes={data.quotes}
												settings={data.settings}
												onChange={change}
												onSave={saveProduct}
												saving={saving}
											/>
											<div className="analysis-panel">
												{!historicalRun && (
													<ProfitCheckPanel
														key={product.id}
														product={product}
													/>
												)}
												<section className="strategy-panel">
													<div className="panel-title">
														<div>
															<span className="section-index">02</span>
															<h2>定价目标</h2>
														</div>
														<span className="muted-text">
															每次只启用一种模式
														</span>
													</div>
													<div className="promotion-picker">
														<label htmlFor="selected-promotions">
															同时试算活动
														</label>
														<Select
															id="selected-promotions"
															aria-label="同时试算活动"
															mode="multiple"
															value={selectedPromotions}
															placeholder="可选，日常情景始终计算"
															options={data.promotions.map((p) => ({
																value: p.id,
																label: p.name,
															}))}
															onChange={(ids) => {
																promotionIds.current = ids;
																setSelectedPromotions(ids);
																revision.current++;
																setDirty(true);
															}}
															maxTagCount="responsive"
														/>
													</div>
													<div
														className="mode-options"
														role="radiogroup"
														aria-label="定价模式"
													>
														{Object.entries(modes).map(([key, m]) => (
															<button
																type="button"
																role="radio"
																disabled={
																	Boolean(historicalRun) || data.store.archived
																}
																aria-checked={product.mode === key}
																key={key}
																className={`mode-option ${product.mode === key ? "selected" : ""}`}
																onClick={() => {
																	const p = { ...product, mode: key };
																	change(p);
																	calculate(p, snapshot);
																}}
															>
																<span className="mode-icon">
																	{iconModes[key as keyof typeof iconModes]}
																</span>
																<strong>{m.name}</strong>
																<small>{m.short}</small>
																{product.mode === key && (
																	<CheckOutlined className="mode-check" />
																)}
															</button>
														))}
													</div>
													<div className="mode-controls">
														<div className="mode-explanation">
															<strong>
																{modes[product.mode]?.description}
															</strong>
															<span>
																汇率保护 ±
																{Math.round(data.settings.stress * 100)}% ·{" "}
																{data.settings.region}
															</span>
														</div>
														<div className="mode-inputs">
															{product.mode === "profit" && (
																<label>
																	目标净利率
																	<InputNumber
																		aria-label="目标净利率"
																		value={product.options.margin * 100}
																		min={0}
																		max={99.99}
																		precision={2}
																		suffix="%"
																		onChange={(v) =>
																			change({
																				...product,
																				options: {
																					...product.options,
																					margin: (v ?? 0) / 100,
																				},
																			})
																		}
																	/>
																</label>
															)}
															{product.mode === "breakeven" && (
																<Tag bordered={false} color="blue">
																	压力情景预计利润 ≥ 0
																</Tag>
															)}
															{product.mode === "delivery" && (
																<label>
																	每件最大亏损
																	<InputNumber
																		aria-label="时效模式亏损额度"
																		value={product.options.delivery_loss}
																		min={0}
																		precision={2}
																		prefix="¥"
																		onChange={(v) =>
																			change({
																				...product,
																				options: {
																					...product.options,
																					delivery_loss: v ?? 0,
																				},
																			})
																		}
																	/>
																</label>
															)}
															{product.mode === "target" && (
																<>
																	<label>
																		每件最大亏损
																		<InputNumber
																			aria-label="目标模式亏损额度"
																			value={product.options.target_loss}
																			min={0}
																			precision={2}
																			prefix="¥"
																			onChange={(v) =>
																				change({
																					...product,
																					options: {
																						...product.options,
																						target_loss: v ?? 0,
																					},
																				})
																			}
																		/>
																	</label>
																	<label>
																		最高标价 CNY
																		<InputNumber
																			aria-label="目标模式最高售价"
																			value={product.options.target_max_price}
																			min={0.01}
																			max={1000000}
																			placeholder="默认竞品P90"
																			onChange={(v) =>
																				change({
																					...product,
																					options: {
																						...product.options,
																						target_max_price: v,
																					},
																				})
																			}
																		/>
																	</label>
																</>
															)}
															<Button
																aria-label="计算方案"
																type="primary"
																size="large"
																icon={<CalculatorOutlined />}
																loading={busy}
																onClick={() => calculate(product, snapshot)}
															>
																计算方案
															</Button>
														</div>
													</div>
													{product.mode === "target" && (
														<div className="target-slider">
															<div>
																<strong>
																	目标至少{" "}
																	<span>{product.options.target_score}</span> 分
																</strong>
																<span>供应商固定，总分可能存在上限</span>
															</div>
															<Slider
																aria-label="竞争力目标分数"
																min={0}
																max={100}
																value={product.options.target_score}
																marks={{
																	0: "0",
																	50: "50",
																	80: "80",
																	100: "100",
																}}
																onChange={(v) =>
																	change({
																		...product,
																		options: {
																			...product.options,
																			target_score: v,
																		},
																	})
																}
															/>
														</div>
													)}
												</section>
												<section
													className={`output-panel ${busy ? "is-calculating" : ""}`}
												>
													<div className="panel-title">
														<div>
															<span className="section-index">03</span>
															<h2>定价结果</h2>
														</div>
														{result && (
															<Tag bordered={false}>
																{result.status === "infeasible" ||
																result.status === "invalid"
																	? "目标不可达"
																	: result.status === "partial"
																		? "部分数据待补充"
																		: "测算约束已满足"}
															</Tag>
														)}
													</div>
													{contexts.length > 0 && (
														<>
															<div className="context-comparison">
																<div className="section-toolbar">
																	<div>
																		<h3>日常与活动对比</h3>
																		<p>同一竞品快照 · 各自定价</p>
																	</div>
																	<ConfigProvider componentDisabled={dirty}>
																		<Button
																			aria-label="导出全部情景"
																			onClick={() =>
																				download(
																					exportBundleCSV(
																						historicalRun?.store?.name ||
																							data.store.name,
																						product,
																						contexts,
																						salesResults,
																					),
																					`PricePilot-全部情景-${product.name}.csv`,
																				)
																			}
																		>
																			导出全部情景
																		</Button>
																	</ConfigProvider>
																</div>
																<Table<ContextResult>
																	rowKey="id"
																	dataSource={contexts}
																	pagination={false}
																	size="small"
																	scroll={{ x: 780 }}
																	rowClassName={(c) =>
																		c.id === activeContext
																			? "recommended-row"
																			: ""
																	}
																	columns={[
																		{
																			title: "情景",
																			render: (_, c) => (
																				<>
																					<ConfigProvider
																						componentDisabled={false}
																					>
																						<Button
																							type="link"
																							aria-label={`查看${c.name}结果`}
																							onClick={() => {
																								setActiveContext(c.id);
																								setResult(c.result);
																							}}
																						>
																							{c.name}
																						</Button>
																					</ConfigProvider>
																					<small className="block muted-text">
																						{c.state}
																					</small>
																				</>
																			),
																		},
																		{
																			title: "建议标价",
																			render: (_, c) =>
																				money(c.result.recommended?.price),
																		},
																		{
																			title: "商家成交价",
																			render: (_, c) =>
																				money(c.result.recommended?.sale_price),
																		},
																		{
																			title: "买家参考价",
																			render: (_, c) =>
																				money(
																					c.result.recommended?.price_rub,
																					"RUB",
																				),
																		},
																		{
																			title: "预计利润",
																			render: (_, c) =>
																				money(c.result.recommended?.profit),
																		},
																		{
																			title: "状态",
																			render: (_, c) => (
																				<Tag
																					color={
																						c.result.status === "ok"
																							? "green"
																							: "gold"
																					}
																				>
																					{c.result.status === "ok"
																						? "满足测算约束"
																						: c.result.status === "invalid"
																							? "待补充"
																							: c.result.status === "partial"
																								? "部分数据缺失"
																								: "不可达"}
																				</Tag>
																			),
																		},
																	]}
																/>
															</div>
														</>
													)}
													<ConfigProvider componentDisabled={dirty}>
														<Results
															result={result}
															product={
																contexts.find((c) => c.id === activeContext)
																	?.input.product || product
															}
															snapshot={snapshot}
															dirty={dirty}
														/>
													</ConfigProvider>
												</section>
												<section className="market-panel">
													<Competitors
														product={product}
														snapshot={snapshot}
														loading={discovering}
														configured={data.seerfar.configured}
														onDiscover={discover}
														onSave={savePeers}
														onError={error}
													/>
												</section>
												<SalesPanel
													data={
														historicalRun
															? {
																	...data,
																	sales_scenarios:
																		historicalRun.sales_scenarios || [],
																	promotions: historicalRun.promotions || [],
																	quotes:
																		historicalRun.contexts?.[0]?.input.quotes ||
																		data.quotes,
																}
															: data
													}
													product={product}
													snapshot={snapshot}
													results={salesResults}
													onSaved={refresh}
													onResults={setSalesResults}
													onError={error}
												/>
											</div>
										</div>
									</ConfigProvider>
									<footer className="workspace-foot">
										Price Pilot{" "}
										<span>单件经济模型 · 人民币结算 · 界面时间为北京时间</span>
									</footer>
								</>
							)}
						</ConfigProvider>
					</main>
				)}
			</ProLayout>
			{data && importOpen && (
				<AlibabaImport
					key={`${storeId}:${data.integrations.version}`}
					open
					dirty={dirty}
					onClose={() => setImportOpen(false)}
					onLoaded={loadImported}
				/>
			)}
		</div>
	);
}
