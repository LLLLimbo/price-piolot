const {chromium}=require('playwright');
const assert=require('node:assert/strict');
async function main(){
 const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 try {
  const context=await browser.newContext({viewport:{width:1450,height:1000},reducedMotion:'reduce'});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
  const root='http://app:8080/api/stores/default';
  const boot=await(await page.request.get(root+'/bootstrap')).json();
  const product=structuredClone(boot.template);product.id='profit-check-browser-fixture';product.name='订单校验浏览器样例';product.returns.unclassified_rate=.05676186583558984;
  product.profit_check={order_ref:'test-reference',date:'2026-10-23',source:'合成验收样本 https://example.test/reference-long-identifier-123703-Black-1PC',carrier:'China Post to OZON Economy PUDO Hangzhou1',freight_kind:'estimated',fee_label:'原型方案',fee_kind:'prototype',amount:29,procurement:7.7,freight:11.1,commission_rate:.16,fixed_fee:1.36,reported_profit:4.2};
  assert.equal((await page.request.post(root+'/products',{data:product})).status(),200);
  await page.addInitScript(id=>localStorage.setItem('price-pilot-product:default',id),product.id);
  await page.goto('http://app:8080/',{waitUntil:'networkidle'});
  const panel=page.getByRole('region',{name:'订单利润校验',exact:true});
  await panel.getByText('同口径一致',{exact:true}).waitFor();
  assert.match(await panel.locator('.profit-check-formula').innerText(),/29.00 − 7.70 − 11.10 − 29.00 × 16% − 1.36 = 4.20 CNY/);
  assert.match(await panel.locator('.profit-check-values > div').nth(2).innerText(),/0.00/);
  await page.getByText('待分类售后异常率 5.68%',{exact:true}).waitFor();
  await page.getByText('订单利润校验样本',{exact:true}).click();
  await page.getByRole('spinbutton',{name:'样本运费',exact:true}).fill('12.10');
  await panel.getByText('存在差异',{exact:true}).waitFor();
  assert.match(await panel.locator('.profit-check-values > div').nth(1).innerText(),/3.20/);
  assert.match(await panel.locator('.profit-check-values > div').nth(2).innerText(),/-.*1.00/);
  const [response]=await Promise.all([page.waitForResponse(r=>r.url().endsWith('/products/'+product.id)&&r.request().method()==='PUT'),page.getByRole('button',{name:/保存商品档案/}).click()]);
  const saved=await response.json();assert.equal(saved.product.profit_check.freight,12.1);assert.equal(saved.product.returns.unclassified_rate,.05676186583558984);
  await page.getByRole('spinbutton',{name:'样本固定费用',exact:true}).fill('');
  await panel.getByText('固定费用待补充；不按0计算',{exact:true}).waitFor();
  assert.equal(await panel.locator('.profit-check-values > div').nth(1).locator('strong').innerText(),'—');
  await page.getByRole('spinbutton',{name:'样本固定费用',exact:true}).fill('0');
  await panel.locator('.profit-check-values > div').nth(1).getByText('¥4.56',{exact:true}).waitFor();
  await panel.getByText('样本费用、来源与校验范围',{exact:true}).click();
  await page.setViewportSize({width:390,height:844});
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth-window.innerWidth)<=2);
  await page.getByRole('button',{name:'切换主题',exact:true}).click();await page.locator('.pilot-app.dark').waitFor();
  await panel.screenshot({path:'/artifacts/profit-check-mobile-dark.png'});
  // A delayed result must not reappear after changing stores.
  await page.route('**/profit-checks/evaluate',async route=>{await new Promise(r=>setTimeout(r,800));await route.continue().catch(()=>{})});
  await page.getByRole('spinbutton',{name:'样本运费',exact:true}).fill('13');
  const switched=page.waitForResponse(r=>r.url().endsWith('/stores/demo-second/calculate'));
  await page.getByRole('combobox',{name:'选择店铺',exact:true}).click();await page.getByText('Ozon 示例二店',{exact:true}).last().click();
  await switched;
  await page.waitForTimeout(1100);assert.equal(await page.locator('.profit-check-panel').count(),0);
  assert.deepEqual(errors,[]);
  assert.equal((await page.request.delete(root+'/products/'+product.id)).status(),200);
  console.log('PASS: source profit comparison, draft edits, missing versus zero costs, persistence, store switch, dark theme and narrow layout.');
 }finally{await browser.close()}
}
main().catch(e=>{console.error(e);process.exit(1)});
