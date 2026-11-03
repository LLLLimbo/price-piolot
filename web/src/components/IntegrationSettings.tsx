import {
	Alert,
	Button,
	Checkbox,
	Form,
	Input,
	Segmented,
	Space,
	Tabs,
	Tag,
} from "antd";
import { CheckCircleOutlined, SaveOutlined } from "@ant-design/icons";
import { useEffect, useRef, useState } from "react";
import type { IntegrationConfig } from "../types";
import { useStoreApi } from "../lib";

const providers = [
	{
		key: "1688",
		name: "1688 商品",
		description: "读取跨境商品详情。建立铺货关系前单独确认。",
		fields: [
			{ key: "app_key", name: "AppKey" },
			{ key: "app_secret", name: "AppSecret" },
			{ key: "access_token", name: "AccessToken" },
		],
	},
	{
		key: "seerfar",
		name: "Seerfar 竞品",
		description: "获取竞品和积分信息。切换连接后需重新获取竞品。",
		fields: [{ key: "api_key", name: "API Key" }],
	},
	{
		key: "frankfurter",
		name: "Frankfurter 汇率",
		description:
			"获取每日 CNY → RUB 参考汇率，确认应用后才更新店铺。公开服务无需凭据。",
		fields: [{ key: "bearer_token", name: "Bearer Token（私有网关可选）" }],
	},
];
export default function IntegrationSettings({
	config,
	onSaved,
	onError,
}: {
	config: IntegrationConfig;
	onSaved: () => Promise<void>;
	onError: (message: string) => void;
}) {
	const [current, setCurrent] = useState(config);
	useEffect(() => setCurrent(config), [config]);
	return (
		<div className="integration-settings">
			<Alert
				type="info"
				showIcon
				title="配置仅属于当前店铺"
				description="Mock 与真实环境分别保存。凭据留空时保留，勾选清除才删除；访问令牌到期后可在这里更换。"
			/>
			<Tabs
				items={providers.map((provider) => ({
					key: provider.key,
					label: provider.name,
					children: (
						<ServiceEditor
							key={`${provider.key}:${current.version}`}
							provider={provider}
							config={current}
							onChanged={async (value) => {
								setCurrent(value);
								await onSaved();
							}}
							onError={onError}
						/>
					),
				}))}
			/>
		</div>
	);
}
function ServiceEditor({
	provider,
	config,
	onChanged,
	onError,
}: {
	provider: (typeof providers)[number];
	config: IntegrationConfig;
	onChanged: (v: IntegrationConfig) => Promise<void>;
	onError: (message: string) => void;
}) {
	const [environment, setEnvironment] = useState(
		config.services[provider.key].active,
	);
	const service = config.services[provider.key];
	return (
		<section className="integration-editor">
			<div className="integration-heading">
				<div>
					<h2>{provider.name}</h2>
					<p className="muted-text">{provider.description}</p>
				</div>
				<Tag color={service.active === "mock" ? "gold" : "blue"}>
					当前启用：{service.active === "mock" ? "Mock" : "真实"}
				</Tag>
			</div>
			<Segmented
				aria-label={`${provider.name}配置环境`}
				value={environment}
				onChange={(value) => setEnvironment(String(value))}
				options={[
					{ label: "Mock 测试", value: "mock" },
					{ label: "真实服务", value: "real" },
				]}
			/>
			<ProfileEditor
				key={`${environment}:${config.version}`}
				provider={provider}
				config={config}
				environment={environment}
				onChanged={onChanged}
				onError={onError}
			/>
		</section>
	);
}
function ProfileEditor({
	provider,
	config,
	environment,
	onChanged,
	onError,
}: {
	provider: (typeof providers)[number];
	config: IntegrationConfig;
	environment: string;
	onChanged: (v: IntegrationConfig) => Promise<void>;
	onError: (s: string) => void;
}) {
	const api = useStoreApi();
	const [form] = Form.useForm();
	const [busy, setBusy] = useState(false),
		[changed, setChanged] = useState(false),
		[test, setTest] = useState("");
	const request = useRef(0);
	useEffect(
		() => () => {
			request.current++;
		},
		[],
	);
	const profile = config.services[provider.key].profiles[environment];
	const active = config.services[provider.key].active === environment;
	const save = async (activate: boolean) => {
		const seq = ++request.current;
		try {
			const value = await form.validateFields();
			setBusy(true);
			setTest("");
			const out = await api<{ integrations: IntegrationConfig }>(
				"/integrations",
				{
					...value,
					provider: provider.key,
					environment,
					version: config.version,
					activate,
				},
				"PUT",
			);
			if (seq === request.current) await onChanged(out.integrations);
		} catch (e) {
			if (seq === request.current && e instanceof Error) onError(e.message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	const check = async () => {
		const seq = ++request.current;
		setBusy(true);
		setTest("");
		try {
			const out = await api<{ message: string }>(
				`/integrations/${provider.key}/${environment}/test`,
				{
					version: config.version,
					product_id: form.getFieldValue("test_product_id"),
				},
			);
			if (seq === request.current) setTest(out.message);
		} catch (e) {
			if (seq === request.current) onError((e as Error).message);
		} finally {
			if (seq === request.current) setBusy(false);
		}
	};
	return (
		<Form
			component={false}
			name={`integration-${provider.key}-${environment}`}
			form={form}
			layout="vertical"
			initialValues={{
				base_url: profile.base_url,
				test_product_id: profile.test_product_id,
				credentials: {},
				clear: [],
			}}
			onValuesChange={() => {
				setChanged(true);
				setTest("");
			}}
		>
			<div className="integration-profile-status">
				<Tag color={profile.configured ? "green" : "default"}>
					{profile.configured ? "凭据已就绪" : "待配置"}
				</Tag>
				<span className="muted-text">
					版本 {profile.version} · {active ? "正在使用" : "保存后可启用"}
				</span>
			</div>
			<Form.Item
				name="base_url"
				label="服务根地址"
				rules={[
					{ required: true, message: "请输入服务根地址" },
					{
						validator: (_, value) => {
							try {
								const parsed = new URL(value);
								if (
									!["http:", "https:"].includes(parsed.protocol) ||
									!parsed.hostname ||
									parsed.username ||
									parsed.password ||
									parsed.search ||
									parsed.hash
								)
									throw new Error();
								return Promise.resolve();
							} catch {
								return Promise.reject(
									new Error("请输入不含凭据或查询参数的HTTP(S)服务地址"),
								);
							}
						},
					},
				]}
			>
				<Input placeholder="https://…" autoComplete="off" />
			</Form.Item>
			<div className="integration-credentials">
				{provider.fields.map((field) => (
					<Form.Item
						key={field.key}
						name={["credentials", field.key]}
						label={field.name}
						extra={
							profile.credentials_set?.[field.key]
								? "已配置，留空保留原值"
								: "尚未配置"
						}
					>
						<Input.Password
							placeholder={
								profile.credentials_set?.[field.key]
									? "已配置 · 输入新值以替换"
									: "请输入"
							}
							autoComplete="new-password"
						/>
					</Form.Item>
				))}
			</div>
			<Form.Item name="clear" label="清除已保存的凭据">
				<Checkbox.Group
					options={provider.fields
						.filter((field) => profile.credentials_set?.[field.key])
						.map((field) => ({
							label: `清除 ${field.name}`,
							value: field.key,
						}))}
				/>
			</Form.Item>
			{provider.key === "1688" && (
				<Form.Item
					name="test_product_id"
					label="只读测试商品ID"
					extra="测试不会建立铺货关系；Mock 可使用1001。"
				>
					<Input inputMode="numeric" placeholder="输入已知商品ID" />
				</Form.Item>
			)}
			<Space wrap>
				{!active && (
					<Button
						aria-label="保存此配置"
						icon={<SaveOutlined />}
						loading={busy}
						onClick={() => save(false)}
					>
						保存此配置
					</Button>
				)}
				<Button
					aria-label={active ? "保存并应用" : "保存并启用"}
					type="primary"
					icon={<CheckCircleOutlined />}
					loading={busy}
					onClick={() => save(true)}
				>
					{active ? "保存并应用" : "保存并启用"}
				</Button>
				<Button
					loading={busy}
					disabled={changed || !profile.configured}
					onClick={check}
				>
					测试已保存连接
				</Button>
			</Space>
			{changed && (
				<p className="muted-text">
					保存后可测试连接；切换配置环境不会自动启用。
				</p>
			)}
			{test && (
				<Alert className="spaced-alert" type="success" showIcon title={test} />
			)}
		</Form>
	);
}
