#!/usr/bin/env python3
"""Exercise local assistant decisions with retained synthetic fixtures, no network."""
import copy
import json
import runpy
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from decimal import Decimal, localcontext
from fractions import Fraction
from itertools import product
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('sales-assistant.py')
MODULE = runpy.run_path(str(SCRIPT), run_name='sales_assistant_tests')
FIXTURES = Path(tempfile.mkdtemp(prefix='sales-assistant-tests-'))


def scenario():
    # Arithmetic fixture only: these rates and amounts are not marketplace fees or carrier prices.
    return {'currency': 'CHF', 'minor_unit_digits': 2, 'min_net': 50,
            'scenarios': [{'name': 'base', 'basis': 'Synthetic arithmetic fixture',
                'item_price': 100, 'shipping_collected': 15, 'postage': 14,
                'packing': 3, 'other_costs': 0, 'risk_allowance': 4,
                'fee_model': {'rate': Decimal('0.0625'), 'fixed_fee': Decimal('0.25'),
                              'tax_rate': Decimal('0.2'), 'includes_shipping': True,
                              'includes_tax': True}}]}


class AssistantTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='case-', dir=FIXTURES))
        (self.root / 'data').mkdir()
        self.ledger = {'schema_version': 1, 'config': {'packing_estimates_authorized': False},
                       'units': [], 'lots': [], 'photos': [], 'listings': [], 'orders': []}

    def save(self):
        (self.root / 'data/ledger.json').write_text(json.dumps(self.ledger), encoding='utf-8')

    def test_decimal_net_and_ceiling_floor_boundary(self):
        data = scenario()
        row = MODULE['economics'](data)['scenarios'][0]
        self.assertEqual(Decimal(row['modeled_fee_base']), Decimal('138'))
        self.assertEqual(Decimal(row['modeled_fees']), Decimal('8.875'))
        self.assertEqual(Decimal(row['modeled_net']), Decimal('85.125'))
        candidate = Decimal(row['minimum_item_price_in_this_model'])
        data['scenarios'][0]['item_price'] = candidate
        self.assertTrue(MODULE['economics'](data)['scenarios'][0]['meets_net_floor'])
        data['scenarios'][0]['item_price'] = candidate - Decimal('.01')
        self.assertFalse(MODULE['economics'](data)['scenarios'][0]['meets_net_floor'])

    def test_subprecision_fee_never_disappears_or_rounds_minimum_down(self):
        data = scenario()
        row = data['scenarios'][0]
        row.update(item_price=50, shipping_collected=0, postage=0, packing=0, other_costs=0, risk_allowance=0)
        row['fee_model'].update(rate=0, tax_rate=0, fixed_fee=Decimal('1e-50'))
        result = MODULE['economics'](data)['scenarios'][0]
        self.assertFalse(result['meets_net_floor'])
        self.assertEqual(result['minimum_item_price_in_this_model'], '50.01')
        self.assertEqual(Decimal(result['margin_above_floor']), Decimal('-1e-50'))

    def test_economics_does_not_inherit_callers_decimal_precision(self):
        with localcontext() as ctx:
            ctx.prec = 2
            result = MODULE['economics'](scenario())['scenarios'][0]
            self.assertEqual(ctx.prec, 2)
        self.assertEqual(Decimal(result['modeled_net']), Decimal('85.125'))

    def test_minimum_prices_against_independent_rational_net_boundaries(self):
        for floor, rate, shipping_base, tax_base, digits in product(
                (0, 50), (Decimal('0'), Decimal('.0625'), Decimal('.333333')),
                (False, True), (False, True), (0, 2, 4)):
            with self.subTest(floor=floor, rate=rate, shipping=shipping_base, tax=tax_base, digits=digits):
                data = scenario()
                data.update(min_net=floor, minor_unit_digits=digits)
                row = data['scenarios'][0]
                row['fee_model'].update(rate=rate, includes_shipping=shipping_base, includes_tax=tax_base)
                result = MODULE['economics'](data)['scenarios'][0]
                candidate = Fraction(Decimal(result['minimum_item_price_in_this_model']))
                quantum = Fraction(1, 10 ** digits)

                def net(price):
                    model = row['fee_model']
                    base = price + (row['shipping_collected'] if shipping_base else 0)
                    if tax_base:
                        base *= 1 + Fraction(model['tax_rate'])
                    return (price + row['shipping_collected'] - base * Fraction(rate)
                            - Fraction(model['fixed_fee']) - row['postage'] - row['packing']
                            - row['other_costs'] - row['risk_allowance'])

                self.assertGreaterEqual(net(candidate), floor)
                if candidate:
                    self.assertLess(net(candidate - quantum), floor)

    def test_money_budget_refuses_excessive_exponents_before_arithmetic(self):
        data = scenario()
        data['scenarios'][0]['item_price'] = Decimal('1e10001')
        with self.assertRaisesRegex(ValueError, 'budget'):
            MODULE['economics'](data)

    def test_shipping_and_tax_switches_change_fee_base_not_revenue(self):
        data = scenario()
        data['scenarios'][0]['fee_model'].update(includes_shipping=False, includes_tax=False)
        row = MODULE['economics'](data)['scenarios'][0]
        self.assertEqual(Decimal(row['modeled_fee_base']), Decimal('100'))
        self.assertEqual(Decimal(row['modeled_net']), Decimal('87.5'))

    def test_downside_can_fail_without_changing_base(self):
        data = scenario()
        downside = copy.deepcopy(data['scenarios'][0])
        downside.update(name='downside', postage=60)
        data['scenarios'].append(downside)
        result = MODULE['economics'](data)
        self.assertTrue(result['scenarios'][0]['meets_net_floor'])
        self.assertFalse(result['scenarios'][1]['meets_net_floor'])
        self.assertFalse(result['all_scenarios_meet_floor'])

    def test_fee_models_fail_closed(self):
        for key, invalid in [('rate', True), ('rate', '0.1'), ('rate', 1.1),
                             ('rate', float('nan')), ('tax_rate', -1),
                             ('fixed_fee', float('inf')), ('includes_tax', 'false')]:
            with self.subTest(key=key, invalid=invalid):
                data = scenario()
                data['scenarios'][0]['fee_model'][key] = invalid
                with self.assertRaises(ValueError):
                    MODULE['economics'](data)
        data = scenario()
        data['scenarios'][0]['fee_model']['rate'] = 1
        with self.assertRaisesRegex(ValueError, 'positive marginal'):
            MODULE['economics'](data)

    def test_explicit_precision_unique_names_and_required_inputs(self):
        for key, value in [('minor_unit_digits', True), ('minor_unit_digits', None),
                           ('currency', ''), ('min_net', '50'), ('scenarios', [])]:
            with self.subTest(key=key):
                data = scenario()
                data[key] = value
                with self.assertRaises(ValueError):
                    MODULE['economics'](data)
        data = scenario()
        data['scenarios'].append(copy.deepcopy(data['scenarios'][0]))
        with self.assertRaisesRegex(ValueError, 'unique'):
            MODULE['economics'](data)

    def test_currency_defaults_to_chf_but_is_reported_and_never_blank(self):
        data = scenario()
        explicit = MODULE['economics'](data)
        self.assertEqual((explicit['currency'], explicit['currency_defaulted']), ('CHF', False))
        del data['currency']
        defaulted = MODULE['economics'](data)
        self.assertEqual((defaulted['currency'], defaulted['currency_defaulted']), ('CHF', True))
        data['currency'] = ''
        with self.assertRaises(ValueError):
            MODULE['economics'](data)

    def test_zero_decimal_currency_floor_rounds_up(self):
        data = scenario()
        data['minor_unit_digits'] = 0
        row = MODULE['economics'](data)['scenarios'][0]
        price = Decimal(row['minimum_item_price_in_this_model'])
        self.assertEqual(price, price.to_integral_value())
        data['scenarios'][0]['item_price'] = price
        self.assertTrue(MODULE['economics'](data)['scenarios'][0]['meets_net_floor'])

    def test_overdue_handoff_access_and_draft_queue(self):
        self.ledger['orders'] = [
            {'id': 'order-label', 'status': 'label_created', 'lot_ids': ['missing-lot'],
             'ship_by_at': '2031-01-01T10:00:00Z'},
            {'id': 'order-report', 'status': 'handoff_reported', 'lot_ids': ['other-lot'],
             'ship_by_at': '2031-01-01T10:00:00Z'}]
        self.ledger['listings'] = [{'id': 'ready-draft', 'status': 'local', 'blockers': []}]
        self.save()
        (self.root / 'operations').mkdir()
        (self.root / 'operations/monitor.json').write_text(json.dumps({
            'surfaces': {'returns': {'status': 'access_failed'}}}), encoding='utf-8')
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        by_id = {row['id']: row for row in result['actions']}
        self.assertTrue(result['structural_errors'])
        self.assertIn('physical carrier handoff', by_id['order-label']['action'])
        self.assertIn('Do not buy another label', by_id['order-report']['action'])
        self.assertLess(by_id['order-label']['priority'], by_id['ready-draft']['priority'])
        self.assertEqual(by_id['returns']['kind'], 'access_gap')

    def test_packet_omits_arbitrary_private_prose_and_reports_gaps(self):
        self.ledger['listings'] = [{'id': 'listing-a', 'status': 'local', 'quantity': 0,
                                   'buyer_name': 'PRIVATE_SENTINEL',
                                   'description': 'PRIVATE_SENTINEL',
                                   'checks': {'identity_contents': {'basis': 'PRIVATE_SENTINEL'}}}]
        self.save()
        result = MODULE['listing_packet'](self.root, 'listing-a')
        self.assertNotIn('PRIVATE_SENTINEL', json.dumps(result))
        self.assertTrue(result['readiness_errors'])
        self.assertFalse(result['checks_with_dated_evidence']['identity_contents'])
        with self.assertRaises(ValueError):
            MODULE['listing_packet'](self.root, 'not-found')

    def test_actual_cli_json_and_invalid_time(self):
        self.save()
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'next-actions', str(self.root),
                                 '--now', '2031-01-01T00:00:00Z'],
                                capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['actions'], [])
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'next-actions', str(self.root),
                                 '--now', '2031-01-01'],
                                capture_output=True, text=True, timeout=10, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('offset', result.stderr)

    def test_nonfinite_json_rejected_at_load(self):
        path = self.root / 'invalid.json'
        path.write_text('{"amount": NaN}', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'nonfinite'):
            MODULE['load'](path)

    def test_duplicate_json_cannot_replace_price_or_authority(self):
        path = self.root / 'duplicate.json'
        path.write_text('{"packing_estimates_authorized":false,"packing_estimates_authorized":true}')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            MODULE['load'](path)

    def test_packet_preserves_intended_photo_order(self):
        self.ledger['listings'] = [{'id': 'listing-a', 'status': 'local',
                                    'photo_ids': ['photo-b', 'photo-a']}]
        self.ledger['photos'] = [{'id': pid, 'approved_for_upload': False, 'listing_refs': []}
                                  for pid in ('photo-a', 'photo-b')]
        self.save()
        result = MODULE['listing_packet'](self.root, 'listing-a')
        self.assertEqual([row['id'] for row in result['photos']], ['photo-b', 'photo-a'])

    def test_malformed_tables_report_errors_without_crashing_queue(self):
        for table in ('orders', 'listings'):
            with self.subTest(table=table):
                self.ledger[table] = None
                self.save()
                result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
                self.assertIn(table + ': must be a list', result['structural_errors'])

    def test_invalid_fulfillment_method_never_creates_shipping_instruction(self):
        self.ledger['orders'] = [{'id': 'order-a', 'status': 'paid', 'fulfillment_method': 'unknown'}]
        self.save()
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        self.assertTrue(result['structural_errors'])
        self.assertFalse(any(row['id'] == 'order-a' for row in result['actions']))

    def test_packet_cli_reports_invalid_decimal_quantity_without_serialization_failure(self):
        self.ledger['listings'] = [{'id': 'listing-a', 'status': 'draft', 'quantity': 1.5}]
        self.save()
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'listing-packet', str(self.root),
            '--listing', 'listing-a'], capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        packet = json.loads(result.stdout)
        self.assertEqual(packet['recorded_quantity'], '1.5')
        self.assertTrue(packet['structural_errors'])
        self.assertTrue(packet['readiness_errors'])

    def test_extreme_decimal_cli_returns_input_error_without_traceback(self):
        data = scenario()
        path = self.root / 'overflow.json'
        body = json.dumps(data, default=float).replace('"item_price": 100', '"item_price": 1e9999999')
        path.write_text(body)
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'economics', str(path)],
            capture_output=True, text=True, check=False, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('Traceback', result.stderr)

    def test_fractional_legacy_draft_survives_decimal_loader(self):
        fixtures = runpy.run_path(str(SCRIPT.with_name('test-project.py')),
                                 run_name='legacy_fixture_for_assistant')
        case = fixtures['ProjectTests']('test_real_file_draft_passes_and_net_is_exact')
        case.setUp()
        (case.root / 'data').mkdir()
        (case.root / 'data/ledger.json').write_text(json.dumps(case.ledger), encoding='utf-8')
        result = MODULE['listing_packet'](case.root, 'listing-a')
        self.assertEqual(result['structural_errors'], [])
        self.assertEqual(result['readiness_errors'], [])
        self.assertEqual((result['listing_ref'], result['marketplace']), ('listing-a', 'ricardo'))
        self.assertNotIn('listing_ids', json.dumps(result))

    def test_listing_packet_does_not_block_ready_stock_on_unrelated_held_photo(self):
        fixtures = runpy.run_path(str(SCRIPT.with_name('test-project.py')), run_name='packet_scope_fixture')
        case = fixtures['ProjectTests']('test_real_file_draft_passes_and_net_is_exact')
        case.setUp()
        case.ledger['listings'].append({'id': 'listing-held', 'status': 'held'})
        case.ledger['photos'].append({'id': 'photo-held', 'approved_for_upload': True,
            'listing_refs': ['listing-held'], 'path': 'photos/listing/missing.jpg', 'sha256': 'stale'})
        (case.root / 'data').mkdir()
        (case.root / 'data/ledger.json').write_text(json.dumps(case.ledger))
        packet = MODULE['listing_packet'](case.root, 'listing-a')
        self.assertEqual(packet['structural_errors'], [])
        self.assertEqual(packet['readiness_errors'], [])
        # The ordinary whole-project check still identifies the held derivative.
        self.assertTrue(MODULE['read_ledger'](case.root)[1])
        case.ledger['lots'].append({'id': 'conflict', 'unit_ids': ['unit-a'], 'state': 'held'})
        (case.root / 'data/ledger.json').write_text(json.dumps(case.ledger))
        packet = MODULE['listing_packet'](case.root, 'listing-a')
        self.assertTrue(any('duplicate allocation' in e for e in packet['structural_errors']))
        self.assertTrue(any('duplicate allocation' in e for e in packet['readiness_errors']))

    def test_large_json_number_remains_finite_decimal_in_cli(self):
        data = scenario()
        data['scenarios'][0]['fee_model'] = {'rate': 0.0625, 'fixed_fee': 0.25,
            'tax_rate': 0.2, 'includes_shipping': True, 'includes_tax': True}
        text = json.dumps(data).replace('"item_price": 100', '"item_price": 1e999')
        path = self.root / 'large-number.json'
        path.write_text(text, encoding='utf-8')
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'economics', str(path)],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        net = Decimal(json.loads(result.stdout)['scenarios'][0]['modeled_net'])
        self.assertTrue(net.is_finite())
        self.assertGreater(net, Decimal('1e998'))

    def test_upcoming_handoff_outranks_access_gap_and_preserves_reported_handoff(self):
        self.ledger['orders'] = [
            {'id': 'label-soon', 'status': 'label_created', 'lot_ids': ['lot-a'],
             'ship_by_at': '2031-01-02T12:00:00Z'},
            {'id': 'reported-soon', 'status': 'handoff_reported', 'lot_ids': ['lot-b'],
             'ship_by_at': '2031-01-02T12:00:00Z'}]
        self.save()
        (self.root / 'operations').mkdir()
        (self.root / 'operations/monitor.json').write_text(json.dumps({
            'required_surfaces': ['returns'], 'surfaces': {'returns': {'status': 'access_failed'}}}), encoding='utf-8')
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        by_id = {row['id']: row for row in result['actions']}
        self.assertLess(by_id['label-soon']['priority'], by_id['returns']['priority'])
        self.assertLess(by_id['reported-soon']['priority'], by_id['returns']['priority'])
        self.assertEqual(by_id['reported-soon']['kind'], 'scan_gap')
        self.assertNotIn('physical carrier handoff', by_id['reported-soon']['action'])

    def test_monitor_missing_not_checked_stale_and_no_evidence_are_gaps(self):
        self.save()
        (self.root / 'operations').mkdir()
        (self.root / 'operations/monitor.json').write_text(json.dumps({
            'required_surfaces': ['orders', 'messages', 'offers', 'returns', 'tracking', 'account'],
            'surfaces': {
                'orders': {'status': 'checked', 'checked_at': '2031-01-01T20:00:00Z', 'evidence_ref': 'proof-orders'},
                'messages': {'status': 'not_checked', 'checked_at': '2031-01-01T20:00:00Z'},
                'offers': {'status': 'checked', 'checked_at': '2030-12-01T20:00:00Z', 'evidence_ref': 'old-proof'},
                'returns': {'status': 'checked', 'checked_at': '2031-01-01T20:00:00Z'},
                'tracking': {'status': 'checked', 'checked_at': '2031-01-03T20:00:00Z', 'evidence_ref': 'future-proof'}}
            }), encoding='utf-8')
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        gaps = {row['id'] for row in result['actions'] if row['kind'] == 'access_gap'}
        self.assertEqual(gaps, {'messages', 'offers', 'returns', 'tracking', 'account'})
        self.assertEqual(result['local_coverage']['orders'], 'checked')
        self.assertEqual(result['local_coverage']['offers'], 'stale')
        self.assertEqual(result['local_coverage']['account'], 'missing')

    def test_queue_windows_are_explicit_finite_and_positive(self):
        self.save()
        for invalid in (True, 0, -1, float('nan'), float('inf')):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc), deadline_hours=invalid)

    def test_exact_deadline_is_overdue_for_both_fulfillment_methods(self):
        self.ledger['orders'] = [
            {'id': 'shipping', 'status': 'paid', 'ship_by_at': '2031-01-02T00:00:00Z'},
            {'id': 'pickup', 'fulfillment_method': 'pickup', 'status': 'pickup_arranged',
             'pickup_by_at': '2031-01-02T00:00:00Z'}]
        self.save()
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        rows = [row for row in result['actions'] if row['id'] in {'shipping', 'pickup'}]
        self.assertEqual(len(rows), 2)
        self.assertTrue(all(row['priority'] == 0 and 'passed' in row['action'] for row in rows))

    def test_pickup_queue_uses_collection_not_postage_or_scans(self):
        self.ledger['orders'] = [{'id': 'pickup-' + state, 'fulfillment_method': 'pickup',
            'status': state, 'lot_ids': ['lot-' + state], 'pickup_by_at': '2031-01-02T12:00:00Z'}
            for state in ('unpaid', 'paid', 'pickup_arranged', 'collected', 'returned')]
        self.save()
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        rows = [row for row in result['actions'] if row['id'].startswith('pickup-')]
        by_id = {row['id']: row for row in rows}
        self.assertEqual(len(rows), 5)
        self.assertEqual(by_id['pickup-pickup_arranged']['kind'], 'pickup_handoff')
        self.assertEqual(by_id['pickup-pickup_arranged']['priority'], 1)
        self.assertEqual(by_id['pickup-paid']['kind'], 'pickup_arrangement')
        self.assertEqual(by_id['pickup-collected']['kind'], 'reconciliation')
        text = json.dumps(rows).lower()
        for forbidden in ('label', 'carrier', 'scan', 'postage'):
            self.assertNotIn(forbidden, text)

    def test_private_order_map_is_validated_but_never_echoed(self):
        self.ledger['orders'] = [{'id': 'order-a', 'status': 'unpaid', 'lot_ids': ['lot-a']}]
        self.save()
        (self.root / 'private').mkdir()
        (self.root / 'private/orders.json').write_text(json.dumps({'schema_version': 1, 'orders': {
            'order-a': {'ricardo_order_id': '9876543210', 'tracking_number': 'PRIVATE_SENTINEL'}}}))
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        self.assertNotIn('PRIVATE_SENTINEL', json.dumps(result))
        self.assertNotIn('9876543210', json.dumps(result))
        self.assertFalse([e for e in result['structural_errors'] if 'private/orders.json' in e])
        (self.root / 'private/orders.json').write_text(json.dumps({'schema_version': 1, 'orders': {
            'order-a': {'ricardo_order_id': 'not-digits'}}}))
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        self.assertTrue(any('digit-only' in e for e in result['structural_errors']))
        self.assertNotIn('not-digits', json.dumps(result))

    def test_cash_at_handover_queue_arranges_then_takes_payment_at_pickup(self):
        self.ledger['orders'] = [
            {'id': 'cash-unpaid', 'fulfillment_method': 'pickup', 'payment_due_at_handover': True,
             'status': 'unpaid', 'lot_ids': ['lot-1']},
            {'id': 'cash-arranged', 'fulfillment_method': 'pickup', 'payment_due_at_handover': True,
             'status': 'pickup_arranged', 'lot_ids': ['lot-2'], 'pickup_by_at': '2031-01-02T12:00:00Z'},
            {'id': 'prepaid-unpaid', 'fulfillment_method': 'pickup', 'status': 'unpaid', 'lot_ids': ['lot-3']}]
        self.save()
        result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
        by_id = {row['id']: row for row in result['actions']}
        self.assertEqual(by_id['cash-unpaid']['kind'], 'pickup_arrangement')
        self.assertIn('due at handover', by_id['cash-unpaid']['action'])
        self.assertEqual(by_id['cash-arranged']['kind'], 'pickup_handoff')
        self.assertIn('take the payment', by_id['cash-arranged']['action'])
        self.assertEqual(by_id['prepaid-unpaid']['kind'], 'pickup_payment')
        text = json.dumps([by_id['cash-unpaid'], by_id['cash-arranged']]).lower()
        for forbidden in ('label', 'carrier', 'scan', 'postage'):
            self.assertNotIn(forbidden, text)

    def test_valid_pickup_records_pass_shared_validator_and_queue(self):
        for state in ('paid', 'pickup_arranged', 'collected', 'returned'):
            with self.subTest(state=state):
                self.ledger['units'] = [{'id': 'unit-pickup'}]
                lot_state = 'returned' if state == 'returned' else 'collected' if state == 'collected' else 'sold'
                self.ledger['lots'] = [{'id': 'lot-pickup', 'unit_ids': ['unit-pickup'], 'state': lot_state}]
                order = {'id': 'pickup-order', 'fulfillment_method': 'pickup', 'lot_ids': ['lot-pickup'],
                         'status': state, 'paid_at': '2031-01-01T00:00:00Z', 'paid_evidence': 'Verified paid order'}
                if state == 'pickup_arranged':
                    order.update(pickup_arranged_at='2031-01-01T01:00:00Z', pickup_arranged_evidence='Agreed collection time',
                                 pickup_by_at='2031-01-02T12:00:00Z', pickup_by_timezone='Etc/UTC')
                if state in ('collected', 'returned'):
                    order.update(collected_at='2031-01-01T12:00:00Z', collected_evidence='Supported marketplace pickup proof')
                if state == 'returned':
                    order.update(returned_at='2031-01-01T15:00:00Z', returned_evidence='Received and quarantined')
                self.ledger['orders'] = [order]
                self.save()
                result = MODULE['next_actions'](self.root, datetime(2031, 1, 2, tzinfo=timezone.utc))
                self.assertEqual(result['structural_errors'], [])
                self.assertEqual(len(result['actions']), 1)


if __name__ == '__main__':
    print(f'Retained synthetic fixtures: {FIXTURES}', flush=True)
    unittest.main(verbosity=2)
