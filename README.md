# Price Pilot

面向1688采购、俄罗斯Ozon销售的自动定价与竞争力分析Demo。支持多店铺、日常与促销独立定价，以及商家手填的销量情景。系统生成测算结果，不向Ozon执行改价，不采集真实订单或训练销量预测模型。

## 启动

需要Docker Engine和Docker Compose。默认使用SQLite，从仓库内的`mocks/seerfar-api`和`mocks/1688-crossboarder-erp-api`构建现有Mock，并运行确定性Frankfurter协议桩；三个外部服务均无需映射宿主机端口，不依赖仓库外的本机目录。

```sh
docker compose up -d --build
```

访问[本地工作台](http://localhost:8090)。`PILOT_BIND`和`PILOT_PORT`可调整监听地址和端口。初始提供两家费率不同的演示店铺；店铺档案不需要平台账号密钥。

局域网访问时，在本地`.env`设置`PILOT_BIND=0.0.0.0`及`PILOT_PORT=8090`，执行`docker compose up -d --no-build --force-recreate --wait`，使用宿主机IP和8090端口访问。本地`.env`不纳入Git。

PostgreSQL 17使用同一应用与API：

```sh
docker compose -f compose.yaml -f compose.postgres.yaml up -d --build
```

两种数据库使用独立持久卷，切换数据库不会自动搬迁记录。已有单店数据库启动时以事务迁移到默认店铺；历史计算保留原口径，只读查看。归档店铺保留资料与记录，并阻止写入及重新测算。

## 初始化数据

默认`INITIAL_DATA_MODE=snapshot`。全新空数据库首次启动自动载入`internal/initialdata/seed.snapshot.json.gz`中的业务快照，SQLite和PostgreSQL均适用；已有任何记录的数据库不重复导入、不覆盖修改或恢复已删除内容。`INITIAL_DATA_MODE=demo`使用原始演示数据，主要用于隔离测试。

当前快照包含2家店铺、4个商品、28条店铺报价、31条公共资料、5份竞品快照、2个促销方案、3个销量情景及商品、报价、设置的历史版本，不包含历史测算记录。双肩包预算包含16%佣金，并保留20%目标净利率建议价47.53元的计算条件；商品内订单校验样本仍然保留。快照数量、导出时间和校验摘要见`internal/initialdata/manifest.json`。

原始JSON、记录时间、预算标记和报价有效期均保留。历史测算、集成凭据、加密密钥和临时预览不进入快照，新环境生成自己的密钥并建立Mock连接；真实服务凭据需在网页重新配置。快照文件是初始化资料，运行后的数据写入各自持久卷。

更新SQLite业务快照可运行：

```sh
docker run --rm -v price-pilot_app-data:/source:ro \
  -v "$PWD/tools/export-initial-data.py:/export.py:ro" \
  -v "$PWD/internal/initialdata:/output" python:3.12-slim \
  python /export.py /source/pricepilot.db /output
docker compose build app
```

## 后台录入与数据范围

- 店铺管理：名称、自有Ozon卖家ID及归档状态。各店铺商品、费率、汇率、评分、协议报价、活动、竞品快照、销量情景和计算记录独立保存；已配置的全部自有店铺商品均从评分样本中排除。
- 商品档案：供货方式、采购阶梯和MOQ、包装重量尺寸、带电带液属性、完整链路时效、供应商评分及来源、成本与售后假设。
- 商品备注保存来源链接、统计范围、实际SKU及待核实资料，并随计算记录冻结。历史“发货后售后异常率”尚未区分拒收和签收后退货时，按原始订单口径记录为待分类比例；分类及费用确认前阻断完整利润结论。
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

## 外部服务与1688商品导入

“费率与评分 → 外部服务”按店铺保存1688、Seerfar和Frankfurter连接。各服务分别保留Mock／真实配置；保存未启用环境不会切换连接，点击“保存并启用”才切换。1688使用AppKey、AppSecret和AccessToken；Seerfar使用API Key；Frankfurter公开服务无需凭据，私有网关可选Bearer Token。当前使用手动提供的已授权令牌，不包含OAuth授权及自动续期。

凭据使用AES-GCM加密，页面仅返回是否已配置。输入留空保留旧值，明确勾选清除才删除。加密密钥默认位于应用数据卷的`/app/data/integrations.key`，可由`INTEGRATIONS_KEY_FILE`指定。备份数据库时须同时备份该密钥；丢失密钥时已有凭据无法解密。密钥、令牌不进入商品来源、计算记录或CSV。

“商品操作 → 载入1688商品”接受单个标准详情链接或数字ID，仅从`/offer/{ID}.html`提取商品ID，忽略查询参数。预览后选择单个SKU和采购数量，生成未保存的新草稿。采购报价按SKU及适用阶梯取值；单位或报价类型不明确时必须人工确认。包装毛重由千克转换为克，接口未提供的费用、供应商评分和备货时间继续待补充。商品质量星级不作为供应商评分。

详情接口为`alibaba.cross.productInfo`。明确缺少商品铺货关系时，商家确认后才调用`alibaba.cross.syncProductListPushed`，并重新获取详情。连接测试仅执行只读接口，不建立关系。写请求结果不明确时，应先重新获取详情。商品及汇率预览15分钟内有效，切店或连接配置变化后须重新获取。

1688 Mock默认商品`1001`已有关系、单价19.90元；`1002`需建立关系、起订量2、单价12.50元；`1003`缺货，`1004`下架。默认Mock凭据为`test-app`／`test-secret`／`test-token`。用户提供的真实商品ID不是Mock样例，不能用其获得伪造详情。Mock载入的商品保留演示标识。

“汇率与权重 → 获取参考汇率”调用Frankfurter v2 `GET /v2/rate/CNY/RUB`；公开地址为`https://api.frankfurter.dev`。页面保留上游实际数据日期及获取时间，经“应用到本店”确认后仅更新汇率和来源，不修改费率、权重或历史计算。默认综合参考汇率不等于Ozon合同汇率；Mock协议桩固定返回2026-10-30的12.0844 RUB/CNY。异常响应不覆盖原值，仍支持手动填写。

### Seerfar

保留原始`/open-api/…`路径及Bearer鉴权。`SEERFAR_BASE_URL`、`SEERFAR_API_KEY`和`SEERFAR_MODE`仅用于首次迁移现有店铺，后续以各店网页配置为准。新店铺不继承其他店铺的真实凭据。默认接入Mock，快照保留环境及配置版本；连接变化后旧快照可查看，但须重新获取才能用于新测算。

“商品操作”可载入测试SKU `149222760`，再获取竞品。查询使用关键词、SKU详情、相似商品及变体特征；不会把HTTP 200中的业务错误当成成功。积分、鉴权、限流和网络异常保留旧快照；不从缺失字段伪造配送时效或买家到手价。

## 验证与接口

```sh
# 容器内Go测试及vet；前端类型、单元测试与构建
sh tools/check.sh
# 独立内部网络：SQLite、Postgres、1688／Seerfar／汇率Mock、API与Chromium
sh tools/test.sh
# 所有镜像与当前源码一致时
sh tools/test.sh --no-build
```

集成测试不映射宿主机端口，结束时移除测试容器、网络及卷。截图和CSV位于被忽略的`tests/artifacts/`。技术栈为Go、Gin 1.12.0、SQLite／PostgreSQL 17，以及Ant Design Pro 6.0.3基线的Umi Max、Ant Design和ProComponents，支持明暗主题与窄屏。

全局接口：`GET/POST /api/stores`、`PUT /api/stores/:store_id`、`GET /api/public-quotes`及`GET /api/health`。

业务接口均以`/api/stores/:store_id`为前缀：`GET /bootstrap`、`PUT /settings`、商品／报价／促销／销量情景的保存接口、`POST /quotes/copy-public`、`GET /quotes/:id/versions`、`POST /competitors/discover`、快照的`POST/PUT`、`POST /calculate`、`POST /sales-scenarios/evaluate`及`GET /runs/:id`。

商品档案可保存一笔手工订单校验样本。`POST /profit-checks/evaluate` 使用同一成本引擎核对订单金额、采购、运费、佣金和固定费用，返回样本利润及与来源的差额。未知金额不按零计算；样本的预计运费和原型费率保留标记，不变更商品售后概率或店铺费率，不生成完整净利润与推荐结论。

GUOO共享资料包含Ozon官方2026-07-24表中的15个历史运输档位及国欧官网的黑河、东莞仓配资料。历史时效仅到Ozon分拣中心；仓配报价正文未公开可读取金额，费用保持待确认。相关资料复制到店铺后仍需确认当前渠道适用性、附加费和有效期。

外部服务接口为`GET/PUT /integrations`、`POST /integrations/:provider/:environment/test`；商品导入为`POST /sourcing/1688/preview`、`POST /sourcing/1688/establish-relation`和`POST /sourcing/1688/load`；汇率为`POST /exchange-rates/preview`和`POST /exchange-rates/apply`。配置及设置更新携带当前版本，陈旧版本返回409；导入和汇率预览编号按店铺隔离。`load`只返回草稿，不写商品档案。

定价请求为`{product, snapshot_id, promotion_ids}`，返回记录ID、日常`result`、完整`contexts`和`sales_results`。销量评估使用同样的商品与快照，可选传入`scenarios`；未传时评估已保存的该商品情景。金融资料缺失以情景状态及原因呈现；跨店引用被拒绝，数据源异常返回502并保留错误类型。
