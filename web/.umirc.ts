import { defineConfig } from "@umijs/max";
export default defineConfig({
	title: "Price Pilot · 商品定价",
	favicons: ["/favicon.svg"],
	antd: {},
	hash: true,
	routes: [{ path: "/*", component: "./Workbench" }],
	proxy: {
		"/api": {
			target: process.env.API_TARGET || "http://app:8080",
			changeOrigin: true,
		},
	},
	npmClient: "npm",
});
