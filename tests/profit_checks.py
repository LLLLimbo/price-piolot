import copy
import unittest
from integration import request


class ProfitChecks(unittest.TestCase):
    def test_reference_and_unknown_costs_on_both_databases(self):
        for base in ['http://app:8080', 'http://app-postgres:8080']:
            with self.subTest(database=base):
                root = '/api/stores/default'
                _, boot = request(base, root + '/bootstrap')
                p = copy.deepcopy(boot['template'])
                p.update(id='profit-check-api-fixture', name='订单校验测试商品')
                p['returns']['unclassified_rate'] = .05676186583558984
                sample = dict(order_ref='test-reference', date='2026-10-23', source='合成验收样本', carrier='样本邮政渠道', freight_kind='estimated', fee_label='原型方案', fee_kind='prototype', amount=29, procurement=7.7, freight=11.1, commission_rate=.16, fixed_fee=1.36, reported_profit=4.2)
                p['profit_check'] = sample
                status, saved = request(base, root + '/products', p)
                self.assertEqual(status, 200, saved)
                self.assertEqual(saved['product']['profit_check'], sample)
                payload = dict(product_id=p['id'], sample=sample)
                status, r = request(base, root + '/profit-checks/evaluate', payload)
                self.assertEqual(status, 200, r)
                self.assertEqual((r['status'], r['profit'], r['difference']), ('matched', 4.2, 0))
                self.assertEqual(len(r['lines']), 4)
                self.assertEqual(request(base, '/api/stores/demo-second/profit-checks/evaluate', payload)[0], 404)
                sample['freight'] = 12.1
                _, r = request(base, root + '/profit-checks/evaluate', payload)
                self.assertEqual((r['status'], r['profit'], r['difference']), ('different', 3.2, -1))
                sample['fixed_fee'] = None
                _, r = request(base, root + '/profit-checks/evaluate', payload)
                self.assertEqual(r['status'], 'incomplete')
                self.assertIsNone(r['profit'])
                sample['fixed_fee'] = 0
                _, r = request(base, root + '/profit-checks/evaluate', payload)
                self.assertAlmostEqual(r['profit'], 4.56)
                _, after = request(base, root + '/bootstrap')
                self.assertEqual(boot['settings'], after['settings'])
                self.assertEqual(boot['quotes'], after['quotes'])
                stored = next(x for x in after['products'] if x['id'] == p['id'])
                self.assertEqual(stored['profit_check']['freight'], 11.1)
                self.assertEqual(stored['returns']['unclassified_rate'], .05676186583558984)
                self.assertEqual(request(base, root + '/products/' + p['id'], method='DELETE')[0], 200)
                guoo = [q for q in request(base, '/api/public-quotes')[1]['quotes'] if q['provider'] == 'GUOO']
                self.assertEqual(len(guoo), 17)
                self.assertTrue(all(q['status'] in ['historical', 'pending'] for q in guoo))


if __name__ == '__main__':
    unittest.main(verbosity=2)
