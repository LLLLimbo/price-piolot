const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
let page;
async function main(){
 const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 const context=await browser.newContext({viewport:{width:1450,height:1050},reducedMotion:'reduce'});
 page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const reset=await page.request.post('http://mock-1688:8081/__mock/reset',{data:{}});assert.equal(reset.status(),200);
 const initial=(await(await page.request.get('http://app:8080/api/stores/default/integrations')).json()).integrations;
 const prepared=await page.request.put('http://app:8080/api/stores/default/integrations',{data:{provider:'1688',environment:'mock',version:initial.version,base_url:'http://mock-1688:8080',test_product_id:'1001',credentials:{app_key:'test-app',app_secret:'test-secret',access_token:'test-token'},activate:true}});assert.equal(prepared.status(),200);
 const response=(suffix,method='POST')=>page.waitForResponse(r=>r.url().endsWith(suffix)&&r.request().method()===method);
 async function clickResponse(suffix,button,method='POST'){const [res]=await Promise.all([response(suffix,method),button.click()]);const out=await res.json();assert.equal(res.status(),200,JSON.stringify(out));return out;}
 await page.goto('http://app:8080/settings',{waitUntil:'networkidle'});
 await page.getByRole('tab',{name:'外部服务',exact:true}).click();
 await page.getByRole('heading',{name:'1688 商品',exact:true}).waitFor();
 await clickResponse('/integrations/1688/mock/test',page.getByRole('button',{name:'测试已保存连接',exact:true}));
 await page.getByText('真实服务',{exact:true}).click();
 await page.getByLabel('服务根地址',{exact:true}).fill('http://mock-1688:8080');
 await page.getByLabel('AppKey',{exact:true}).fill('test-app');
 await page.getByLabel('AppSecret',{exact:true}).fill('test-secret');
 await page.getByLabel('AccessToken',{exact:true}).fill('test-token');
 const saved=await clickResponse('/integrations',page.getByRole('button',{name:'保存并启用',exact:true}),'PUT');
 assert.equal(saved.integrations.services['1688'].active,'real');assert.ok(!JSON.stringify(saved).includes('test-secret'));
 await page.getByText('当前启用：真实',{exact:true}).waitFor();
 assert.equal(await page.getByLabel('AppSecret',{exact:true}).inputValue(),'');
 await page.getByText('Mock 测试',{exact:true}).click();
 await clickResponse('/integrations',page.getByRole('button',{name:'保存并启用',exact:true}),'PUT');
 await page.getByText('当前启用：Mock',{exact:true}).waitFor();
 await page.screenshot({path:'/artifacts/external-services.png',fullPage:true});
 await page.getByRole('tab',{name:'Frankfurter 汇率',exact:true}).click();
 await clickResponse('/integrations/frankfurter/mock/test',page.getByRole('button',{name:'测试已保存连接',exact:true}));
 await page.getByRole('tab',{name:'汇率与权重',exact:true}).click();
 const oldRate=await page.getByRole('spinbutton',{name:'参考汇率 CNY → RUB',exact:true}).inputValue();
 await page.getByRole('spinbutton',{name:'价格权重',exact:true}).fill('45');
 await clickResponse('/exchange-rates/preview',page.getByRole('button',{name:'获取参考汇率',exact:true}));
 assert.equal(await page.getByRole('spinbutton',{name:'参考汇率 CNY → RUB',exact:true}).inputValue(),oldRate);
 await page.getByText('2026-10-30',{exact:true}).waitFor();
 await clickResponse('/exchange-rates/apply',page.getByRole('button',{name:'应用到本店',exact:true}));
 assert.equal(Number(await page.getByRole('spinbutton',{name:'参考汇率 CNY → RUB',exact:true}).inputValue()),12.0844);
 assert.equal(await page.getByRole('spinbutton',{name:'价格权重',exact:true}).inputValue(),'45');
 await page.getByRole('link',{name:'定价工作台',exact:true}).click();
 async function preview(id){
  await page.getByRole('button',{name:'商品操作',exact:true}).click();await page.getByText('载入1688商品',{exact:true}).click();
  await page.getByRole('textbox',{name:'1688商品链接或ID',exact:true}).fill(`https://detail.1688.com/offer/${id}.html?spm-auction=9999`);
  return (await clickResponse('/sourcing/1688/preview',page.getByRole('button',{name:'获取商品详情',exact:true}))).preview;
 }
 async function load(){
  if(await page.getByRole('textbox',{name:'确认计价单位',exact:true}).isVisible().catch(()=>false)){
   await page.getByRole('textbox',{name:'确认计价单位',exact:true}).fill('个');
   await page.getByRole('checkbox',{name:'已核对所选规格的单价、销售单位及起订条件',exact:true}).check();
  }
  const waiting=response('/sourcing/1688/load');await page.getByRole('button',{name:'载入新草稿',exact:true}).click();
  const modal=page.locator('.ant-modal-confirm');await modal.waitFor({state:'visible',timeout:1500}).catch(()=>{});if(await modal.isVisible().catch(()=>false))await modal.getByRole('button',{name:'载入新草稿',exact:true}).click();
  const res=await waiting;const out=await res.json();assert.equal(res.status(),200,JSON.stringify(out));await page.locator('.alibaba-import').waitFor({state:'hidden'});return out.product;
 }
 const first=await preview('1001');assert.equal(first.offer_id,'1001');assert.equal(first.status,'ready');
 await page.screenshot({path:'/artifacts/1688-preview.png',fullPage:true});
 const draft=await load();assert.equal(draft.sourcing.offer_id,'1001');
 assert.equal(await page.getByLabel('商品名称',{exact:true}).inputValue(),'棉质短袖 T-shirt');
 await page.getByText('1688采购来源 · Mock演示',{exact:true}).click();await page.getByRole('link',{name:'商品 1001',exact:true}).waitFor();
 // A distinct authorised account demonstrates the confirmation flow without resetting another service.
 await page.getByRole('link',{name:'费率与评分',exact:true}).click();await page.getByRole('tab',{name:'外部服务',exact:true}).click();
 await page.getByRole('tab',{name:'1688 商品',exact:true}).click();
 await page.getByLabel('AccessToken',{exact:true}).fill('other-token');
 await clickResponse('/integrations',page.getByRole('button',{name:'保存并应用',exact:true}),'PUT');
 await page.getByRole('link',{name:'定价工作台',exact:true}).click();
 const second=await preview('1002');assert.equal(second.status,'relation_required');
 await page.locator('.alibaba-import').getByRole('button',{name:'确认建立关系',exact:true}).click();
 await page.locator('.ant-modal-confirm').getByRole('button',{name:/^取\s*消$/}).click();
 await page.locator('.ant-modal-confirm').waitFor({state:'hidden'});
 const [result]=await Promise.all([response('/sourcing/1688/establish-relation'),(async()=>{await page.locator('.alibaba-import').getByRole('button',{name:'确认建立关系',exact:true}).click();await page.locator('.ant-modal-confirm').getByRole('button',{name:'确认建立关系',exact:true}).click();})()]);
 assert.equal(result.status(),200,JSON.stringify(await result.json()));
 assert.equal(Number(await page.getByRole('spinbutton',{name:'导入采购数量',exact:true}).inputValue()),2);
 const secondDraft=await load();assert.equal(secondDraft.sourcing.offer_id,'1002');
 assert.equal(await page.getByLabel('商品名称',{exact:true}).inputValue(),'陶瓷杯 Ceramic cup');
 // A dismissed delayed request must not load into the next store.
 await page.route('**/sourcing/1688/preview',async route=>{await new Promise(resolve=>setTimeout(resolve,700));await route.continue().catch(()=>{});});
 await page.getByRole('button',{name:'商品操作',exact:true}).click();await page.getByText('载入1688商品',{exact:true}).click();await page.getByRole('textbox',{name:'1688商品链接或ID',exact:true}).fill('1002');await page.getByRole('button',{name:'获取商品详情',exact:true}).click();
 await page.locator('.alibaba-import').getByRole('button',{name:/^取\s*消$/}).click();
 await page.getByRole('combobox',{name:'选择店铺',exact:true}).click();await page.getByText('Ozon 示例二店',{exact:true}).last().click();
 await page.waitForTimeout(1000);assert.notEqual(await page.getByLabel('商品名称',{exact:true}).inputValue(),'陶瓷杯 Ceramic cup');
 await page.unroute('**/sourcing/1688/preview');
 await page.getByRole('button',{name:'切换主题',exact:true}).click();await page.setViewportSize({width:390,height:844});
 await preview('1001');
 assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth-window.innerWidth)<=2);
 await page.screenshot({path:'/artifacts/1688-mobile-dark.png',fullPage:true});
 await page.locator('.alibaba-import').getByRole('button',{name:/^取\s*消$/}).click();
 // Restore the default account for repeatable manual demos.
 const cfg=(await(await page.request.get('http://app:8080/api/stores/default/integrations')).json()).integrations;
 const restored=await page.request.put('http://app:8080/api/stores/default/integrations',{data:{provider:'1688',environment:'mock',version:cfg.version,base_url:'http://mock-1688:8080',credentials:{access_token:'test-token'},activate:true}});assert.equal(restored.status(),200);
 assert.deepEqual(errors,[]);console.log('PASS: editable profiles, credential masking, explicit FX application, SKU drafts, relation confirmation, stale requests, mobile and dark theme.');
 await browser.close();
}
main().catch(async error=>{console.error(error);if(page){await page.screenshot({path:'/artifacts/services-browser-error.png',fullPage:true}).catch(()=>{});fs.writeFileSync('/artifacts/services-browser-error.html',await page.content().catch(()=>''));}process.exit(1);});
