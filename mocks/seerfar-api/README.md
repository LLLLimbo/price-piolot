# Seerfar Ozon Mock API

独立的 Rust / Axum 服务，覆盖 Seerfar 文档 v1.1 的 21 个 Ozon 接口和公共积分查询。业务数据、凭证、积分、采集箱、任务、结算和故障均保存在进程内存中，重启恢复种子数据。

**API 版本：2026-10-28**。版本日期取自[官方更新记录](http://doc.seerfar.cn/api-updates.json)的最新记录，对应官方文档 v1.1；本地契约快照日期为 2026-10-29。

## 启动与接入

在仓库根目录配置 `.env` 后运行 `docker compose up -d --build`。统一控制台默认端口为 8000，Seerfar 业务端口为 8084，可通过 `SEERFAR_PORT` 调整。ERP 只需将原来的 `http://api.seerfar.cn` 替换为 Mock 地址，保留原始路径。

```sh
curl http://localhost:8084/open-api/quota \
  -H 'Authorization: Bearer seerfar-test-key'

curl http://localhost:8084/open-api/product/detail/search/ozon \
  -H 'Authorization: Bearer seerfar-test-key' \
  -H 'Content-Type: application/json' \
  -d '{"sku":"149222760"}'
```

| 主账号 | API Key | 初始积分 |
| --- | --- | --- |
| `123` | `seerfar-test-key` | 10,000 |
| `123` | `seerfar-same-account-key` | 10,000 |
| `456` | `seerfar-other-key` | 10,000 |

同账号的 Key 共享采集箱和评论分析任务，积分分别核算。默认子账号手机号 `13800000000` 对应 `sub-1`。测试店铺为 `118364`，商品包括 `149222760`、`149222761`、`175924376` 和 `2100100166`；商品、店铺及类目之间的关系可在控制台查看。

## 业务接口

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET | `/open-api/quota` | 查询 API Key 积分 |
| POST | `/open-api/shop/detail/search/ozon` | Ozon 查店铺 |
| POST | `/open-api/shop/detail/brand/search/ozon` | Ozon 查店铺品牌 |
| POST | `/open-api/shop/detail/category/search/ozon` | Ozon 查店铺类目 |
| POST | `/open-api/product/detail/search/ozon` | Ozon 查竞品 |
| POST | `/open-api/product/detail/similar/ozon` | Ozon 查相似商品 |
| POST | `/open-api/product/detail/variations/characteristics/ozon` | Ozon 查变体特征 |
| POST | `/open-api/product/detail/variations/top/ozon` | Ozon 查 Top 变体 |
| POST | `/open-api/product/detail/variations/search/ozon` | Ozon 查商品变体 |
| POST | `/open-api/product/monthlySalesTrend/ozon` | Ozon 月度销量趋势 |
| POST | `/open-api/product/collect/ozon` | Ozon 采集商品 |
| POST | `/open-api/collect-box/page/ozon` | Ozon 查询采集商品 |
| POST | `/open-api/category/detail/search/ozon` | Ozon 查类目 |
| POST | `/open-api/productReport/search/ozon` | Ozon 查热销榜单 |
| POST | `/open-api/market/search/ozon` | Ozon 查市场热词 |
| POST | `/open-api/keyword/mining/ozon` | Ozon 关键词挖掘 |
| POST | `/open-api/keyword/backSearch/ozon` | Ozon 关键词反查 |
| POST | `/open-api/review/analyse/ozon/task/add` | Ozon 创建评论分析任务 |
| POST | `/open-api/review/analyse/ozon/report` | Ozon 查评论分析报表 |
| POST | `/open-api/review/analyse/ozon/wordFrequency` | Ozon 查评论词频 |
| POST | `/open-api/review/analyse/ozon/reviewStatistic` | Ozon 查评论统计 |
| POST | `/open-api/review/analyse/ozon/originalReview` | Ozon 查原始评论 |

响应保留 Seerfar 的 HTTP 状态和 `code/msg/data` 结构。请同时检查 HTTP 状态和业务 `code`；例如评论任务处理中会通过 HTTP 200 返回业务错误。

## 管理

控制台选择 **Seerfar**，可管理账号、Key、积分、公共商品、店铺、类目、销量、市场样例、采集记录、评论结果、任务状态、费用、限流和故障规则。公共数据由所有测试账号共享；采集箱、任务和结算按主账号隔离。完整状态页显示所有账号，管理界面遵循仓库现有的可信局域网访问约定。

业务监听地址默认为 `0.0.0.0:8080`，管理监听地址默认为 `0.0.0.0:8081`。根 Compose 仅发布业务端口，控制台通过 `/api/seerfar/admin/` 代理管理请求。

| 管理接口 | 请求 |
| --- | --- |
| `GET /__mock/state` | 完整状态和快照 |
| `POST /__mock/actions` | `{action, account, ...}`，动作参数见控制台结构化表单 |
| `POST /__mock/reset` | `{}` 恢复种子；`{time: Unix毫秒}` 指定种子时钟；`{state: 完整快照}` 导入 |
| `POST /__mock/clock/advance` | `{milliseconds: 非负整数}` |
| `PUT /__mock/faults` | `[{path, remaining, account?, api_key?, method?, delay_ms?, status?, body?}]` |

`MOCK_BASE_URL` 控制返回的图片地址。图片由服务本地提供。`MOCK_LISTEN`、`MOCK_ADMIN_LISTEN` 可覆盖监听地址；`MOCK_FIXED_TIME` 为空时使用真实时钟，根 Compose 对应 `SEERFAR_FIXED_TIME`。真实时钟下任务在后续 API 或管理请求到达时按截止时间推进；固定时钟下通过推进时间或任务管理操作控制。每个异步阶段默认 1 秒，可修改为 1–60000 毫秒。

快照导入、重置和管理修改先完整校验再提交。快照保留其 `base_url`；跨机器导入时需确认图片地址。请求大小上限为 16 MiB。快照是用户下载的文件，不是服务端持久化。限流窗口为运行时状态，导入和重启后清空。

## 验证

```sh
sh seerfar-api/tools/check.sh
sh seerfar-api/tools/test.sh
sh tools/check-console.sh
sh tools/test-console.sh
```

Rust 检查、构建和前端检查使用 Docker。API 集成测试与控制台浏览器测试分别运行在完整的 Compose 模拟环境中，网络为 internal，无宿主机端口映射，退出时自动销毁容器与网络。已有镜像可使用测试脚本的 `--no-build` 参数。

文档及响应样例保存在 `contracts/api-docs.json`，定价表保存在 `contracts/pricing.json`，种子数据位于 `fixtures/seed.json`。来源为 [Seerfar API 文档](http://doc.seerfar.cn/api-docs.html?lang=zh-CN#overview)。具体 Mock 约定见 [兼容性说明](COMPATIBILITY.md)。
