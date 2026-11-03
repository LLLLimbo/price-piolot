import copy
import time
import unittest
from integration import request


class Services(unittest.TestCase):
    def test_external_services_on_both_databases(self):
        for base in ['http://app:8080', 'http://app-postgres:8080']:
            with self.subTest(database=base):
                self.contract(base)

    def contract(self, base):
        code, created = request(base, '/api/stores', {'name': '服务接入验收'})
        self.assertEqual(code, 200, created)
        root = '/api/stores/' + created['store']['id']
        _, other = request(base, '/api/stores', {'name': '隔离验收'})
        foreign = '/api/stores/' + other['store']['id']

        def config():
            return request(base, root + '/integrations')[1]['integrations']

        def save(provider, environment='mock', **patch):
            current = config()
            data = dict(provider=provider, environment=environment, version=current['version'], base_url=current['services'][provider]['profiles'][environment]['base_url'])
            data.update(patch)
            code, out = request(base, root + '/integrations', data, 'PUT')
            self.assertEqual(code, 200, out)
            return out['integrations']

        cfg = config()
        self.assertEqual(cfg['services']['1688']['active'], 'mock')
        self.assertNotIn('ciphertext', str(cfg))
        self.assertNotIn('test-secret', str(cfg))
        for provider, extra in [('1688', {'product_id':'1001'}), ('seerfar', {}), ('frankfurter', {})]:
            code, out = request(base, root + f'/integrations/{provider}/mock/test', dict(version=config()['version'], **extra))
            if out.get('kind') == 'rate_limit':
                time.sleep(3.1)
                code, out = request(base, root + f'/integrations/{provider}/mock/test', dict(version=config()['version'], **extra))
            self.assertEqual(code, 200, out)

        # No account writes on missing relation or a cancelled confirmation.
        code, out = request(base, root + '/sourcing/1688/preview', {'input':'https://detail.1688.com/offer/1002.html?spm-auction=1001'})
        self.assertEqual(code, 200, out)
        preview = out['preview']
        self.assertEqual(preview['offer_id'], '1002')
        if preview['status'] == 'relation_required':
            self.assertEqual(request(base, root + '/sourcing/1688/establish-relation', {'preview_id':preview['id'], 'confirmed':False})[0], 400)
            self.assertEqual(request(base, foreign + '/sourcing/1688/establish-relation', {'preview_id':preview['id'], 'confirmed':True})[0], 404)
            code, out = request(base, root + '/sourcing/1688/establish-relation', {'preview_id':preview['id'], 'confirmed':True})
            self.assertEqual(code, 200, out)
            preview = out['preview']
        self.assertEqual(preview['status'], 'ready')
        product = preview['product']
        self.assertEqual(product['moq'], 2)
        before = request(base, root + '/bootstrap')[1]
        payload = dict(preview_id=preview['id'], sku_id=product['skus'][0]['id'], quantity=2, quote_confirmed=True, unit='个', unit_price=12.5, moq=2)
        self.assertEqual(request(base, root + '/sourcing/1688/load', dict(payload, quantity=1))[0], 400)
        self.assertEqual(request(base, foreign + '/sourcing/1688/load', payload)[0], 404)
        code, loaded = request(base, root + '/sourcing/1688/load', payload)
        self.assertEqual(code, 200, loaded)
        draft = loaded['product']
        self.assertIsNone(draft['supplier_score'])
        self.assertIsNone(draft['supplier_days'])
        self.assertTrue(draft['demo'])
        self.assertEqual(draft['sourcing']['offer_id'], '1002')
        self.assertEqual(draft['costs'][0]['amount'], 12.5)
        self.assertTrue(all(f['amount'] is None and f['rate'] is None and f['status']=='pending' for f in draft['costs'][1:]))
        self.assertEqual(request(base, root + '/bootstrap')[1]['products'], before['products'])
        for offer_id in ['1003', '1004', '1049760039236']:
            code, out = request(base, root + '/sourcing/1688/preview', {'input':offer_id})
            if offer_id == '1003' and code == 200 and out['preview']['status']=='ready':
                p = out['preview']
                self.assertEqual(request(base, root + '/sourcing/1688/load', dict(payload, preview_id=p['id'], sku_id=p['product']['skus'][0]['id']))[0], 400)
            else:
                self.assertTrue(code != 200 or out['preview']['status'] != 'ready')

        # Real and Mock slots remain independent; blank values preserve secrets.
        save('1688', 'real', credentials={'app_key':'private-app', 'app_secret':'private-secret', 'access_token':'private-token'})
        self.assertEqual(config()['services']['1688']['active'], 'mock')
        save('1688', 'real', credentials={'access_token':''})
        self.assertTrue(config()['services']['1688']['profiles']['real']['credentials_set']['access_token'])
        self.assertNotIn('private-token', str(request(base, root + '/bootstrap')[1]))
        self.assertFalse(request(base, foreign + '/integrations')[1]['integrations']['services']['1688']['profiles']['real']['credentials_set']['access_token'])
        save('1688', 'real', clear=['access_token'])
        self.assertFalse(config()['services']['1688']['profiles']['real']['configured'])
        self.assertEqual(request(base, root + '/sourcing/1688/load', payload)[0], 409)

        # Preview never updates settings; application only changes FX, retaining pending fees.
        initial = request(base, root + '/bootstrap')[1]['settings']
        code, out = request(base, root + '/exchange-rates/preview', {})
        self.assertEqual(code, 200, out)
        fx = out['preview']
        self.assertEqual(fx['rate']['date'], '2026-10-30')
        self.assertEqual(request(base, root + '/bootstrap')[1]['settings'], initial)
        self.assertEqual(request(base, foreign + '/exchange-rates/apply', {'preview_id':fx['id']})[0], 404)
        code, applied = request(base, root + '/exchange-rates/apply', {'preview_id':fx['id']})
        self.assertEqual(code, 200, applied)
        settings = applied['settings']
        self.assertEqual(settings['fx'], 12.0844)
        self.assertTrue(settings['fx_meta']['demo'])
        for field in ['fees', 'weights', 'stress', 'demo', 'category_fees']:
            self.assertEqual(settings[field], initial[field])
        self.assertEqual(request(base, root + '/exchange-rates/apply', {'preview_id':fx['id']})[0], 409)
        for prefix in ['invalid', 'inverted', 'rate-limit']:
            save('frankfurter', base_url='http://mock-fx:8080/'+prefix, activate=True)
            code, out = request(base, root + '/exchange-rates/preview', {})
            self.assertEqual(code, 502, out)
            self.assertEqual(request(base, root + '/bootstrap')[1]['settings'], settings)
        save('frankfurter', base_url='http://mock-fx:8080', activate=True)
        fx = request(base, root + '/exchange-rates/preview', {})[1]['preview']
        save('frankfurter', base_url='http://mock-fx:8080', activate=True)
        self.assertEqual(request(base, root + '/exchange-rates/apply', {'preview_id':fx['id']})[0], 409)


if __name__ == '__main__':
    unittest.main(verbosity=2)
