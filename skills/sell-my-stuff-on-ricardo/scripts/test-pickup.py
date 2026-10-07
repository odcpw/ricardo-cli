#!/usr/bin/env python3
"""Exercise pickup lifecycle and shipment isolation with retained synthetic fixtures."""
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('validate-project.py')
VALIDATE = runpy.run_path(str(SCRIPT), run_name='pickup_validator_tests')['validate']
FIXTURES = Path(tempfile.mkdtemp(prefix='resale-pickup-tests-'))
PAID = '2031-06-03T10:00:00Z'
ARRANGED = '2031-06-03T11:00:00Z'
COLLECTED = '2031-06-04T12:00:00Z'
RETURNED = '2031-06-06T12:00:00Z'


class PickupTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='case-', dir=FIXTURES))
        self.ledger = {
            'schema_version': 1,
            'config': {'marketplace': 'tutti', 'currency': 'CHF', 'min_net': 0,
                       'packing_estimates_authorized': False},
            'units': [{'id': 'unit-a'}],
            'lots': [{'id': 'lot-a', 'unit_ids': ['unit-a'], 'state': 'sold'}],
            'photos': [], 'listings': [], 'orders': []}

    def order(self, status='paid'):
        order = {'id': 'order-a', 'lot_ids': ['lot-a'],
                 'fulfillment_method': 'pickup', 'status': status}
        if status not in {'unpaid', 'cancelled'}:
            order.update(paid_at=PAID, paid_evidence='Payment received per seller report; account details omitted')
        if status == 'pickup_arranged':
            order.update(pickup_arranged_at=ARRANGED,
                         pickup_arranged_evidence='Agreed collection window; details retained privately',
                         pickup_by_at='2031-06-04T14:00:00Z', pickup_by_timezone='Etc/UTC')
        if status in {'collected', 'returned'}:
            order.update(collected_at=COLLECTED,
                         collected_evidence='Seller-reported handover with payment evidence; private details omitted')
        if status == 'returned':
            order.update(returned_at=RETURNED, returned_evidence='Returned unit received; inspection pending')
        if status == 'cancelled':
            order['cancellation_disposition'] = {
                'verified_at': ARRANGED, 'basis': 'Cancellation verified and stock held for reconciliation'}
        self.ledger['lots'][0]['state'] = (
            'held' if status == 'cancelled' else
            status if status in {'collected', 'returned'} else 'sold')
        self.ledger['orders'] = [order]
        return order

    def check(self):
        return VALIDATE(self.root, self.ledger)

    def invalid(self, fragment):
        errors = self.check()
        self.assertTrue(any(fragment in error for error in errors), errors)

    def test_all_pickup_states_pass_with_truthful_evidence(self):
        for status in ('unpaid', 'paid', 'pickup_arranged', 'collected', 'returned', 'cancelled'):
            with self.subTest(status=status):
                self.order(status)
                self.assertEqual(self.check(), [])

    def test_payment_and_collection_proof_required(self):
        order = self.order('collected')
        for field in ('paid_at', 'paid_evidence', 'collected_at', 'collected_evidence'):
            with self.subTest(field=field):
                original = order.pop(field)
                self.invalid('needs offset timestamp and source evidence')
                order[field] = original
        order['collected_evidence'] = True
        self.invalid('collected needs')

    def test_no_invented_deadline_for_paid_or_observed_collection(self):
        self.order('paid')
        self.assertEqual(self.check(), [])
        self.order('collected')
        self.assertEqual(self.check(), [])

    def test_arranged_pickup_requires_agreement_and_valid_deadline(self):
        order = self.order('pickup_arranged')
        for field in ('pickup_arranged_at', 'pickup_arranged_evidence',
                      'pickup_by_at', 'pickup_by_timezone'):
            with self.subTest(field=field):
                original = order.pop(field)
                self.assertTrue(self.check())
                order[field] = original
        order['pickup_by_at'] = '2031-06-04T14:00:00+04:00'
        self.invalid('offset conflicts')
        order['pickup_by_timezone'] = 'Invalid/Zone'
        self.invalid('valid IANA')

    def test_retained_deadline_remains_validated_after_collection(self):
        order = self.order('collected')
        order.update(pickup_by_at='2031-06-04T14:00:00Z', pickup_by_timezone='Etc/UTC')
        self.assertEqual(self.check(), [])
        order['pickup_by_at'] = '2031-06-04T14:00:00'
        self.invalid('pickup_by_at requires')
        order['pickup_by_at'] = '2031-06-04T14:00:00Z'
        order.pop('pickup_by_timezone')
        self.invalid('valid IANA')

    def test_pickup_rejects_shipping_states_and_event_fields(self):
        for state in ('label_created', 'handoff_reported', 'carrier_accepted', 'in_transit', 'delivered'):
            with self.subTest(state=state):
                self.order()['status'] = state
                self.invalid('invalid order status')
        for field in ('label_created', 'handoff_reported', 'carrier_accepted', 'in_transit', 'delivered'):
            for suffix in ('_at', '_evidence'):
                with self.subTest(field=field + suffix):
                    self.order()[field + suffix] = COLLECTED
                    self.invalid('incompatible fulfillment')
        for field in ('ship_by_at', 'ship_by_timezone'):
            with self.subTest(field=field):
                self.order()[field] = None
                self.invalid('incompatible fulfillment')

    def test_method_defaults_shipping_and_hybrid_shipping_is_rejected(self):
        for method in ('omitted', 'shipping'):
            order = self.order()
            if method == 'omitted':
                order.pop('fulfillment_method')
            else:
                order['fulfillment_method'] = method
            order.update(ship_by_at='2031-06-04T14:00:00Z', ship_by_timezone='Etc/UTC')
            self.assertEqual(self.check(), [])
            order.update(collected_at=COLLECTED, collected_evidence='Not a carrier scan')
            self.invalid('incompatible fulfillment')
        order = self.order('collected')
        order.pop('fulfillment_method')
        self.invalid('invalid order status')
        for value in (None, True, [], 'courier', ''):
            with self.subTest(method=value):
                self.order()['fulfillment_method'] = value
                self.invalid('fulfillment_method must be')

    def test_chronology_and_status_cannot_hide_later_pickup_events(self):
        order = self.order('collected')
        order['collected_at'] = '2031-06-03T09:00:00Z'
        self.invalid('chronology')
        order = self.order('returned')
        order['returned_at'] = '2031-06-03T12:00:00Z'
        self.invalid('chronology')
        order = self.order('collected')
        order.update(pickup_arranged_at='2031-06-05T12:00:00Z',
                     pickup_arranged_evidence='Arrangement recorded too late')
        self.invalid('chronology')
        for status in ('unpaid', 'paid', 'pickup_arranged'):
            with self.subTest(status=status):
                order = self.order(status)
                order.update(collected_at=COLLECTED, collected_evidence='Collection proof')
                self.invalid('unpaid order has later' if status == 'unpaid' else 'status predates')
        order = self.order('paid')
        order['collected_evidence'] = 'Evidence without timestamp cannot hide a collection'
        self.invalid('collected needs')
        self.invalid('status predates')

    def test_stock_moves_only_after_collection_and_returns_stay_quarantined(self):
        self.order('pickup_arranged')
        self.ledger['lots'][0]['state'] = 'collected'
        self.invalid('must be sold')
        self.order('collected')
        self.ledger['lots'][0]['state'] = 'shipped'
        self.invalid('must be collected')
        order = self.order('returned')
        order.pop('collected_at')
        self.invalid('collected needs')
        self.order('returned')
        self.ledger['lots'][0]['state'] = 'held'
        self.invalid('must be returned')
        self.order('collected')
        self.ledger['orders'] = []
        self.invalid('lacks a current order')

    def test_orders_keep_exclusive_allocation_across_methods(self):
        self.order()
        duplicate = copy.deepcopy(self.ledger['orders'][0])
        duplicate.update(id='order-b', fulfillment_method='shipping',
                         ship_by_at='2031-06-04T14:00:00Z', ship_by_timezone='Etc/UTC')
        self.ledger['orders'].append(duplicate)
        self.invalid('another noncancelled order')
        self.ledger['orders'].pop()
        self.ledger['orders'][0]['lot_ids'].append('lot-a')
        self.invalid('another noncancelled order')

    def test_cancellation_requires_disposition_before_release(self):
        order = self.order('cancelled')
        order.pop('cancellation_disposition')
        self.invalid('cancellation disposition')
        self.order('cancelled')
        self.assertEqual(self.check(), [])
        self.ledger['lots'][0]['state'] = 'sold'
        self.invalid('lacks a current order')

    def handover_order(self, status='pickup_arranged'):
        """Cash (or other payment) taken at the handover: the sale is arranged before any payment."""
        order = {'id': 'order-a', 'lot_ids': ['lot-a'], 'fulfillment_method': 'pickup',
                 'payment_due_at_handover': True, 'status': status}
        if status in {'pickup_arranged', 'collected', 'returned'}:
            order.update(pickup_arranged_at=ARRANGED, pickup_arranged_evidence='Agreed collection window; details retained privately',
                         pickup_by_at='2031-06-04T14:00:00Z', pickup_by_timezone='Etc/UTC')
        if status in {'collected', 'returned'}:
            order.update(paid_at=COLLECTED, paid_evidence='Cash received at handover per seller report',
                         collected_at=COLLECTED, collected_evidence='Seller-reported handover; private details omitted')
        if status == 'returned':
            order.update(returned_at=RETURNED, returned_evidence='Returned unit received; inspection pending')
        self.ledger['lots'][0]['state'] = (status if status in {'collected', 'returned'} else 'sold')
        self.ledger['orders'] = [order]
        return order

    def test_cash_at_handover_allows_arrangement_before_payment(self):
        for status in ('unpaid', 'pickup_arranged', 'collected', 'returned'):
            with self.subTest(status=status):
                self.handover_order(status)
                self.assertEqual(self.check(), [])
        # The default stays pay-first: the same arranged order without the flag needs payment evidence.
        order = self.handover_order('pickup_arranged')
        del order['payment_due_at_handover']
        self.invalid('paid needs offset timestamp and source evidence')
        order['payment_due_at_handover'] = False
        self.invalid('paid needs offset timestamp and source evidence')

    def test_cash_at_handover_still_requires_payment_and_handover_evidence_at_collection(self):
        order = self.handover_order('collected')
        for field in ('paid_at', 'paid_evidence', 'collected_at', 'collected_evidence'):
            with self.subTest(field=field):
                original = order.pop(field)
                self.assertTrue(self.check())
                order[field] = original
        self.assertEqual(self.check(), [])
        order['paid_evidence'] = True
        self.invalid('paid needs')

    def test_cash_at_handover_contradictions_are_rejected(self):
        order = self.handover_order('pickup_arranged')
        order.update(paid_at=ARRANGED, paid_evidence='Cash already received')
        self.invalid('status predates')
        order = self.handover_order('pickup_arranged')
        order['status'] = 'paid'
        self.invalid('contradicts status paid')
        order = self.handover_order('collected')
        order['paid_at'] = '2031-06-03T09:00:00Z'  # before the arrangement
        self.invalid('paid chronology')
        order = self.handover_order('collected')
        order['payment_due_at_handover'] = 'true'
        self.invalid('payment_due_at_handover must be boolean')
        order = self.handover_order('pickup_arranged')
        order.update(fulfillment_method='shipping', ship_by_at='2031-06-04T14:00:00Z', ship_by_timezone='Etc/UTC')
        for field in ('pickup_arranged_at', 'pickup_arranged_evidence', 'pickup_by_at', 'pickup_by_timezone'):
            order.pop(field)
        order['status'] = 'paid'
        order.update(paid_at=PAID, paid_evidence='Payment confirmed')
        self.invalid('only valid for pickup orders')

    def test_cash_at_handover_collection_may_be_simultaneous_with_payment_in_either_order(self):
        order = self.handover_order('collected')
        order['paid_at'] = '2031-06-04T12:00:01Z'
        self.assertEqual(self.check(), [])
        order['paid_at'] = '2031-06-04T11:59:59Z'
        self.assertEqual(self.check(), [])

    def test_collection_does_not_establish_bank_arrival(self):
        order = self.order('collected')
        order['bank_arrived'] = True
        self.invalid('bank_arrived needs')
        self.invalid('lacks payout-sent')
        order.update(payout_sent_at='2031-06-05T10:00:00Z', payout_sent_evidence='Transfer sent',
                     bank_arrived_at='2031-06-06T10:00:00Z', bank_arrived_evidence='Deposit confirmed')
        self.assertEqual(self.check(), [])
        order['bank_arrived_at'] = '2031-06-04T10:00:00Z'
        self.invalid('precedes payout')

    def test_real_cli_validates_pickup_and_rejects_missing_collection_proof(self):
        order = self.order('collected')
        (self.root / 'data').mkdir()
        path = self.root / 'data/ledger.json'
        for valid in (True, False):
            with self.subTest(valid=valid):
                if not valid:
                    order.pop('collected_evidence')
                path.write_text(json.dumps(self.ledger))
                result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(self.root)],
                                        capture_output=True, text=True, check=False, timeout=10)
                self.assertEqual(result.returncode, 0 if valid else 1, result.stdout + result.stderr)
                self.assertIn('PASS:' if valid else 'collected needs', result.stdout)


if __name__ == '__main__':
    print(f'Retained synthetic fixtures: {FIXTURES}', flush=True)
    unittest.main(verbosity=2)
