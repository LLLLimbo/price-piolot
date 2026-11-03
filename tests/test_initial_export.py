import gzip
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('initial_export', '/export.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class InitialExport(unittest.TestCase):
    def test_business_only_and_original_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'source.db'
            db = sqlite3.connect(source)
            db.execute('CREATE TABLE records(store_id TEXT,kind TEXT,id TEXT,payload TEXT,updated_at TEXT)')
            rows = [('', 'stores', 's', '{"id":"s"}', '2026-11-03T00:00:00Z'),
                    ('s', 'products', 'p', '{"price":47.5300}', '2026-11-03T00:00:00Z'),
                    ('s', 'integrations', 'default', '{"ciphertext":"private-value"}', '2026-11-03T00:00:00Z'),
                    ('s', 'fx_previews', 'tmp', '{"id":"tmp"}', '2026-11-03T00:00:00Z'),
                    ('s', 'runs', 'old', '{"profit":4.2}', '2026-11-03T00:00:00Z')]
            db.executemany('INSERT INTO records VALUES(?,?,?,?,?)', rows)
            db.commit()
            db.close()
            output = Path(directory) / 'output'
            manifest = module.export(source, output)
            data = json.loads(gzip.decompress((output / 'seed.snapshot.json.gz').read_bytes()))
            self.assertEqual(manifest['record_count'], 2)
            self.assertEqual(data['records'][1]['payload'], rows[1][3])
            self.assertNotIn('private-value', str(data))
            self.assertEqual(manifest['excluded_kinds'], ['fx_previews', 'integrations', 'runs'])

    def test_secret_in_business_record_is_rejected(self):
        with self.assertRaises(ValueError):
            module.verify_payload({'sourcing': {'access_token': 'sensitive'}})


if __name__ == '__main__':
    unittest.main(verbosity=2)
