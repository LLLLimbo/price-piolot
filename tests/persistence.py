import json
import sys
from integration import request


def snapshot():
    out = {}
    for base in ['http://app:8080', 'http://app-postgres:8080']:
        code, rows = request(base, '/api/stores')
        assert code == 200, rows
        stores = {}
        for store in rows['stores']:
            root = '/api/stores/' + store['id']
            code, config = request(base, root + '/integrations')
            assert code == 200, config
            code, bootstrap = request(base, root + '/bootstrap')
            assert code == 200, bootstrap
            stores[store['id']] = dict(integrations=config['integrations'], settings=bootstrap['settings'], products=bootstrap['products'])
        out[base] = stores
    return out


path = '/artifacts/service-persistence.json'
if sys.argv[1] == 'capture':
    with open(path, 'w') as f:
        json.dump(snapshot(), f, sort_keys=True)
else:
    with open(path) as f:
        assert snapshot() == json.load(f), 'Configurations, FX or products changed after container recreation'
    # A successful signed request also verifies that persisted credentials still decrypt.
    for base in ['http://app:8080', 'http://app-postgres:8080']:
        cfg = request(base, '/api/stores/default/integrations')[1]['integrations']
        code, result = request(base, '/api/stores/default/integrations/1688/mock/test', {'version':cfg['version'], 'product_id':'1001'})
        assert code == 200, result
    print('PASS: SQLite/PostgreSQL profiles, encrypted credentials, FX and products survive container recreation.')
