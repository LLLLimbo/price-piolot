# 1688 Mock Server

独立 Rust / Axum 服务，使用进程内存保存账号、商品、库存、订单、支付、退款、合同、任务及消息。协议清单来自 [1688 跨境 ERP 方案 8.8](https://open.1688.com/solution/detail?key=1697015308755#apiAndMessageList)：194 个 API、40 类消息。支持有状态业务调用及独立管理端口。

**API 版本：2026-10-27**。

当前提供全清单请求与响应结构测试，以及交易、授权、消息等业务验收（[已执行验证结果](docs/VERIFICATION.md)）。**不能据此认定所有接口的全部生产语义已精确还原**；两个 API 详情和一个嵌套模型缺失。详见 [兼容性说明](docs/COMPATIBILITY.md)、[逐接口覆盖清单](docs/COVERAGE.md) 和 [独立验收结论](tests/acceptance/REPORT.md)。

## 启动

在本目录执行：

```sh
docker build -t mock1688:local .
docker run --rm --name mock1688 \
  -p 127.0.0.1:8080:8080 -p 127.0.0.1:8081:8081 \
  -e MOCK_BASE_URL=http://localhost:8080 mock1688:local
```

业务端口 `8080`，管理端口 `8081`。管理端口可读取令牌、密钥和全部样例状态，不设鉴权，只应对本地测试环境开放。容器以非 root 用户运行，停止即丢失运行数据。以上为开发启动；集成测试使用下述无端口映射的 Compose 环境。

| 环境变量 | 默认值 | 含义 |
|---|---|---|
| `MOCK_LISTEN` | `0.0.0.0:8080` | 业务监听地址 |
| `MOCK_ADMIN_LISTEN` | `0.0.0.0:8081` | 管理监听地址 |
| `MOCK_BASE_URL` | `http://localhost:8080` | 返回的支付、下载、商品图片等本地链接前缀 |
| `MOCK_FIXED_TIME` | 不设置 | 测试固定时间，Unix 毫秒；不设置时使用系统时间 |

默认应用为 `test-app` / `test-secret`。默认网盟 Key 为 `test-web-union`；平台注册、已同步账号和应用产品状态保存在 `records.app_configs`。账号 `buyer-1`、`buyer-2`、`seller-1` 的令牌分别为 `test-token`、`other-token`、`seller-token`。两个买家用于测试数据隔离。默认商品 `1001` 单价 19.90 元、库存 100；`1002` 最小采购数量 2；`1003` 缺货；`1004` 下架。默认还包含关联地址、发票抬头、待支付/已完成订单、退款、采购金、合同、素材及已完成任务。完整种子见 `src/state.rs::Store::seed`；可导入样例为 `fixtures/reset.json`，各 API 请求样例见 `fixtures/requests.json`。

## API 调用与授权

入口：`POST /openapi/param2/{version}/{namespace}/{name}/{appKey}`。使用 UTF-8 `application/x-www-form-urlencoded`，复杂参数以 JSON 字符串作为表单值。签名使用 HMAC-SHA1，输入为 `param2/...` 路径与将每个 `key + value` 拼接串排序后再拼接，排除 `_aop_signature`；输出大写十六进制。

```python
import hashlib, hmac, json, urllib.parse, urllib.request

base = 'http://localhost:8080'
path = 'param2/1/com.alibaba.trade/alibaba.trade.createCrossOrder/test-app'
params = {
    'access_token': 'test-token', 'flow': 'general',
    'addressParam': json.dumps({'addressId': 1}, separators=(',', ':')),
    'cargoParamList': json.dumps([{'offerId': 1001, 'specId': 'sku-1001', 'quantity': 1}], separators=(',', ':')),
}
raw = path + ''.join(sorted(k + v for k, v in params.items()))
params['_aop_signature'] = hmac.new(b'test-secret', raw.encode(), hashlib.sha1).hexdigest().upper()
request = urllib.request.Request(base + '/openapi/' + path,
    data=urllib.parse.urlencode(params).encode(),
    headers={'Content-Type': 'application/x-www-form-urlencoded'})
print(json.load(urllib.request.urlopen(request)))
```

签名与令牌要求来自每个接口详情。文件接口支持文档所述 Base64 / 字节数组；退款凭证还支持 multipart，二进制文件部分不参与签名。签名必须基于实际传输的 JSON 字符串，空格和字符转义也参与计算。

授权入口为 `GET /oauth/authorize?client_id=test-app&site=1688&redirect_uri=...&state=...`，默认授权 `buyer-1`。`mock_account` 可选择其他内置账号；这是模拟登录扩展。旧入口 `/auth/authorize.htm` 的 `site=china` 请求需要签名。跳转返回一次性 `code`；使用 `redirect_uri=urn:ietf:wg:oauth:2.0:oob` 时直接返回 JSON。

授权码兑换/刷新：`POST /openapi/http/1/system.oauth2/getToken/test-app`，表单包含 `client_id`、`client_secret`，以及 `grant_type=authorization_code`、`code`、`redirect_uri`，或 `grant_type=refresh_token`、`refresh_token`。延期使用同一路径下 `postponeToken`。授权码有效期 120 秒，新访问令牌 36000 秒，刷新令牌 90 天；默认内置令牌有效至 2100 年。子账号授权使用清单中的 `subaccount.auth.*` 接口，撤销父授权会撤销关联子授权。

## 运费模板

`com.alibaba.logistics:alibaba.logistics.myFreightTemplate.list.get-1` 不传 `templateId` 时返回当前授权账号的全部模板，按 ID 升序排列，不分页。指定普通模板 ID 时允许买家读取供应商模板；未知 ID 返回空数组，负数 ID 返回 `PARAM_ILLEGAL`。保留 ID `0`（运费说明）、`1`（卖家承担运费）按当前授权账号查询。

`querySubTemplate` 和 `queryRate` 默认均为 `false`。前者为 `true` 时展开实际配置的快递、货运和货到付款子模板；两者均为 `true` 时才展开费率。未请求或未配置的字段省略；显式配置的空费率列表在请求费率时返回 `[]`。普通、系统费率分别返回 `rateDTO`、`sysRateDTO`，数值保持官方模型声明的整数类型。

每个内置账号有 `0/1` 模板；`seller-1` 还有普通模板 `10001` 和官方模板 `10002`。使用买家令牌查询供应商模板的表单参数示例（签名和授权同上）：

```text
templateId=10001&querySubTemplate=true&queryRate=true
```

完整配置位于 `records.freight_templates["会员ID:模板ID"]`，可通过管理快照修改名称、发货地、状态、子模板及费率。普通模板 ID 全局唯一，`0/1` 可按账号重复。缺少模板表的旧快照按空表处理，不自动补充模板。账号访问规则和开关组合是固定 Mock 规则；商品可引用供应商模板进行运费预估。专项断言见 [运费模板测试](tests/acceptance/test_freight_templates.py)，相同场景也通过官方 SDK 执行。

## 商品运费预估

`com.alibaba.fenxiao.crossborder:product.freight.estimate-1` 按商品所属供应商的模板、收货省市区、多 SKU 包装参数及数量返回国内运费。以下对象作为表单参数 `productFreightQueryParamsNew` 的 JSON 字符串发送，签名方式同上：

```json
{
  "offerId": 1002,
  "toProvinceCode": "330000",
  "toCityCode": "330100",
  "toCountryCode": "330108",
  "totalNum": 3,
  "logisticsSkuNumModels": [
    {"skuId": "1002", "number": 2},
    {"skuId": "2002", "number": 1}
  ]
}
```

默认商品 `1001` 包邮；`1002` 关联供应商 `seller-1` 的模板 `10001`，物流 SKU `1002/2002` 分别重 0.2/0.8 kg。上述请求合计 1.2 kg，按首重 1 kg 8 元、每续 1 kg 2 元预估，返回 `result.result.freight="10.00"`、`freePostage=false`。主 SKU 也接受原 `specId` 别名 `sku-1002`。

商品的可选 `freight` 配置包含 `template_id`、`service_type`（0 快递、1 货运、2 货到付款）、`package` 和 `skus`。包装字段为 `weight`（千克）、`width/height/length`（厘米）；`skus` 以数字 SKU ID 字符串为键配置独立包装，未单独配置的主 SKU 使用商品级包装。商品详情和预估响应复用这些包装数据。额外物流 SKU 用于包装和运费预估，交易 SKU、订单金额和库存规则沿用现有逻辑。

模板费率中的内部 `mockPricing` 例如 `{"toAreaCodes":["330000"],"nextUnit":1000,"nextUnitFee":200}`：地区按区县、城市、省份、显式全国默认匹配；空编码列表表示全国默认。重量计费单位为克、件数为件、体积为立方米。首费、最低费用和续费金额按分解释，预估响应费用统一为元、两位小数。官方费率复用 `sysRateDTO.nextUnit`，其 `mockPricing` 不再填写 `nextUnit`。这些内部字段仅在管理快照中出现，模板查询响应中省略。

各 SKU 计费量汇总后统一计算首续费和最低费用；不自动计算满额、满件或地区条件包邮。无覆盖地区、缺失计费规则及运费说明模板 `0` 返回 `FREIGHT_NOT_AVAILABLE`，不会回退为零运费。缺少商品 `freight` 字段的旧快照保持原有单 SKU、默认包装和包邮行为；显式配置要求模板归属商品供应商且所选子模板存在。算法、单位及匹配规则均为固定 Mock 约定，详见 [兼容性说明](docs/COMPATIBILITY.md#商品运费预估)及 [专项测试](tests/acceptance/test_freight_estimate.py)。

## 管理接口

管理请求使用 JSON；失败返回 HTTP 400，不提交业务状态。普通业务请求无需携带模拟控制字段。

| 方法与路径 | 请求/行为 |
|---|---|
| `POST /__mock/reset` | `{}` 恢复默认样例；`{"time":1792972800000}` 固定时间；`{"state":完整状态}` 校验后原子替换 |
| `GET /__mock/state` | 全部状态、消息、故障规则和投递记录 |
| `POST /__mock/clock/advance` | `{"milliseconds":1000}`，只向前推进时间，执行到期消息重试及 AUTO_CONFIRM 合同确认；令牌到期在访问时判定 |
| `POST /__mock/actions` | 预定义业务动作，见下表；`account` 默认 `buyer-1`，`appKey` 默认 `test-app` |
| `PUT /__mock/faults` | 规则数组；`[]` 清除 |
| `PUT /__mock/subscriptions` | 订阅数组；`[]` 清除 |

可通过 `curl -s http://localhost:8081/__mock/reset -H 'Content-Type: application/json' --data-binary @fixtures/reset.json` 导入随附样例。容器内客户端应相应修改样例中的 `state.base_url` 和本地资源链接。

商品搜索指标、混批阈值、开票能力等在 `products[id].merchandise` 中配置，字段规则见 [兼容性说明](docs/COMPATIBILITY.md)。订单保存支付时开票能力、明细收货时间；包裹和退款均关联具体子订单。`metadata.seller_order` 配置邀约订单，`stock_at_payment` 配置付款扣库存，`pay_channels` 限制支付渠道；账期额度由 `records.account_periods` 配置，查询与订单付款、退款关联。代扣协议保存在 `records.payment_agreements`，申请后使用管理付款动作完成支付；分期付款明细保存在 `records.staged_payments`。

重置清空故障、授权码和消息队列。自定义状态来自 `GET /__mock/state`，金额和库存用十进制字符串；核心实体引用、金额、状态和时间范围校验失败时原状态保留。默认重置会移除前一次运行创建的任务；自定义快照中的业务任务是导入数据的一部分。

```sh
curl -s http://localhost:8081/__mock/actions -H 'Content-Type: application/json' \
  -d '{"action":"payment","orderId":9001}'
curl -s http://localhost:8081/__mock/actions -H 'Content-Type: application/json' \
  -d '{"action":"shipment","orderId":9001,"tracking":"MOCK-TRACK-1"}'
```

| 动作 | 主要参数 |
|---|---|
| `payment` | `orderId`、可选 `success:false`；成功才扣款 |
| `shipment` / `tracking_update` | `orderId`、`tracking`；单号修改可传 `logisticsId` 选择包裹 |
| `partial_shipment` / `staged_payment` | `orderId`、`quantity` / `amount` |
| `seller_cancel` / `system_cancel` | `orderId` |
| `order_price` / `seller_address` | `orderId`、`amount` / `address` |
| `refund_approve` / `refund_reject` / `refund_complete` | `refundId`；退货需要先调用官方退货物流接口 |
| `fund_settle` / `fund_refund_settle` | `applyNo` / `refundNo`，可选 `success:false` |
| `invoice_issue` | `applyId`；生成可下载的本地模拟发票 XML |
| `contract_pay` / `contract_refund` | `contractId` |
| `contract_content` / `contract_reject` | `draftNo`、`content` / `reason` |
| `task_transition` | `taskId`、`status`，可选 `content`、`rows` |
| `product_update` / `product_audit` / `product_delete` | `offerId`；更新可传 `price`、`stock`、`active` |
| `lp_distribute` | `offerId` |
| `token_revoke` | `accessToken` |
| `message_emit` | `topic`、`data`；`validate:false` 显式允许异常消息测试 |
| `message_replay` | `messageId`；重复投递和乱序场景 |

支付、发货、退款审批和任务执行不会自动成功。任务合法转换为 `INIT → RUNNING → END`；运行中可进入 `WAIT_USER` / `WAIT_SKILL`，恢复、终止按状态校验。消息重试会自动运行；固定时钟下需要推进模拟时间。

故障例子：下一次 `buyer-1` 下单返回指定错误，后续恢复正常。

```json
[{"api":"alibaba.trade.createCrossOrder","account":"buyer-1","remaining":1,"code":"SYSTEM_ERROR","message":"模拟系统错误","delay_ms":0}]
```

省略 `account` 匹配所有账号；`api` 支持完整契约键或接口名。`delay_ms` 最大 60000；将其设为大于客户端超时即可测试超时。省略 `code` 时仅延迟。错误规则优先于参数和业务校验，且只在协议鉴权通过后消耗次数。

订阅例子：

```json
[{"app_key":"test-app","topic":"ORDER_BUYER_VIEW_ORDER_PAY","account":"buyer-1","url":"http://receiver:8082/callback","secret":"message-secret"}]
```

消息以表单 `message` 与 `_aop_signature` 推送。签名输入为 `message` 加原始 JSON 字符串；仅 HTTP 200 成功。失败后重试 3 次，间隔为 1、2、4 秒；失败消息保留 7 天。官方查询、游标和确认接口均可使用。确认与在途回调并发时，确认结果优先。模拟接收器见 `tests/acceptance/receiver.py`。

## 验证

```sh
# 完整主套件：构建服务 + 接收器 + 测试执行器
sh tools/test.sh
# 独立验收：独立 Compose 项目及独立请求构造
bash tests/acceptance/run.sh
# 官方 Python SDK：192 个官方请求类、公共授权/校时及业务闭环
sh tests/sdk/run.sh
# SDK 独立复验：独立用例、主 SDK 套件及逐接口证据审计
bash tests/sdk/independent/run.sh
# 冻结清单、样例及实现关联校验
python3 tools/coverage.py
# Rust 检查（只做构建和单元测试，不访问业务服务）
docker run --rm -v "$PWD:/app" -w /app rust:1.90-bookworm sh -c \
  'rustup component add rustfmt clippy && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked'
```

所有集成测试 Compose 环境均使用内部网络、无宿主机端口映射，成功、失败和中断时清理容器及网络。构建需要访问 Rust/Docker 仓库；可以通过 `HTTP_PROXY`、`HTTPS_PROXY` 传入构建代理。测试运行不调用真实 1688 服务。

官方 SDK 的原版结果、70 处声明适配及逐接口 HTTP 证据见 [SDK 测试说明](tests/sdk/README.md) 和 [SDK 验证结果](docs/SDK_VERIFICATION.md)。SDK 使用的 `param2` 授权入口与现有 `http` 授权入口共享逻辑；`/openapi/param2/1/system/currentTimeMillis/{appKey}` 返回模拟时钟的 JSON 毫秒整数。

冻结来源与 SHA-256 在 `contracts/manifest.json`。`tools/freeze_contracts.py` 会访问官方文档并重写契约快照，不属于普通测试步骤；`contracts/validation-rules.json` 是从公开说明人工抽取的规则，不是官方原始响应。
