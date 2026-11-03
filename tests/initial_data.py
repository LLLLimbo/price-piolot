import copy
import json
import sys
from pathlib import Path
from integration import request

path = Path('/artifacts/initial-data-persistence.json')
bases = ['http://snapshot-app:8080', 'http://snapshot-app-postgres:8080']
root = '/api/stores/default'
product_id = 'xzg-123703-Black-1PC'

if sys.argv[1] == 'capture':
    saved = {}
    for base in bases:
        code, boot = request(base, root + '/bootstrap')
        assert code == 200
        assert len(request(base, '/api/stores')[1]['stores']) == 2
        assert len(boot['products']) == 3 and boot['runs'] == []
        assert len(boot['quotes']) == 25
        assert next(f for f in boot['settings']['fees'] if f['code'] == 'commission')['rate'] == .16
        assert boot['seerfar']['configured'] and boot['seerfar']['mock']
        p = next(p for p in boot['products'] if p['id'] == product_id)
        assert p['profit_check']['reported_profit'] == 4.2
        code, run = request(base, root + '/calculate', {'product': p, 'snapshot_id': '', 'promotion_ids': []})
        assert code == 200 and run['result']['recommended']['price'] == 47.53, run
        frozen = request(base, root + '/runs/' + run['id'])[1]
        p = copy.deepcopy(p)
        p['name'] = '部署后修改 · 不得被初始化覆盖'
        code, out = request(base, root + '/products/' + product_id, p, 'PUT')
        assert code == 200
        saved[base] = {'product': out['product'], 'run': frozen, 'settings': boot['settings']}
    path.write_text(json.dumps(saved, ensure_ascii=False))
    print('PASS: fresh SQLite/PostgreSQL load configured stores, products, rates and snapshots; history starts empty; 47.53 CNY pricing works.')
elif sys.argv[1] == 'verify':
    for base, old in json.loads(path.read_text()).items():
        boot = request(base, root + '/bootstrap')[1]
        assert next(p for p in boot['products'] if p['id'] == product_id) == old['product']
        assert boot['settings'] == old['settings']
        assert request(base, root + '/runs/' + old['run']['id'])[1] == old['run']
        assert len(boot['runs']) == 1
    print('PASS: container recreation preserves deployed edits and new runs; initialization is not repeated.')
else:
    raise SystemExit('Use capture or verify')
