import copy
import json
import time
import unittest
import urllib.error
import urllib.request
import uuid


def request(base, path, data=None, method=None):
    req = urllib.request.Request(base + path, data=None if data is None else json.dumps(data).encode(), method=method or ('GET' if data is None else 'POST'), headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=45) as r:
            return r.status, json.load(r)
    except urllib.error.HTTPError as e:
        return e.code, json.load(e)


class Integration(unittest.TestCase):
    def test_complete_contract_on_both_databases(self):
        for base in ['http://app:8080', 'http://app-postgres:8080']:
            with self.subTest(database=base):
                self.run_contract(base)

    def run_contract(self, base):
        status, boot = request(base, '/api/stores/default/bootstrap')
        self.assertEqual(status, 200)
        self.assertTrue(boot['seerfar']['configured'])
        self.assertGreaterEqual(len(boot['quotes']), 3)
        self.assertGreaterEqual(len(request(base, '/api/public-quotes')[1]['quotes']), 15)
        with urllib.request.urlopen(base + '/') as r:
            self.assertIn('Price Pilot', r.read().decode())
        product = copy.deepcopy(boot['products'][0])
        settings = copy.deepcopy(boot['settings'])
        product.update(id='integration-' + uuid.uuid4().hex, name='完整成本测试商品', demo=False, supply='stock', supplier='测试供应商', supplier_score=4.5, supplier_scale=5, supplier_source='测试确认', weight=200, length=10, width=10, height=10, warehouse_quote_id='self', discount=0, seed_sku='', keywords=[], category_id='', commission_category='', attributes={}, own_seller_ids=[], supplier_days={'min': 0, 'max': 0}, domestic_days={'min': 1, 'max': 2}, tail_days={'min': 1, 'max': 2}, purchase_quantity=1, moq=1, purchase_tiers=[])
        product['returns'] = {'rate': 0, 'cancel_rate': 0, 'reverse': None, 'disposal': None, 'recovery': None, 'cancel_cost': None}
        cost = copy.deepcopy(product['costs'][0]); cost.update(code='procurement', name='采购价', amount=60, on='shipped', source='测试确认')
        product['costs'] = [cost]
        settings.update(demo=False, fx=12, stress=.05, fx_source='测试确认', fx_date=time.strftime('%Y-%m-%d'), weights={'price': 50, 'delivery': 30, 'supplier': 20}, category_fees={}, delivery_references=[], category_mappings=[])
        fee = copy.deepcopy(settings['fees'][0]); fee.update(code='commission', rate=.25, source='测试确认', tiers=[], minimum=None, maximum=None, basis='sale', currency='CNY', on='kept')
        settings['fees'] = [fee]
        self.assertEqual(request(base, '/api/stores/default/settings', settings, 'PUT')[0], 200)
        route = copy.deepcopy(next(q for q in boot['quotes'] if q['id'] == 'demo-economy'))
        route.update(id='', demo=False, provider='测试签约货代', name='测试协议渠道', source='测试签约合同', includes=[], bands=[{'up_to': None, 'min_weight': 1, 'max_weight': 35000, 'fixed': 0, 'per_gram': 0, 'step': 1, 'volume_divisor': 0, 'minimum_weight': 0, 'max_length': 0, 'max_sum': 0}], time_scope='buyer', delivery={'min': 10, 'max': 15})
        status, response = request(base, '/api/stores/default/quotes', route)
        self.assertEqual(status, 200, response)
        product['route_ids'] = [response['quote']['id']]
        self.assertEqual(request(base, '/api/stores/default/products', product)[0], 200)
        peers = [{'sku': 'manual-A', 'title': '同规格A', 'price_rub': 1500, 'include': True, 'verified': True, 'delivery': {'min': 30, 'max': 40}, 'delivery_source': '测试承诺'}, {'sku': 'manual-B', 'title': '同规格B', 'price_rub': 1800, 'include': True, 'verified': True, 'delivery': {'min': 30, 'max': 40}, 'delivery_source': '测试承诺'}]
        status, response = request(base, '/api/stores/default/snapshots', {'product': product, 'competitors': peers})
        self.assertEqual(status, 200, response)
        snap = response['snapshot']
        run_id = None
        for mode, expected in [('profit', 109.1), ('breakeven', 80), ('delivery', 137.5), ('target', 124.99)]:
            product['mode'] = mode
            product['options'].update(margin=.2, target_score=95, target_max_price=200, target_loss=0, delivery_loss=0)
            status, calc = request(base, '/api/stores/default/calculate', {'product': product, 'snapshot_id': snap['id']})
            self.assertEqual(status, 200, calc)
            self.assertIsNotNone(calc['result']['recommended'], calc)
            ev = calc['result']['recommended']
            self.assertAlmostEqual(ev['price'], expected, places=2)
            self.assertGreaterEqual(ev['worst_profit'], -1e-8)
            if mode == 'profit': self.assertGreaterEqual(ev['worst_margin'], .2)
            self.assertFalse(calc['result']['demo'])
            run_id = calc['id']
        self.assertEqual(request(base, '/api/stores/default/runs/' + run_id)[0], 200)
        # Same snapshot, independent prices, fixed merchant discount, no subsidy double counting.
        promo = {'name':'接口验收活动','source':'商家测试假设','merchant_discount':{'kind':'fixed','value':10,'currency':'CNY'},'platform_discount':{'kind':'fixed','value':120,'currency':'RUB','confirmed':True,'source':'测试平台全额承担'}}
        status, saved = request(base, '/api/stores/default/promotions', promo)
        self.assertEqual(status,200,saved); promotion=saved['promotion']
        product['mode']='profit'
        status, multi=request(base,'/api/stores/default/calculate',{'product':product,'snapshot_id':snap['id'],'promotion_ids':[promotion['id']]})
        self.assertEqual(status,200,multi); self.assertEqual(len(multi['contexts']),2)
        normal,activity=multi['contexts']
        self.assertAlmostEqual(normal['result']['recommended']['price'],109.1)
        self.assertAlmostEqual(activity['result']['recommended']['price'],119.1)
        self.assertAlmostEqual(activity['result']['recommended']['revenue'],109.1)
        self.assertAlmostEqual(activity['result']['recommended']['price_rub'],1189.2)
        self.assertEqual(normal['result']['snapshot_id'],activity['result']['snapshot_id'])
        # Draft platform discounts can be saved but cannot manufacture a complete activity result.
        promo['platform_discount']['confirmed']=False; promo['name']='待确认活动'
        unknown=request(base,'/api/stores/default/promotions',promo)[1]['promotion']
        status, draft=request(base,'/api/stores/default/calculate',{'product':product,'snapshot_id':snap['id'],'promotion_ids':[unknown['id']]})
        self.assertEqual(status,200);self.assertIsNotNone(draft['contexts'][0]['result']['recommended']);self.assertEqual(draft['contexts'][1]['result']['status'],'invalid')
        # Sales expectations stay explicit, including missing and zero values.
        assumptions=[{'id':'assumption','product_id':product['id'],'price':100,'quote_id':product['route_ids'][0],'daily_units':.5}]
        status, evaluated=request(base,'/api/stores/default/sales-scenarios/evaluate',{'product':product,'snapshot_id':snap['id'],'scenarios':assumptions})
        self.assertEqual(status,200,evaluated);ev=evaluated['evaluations'][0]
        self.assertAlmostEqual(ev['daily_profit'],7.5);self.assertFalse(ev['meets_constraints'])
        assumptions[0]['daily_units']=None
        self.assertIsNone(request(base,'/api/stores/default/sales-scenarios/evaluate',{'product':product,'snapshot_id':snap['id'],'scenarios':assumptions})[1]['evaluations'][0]['daily_profit'])
        # Quote revisions and calculations preserve their earlier values.
        quote=request(base,'/api/stores/default/bootstrap')[1]['quotes']
        quote=next(q for q in quote if q['id']==product['route_ids'][0]);quote['bands'][0]['fixed']=1
        status,changed=request(base,'/api/stores/default/quotes/'+quote['id'],quote,'PUT')
        self.assertEqual(status,200,changed);self.assertEqual(changed['quote']['version'],2)
        versions=request(base,'/api/stores/default/quotes/'+quote['id']+'/versions')[1]['versions']
        self.assertEqual({q['version'] for q in versions},{1,2})
        frozen=request(base,'/api/stores/default/runs/'+multi['id'])[1]
        self.assertAlmostEqual(frozen['contexts'][0]['result']['recommended']['price'],109.1)
        self.assertEqual(frozen['contexts'][0]['input']['quotes'][0]['store_id'],'default')
        # OGL is a carrier price plus separate confirmed services, not a substitute for bank acquiring.
        copied = request(base, '/api/stores/default/quotes/copy-public', {'public_id': 'public-ogl'})[1]['quote']
        self.assertEqual(len(copied['fees']), 2)
        copied.update(name='接口OGL附加费', status='active', valid_to='2027-12-31')
        self.assertEqual(request(base, '/api/stores/default/quotes/'+copied['id'], copied, 'PUT')[0], 200)
        ogl_product = copy.deepcopy(product); ogl_product['route_ids'] = [copied['id']]
        pending = request(base, '/api/stores/default/calculate', {'product': ogl_product, 'snapshot_id': snap['id']})[1]['result']
        self.assertEqual(pending['status'], 'incomplete'); self.assertIsNone(pending['recommended'])
        for component in copied['fees']: component['status'] = 'confirmed'
        confirmed = request(base, '/api/stores/default/quotes/'+copied['id'], copied, 'PUT')[1]['quote']
        self.assertTrue(all(f['version'] == confirmed['version'] for f in confirmed['fees']))
        scenario = {'product_id': product['id'], 'price': 100, 'quote_id': copied['id'], 'daily_units': .5}
        assessed = request(base, '/api/stores/default/sales-scenarios/evaluate', {'product': ogl_product, 'snapshot_id': snap['id'], 'scenarios': [scenario]})[1]['evaluations'][0]
        self.assertAlmostEqual(assessed['evaluation']['cost'], 95.17)
        self.assertAlmostEqual(assessed['daily_profit'], 2.415)
        self.assertEqual(len([l for l in assessed['evaluation']['lines'] if l['code'] in ['logistics_access', 'international_contract']]), 2)
        # Actual return routing must be known; WHD resale is outside the cross-border-only model.
        returns_product = copy.deepcopy(product)
        returns_product['returns'].update(rate=.1, reverse=0, disposal=0, recovery=0)
        for destination in ['', 'whd']:
            returns_product['returns'].update(destination=destination, source='测试后台设置' if destination else '')
            blocked = request(base, '/api/stores/default/calculate', {'product': returns_product, 'snapshot_id': snap['id']})[1]['result']
            self.assertIsNone(blocked['recommended']); self.assertEqual(blocked['status'], 'invalid')
        # The same merchant resource ID can exist in both stores, but one store cannot read the other's snapshot.
        foreign=copy.deepcopy(product);foreign['store_id']='default'
        self.assertEqual(request(base,'/api/stores/demo-second/calculate',{'product':foreign,'snapshot_id':snap['id']})[0],400)
        foreign['store_id']='demo-second';foreign['route_ids']=[];foreign['warehouse_quote_id']='self'
        self.assertEqual(request(base,'/api/stores/demo-second/calculate',{'product':foreign,'snapshot_id':snap['id']})[0],404)
        # Snapshot editing must never make old upstream data appear freshly fetched.
        old_time = snap['fetched_at']; snap['fetched_at'] = '2099-01-01T00:00:00Z'; snap['competitors'][0]['include'] = False
        status, edited = request(base, '/api/stores/default/snapshots/' + snap['id'], snap, 'PUT')
        self.assertEqual(status, 200); self.assertEqual(edited['snapshot']['fetched_at'], old_time)
        # Read actual responses from the supplied Mock, not locally generated competitors.
        mock_product = copy.deepcopy(product)
        mock_product.update(name='打印纸 A4', seed_sku='149222760', keywords=['Бумага'], category_id='17027492_17029017_91400', attributes={}, own_seller_ids=[])
        status, found = request(base, '/api/stores/default/competitors/discover', {'product': mock_product})
        if status == 502 and found.get('kind') == 'rate_limit':
            time.sleep(3.1); status, found = request(base, '/api/stores/default/competitors/discover', {'product': mock_product})
        self.assertEqual(status, 200, found)
        self.assertEqual(found['snapshot']['source'], 'seerfar')
        self.assertTrue(any(c['sku'] == '149222760' for c in found['snapshot']['competitors']))
        self.assertTrue(all(c['delivery'] is None for c in found['snapshot']['competitors']))
        # HTTP 200 / non-200 business code is a failure, and preserves snapshots.
        before = len(request(base, '/api/stores/default/bootstrap')[1]['snapshots'])
        faults = [{'path': '/open-api/product/detail/search/ozon', 'remaining': 1, 'status': 200, 'body': {'code': 402, 'msg': '积分不足'}}]
        self.assertEqual(request('http://mock-seerfar:8081', '/__mock/faults', faults, 'PUT')[0], 200)
        time.sleep(3.1)
        status, failed = request(base, '/api/stores/default/competitors/discover', {'product': mock_product})
        self.assertEqual(status, 502); self.assertEqual(failed['kind'], 'quota')
        self.assertEqual(len(request(base, '/api/stores/default/bootstrap')[1]['snapshots']), before)
        request('http://mock-seerfar:8081', '/__mock/faults', [], 'PUT')
        blocked = copy.deepcopy(next(q for q in request(base, '/api/public-quotes')[1]['quotes'] if q['id'] == 'excluded-uni-offline')); blocked.update(status='active', platforms=['ozon'], public_id='excluded-uni-offline')
        self.assertEqual(request(base, '/api/stores/default/quotes/' + blocked['id'], blocked, 'PUT')[0], 400)
        self.assertEqual(request(base, '/api/stores/default/products/' + product['id'], method='DELETE')[0], 200)
        self.assertEqual(request(base, '/api/stores/default/runs/' + run_id)[0], 200)


if __name__ == '__main__':
    unittest.main(verbosity=2)
