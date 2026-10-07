#!/usr/bin/env python3
"""Regression checks with retained temporary fixtures; no account/network actions."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest
from decimal import Decimal

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('validate-project.py')
MODULE = runpy.run_path(str(SCRIPT), run_name='project_validator_tests')
validate = MODULE['validate']
expected_net = MODULE['expected_net']
CHECKS = MODULE['CHECKS']
STAMP = '2031-02-03T10:00:00+00:00'
FIXTURES = Path(tempfile.mkdtemp(prefix='resale-skill-tests-'))


def proof(basis='Synthetic fixture observation'):
    return {'verified_at': STAMP, 'basis': basis}


class ProjectTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='case-', dir=FIXTURES))
        photo = self.root / 'photos/listing/detail.jpg'
        photo.parent.mkdir(parents=True)
        # Hash checking is structural; these bytes deliberately are not an image.
        photo.write_bytes(b'synthetic derivative fixture')
        self.ledger = {
            'schema_version': 1,
            'config': {'marketplace': 'ricardo', 'currency': 'CHF', 'min_net': 100,
                       'packing_estimates_authorized': True},
            'units': [{'id': 'unit-a'}, {'id': 'unit-b'}],
            'lots': [{'id': 'lot-a', 'unit_ids': ['unit-a', 'unit-b'],
                      'state': 'draft_reserved', 'listing_ref': 'listing-a'}],
            'photos': [{'id': 'photo-a', 'path': 'photos/listing/detail.jpg',
                        'sha256': hashlib.sha256(photo.read_bytes()).hexdigest(),
                        'approved_for_upload': True, 'listing_refs': ['listing-a']}],
            'listings': [{'id': 'listing-a', 'status': 'draft', 'quantity': 1,
                          'marketplace': 'ricardo', 'draft_id': 'synthetic-draft', 'photo_ids': ['photo-a'],
                          'blockers': [], 'checks': {name: proof(name) for name in CHECKS},
                          'economics': {'item_price': 200, 'shipping_collected': 20,
                                        'fee_base': 220, 'fee_rate': 0.1, 'order_fee': 1,
                                        'postage': 18, 'packing': 4, 'other_costs': 2,
                                        'risk_allowance': 3, 'fee_basis': proof('Fee schedule'),
                                        'packing_basis': proof('Packing calculation'),
                                        'packing_estimated': True}}],
            'orders': []}

    def check(self, target='listing-a'):
        return validate(self.root, self.ledger, target)

    def assertInvalid(self, fragment, target='listing-a'):
        errors = self.check(target)
        self.assertTrue(any(fragment in error for error in errors), errors)

    def order(self, state='paid'):
        self.ledger['listings'][0]['status'] = 'ended'
        self.ledger['listings'][0]['quantity'] = 0
        lot = self.ledger['lots'][0]
        lot['state'] = ('shipped' if state in {'carrier_accepted', 'in_transit', 'delivered'}
                        else 'returned' if state == 'returned' else 'sold')
        order = {'id': 'order-a', 'lot_ids': ['lot-a'], 'status': state,
                 'ship_by_at': '2031-02-07T17:00:00+00:00', 'ship_by_timezone': 'Etc/UTC'}
        if state not in {'unpaid', 'cancelled'}:
            order.update(paid_at=STAMP, paid_evidence='Payment status in order view')
        if state in {'label_created', 'handoff_reported'}:
            order.update(label_created_at='2031-02-03T11:00:00+00:00',
                         label_created_evidence='Label purchase receipt')
        if state == 'handoff_reported':
            order.update(handoff_reported_at='2031-02-03T12:00:00+00:00',
                         handoff_reported_evidence='Owner handoff report')
        if state in {'carrier_accepted', 'in_transit', 'delivered', 'returned'}:
            order.update(carrier_accepted_at='2031-02-03T13:00:00+00:00',
                         carrier_accepted_evidence='Carrier acceptance scan')
        if state in {'in_transit', 'delivered', 'returned'}:
            order[state + '_at'] = '2031-02-04T13:00:00+00:00'
            order[state + '_evidence'] = 'Detailed tracking or return receipt'
        self.ledger['orders'] = [order]
        return order

    def test_real_file_draft_passes_and_net_is_exact(self):
        self.assertEqual(self.check(), [])
        self.assertEqual(expected_net(self.ledger['listings'][0]['economics']), Decimal('170.0'))

    def test_large_cancelling_amounts_preserve_small_fee(self):
        data = {key: 0 for key in MODULE['COST_FIELDS']}
        data.update(item_price=Decimal('1e999'), postage=Decimal('1e999'), order_fee=Decimal('.4'))
        self.assertEqual(expected_net(data), Decimal('-.4'))

    def test_json_loader_rejects_duplicate_authority_and_nonfinite_values(self):
        for body in ('{"config":{"packing_estimates_authorized":false,"packing_estimates_authorized":true}}',
                     '{"amount":NaN}'):
            with self.subTest(body=body):
                path = self.root / 'invalid.json'
                path.write_text(body)
                with self.assertRaises(ValueError):
                    MODULE['load'](path)

    def test_cli_preserves_decimal_floor_without_float_rounding(self):
        self.ledger['config']['min_net'] = 170
        (self.root / 'data').mkdir()
        body = json.dumps(self.ledger).replace('"min_net": 170', '"min_net": 170.00000000000001')
        (self.root / 'data/ledger.json').write_text(body)
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(self.root), '--listing', 'listing-a'],
            capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn('below minimum', result.stdout)

    def test_evidence_without_event_time_is_not_silently_ignored(self):
        for field in ('carrier_accepted', 'payout_sent', 'bank_arrived'):
            for suffix, value in (('_evidence', 'Observed evidence without a timestamp'), ('_at', None)):
                with self.subTest(field=field, suffix=suffix):
                    order = self.order('paid')
                    order[field + suffix] = value
                    self.assertInvalid(field + ' needs', target=None)

    def test_duplicate_photo_allowlist_cannot_pass_readiness(self):
        self.ledger['listings'][0]['photo_ids'].append('photo-a')
        self.assertInvalid('unique photo_ids')

    def test_bootstrap_and_cli_pass(self):
        init = runpy.run_path(str(SCRIPT.with_name('init-project.py')), run_name='initializer_tests')
        destination = self.root / 'new-project'
        init['initialize'](destination)
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(destination)],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((destination / '.gitignore').read_text().startswith('originals/\nprivate/'))
        config = json.loads((destination / 'data/ledger.json').read_text())['config']
        self.assertEqual(config['currency'], 'CHF')
        self.assertIsNone(config['marketplace'])
        self.assertEqual(json.loads((destination / 'private/orders.json').read_text()),
                         {'schema_version': 1, 'orders': {}})
        self.assertIn('private/', (destination / '.gitignore').read_text().splitlines())
        with self.assertRaises(FileExistsError):
            init['initialize'](destination)

    def test_cli_invalid_root_is_nonzero(self):
        (self.root / 'data').mkdir()
        (self.root / 'data/ledger.json').write_text(json.dumps([]))
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(self.root)],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Invalid workspace', result.stderr)

    def test_duplicate_ids_and_unit_allocations_rejected(self):
        self.ledger['units'].append({'id': 'unit-a'})
        self.assertInvalid('duplicate id')
        self.ledger['units'].pop()
        self.ledger['lots'].append({'id': 'lot-b', 'state': 'held', 'unit_ids': ['unit-a']})
        self.assertInvalid('duplicate allocation')

    def test_unknown_and_repeated_members_rejected(self):
        self.ledger['lots'][0]['unit_ids'].append('unknown')
        self.assertInvalid('unknown unit')
        self.ledger['lots'][0]['unit_ids'][-1] = 'unit-a'
        self.assertInvalid('duplicate allocation')

    def test_held_excluded_disposed_stock_rejected(self):
        for disposition in ('held', 'excluded', 'disposed'):
            with self.subTest(disposition=disposition):
                self.ledger['units'][0]['disposition'] = disposition
                self.assertInvalid('unit unit-a allocated')
        self.ledger['units'][0] = {'id': 'unit-a', 'excluded': True}
        self.assertInvalid('excluded/disposed')

    def test_listing_and_lot_states_are_reciprocal(self):
        self.ledger['lots'][0]['state'] = 'listed'
        self.ledger['listings'][0]['status'] = 'held'
        self.assertInvalid('incompatible', target=None)
        self.ledger['lots'][0]['state'] = 'shipped'
        self.assertInvalid('lacks a current order', target=None)

    def test_bundle_quantity_is_lots_not_components(self):
        self.ledger['listings'][0]['quantity'] = 2
        self.assertInvalid('quantity differs')
        self.ledger['listings'][0]['quantity'] = True
        self.assertInvalid('quantity differs')

    def activate(self, marketplace='ricardo', listing_id='1234567890'):
        listing = self.ledger['listings'][0]
        listing.update(status='active', marketplace=marketplace, listing_id=listing_id,
                       active_verified_at=STAMP, active_evidence='Verified live listing page')
        if marketplace == 'tutti':
            listing['seller_confirmed'] = False  # page-declared availability only
        self.ledger['lots'][0]['state'] = 'listed'
        return listing

    def test_active_listing_requires_live_proof_and_unique_ids(self):
        listing = self.activate()
        self.assertEqual(self.check(), [])
        listing['active_verified_at'] = True
        self.assertInvalid('verified marketplace listing ID/time')
        listing['active_verified_at'] = STAMP
        listing.pop('active_evidence')
        self.assertInvalid('active account/page evidence')
        self.ledger['listings'].append({'id': 'listing-b', 'status': 'held', 'marketplace': 'ricardo',
                                        'listing_id': '1234567890'})
        self.assertInvalid('also belongs')

    def test_ricardo_article_ids_are_six_to_fifteen_digit_strings(self):
        listing = self.activate()
        for good in ('123456', '1327242570', '123456789012345'):
            with self.subTest(good=good):
                listing['listing_id'] = good
                self.assertEqual(self.check(), [])
        for bad in ('12345', '1234567890123456', '12345a', ' 1234567', 1234567890, True, None,
                    'https://www.ricardo.ch/de/a/x-1234567890/'):
            with self.subTest(bad=bad):
                listing['listing_id'] = bad
                self.assertTrue(self.check(), bad)

    def test_tutti_listing_ids_are_canonical_german_detail_urls(self):
        base = 'https://www.tutti.ch/de/vi/zuerich/garten-handwerk/werkzeuge-maschinen/bambu-lab-p2s/82644754'
        listing = self.activate('tutti', base)
        self.assertEqual(self.check(), [])
        for bad in ('82644754', base + '?utm=x', base + '#top', base.replace('/de/', '/fr/'),
                    base.replace('https://www.', 'https://'), base.replace('https://', 'http://'),
                    'https://me@www.tutti.ch/de/vi/a/b/c/123', 'https://www.tutti.ch/de/vi/a/%2e%2e/c/123',
                    'https://www.tutti.ch/de/vi/a/c/123abc', 'https://www.tutti.ch/de/vi/123',
                    'https://www.tutti.ch/de/messages', 'https://www.ricardo.ch/de/vi/a/b/123'):
            with self.subTest(bad=bad):
                listing['listing_id'] = bad
                self.assertTrue(any('not a valid tutti identifier' in error for error in self.check()), bad)

    def test_marketplace_enum_and_cross_marketplace_identifier_mismatch(self):
        listing = self.activate()
        listing['marketplace'] = 'other-market'
        self.assertInvalid('marketplace must be ricardo or tutti')
        listing['marketplace'] = 'tutti'  # a Ricardo-style numeric id is not a Tutti URL
        self.assertInvalid('not a valid tutti identifier')
        listing['marketplace'] = 'ricardo'
        listing['listing_id'] = 'https://www.tutti.ch/de/vi/a/b/c/123'
        self.assertInvalid('not a valid ricardo identifier')
        listing.pop('marketplace')
        self.assertInvalid('needs marketplace')
        self.ledger['config']['marketplace'] = 'other-market'
        self.assertInvalid('config: marketplace must be ricardo or tutti', target=None)

    def test_same_numeric_id_on_different_marketplaces_is_not_confused(self):
        listing = self.activate('ricardo', '123456789')
        self.ledger['listings'].append({'id': 'listing-b', 'status': 'held', 'marketplace': 'tutti',
            'listing_id': 'https://www.tutti.ch/de/vi/a/b/c/123456789'})
        self.assertEqual(self.check(), [])
        self.ledger['listings'][1]['marketplace'] = 'ricardo'
        self.ledger['listings'][1]['listing_id'] = '123456789'
        self.assertInvalid('also belongs')
        self.assertEqual(listing['listing_id'], '123456789')

    def test_draft_listing_requires_marketplace_for_readiness(self):
        self.ledger['listings'][0].pop('marketplace')
        self.assertInvalid('needs marketplace')
        self.ledger['listings'][0]['marketplace'] = 'ricardo'
        self.ledger['config']['marketplace'] = None
        self.assertInvalid('marketplace (ricardo or tutti) required')

    def test_photo_hash_missing_approval_and_authorization(self):
        photo = self.ledger['photos'][0]
        photo['sha256'] = 'changed'
        self.assertInvalid('hash changed')
        photo['approved_for_upload'] = False
        self.assertInvalid('not approved')
        photo['approved_for_upload'] = True
        photo['listing_refs'] = 'prefix-listing-a-suffix'
        self.assertInvalid('listing_refs must be a string list')
        photo['listing_refs'] = ['listing-a']
        photo['path'] = 'photos/listing/nonexistent.jpg'
        self.assertInvalid('missing photo')

    def test_photo_escape_rejected(self):
        self.ledger['photos'][0]['path'] = '../../outside.jpg'
        self.assertInvalid('inside photos/listing')
        self.ledger['photos'][0]['path'] = str(self.root / 'photos/listing/detail.jpg')
        self.assertInvalid('photo path required')

    def test_unrelated_held_photo_does_not_block_target(self):
        self.ledger['listings'].append({'id': 'listing-held', 'status': 'held'})
        self.ledger['photos'].append({'id': 'photo-held', 'path': 'photos/listing/missing.jpg',
                                     'approved_for_upload': True, 'sha256': 'stale',
                                     'listing_refs': ['listing-held']})
        self.assertEqual(self.check(), [])
        self.assertInvalid('missing photo', target=None)

    def test_each_readiness_surface_requires_dated_evidence(self):
        for name in CHECKS:
            for invalid in (True, 'done', {'verified_at': STAMP},
                            {'verified_at': '2031-02-03T10:00:00', 'basis': 'Observation'}):
                with self.subTest(surface=name, invalid=invalid):
                    self.ledger['listings'][0]['checks'][name] = invalid
                    self.assertInvalid('missing dated evidence for ' + name)
            self.ledger['listings'][0]['checks'][name] = proof(name)

    def test_invalid_offsets_calendar_dates_and_basis_rejected(self):
        for invalid in ('2031-02-30T10:00:00Z', '2031-02-03T10:00:00+25:00',
                        '2031-02-03T10:00:00+00:60', True):
            with self.subTest(timestamp=invalid):
                self.ledger['listings'][0]['checks']['buyer_preview']['verified_at'] = invalid
                self.assertInvalid('buyer_preview')
        self.ledger['listings'][0]['checks']['buyer_preview'] = proof(' ')
        self.assertInvalid('buyer_preview')

    def test_estimate_authority_cannot_be_omitted_or_string(self):
        economics = self.ledger['listings'][0]['economics']
        for invalid in (None, 'false', 0):
            with self.subTest(estimated=invalid):
                economics['packing_estimated'] = invalid
                self.assertInvalid('packing_estimated must be boolean')
        economics['packing_estimated'] = True
        self.ledger['config']['packing_estimates_authorized'] = False
        self.assertInvalid('estimates not authorized')
        economics['packing_estimated'] = False
        self.assertEqual(self.check(), [])

    def test_malformed_types_fail_closed(self):
        mutations = (
            ('config', []), ('units', {}), ('photos', None), ('schema_version', True))
        for key, invalid in mutations:
            with self.subTest(field=key):
                old = self.ledger[key]
                self.ledger[key] = invalid
                self.assertTrue(self.check())
                self.ledger[key] = old
        self.ledger['listings'][0]['status'] = []
        self.assertInvalid('invalid listing status')

    def test_economics_reject_nonfinite_negative_strings_and_fee_rate(self):
        economics = self.ledger['listings'][0]['economics']
        for value in (True, '200', -1, float('inf'), float('nan'), None):
            with self.subTest(amount=value):
                economics['item_price'] = value
                self.assertInvalid('incomplete economics')
        economics['item_price'] = 200
        economics['fee_rate'] = 1.1
        self.assertInvalid('fraction')
        economics['fee_rate'] = 0.1
        economics['fee_basis'] = True
        self.assertInvalid('evidence basis required')

    def test_net_floor_and_known_independent_fee_base(self):
        self.ledger['config']['min_net'] = 171
        self.assertInvalid('below minimum')
        economics = copy.deepcopy(self.ledger['listings'][0]['economics'])
        economics['fee_base'] = 242
        self.assertEqual(expected_net(economics), Decimal('167.8'))

    def write_private(self, body):
        (self.root / 'private').mkdir(exist_ok=True)
        path = self.root / 'private/orders.json'
        path.write_text(body if isinstance(body, str) else json.dumps(body), encoding='utf-8')
        return path

    def test_tracked_ledger_rejects_private_order_data(self):
        order = self.order('paid')
        self.assertEqual(self.check(None), [])
        for field, value in (('ricardo_order_id', '1234567890'), ('tracking_number', 'X'),
                             ('tracking_url', 'https://example.test/t'), ('total_price_chf', 60),
                             ('seller_success_fee_rate', 0.1), ('iban', 'X')):
            with self.subTest(field=field):
                order[field] = value
                self.assertInvalid(field + ' is private order data', target=None)
                del order[field]

    def test_private_order_map_is_optional_and_validated_when_present(self):
        self.order('paid')
        self.assertEqual(self.check(None), [])  # absent map is fine
        self.write_private({'schema_version': 1, 'orders': {'order-a': {
            'ricardo_order_id': '1234567890', 'tracking_number': 'ABC123',
            'total_price_chf': 60, 'seller_success_fee_rate': 0.1}}})
        self.assertEqual(self.check(None), [])
        self.write_private({'schema_version': 1, 'orders': {}})
        self.assertEqual(self.check(None), [])

    def test_private_order_map_rejects_bad_ids_unknown_refs_and_duplicates(self):
        self.order('paid')
        for bad in ('', 'A123', '12 34', '-1', 1234567890, True, None, '1' * 21):
            with self.subTest(bad=bad):
                self.write_private({'schema_version': 1, 'orders': {'order-a': {'ricardo_order_id': bad}}})
                self.assertTrue(any('digit-only' in e for e in self.check(None)), bad)
        self.write_private({'schema_version': 1, 'orders': {'order-x': {}}})
        self.assertInvalid('unknown local order id order-x', target=None)
        duplicate = copy.deepcopy(self.ledger['orders'][0])
        duplicate.update(id='order-b', status='cancelled',
                         cancellation_disposition=proof('Cancelled and reconciled'))
        self.ledger['orders'].append(duplicate)
        self.write_private({'schema_version': 1, 'orders': {'order-a': {'ricardo_order_id': '42'},
                                                              'order-b': {'ricardo_order_id': '42'}}})
        self.assertInvalid('ricardo_order_id also belongs', target=None)
        self.write_private({'schema_version': 1, 'orders': {'order-a': {'total_price_chf': -1,
                                                                          'tracking_number': ' '}}})
        errors = self.check(None)
        self.assertTrue(any('total_price_chf' in e for e in errors), errors)
        self.assertTrue(any('tracking_number must be' in e for e in errors), errors)

    def test_private_order_map_malformed_shapes_fail_closed_without_crashing(self):
        self.order('paid')
        for body in ('not json', '[]', '{"orders": {}}', '{"schema_version": 1, "orders": []}',
                     '{"schema_version": 1, "orders": {"order-a": 5}}',
                     '{"schema_version": 1, "schema_version": 2, "orders": {}}'):
            with self.subTest(body=body):
                self.write_private(body)
                self.assertTrue(any(e.startswith('private/orders.json') for e in self.check(None)), body)

    def test_private_order_map_symlink_is_refused(self):
        self.order('paid')
        outside = self.root / 'outside.json'
        outside.write_text('{"schema_version": 1, "orders": {}}')
        (self.root / 'private').mkdir()
        (self.root / 'private/orders.json').symlink_to(outside)
        self.assertInvalid('private/orders.json: unreadable', target=None)

    def test_tutti_uniqueness_uses_numeric_ad_id_not_slug_or_percent_case(self):
        first = 'https://www.tutti.ch/de/vi/zuerich/garten/werkzeuge/schraubstock-m%C3%BCller/82644754'
        listing = self.activate('tutti', first)
        self.assertEqual(self.check(), [])
        for variant in ('https://www.tutti.ch/de/vi/bern/anderes/slug/82644754',
                        first.replace('%C3%BC', '%c3%bc'),
                        'https://www.tutti.ch/de/vi/zuerich/garten/werkzeuge/schraubstock-m%C3%BCller/082644754'):
            with self.subTest(variant=variant):
                self.ledger['listings'] = [listing, {'id': 'listing-b', 'status': 'held',
                                                     'marketplace': 'tutti', 'listing_id': variant}]
                self.assertInvalid('also belongs')
        other = first.rsplit('/', 1)[0] + '/82644755'
        self.ledger['listings'] = [listing, {'id': 'listing-b', 'status': 'held',
                                             'marketplace': 'tutti', 'listing_id': other}]
        self.assertEqual(self.check(), [])

    def test_active_tutti_listing_needs_explicit_seller_confirmed_boolean(self):
        listing = self.activate('tutti', 'https://www.tutti.ch/de/vi/zuerich/a/b/c/123456')
        self.assertEqual(self.check(), [])
        listing['seller_confirmed'] = True
        self.assertEqual(self.check(), [])
        for invalid in ('false', 0, None):
            with self.subTest(invalid=invalid):
                listing['seller_confirmed'] = invalid
                self.assertInvalid('seller_confirmed must be boolean')
        listing.pop('seller_confirmed')
        self.assertInvalid('needs boolean seller_confirmed')
        ricardo = self.activate('ricardo', '1234567890')
        ricardo.pop('seller_confirmed', None)
        self.assertEqual(self.check(), [])  # Ricardo active status does not need the flag

    def test_one_unit_cannot_be_reserved_on_two_channels_at_once(self):
        self.activate('ricardo', '1234567890')
        self.ledger['listings'].append({'id': 'listing-t', 'status': 'draft', 'quantity': 1,
                                        'marketplace': 'tutti', 'draft_id': 'tutti-draft'})
        self.ledger['lots'].append({'id': 'lot-t', 'unit_ids': ['unit-a'], 'state': 'draft_reserved',
                                    'listing_ref': 'listing-t'})
        self.assertInvalid('duplicate allocation', target=None)
        # Moving a unit means ending the first listing and releasing its lot before reserving anew.
        self.ledger['listings'][0]['status'] = 'ended'
        self.ledger['listings'][0]['quantity'] = 0
        self.ledger['lots'][0].update(state='held')
        self.ledger['lots'][0]['unit_ids'] = ['unit-b']
        self.assertEqual(self.check(None), [])

    def test_legitimate_order_states_pass(self):
        for status in ('unpaid', 'paid', 'label_created', 'handoff_reported',
                       'carrier_accepted', 'in_transit', 'delivered', 'returned'):
            with self.subTest(status=status):
                self.order(status)
                self.assertEqual(self.check(None), [])

    def test_label_is_not_carrier_acceptance(self):
        self.order('label_created')
        self.ledger['lots'][0]['state'] = 'shipped'
        self.assertInvalid('must be sold', target=None)
        self.ledger['orders'][0]['status'] = 'carrier_accepted'
        self.assertInvalid('carrier_accepted needs', target=None)

    def test_paid_label_handoff_chronology_and_evidence(self):
        order = self.order('handoff_reported')
        order['label_created_at'] = '2031-02-03T09:00:00Z'
        self.assertInvalid('chronology', target=None)
        order['label_created_at'] = '2031-02-03T11:00:00Z'
        order['handoff_reported_evidence'] = True
        self.assertInvalid('handoff_reported needs', target=None)
        order['status'] = 'unpaid'
        self.assertInvalid('unpaid order has later', target=None)

    def test_deadline_and_timezone_required_and_consistent(self):
        order = self.order('paid')
        order.pop('ship_by_at')
        self.assertInvalid('ship_by_at', target=None)
        order['ship_by_at'] = '2031-02-07T17:00:00+04:00'
        self.assertInvalid('offset conflicts', target=None)
        order['ship_by_timezone'] = 'Invalid/Region'
        self.assertInvalid('IANA', target=None)

    def test_order_status_cannot_lag_observed_events(self):
        order = self.order('paid')
        order.update(carrier_accepted_at='2031-02-03T13:00:00Z',
                     carrier_accepted_evidence='Carrier scan')
        self.assertInvalid('status predates', target=None)

    def test_bank_false_flag_conflicts_with_deposit_event(self):
        order = self.order('delivered')
        order.update(bank_arrived=False, bank_arrived_at='2031-02-06T12:00:00Z',
                     bank_arrived_evidence='Deposit record')
        self.assertInvalid('deposit event requires', target=None)

    def test_duplicate_order_and_cancelled_disposition(self):
        order = self.order('paid')
        duplicate = copy.deepcopy(order)
        duplicate['id'] = 'order-b'
        self.ledger['orders'].append(duplicate)
        self.assertInvalid('another noncancelled order', target=None)
        duplicate['status'] = 'cancelled'
        self.assertInvalid('cancellation disposition', target=None)
        duplicate['cancellation_disposition'] = proof('Cancelled duplicate reservation reconciled')
        self.assertEqual(self.check(None), [])

    def test_delivery_and_bank_arrival_need_separate_evidence(self):
        order = self.order('delivered')
        order['delivered_at'] = True
        self.assertInvalid('delivered needs', target=None)
        order['delivered_at'] = '2031-02-04T13:00:00Z'
        order['bank_arrived'] = True
        self.assertInvalid('bank_arrived needs', target=None)
        order.update(payout_sent_at='2031-02-05T12:00:00Z', payout_sent_evidence='Transfer status',
                     bank_arrived_at='2031-02-06T12:00:00Z', bank_arrived_evidence='Deposit confirmation')
        self.assertEqual(self.check(None), [])
        order['bank_arrived_at'] = '2031-02-04T12:00:00Z'
        self.assertInvalid('precedes payout', target=None)


if __name__ == '__main__':
    print(f'Retained synthetic fixtures: {FIXTURES}', flush=True)
    unittest.main(verbosity=2)
