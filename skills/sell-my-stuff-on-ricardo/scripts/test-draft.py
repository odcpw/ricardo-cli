#!/usr/bin/env python3
"""Regression tests for missing UI controls, identity binding, format/start-price and exact comparison."""
import copy
import json
import runpy
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('compare-draft.py')
MODULE = runpy.run_path(str(SCRIPT), run_name='draft_comparison_tests')
STAMP = '2031-05-01T10:00:00Z'
NOW = MODULE['stamp'](STAMP)
FIXTURES = Path(tempfile.mkdtemp(prefix='draft-comparison-tests-'))


def receipt(value):
    return {'present': True, 'value': value, 'evidence_ref': 'view-proof', 'observed_at': STAMP}


def with_format(data, format_, **values):
    """Re-target a packet at another format; buyer_offers exists only for fixed-price formats."""
    values = dict(values, format=format_)
    data['expected'].update(values)
    if format_ != 'buy_now':
        data['expected'].pop('buyer_offers', None)
        data['observed'].pop('buyer_offers', None)
    for key, value in values.items():
        data['observed'][key] = receipt(value)
    return data


def packet():
    expected = {'title': 'Example vase', 'quantity': 1, 'price': 100, 'currency': 'CHF',
                'format': 'buy_now', 'buy_now_price': 'none', 'duration': 'not_applicable',
                'condition': 'Used', 'photos': ['photo-front', 'photo-mark'],
                'description': 'One vase; visible chip shown in the detail photo.',
                'category': 'Decorative vases', 'category_attributes': {'Material': 'Ceramic'},
                'shipping': {'service': 'parcel'}, 'handling': 'dispatch within 3 days',
                'payment': 'methods as shown in the draft', 'returns': 'No returns stated',
                'buyer_offers': False, 'promotions': False}
    return {'listing_ref': 'LIST-001', 'draft_id': '123456', 'stage': 'reopened',
            'expected': expected,
            'identity': {'listing_ref': receipt('LIST-001'), 'draft_id': receipt('123456')},
            'observed': {key: receipt(copy.deepcopy(value)) for key, value in expected.items()}}


