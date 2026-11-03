# Price Pilot

面向1688采购、俄罗斯Ozon销售的自动定价与竞争力分析Demo。支持多店铺、日常与促销独立定价，以及商家手填的销量情景。系统生成测算结果，不向Ozon执行改价，不采集真实订单或训练销量预测模型。

## 启动

需要Docker Engine和Docker Compose。默认使用SQLite，并从相邻的`../mock-servers/seerfar-api`构建现有Seerfar Mock。

```sh
docker compose up -d --build
```

访问[本地工作台](http://localhost:8090)。`PILOT_BIND`和`PILOT_PORT`可调整监听地址和端口。初始提供两家费率不同的演示店铺；店铺档案不需要平台账号密钥。

PostgreSQL 17使用同一应用与API：

```sh
docker compose -f compose.yaml -f compose.postgres.yaml up -d --build
```

两种数据库使用独立持久卷，切换数据库不会自动搬迁记录。已有单店数据库启动时以事务迁移到默认店铺；历史计算保留原口径，只读查看。归档店铺保留资料与记录，并阻止写入及重新测算。

## 后台录入与数据范围

- 店铺管理：名称、自有Ozon卖家ID及归档状态。各店铺商品、费率、汇率、评分、协议报价、活动、竞品快照、销量情景和计算记录独立保存；已配置的全部自有店铺商品均从评分样本中排除。
- 商品档案：供货方式、采购阶梯和MOQ、包装重量尺寸、带电带液属性、完整链路时效、供应商评分及来源、成本与售后假设。
- 费率与评分：人民币结算、CNY→RUB汇率及压力区间、费用基数、货值阶梯、最低收费、封顶、适用渠道及供货方式。来源确认、版本及适用日期随记录保存。
- 物流与报价：国内仓配与跨境渠道独立组合。运费和另行收取的平台／渠道费用分别保存。公共报价作为共享资料库；复制到店铺后仍待确认。历史、过期、待确认及排除Ozon的方案不自动推荐。公开来源及限制说明保留在资料中。
- 促销方案：活动时间、来源、商家总优惠、平台承担优惠及买家优惠条件。活动总优惠替代日常优惠，不再次叠加；未开始和已结束的活动可以作为假设试算并显示时间状态。
- 销量情景：指定商品、活动、标价、渠道、预计日销量和假设依据。空数量不生成日利润，明确0表示预期零销量，小数表示平均预期。

新店铺关键费率为待补充，不继承已确认演示成本。未知金额不按0处理。演示商品、费率、汇率、物流与竞品均有标识，真实商品须先确认成本假设。

## 计算口径

四种模式互斥，参数分别保存：利润优先守住目标净利率并择竞争力最高方案；保本寻找完整成本下的最低不亏价；时效优先先选妥投上限最短的方案；竞争力目标优先在达标组合中选择单件预计净利润最高方案。后两种模式分别具有亏损额度，默认0元。

设人民币标价为`P`，商家优惠比例为`d`，商家固定优惠为`F`。百分比和固定优惠二选一：

```text
商家折后成交价 = P × (1 − d) − F
```

平台优惠单独从买家商品参考价扣减，仅在确认有等额补偿且不减少商家净回款后使用，不重复增加收入。补偿可能通过折扣积分抵扣服务费及期末余额奖金完成；本模型以优惠前收入减完整费用作等价经济测算，留存销售收入不代表现金流水。固定卢布优惠按情景汇率换算。类目佣金不能按平台优惠后的买家价格降低基数，佣金分档只能使用下单日卢布货值；其他费用及税费仍使用各自明确配置的基数。

设发货前取消率为`c`、拒收／未签收率为`u`、签收后退货率为`r`。三者以原始订单为分母，互斥且合计小于100%：

- 商家预计留存销售收入：`商家折后成交价 × (1 − c − u − r)`。
- 采购与发货成本按`1 − c`计；仅签收发生的费用按`1 − c − u`计；最终留存费用按`1 − c − u − r`计。
- 国际运费按已发货订单预留，不自动假定拒收时免除正向费用；未送达货件的承运商另行结算及退款，须按实际协议反映在售后预算中。
- 按全部原始订单分摊的广告或经营预算使用`ordered`，不因取消或退货自动退还。费用的单笔最低收费及封顶先计算，再按发生概率折算；`retained_revenue`也按此顺序处理。
- 拒收与签收后退货分别计算退运、处置及回收价值；取消损失单独计算。
- 单件预计净利润为留存收入减完整预期成本；净利率以留存销售收入为分母，区别于成本加成率。
- 提现净结算基数扣除国际运费及指定结算扣费，最低为0。

建议标价按人民币分求解并重新验证。支持标价0.01–1,000,000元。汇率压力检查包括区间端点及内部货值费率临界点。每个日常／活动情景独立选择价格和物流；同次计算使用相同竞品快照。竞争力目标默认以有效竞品价格P90反推该情景的标价上限，人工上限始终按人民币标价。

竞争力为内部决策分：价格、完整妥投保守上限及1688供应商评分，默认权重50/30/20。价格和时效按同组竞品排名，相同值按半个样本计数。默认比较商品展示价，未含买家运费。已知优惠条件不一致的样本不参与；时效缺失且无可靠参考时显示评分区间。评分不转换为成交概率。

销量情景复用同一成本与评分引擎：`预计日利润 = 单件预计净利润 × 商家填写的预计日销量`。压力日利润保持销量假设不变，只重新计算汇率影响；不重复扣除已计入单件成本的广告和经营费用。违反模式约束的情景仍可查看，但不作为推荐。销量情景不改变四种模式的定价结果。

计算记录冻结店铺、商品、设置、报价、活动、竞品及销量假设。CSV可导出单情景或全部情景，保留来源版本及演示标识。

## Ozon规则与适用边界

适用范围为中国跨境卖家、人民币合同、rFBS／OGL、单件独立货件的预算测算。规则核对日期为2026年11月3日。当前生效的跨境合同为2026年10月1日版本；官网提前展示的2026年11月9日合同尚未生效。[生效合同PDF](https://cdn.ozone.ru/s3/ozon-disk-api/techdoc/global/attachments/contracts-for-sellers/spisok-izmenenii-v-dogovore/contract-global-en-01-10-2026_1790160865.pdf)、[合同版本与生效日期](https://global-help.ozon.com/zh/contracts-for-sellers/spisok-izmenenii-v-dogovore/)。

| 事项 | 官方规则及本模型口径 |
| --- | --- |
| 人民币与汇率 | 按合同货币定价结算；销售换算采用下单日汇率，部分物流、推广等服务采用提供日汇率。当前±5%为同一参考汇率统一变化的预算，未模拟不同日期汇率的全部组合；跨日期服务费用可先折成人民币预算录入。[结算货币](https://global-help.ozon.com/zh/accounting/receiving-payments/payment-currency/)、[兑换率](https://global-help.ozon.com/zh/accounting/receiving-payments/conversion-rate/) |
| 价格与活动 | 本工具的建议标价是优惠前试算基价，不是Ozon划线价；商家总优惠按后台最终叠加结果录入。不同活动可能取最低价或叠加促销码、满额折扣，不由本工具自动推断。活动入选、历史价门槛、价格锁定及促销限制有效期由平台后台决定。[价格管理](https://global-help.ozon.com/zh/prices/price-control/)、[活动重叠规则](https://global-help.ozon.com/zh/promotion/promotions/promo-faq/) |
| 折扣积分 | 积分补偿不等于额外现金收入；普通类目佣金仍按扣除商家优惠、未扣积分折扣的价格计算。退货对应积分需冲回；本模型通过留存概率同时冲回相应收入及补偿，不重复计入。[折扣积分](https://global-help.ozon.com/zh/accounting/discounts-for-customers/discount-points/) |
| 类目佣金 | 费率依销售模式、类目及价格档确定；公开类目表的价格档为≤1500、≤5000、>5000 RUB，按下单日汇率判断。佣金与物流的人民币货值档分别配置，不能混用。演示百分比不代表官方类目费率。[销售佣金](https://global-help.ozon.com/zh/commissions/ozon-fees/commissions/) |
| OGL | 国际运输报价含向买家配送；电子服务另收货件价值2%，每票15–200 RUB，与银行收单费独立。公开表的13–17天仅截止分拣中心，需另加尾程时间。Big／Premium Big采用实际重与长×宽×高÷12（cm³/g）的较大值。[OGL费用](https://global-help.ozon.com/zh/commissions/ozon-fees/ogl-rates/)、[官方运费表](https://cdn.ozone.ru/s3/ozon-disk-api/Partner_Delivery/OGL_ENG_CN_24_08_26_added_Pick-up_1786697358.xlsx) |
| 其他服务 | 银行收单费由合作银行设置；国际运输组织合同签订服务公布费率为0.33%，适用于2026年9月1日起创建的订单，在已签收／已通过仓库退回时收费。部分推广及订阅服务退货不退费。OGL资料携带单独待确认费用项，店铺须核实适用性和扣费事件；不能把演示收单率或广告预算当作统一官方费率。[其他服务开支](https://global-help.ozon.com/zh/commissions/ozon-fees/commissions-for-other-services/)、[点击广告](https://global-help.ozon.com/zh/promotion/product-promotions/pay-per-click/what-is-cost-per-click/) |
| 售后 | 俄罗斯订单可默认进入WHD退货转售，取决于价值、质量及后台设置。真实商品有拒收／退货概率时必须确认去向与费用依据；WHD二次销售涉及仓储、再次履约及税费，当前不生成完整利润结论。销毁方案的回收价值须为0。普通合作伙伴退回费用由商家据合同录入，不能统一套用WHD费率。[退货去向](https://global-help.ozon.com/zh/fulfillment/fbp-rfbs-returns-cancellations/where-send-returns/)、[退货费用](https://global-help.ozon.com/zh/commissions/ozon-fees/returns-expenses/) |
| 买家价与评分 | Ozon区域及推广加价可能改变买家看到的价格，而不改变商家结算金额。当前买家参考价未包含这些加价，也未包含买家另付运费。100分竞争力为本工具的经营指标，不是Ozon官方价格指数、流量排名或成交概率。[区域价格系数](https://global-help.ozon.com/zh/prices/regional-price/) |

物流方案是否可用以店铺后台为准。旧合作渠道保留为历史参考；“2026年10月底全部停用”的第三方转述不作为公共官方规则写入推荐逻辑。多店铺仅为本地资料隔离，不创建或连接Ozon平台账号。

## Seerfar

保留原始`/open-api/…`路径及Bearer鉴权。复制`.env.example`为`.env`可配置`SEERFAR_BASE_URL`、`SEERFAR_API_KEY`和`SEERFAR_MODE`；凭证仅保存在服务端环境，不在页面返回。默认接入Mock，快照标记演示来源。

“商品操作”可载入测试SKU `149222760`，再获取竞品。查询使用关键词、SKU详情、相似商品及变体特征；不会把HTTP 200中的业务错误当成成功。积分、鉴权、限流和网络异常保留旧快照；不从缺失字段伪造配送时效或买家到手价。

## 验证与接口

```sh
# 容器内Go测试及vet；前端类型、单元测试与构建
sh tools/check.sh
# 独立内部网络：SQLite、Postgres、Seerfar Mock、API与Chromium
sh tools/test.sh
# 所有镜像与当前源码一致时
sh tools/test.sh --no-build
```

集成测试不映射宿主机端口，结束时移除测试容器、网络及卷。截图和CSV位于被忽略的`tests/artifacts/`。技术栈为Go、Gin 1.12.0、SQLite／PostgreSQL 17，以及Ant Design Pro 6.0.3基线的Umi Max、Ant Design和ProComponents，支持明暗主题与窄屏。

全局接口：`GET/POST /api/stores`、`PUT /api/stores/:store_id`、`GET /api/public-quotes`及`GET /api/health`。

业务接口均以`/api/stores/:store_id`为前缀：`GET /bootstrap`、`PUT /settings`、商品／报价／促销／销量情景的保存接口、`POST /quotes/copy-public`、`GET /quotes/:id/versions`、`POST /competitors/discover`、快照的`POST/PUT`、`POST /calculate`、`POST /sales-scenarios/evaluate`及`GET /runs/:id`。

定价请求为`{product, snapshot_id, promotion_ids}`，返回记录ID、日常`result`、完整`contexts`和`sales_results`。销量评估使用同样的商品与快照，可选传入`scenarios`；未传时评估已保存的该商品情景。金融资料缺失以情景状态及原因呈现；跨店引用被拒绝，数据源异常返回502并保留错误类型。
