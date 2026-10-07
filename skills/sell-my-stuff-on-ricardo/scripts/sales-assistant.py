#!/usr/bin/env python3
"""Read-only economics, work triage and listing worksheets for a Ricardo/Tutti sale. No account actions."""
import argparse
import json
import math
import runpy
import sys
from datetime import datetime, timezone, timedelta
from decimal import Decimal, DecimalException, InvalidOperation
from fractions import Fraction
from pathlib import Path

sys.dont_write_bytecode = True
VALIDATOR = runpy.run_path(str(Path(__file__).with_name('validate-project.py')),
                         run_name='sales_assistant_validator')
load = VALIDATOR['load']


def number(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, float, Decimal)):
        raise ValueError(f'{label}: finite nonnegative number required')
    try:
        result = Decimal(str(value))
    except InvalidOperation as exc:
        raise ValueError(f'{label}: invalid number') from exc
    if not result.is_finite() or result < 0:
        raise ValueError(f'{label}: finite nonnegative number required')
    return result


def required_text(value, label):
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f'{label}: nonempty string required')
    return value


def economics(data):
    if not isinstance(data, dict):
        raise ValueError('scenario input must be an object')
    # Swiss seller default. An absent currency is reported as defaulted; a present but empty one is an error.
    currency_defaulted = 'currency' not in data
    currency = 'CHF' if currency_defaulted else required_text(data.get('currency'), 'currency')
    floor = number(data.get('min_net'), 'min_net')
    precision = data.get('minor_unit_digits')
    if type(precision) is not int or not 0 <= precision <= 4:
        raise ValueError('minor_unit_digits: explicit integer from 0 to 4 required')
    quantum = Decimal(1).scaleb(-precision)
    scenarios = data.get('scenarios')
    if not isinstance(scenarios, list) or not scenarios:
        raise ValueError('scenarios: nonempty list required')
    results, names = [], set()
    for scenario in scenarios:
        if not isinstance(scenario, dict):
            raise ValueError('each scenario must be an object')
        name = required_text(scenario.get('name'), 'scenario name')
        if name in names:
            raise ValueError('scenario names must be unique')
        names.add(name)
        basis = required_text(scenario.get('basis'), name + ' basis')
        model = scenario.get('fee_model')
        if not isinstance(model, dict):
            raise ValueError(name + ': fee_model object required')
        for flag in ('includes_shipping', 'includes_tax'):
            if type(model.get(flag)) is not bool:
                raise ValueError(name + ': fee_model.' + flag + ' must be boolean')
        rate = number(model.get('rate'), name + ' fee rate')
        tax = number(model.get('tax_rate'), name + ' tax rate')
        if rate > 1 or tax > 1:
            raise ValueError(name + ': rates must be fractions between 0 and 1')
        fixed = number(model.get('fixed_fee'), name + ' fixed fee')
        n = {key: number(scenario.get(key), name + ' ' + key) for key in
             ('item_price', 'shipping_collected', 'postage', 'packing',
              'other_costs', 'risk_allowance')}
        # This model explicitly assumes one proportional tax multiplier on the
        # selected fee-bearing revenue. It is not a universal tax/fee schedule.
        with VALIDATOR['money_context']([floor, rate, tax, fixed, quantum, *n.values()]):
            multiplier = 1 + tax if model['includes_tax'] else Decimal(1)
            fee_shipping = n['shipping_collected'] if model['includes_shipping'] else Decimal(0)
            base = (n['item_price'] + fee_shipping) * multiplier
            fees = base * rate + fixed
            costs = sum(n[key] for key in ('postage', 'packing', 'other_costs', 'risk_allowance'))
            net = n['item_price'] + n['shipping_collected'] - fees - costs
            slope = 1 - rate * multiplier
            if slope <= 0:
                raise ValueError(name + ': fee model has no positive marginal proceeds; no price floor can be solved')
            numerator = floor + costs + fixed - n['shipping_collected'] + rate * multiplier * fee_shipping
            # Rational division and integer ceiling prevent even an arbitrarily
            # small positive remainder from rounding down to a currency increment.
            ticks = max(Fraction(0), Fraction(numerator) / Fraction(slope) / Fraction(quantum))
            rounded_minimum = Decimal(-(-ticks.numerator // ticks.denominator)) * quantum
            results.append({'name': name, 'basis': basis, 'modeled_fee_base': str(base),
                            'modeled_fees': str(fees), 'modeled_net': str(net),
                            'meets_net_floor': net >= floor,
                            'minimum_item_price_in_this_model': str(rounded_minimum),
                            'margin_above_floor': str(net - floor)})
    return {'currency': currency, 'currency_defaulted': currency_defaulted,
            'min_net': str(floor), 'scenarios': results,
            'all_scenarios_meet_floor': all(row['meets_net_floor'] for row in results),
            'proof_limit': 'Assumption-based arithmetic for one sale or shipment, not verified fees, a market valuation, a start-price or buy-now recommendation, or offer authority. Verify live marketplace fees and postage quotes and recompute for the actual destination, quantity and any tiered, capped or fixed fees.'}


def read_ledger(project, *, photo_scope=None):
    ledger = load(project / 'data/ledger.json')
    if not isinstance(ledger, dict):
        raise ValueError('ledger must be an object')
    errors = VALIDATOR['validate'](project, ledger, photo_scope=photo_scope)
    return ledger, errors


def rows(ledger, table):
    """Let the validator report malformed tables without crashing advisory output."""
    value = ledger.get(table)
    return value if isinstance(value, list) else []


def next_actions(project, now, freshness_hours=24, deadline_hours=24):
    if not isinstance(now, datetime) or now.tzinfo is None or now.utcoffset() is None:
        raise ValueError('now needs a UTC offset')
    if any(isinstance(value, bool) or not isinstance(value, (int, float))
           or not math.isfinite(value) or value <= 0 for value in (freshness_hours, deadline_hours)):
        raise ValueError('freshness and deadline windows must be positive and finite')
    ledger, errors = read_ledger(project)
    actions, coverage = [], {}

    def add(priority, kind, local_id, action):
        actions.append({'priority': priority, 'kind': kind, 'id': local_id, 'action': action})

    if errors:
        add(0, 'ledger_integrity', 'project', 'Reconcile structural errors before mutations; continue independently verifiable work.')
    for order in rows(ledger, 'orders'):
        if not isinstance(order, dict) or not isinstance(order.get('id'), str):
            continue
        oid, state = order['id'], order.get('status')
        method = order.get('fulfillment_method', 'shipping')
        if method not in ('shipping', 'pickup'):
            continue
        if order.get('fulfillment_method') == 'pickup':
            pickup_by = VALIDATOR['timestamp'](order.get('pickup_by_at'))
            pickup_late = pickup_by is not None and pickup_by <= now
            pickup_soon = pickup_by is not None and not pickup_late and pickup_by - now <= timedelta(hours=deadline_hours)
            at_handover = order.get('payment_due_at_handover') is True
            if state == 'unpaid' and at_handover:
                add(3, 'pickup_arrangement', oid, 'Payment is due at handover: arrange authorized pickup of the exact lot, agree how the buyer will pay, and do not treat the sale as paid until the seller observes the payment.')
            elif state == 'unpaid':
                add(4, 'pickup_payment', oid, 'Verify payment evidence (marketplace status, or the seller-reported receipt for direct payment) and any cancellation obligations before releasing the exact pickup lot.')
            elif state == 'paid':
                add(3, 'pickup_arrangement', oid, 'Arrange authorized pickup of the exact lot; record the agreed collection time and the handover and payment evidence the seller will actually observe.')
            elif state == 'pickup_arranged':
                add(0 if pickup_late else 1 if pickup_soon else 3, 'pickup_handoff', oid,
                    ('Agreed pickup deadline passed: ' if pickup_late else 'Agreed pickup is approaching: ' if pickup_soon else '') +
                    ('confirm the arrangement; at pickup take the payment as agreed, then record the seller-reported handover and the payment evidence together; do not mark collected or paid from scheduling alone.'
                     if at_handover else
                     'confirm the arrangement and record the seller-reported handover plus payment evidence at pickup; do not mark collected from scheduling alone.'))
            elif state == 'collected':
                add(6, 'reconciliation', oid, 'Reconcile recorded pickup proof, actual costs, open cases and separate money evidence.')
            elif state == 'returned':
                add(2, 'return_resolution', oid, 'Check pickup return case, receipt, condition and authorized refund/disposition; quarantine the exact returned units.')
            continue
        deadline = VALIDATOR['timestamp'](order.get('ship_by_at'))
        overdue = deadline is not None and deadline <= now
        due_soon = deadline is not None and not overdue and deadline - now <= timedelta(hours=deadline_hours)
        if state in ('paid', 'label_created'):
            add(0 if overdue else 1 if due_soon else 3, 'fulfillment', oid,
                ('Ship-by deadline passed: ' if overdue else 'Ship-by deadline approaching: ' if due_soon else '') +
                ('pack exact lot and arrange physical carrier handoff; reuse the existing label.'
                 if state == 'label_created' else 'prepare exact lot, arrange authorized postage and carrier handoff.'))
        elif state == 'handoff_reported':
            add(0 if overdue else 1 if due_soon else 4, 'scan_gap', oid,
                ('Ship-by deadline passed: ' if overdue else 'Ship-by deadline approaching: ' if due_soon else '') +
                'seller reported handoff; verify carrier evidence and any deadline exception. Do not buy another label or claim acceptance.')
        elif state in ('carrier_accepted', 'in_transit'):
            add(5, 'tracking', oid, 'Check detailed carrier status and exceptions; historical late handoff remains a separate fact.')
        elif state == 'returned':
            add(2, 'return_resolution', oid, 'Check case deadline, receipt, condition and authorized refund/disposition; keep returned units quarantined.')
        elif state == 'unpaid':
            add(4, 'payment', oid, 'Verify payment and any cancellation deadline; do not ship from an unpaid record.')
        elif state == 'delivered':
            add(6, 'reconciliation', oid, 'Reconcile actual costs, open cases and money evidence; delivery alone does not prove bank arrival or financial closure.')
    monitor_path = project / 'operations/monitor.json'
    if monitor_path.is_file():
        monitor = load(monitor_path)
        if not isinstance(monitor, dict) or not isinstance(monitor.get('surfaces', {}), dict):
            raise ValueError('monitor must be an object with a surfaces object')
        required = monitor.get('required_surfaces', ['orders', 'messages', 'offers', 'returns', 'tracking', 'account'])
        if not isinstance(required, list) or not required or any(not isinstance(name, str) or not name for name in required) or len(set(required)) != len(required):
            raise ValueError('monitor required_surfaces must be a nonempty unique string list')
        for surface in sorted(set(required) | set(monitor.get('surfaces', {}))):
            observation = monitor.get('surfaces', {}).get(surface)
            status = 'missing' if observation is None else 'invalid_record'
            if isinstance(observation, dict):
                status = observation.get('status', 'unknown')
                if not isinstance(status, str) or status not in {'checked', 'not_checked', 'failed', 'access_failed', 'unknown', 'inaccessible', 'stale'}:
                    status = 'invalid_status'
                checked = VALIDATOR['timestamp'](observation.get('checked_at'))
                if status == 'checked':
                    if checked is None or checked > now:
                        status = 'invalid_timestamp'
                    elif now - checked > timedelta(hours=freshness_hours):
                        status = 'stale'
                    elif not isinstance(observation.get('evidence_ref'), str) or not observation['evidence_ref'].strip():
                        status = 'missing_evidence'
            coverage[surface] = status
            if status != 'checked':
                add(2, 'access_gap', surface, 'Local coverage is ' + str(status) +
                    '; verify this surface live, preserve last verified state and report the exact gap.')
    for listing in rows(ledger, 'listings'):
        if not isinstance(listing, dict) or not isinstance(listing.get('id'), str):
            continue
        if listing.get('status') in ('local', 'draft'):
            blockers = listing.get('blockers')
            add(7 if blockers == [] else 8, 'listing', listing['id'],
                'Finish evidence and draft verification, then apply current publication authority.'
                if blockers == [] else 'Resolve only this listing’s recorded or unspecified blockers; advance other ready stock.')
    actions.sort(key=lambda row: (row['priority'], row['kind'], row['id']))
    return {'as_of': now.isoformat(), 'structural_errors': errors, 'actions': actions, 'local_coverage': coverage,
            'proof_limit': 'Queue derived from local records, not a live account check or a complete deadline inventory. Priority is advisory. Refresh messages, offers, cases and account surfaces before acting.'}


def listing_packet(project, local_id):
    ledger, errors = read_ledger(project, photo_scope=local_id)
    candidates = [row for row in rows(ledger, 'listings') if isinstance(row, dict)
                  and row.get('id') == local_id]
    if len(candidates) != 1:
        raise ValueError('exactly one listing must match --listing')
    listing = candidates[0]
    lots = [row for row in rows(ledger, 'lots') if isinstance(row, dict)
            and row.get('listing_ref') == local_id]
    photo_ids = listing.get('photo_ids')
    photos = [row for pid in (photo_ids if isinstance(photo_ids, list) else [])
              for row in rows(ledger, 'photos') if isinstance(row, dict) and row.get('id') == pid]
    checks = listing.get('checks', {})
    if not isinstance(checks, dict):
        checks = {}
    # Whitelist the output: do not dump arbitrary claims, customer records,
    # correspondence or evidence prose into a shareable listing worksheet.
    # listing_ref is the local record; marketplace/listing_id are the public channel identifiers once published.
    return {'listing_ref': local_id, 'marketplace': listing.get('marketplace'),
            'listing_id': listing.get('listing_id'), 'seller_confirmed': listing.get('seller_confirmed'),
            'recorded_status': listing.get('status'),
            'recorded_quantity': listing.get('quantity'),
            'allocations': [{key: row.get(key) for key in ('id', 'state', 'unit_ids')} for row in lots],
            'photos': [{key: row.get(key) for key in
                        ('id', 'path', 'sha256', 'approved_for_upload', 'listing_refs')} for row in photos],
            'checks_with_dated_evidence': {key: VALIDATOR['evidence'](checks.get(key)) for key in VALIDATOR['CHECKS']},
            'structural_errors': errors,
            'readiness_errors': VALIDATOR['validate'](project, ledger, local_id),
            'copy_sections_to_complete': ['Exact identity and quantity', 'Included and excluded contents',
                'Evidence-supported specifications', 'Visible condition and defects',
                'Owner reports versus actual tests', 'Format, start price and buy-now decision',
                'Shipping, pickup and payment', 'Returns statement'],
            'proof_limit': 'Internal worksheet only, not buyer-facing copy. Dated evidence is syntactically present, not independently verified or current. Inspect exact contents, photos, saved draft and the live listing in the seller account before action.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    econ = commands.add_parser('economics', help='compare explicit one-shipment scenarios')
    econ.add_argument('input', type=Path)
    actions = commands.add_parser('next-actions', help='derive advisory work queue from existing records')
    actions.add_argument('project', type=Path)
    actions.add_argument('--now', help='ISO timestamp with offset; defaults to actual UTC time')
    actions.add_argument('--freshness-hours', type=float, default=24)
    actions.add_argument('--deadline-hours', type=float, default=24)
    packet = commands.add_parser('listing-packet', help='emit a bounded internal worksheet, not sales copy')
    packet.add_argument('project', type=Path)
    packet.add_argument('--listing', required=True)
    args = parser.parse_args()
    try:
        if args.command == 'economics':
            result = economics(load(args.input))
        elif args.command == 'listing-packet':
            result = listing_packet(args.project, args.listing)
        else:
            now = VALIDATOR['timestamp'](args.now) if args.now else datetime.now(timezone.utc)
            if now is None:
                raise ValueError('--now requires an ISO timestamp with offset')
            result = next_actions(args.project, now, args.freshness_hours, args.deadline_hours)
        print(json.dumps(result, indent=2, ensure_ascii=False,
                         default=lambda value: str(value) if isinstance(value, Decimal) else value))
    except (OSError, ValueError, TypeError, KeyError, AttributeError, DecimalException, OverflowError) as exc:
        parser.exit(1, f'Invalid input: {exc}\n')


if __name__ == '__main__':
    main()