class DraftTests(unittest.TestCase):
    def test_complete_legitimate_false_switches_pass(self):
        result = MODULE['compare'](packet(), NOW)
        self.assertTrue(result['ready'])
        self.assertEqual(result['issues'], [])

    def test_missing_switch_never_implies_disabled(self):
        data = packet()
        del data['observed']['promotions']
        result = MODULE['compare'](data, NOW)
        self.assertFalse(result['ready'])
        self.assertIn({'field': 'promotions', 'reason': 'unobserved'}, result['issues'])
        data['observed']['promotions'] = {'present': False, 'evidence_ref': 'view-proof', 'observed_at': STAMP}
        self.assertIn({'field': 'promotions', 'reason': 'not_present'}, MODULE['compare'](data, NOW)['issues'])

    def test_false_is_not_numeric_zero_or_string_false(self):
        for value in (0, 'false', None):
            with self.subTest(value=value):
                data = packet()
                data['observed']['promotions']['value'] = value
                self.assertFalse(MODULE['compare'](data, NOW)['ready'])

    def test_quantity_boolean_is_not_one(self):
        data = packet()
        data['observed']['quantity']['value'] = True
        self.assertFalse(MODULE['compare'](data, NOW)['ready'])

    def test_exact_changed_fields_and_photo_order_reported(self):
        data = packet()
        data['observed']['price']['value'] = 80
        data['observed']['photos']['value'].reverse()
        result = MODULE['compare'](data, NOW)
        self.assertEqual({row['field'] for row in result['issues']}, {'price', 'photos'})
        price = next(row for row in result['issues'] if row['field'] == 'price')
        self.assertEqual((price['expected'], price['observed']), (100, 80))

    def test_wrong_target_and_missing_identity_block_readiness(self):
        for field in ('listing_ref', 'draft_id'):
            with self.subTest(field=field):
                data = packet()
                data['identity'][field]['value'] = 'wrong-target'
                self.assertFalse(MODULE['compare'](data, NOW)['ready'])
                del data['identity'][field]
                self.assertFalse(MODULE['compare'](data, NOW)['ready'])

    def test_empty_expected_or_unknown_field_rejected(self):
        data = packet()
        data['expected'] = {}
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)

        data = packet()
        data['observed']['unknown_control'] = receipt(False)
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)

    def test_blank_policies_and_duplicate_photos_are_not_valid_intent(self):
        for key, value in [('shipping', ' '), ('handling', '\t'), ('payment', ' '), ('returns', '\n'),
                           ('photos', ['photo-a', 'photo-a']), ('photos', [' '])]:
            with self.subTest(key=key, value=value):
                data = packet()
                data['expected'][key] = value
                data['observed'][key] = receipt(value)
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)

    def test_format_and_start_price_are_material_and_not_interchangeable(self):
        data = with_format(packet(), 'auction', price=20, buy_now_price='none', duration=7)
        self.assertTrue(MODULE['compare'](data, NOW)['ready'])
        # A saved draft that silently fell back to a token start price must be caught.
        data['observed']['price']['value'] = 1
        result = MODULE['compare'](data, NOW)
        self.assertFalse(result['ready'])
        self.assertIn('price', {row['field'] for row in result['issues']})
        data['observed']['price']['value'] = 20
        data['observed']['format']['value'] = 'buy_now'
        self.assertIn('format', {row['field'] for row in MODULE['compare'](data, NOW)['issues']})

    def test_buy_now_price_must_match_format_and_start_price(self):
        for format_, buy_now in [('auction_with_buy_now', 'none'), ('auction', 50),
                                 ('classified', 50), ('auction_with_buy_now', 5),
                                 ('auction_with_buy_now', 10)]:
            with self.subTest(format=format_, buy_now=buy_now):
                data = with_format(packet(), format_, price=10, buy_now_price=buy_now)
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)
        data = with_format(packet(), 'auction_with_buy_now', price=10, buy_now_price=40, duration=7)
        self.assertTrue(MODULE['compare'](data, NOW)['ready'])

    def test_unknown_format_and_blank_duration_rejected(self):
        for key, value in [('format', 'dutch_auction'), ('duration', ' '), ('duration', 0), ('duration', True)]:
            with self.subTest(key=key, value=value):
                data = packet()
                data['expected'][key] = value
                data['observed'][key] = receipt(value)
                if key == 'format':
                    data['expected'].pop('buyer_offers')  # not fixed price: isolate the format error
                    data['observed'].pop('buyer_offers')
                with self.assertRaisesRegex(ValueError, 'format must be|duration'):
                    MODULE['compare'](data, NOW)

    def test_auto_accept_is_not_a_field(self):
        for stage in ('expected', 'observed'):
            data = packet()
            data[stage]['auto_accept'] = False if stage == 'expected' else receipt(False)
            with self.subTest(stage=stage), self.assertRaises(ValueError):
                MODULE['compare'](data, NOW)

    def test_buyer_offers_is_required_for_fixed_price_and_unknown_elsewhere(self):
        data = packet()
        self.assertIn('buyer_offers', data['expected'])
        del data['expected']['buyer_offers']  # fixed price without the field is incomplete intent
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)
        data = packet()
        del data['observed']['buyer_offers']
        result = MODULE['compare'](data, NOW)
        self.assertFalse(result['ready'])
        self.assertIn({'field': 'buyer_offers', 'reason': 'unobserved'}, result['issues'])
        for format_, extra in (('auction', {}), ('auction_with_buy_now', {'buy_now_price': 150}), ('classified', {})):
            with self.subTest(format=format_):
                data = with_format(packet(), format_, **extra)
                self.assertTrue(MODULE['compare'](data, NOW)['ready'])
                data['expected']['buyer_offers'] = False  # no such control on this format
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)
                del data['expected']['buyer_offers']
                data['observed']['buyer_offers'] = receipt(False)
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)

    def test_buyer_offers_can_be_waived_only_in_preview(self):
        data = packet()
        data['stage'] = 'preview'
        del data['observed']['buyer_offers']
        data['not_shown'] = {'buyer_offers': {'reason': 'Buyer preview does not expose price proposals',
                                              'evidence_ref': 'preview-proof', 'observed_at': STAMP}}
        self.assertTrue(MODULE['compare'](data, NOW)['ready'])

    def test_stale_identity_is_not_current_proof(self):
        data = packet()
        data['identity']['draft_id']['observed_at'] = '2031-04-01T10:00:00Z'
        self.assertIn({'field': 'draft_id', 'reason': 'stale_receipt'}, MODULE['compare'](data, NOW)['issues'])

    def test_future_naive_and_malformed_receipts_rejected(self):
        for stamp in ('2031-05-02T10:00:00Z', '2031-05-01T10:00:00',
                      '2031-05-01T10:00:00+00:60', True):
            with self.subTest(stamp=stamp):
                data = packet()
                data['observed']['price']['observed_at'] = stamp
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)
        data = packet()
        data['observed']['price']['present'] = 'true'
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)

    def test_presence_value_contradictions_rejected(self):
        for row in ({'present': True, 'evidence_ref': 'proof', 'observed_at': STAMP},
                    {'present': False, 'value': False, 'evidence_ref': 'proof', 'observed_at': STAMP}):
            with self.subTest(row=row):
                data = packet()
                data['observed']['promotions'] = row
                with self.assertRaises(ValueError):
                    MODULE['compare'](data, NOW)

    def test_preview_internal_controls_require_explicit_not_shown_evidence(self):
        data = packet()
        data['stage'] = 'preview'
        del data['observed']['promotions']
        self.assertFalse(MODULE['compare'](data, NOW)['ready'])
        data['not_shown'] = {'promotions': {'reason': 'Buyer preview does not expose seller paid options',
                            'evidence_ref': 'preview-proof', 'observed_at': STAMP}}
        self.assertTrue(MODULE['compare'](data, NOW)['ready'])
        data['stage'] = 'reopened'
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)

    def test_preview_cannot_waive_price_or_have_conflicting_receipts(self):
        data = packet()
        data['stage'] = 'preview'
        row = {'reason': 'Unavailable', 'evidence_ref': 'proof', 'observed_at': STAMP}
        data['not_shown'] = {'price': row}
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)
        data['not_shown'] = {'promotions': row}
        with self.assertRaises(ValueError):
            MODULE['compare'](data, NOW)

    def test_duplicate_json_key_and_nonfinite_rejected(self):
        for text in ('{"id":1,"id":2}', '{"price":NaN}'):
            path = Path(tempfile.mkdtemp(prefix='invalid-', dir=FIXTURES)) / 'invalid.json'
            path.write_text(text, encoding='utf-8')
            with self.assertRaises(ValueError):
                MODULE['load'](path)

    def test_extreme_decimal_exponent_is_clean_cli_error(self):
        path = Path(tempfile.mkdtemp(prefix='exponent-', dir=FIXTURES)) / 'packet.json'
        path.write_text('{"price":1e9999999999999999999}')
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(path), '--now', STAMP],
            capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertNotIn('Traceback', result.stderr)

    def test_cli_mismatch_is_report_not_structural_error(self):
        folder = Path(tempfile.mkdtemp(prefix='case-', dir=FIXTURES))
        path = folder / 'packet.json'
        data = packet()
        del data['observed']['promotions']
        path.write_text(json.dumps(data), encoding='utf-8')
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(path), '--now', STAMP],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(json.loads(result.stdout)['ready'])
        data['expected'] = {}
        path.write_text(json.dumps(data), encoding='utf-8')
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(path), '--now', STAMP],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 2)


if __name__ == '__main__':
    print(f'Retained synthetic fixtures: {FIXTURES}', flush=True)
    unittest.main(verbosity=2)
