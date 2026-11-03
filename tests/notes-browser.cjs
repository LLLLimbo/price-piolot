const {chromium}=require('playwright');
const assert=require('node:assert/strict');

async function main(){
 const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 const context=await browser.newContext({viewport:{width:1450,height:1000},reducedMotion:'reduce'});
 const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const root='http://app:8080/api/stores/default';
 const boot=await(await page.request.get(root+'/bootstrap')).json();
 const product=structuredClone(boot.template);product.id='browser-reference-fixture';product.name='商品资料验收样例';product.notes='参考资料；统计范围已注明；采购价格与包装参数待核实。';product.returns.unclassified_rate=.05676186583558984;
 const saved=await page.request.post(root+'/products',{data:product});assert.equal(saved.status(),200);
 await page.addInitScript(id=>localStorage.setItem('price-pilot-product:default',id),product.id);
 await page.goto('http://app:8080/',{waitUntil:'networkidle'});
 await page.getByText('待分类售后异常率 5.68%',{exact:true}).waitFor();
 await page.getByText('商品资料与来源',{exact:true}).click();await page.getByText(product.notes,{exact:true}).waitFor();
 await page.getByText('折扣与售后损失',{exact:true}).click();
 const ratio=Number(await page.getByRole('spinbutton',{name:'待分类售后异常率（原始订单口径）',exact:true}).inputValue());assert.ok(Math.abs(ratio-5.676)<.001);
 await page.getByText('商品备注与数据来源',{exact:true}).click();
 await page.getByLabel('资料、统计范围及待核对项',{exact:true}).fill(product.notes+'已核对来源。');
 const [response]=await Promise.all([page.waitForResponse(r=>r.url().endsWith('/products/'+product.id)&&r.request().method()==='PUT'),page.getByRole('button',{name:/保存商品档案/}).click()]);
 const result=await response.json();assert.equal(response.status(),200);assert.ok(result.product.notes.endsWith('已核对来源。'));assert.equal(result.product.returns.unclassified_rate,.05676186583558984);
 await page.setViewportSize({width:390,height:844});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth-window.innerWidth)<=2);
 assert.deepEqual(errors,[]);assert.equal((await page.request.delete(root+'/products/'+product.id)).status(),200);
 await browser.close();console.log('PASS: product notes, source preservation, unresolved after-sales warning, persistence and narrow layout.');
}
main().catch(e=>{console.error(e.message);process.exit(1)});
