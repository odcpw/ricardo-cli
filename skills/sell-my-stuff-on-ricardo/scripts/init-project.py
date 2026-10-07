#!/usr/bin/env python3
"""Create a new local sales workspace. No network, Git, tracker or account actions."""
import argparse
import json
from pathlib import Path


def initialize(root):
    # Never merge a scaffold into existing work or follow a preexisting symlink.
    root.mkdir(parents=True, exist_ok=False)
    (root / '.gitignore').write_text(
        'originals/\nprivate/\n.env*\n*.sqlite*\n*.db*\n*.log\n'
        '__pycache__/\n.DS_Store\n.beads/*lock*\n.beads/*-wal*\n'
        '.beads/*-shm*\n', encoding='utf-8')
    for name in ('originals', 'private', 'data', 'photos/listing', 'listings',
                 'evidence', 'operations'):
        (root / name).mkdir(parents=True, exist_ok=True)
    ledger = {
        'schema_version': 1,
        # marketplace: primary channel, 'ricardo' or 'tutti'; each listing also records its own.
        'config': {'marketplace': None, 'currency': 'CHF', 'min_net': None,
                   'packing_estimates_authorized': False},
        'units': [], 'lots': [], 'photos': [], 'listings': [], 'orders': []
    }
    (root / 'data/ledger.json').write_text(json.dumps(ledger, indent=2) + '\n')
    # Order identifiers, tracking numbers and order amounts stay here (ignored), keyed by the
    # local order id used in data/ledger.json. The tracked ledger never carries them.
    (root / 'private/orders.json').write_text(
        json.dumps({'schema_version': 1, 'orders': {}}, indent=2) + '\n')
    (root / 'PLAN.md').write_text(
        '# Selling brief\n\nRecord objectives, exclusions, channel choice (Ricardo auction, '
        'buy-now, Tutti, pickup, donate or hold), listing language (DE/FR/IT), dispatch region, '
        'currency (CHF unless the live form says otherwise), minimum worthwhile net, '
        'preparation capacity, shipping/pickup/returns choices and review date.\n\n'
        '## Authority\n\nRecord what is authorized for publication, offers, spending, '
        'buyer messages, image editing and package estimates. Note later corrections. '
        'Do not store credentials or private correspondence here.\n')
    (root / 'README.md').write_text(
        '# Sales workspace\n\nThe canonical stock ledger is data/ledger.json. '
        'Unallocated units are available unless marked excluded or held. '
        'Beads tracks work; the ledger tracks goods.\n\n'
        'Original photos and the private order map (private/orders.json: Ricardo order IDs, tracking numbers, amounts) stay in ignored local directories. '
        'Only reviewed derivatives and sanitized evidence may be committed. '
        'Verify repository privacy before pushing.\n\n'
        'Preparation, draft, active listing, paid order, purchased label, carrier '
        'acceptance or pickup collection, delivery and financial reconciliation are separate outcomes. '
        'A listing is active only after its live page is verified; unknown status is not active.\n')
    (root / 'operations/monitor.json').write_text(json.dumps({
        'last_run_at': None, 'surfaces': {}, 'last_alerts': [],
        'scheduled_access_verified': False
    }, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    try:
        initialize(args.destination.expanduser().absolute())
    except OSError as exc:
        parser.exit(1, f'Not initialized: {exc}\n')
    print('Created local workspace. No repository, Ricardo or Tutti account was changed.')


if __name__ == '__main__':
    main()
