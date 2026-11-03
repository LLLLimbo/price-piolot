#!/usr/bin/env python3
"""Export the current SQLite business records as a credential-free startup snapshot."""
import argparse
import collections
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import re
import sqlite3

KINDS = {
    'meta', 'stores', 'public_quotes', 'settings', 'settings_versions',
    'products', 'products_versions', 'quotes', 'quotes_versions',
    'promotions', 'promotions_versions', 'sales_scenarios', 'snapshots',
}
SECRET_KEYS = {'password', 'access_token', 'app_secret', 'api_key', 'bearer_token', 'ciphertext', 'authorization'}
SECRET_PATTERN = re.compile(r'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{30,})\b')


def verify_payload(value):
    if isinstance(value, dict):
        for key, item in value.items():
            if key.lower() in SECRET_KEYS and item:
                raise ValueError('Credential field found in business data: ' + key)
            verify_payload(item)
    elif isinstance(value, list):
        for item in value:
            verify_payload(item)
    elif isinstance(value, str) and SECRET_PATTERN.search(value):
        raise ValueError('Credential-like content found in business data')


def export(source, destination):
    db = sqlite3.connect(source.resolve().as_uri() + '?mode=ro', uri=True)
    db.row_factory = sqlite3.Row
    db.execute('BEGIN')
    rows = [dict(r) for r in db.execute('SELECT store_id,kind,id,payload,updated_at FROM records ORDER BY store_id,kind,id')]
    db.close()
    records = [r for r in rows if r['kind'] in KINDS]
    if not any(r['kind'] == 'stores' for r in records):
        raise ValueError('No configured stores found in source database')
    for r in records:
        verify_payload(json.loads(r['payload']))
    exported_at = datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z')
    snapshot = {'schema_version': 1, 'exported_at': exported_at, 'records': records}
    # Payload strings preserve all historical numeric formatting and JSON bytes.
    encoded = json.dumps(snapshot, ensure_ascii=False, separators=(',', ':')).encode()
    compressed = gzip.compress(encoded, mtime=0)
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'seed.snapshot.json.gz').write_bytes(compressed)
    manifest = {
        'schema_version': 1, 'exported_at': exported_at,
        'record_count': len(records),
        'records_by_kind': dict(sorted(collections.Counter(r['kind'] for r in records).items())),
        'stores': [r['id'] for r in records if r['kind'] == 'stores'],
        'excluded_kinds': sorted({r['kind'] for r in rows if r['kind'] not in KINDS}),
        'snapshot_sha256': hashlib.sha256(compressed).hexdigest(),
        'uncompressed_bytes': len(encoded), 'compressed_bytes': len(compressed),
        'external_services': 'Connections use fresh Mock defaults; credentials and encryption keys are not included.',
    }
    (destination / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('database', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    print(json.dumps(export(args.database, args.destination), ensure_ascii=False, indent=2))
