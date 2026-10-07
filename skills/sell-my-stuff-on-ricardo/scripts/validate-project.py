#!/usr/bin/env python3
"""Read-only structural checks for a Ricardo/Tutti sales ledger; never substitutes for product, UI or policy review."""
import argparse
import hashlib
import json
import re
from datetime import datetime
from decimal import Context, Decimal, DecimalException, Inexact, InvalidOperation, localcontext
from pathlib import Path
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

MARKETPLACES = {'ricardo', 'tutti'}
LOT_STATES = {'draft_reserved', 'listed', 'sold', 'shipped', 'collected', 'returned', 'held'}
SHIPPING_STATES = {'unpaid', 'paid', 'label_created', 'handoff_reported',
                   'carrier_accepted', 'in_transit', 'delivered', 'returned', 'cancelled'}
PICKUP_STATES = {'unpaid', 'paid', 'pickup_arranged', 'collected', 'returned', 'cancelled'}
ORDER_STATES = SHIPPING_STATES | PICKUP_STATES
SHIPPING_EVENTS = ('label_created', 'handoff_reported', 'carrier_accepted', 'in_transit', 'delivered')
PICKUP_EVENTS = ('pickup_arranged', 'collected')
CHECKS = ('identity_contents', 'condition_claims', 'photo_match', 'category_attributes',
          'account_limits', 'notification_access', 'packing_supplies',
          'shipping_returns_handling', 'offers_promotions', 'draft_reopened',
          'buyer_preview', 'publication_authority')
# Fields that belong in the ignored private/orders.json map, never in the tracked ledger.
PRIVATE_ORDER_FIELDS = ('ricardo_order_id', 'order_id', 'tracking_number', 'tracking_url',
                        'unit_price_chf', 'shipping_cost_chf', 'total_price_chf',
                        'seller_success_fee_chf', 'seller_success_fee_rate',
                        'buyer', 'buyer_name', 'buyer_ref', 'address', 'shipping_address',
                        'iban', 'payment_reference')
PRIVATE_ORDER_AMOUNTS = ('unit_price_chf', 'shipping_cost_chf', 'total_price_chf',
                         'seller_success_fee_chf', 'seller_success_fee_rate')
COST_FIELDS = ('item_price', 'shipping_collected', 'fee_base', 'fee_rate',
               'order_fee', 'postage', 'packing', 'other_costs', 'risk_allowance')


def load(path):
    """One exact, ambiguity-rejecting JSON reader shared by ledger consumers."""
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate JSON key: ' + key)
            result[key] = value
        return result

    def constant(value):
        raise ValueError('nonfinite JSON number: ' + value)

    return json.loads(path.read_text(encoding='utf-8'), parse_float=Decimal,
                      object_pairs_hook=pairs, parse_constant=constant)


def text(value):
    return isinstance(value, str) and bool(value.strip())


def strings(value, nonempty=True):
    return (isinstance(value, list) and (bool(value) or not nonempty)
            and all(text(item) for item in value))


def one_of(value, choices):
    return isinstance(value, str) and value in choices


def listing_identifier(marketplace, value):
    """Marketplace-specific public listing identifier; never a local reference."""
    if not isinstance(value, str):
        return False
    if marketplace == 'ricardo':
        return re.fullmatch(r'[0-9]{6,15}', value) is not None
    if marketplace == 'tutti':
        # Canonical German detail URL, no query/fragment: /de/vi/<2-6 segments>/<numeric id>.
        if not re.fullmatch(r'https://www\.tutti\.ch/de/vi/(?:(?:[A-Za-z0-9_~.-]|%[0-9A-Fa-f]{2})+/){2,6}[0-9]{1,20}', value):
            return False
        segments = value.split('/')[5:]
        return not any(part in ('.', '..') for part in segments) and not re.search(r'%(?:2e|2f|5c)', value, re.I)
    return False


def listing_key(marketplace, value):
    """Identity of an ad for uniqueness: the numeric ID, so slug or percent-encoding variants collide."""
    if marketplace == 'tutti':
        return (marketplace, str(int(value.rsplit('/', 1)[-1])))
    return (marketplace, value)


def timestamp(value):
    if not text(value) or not re.fullmatch(
            r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)', value):
        return None
    try:
        return datetime.fromisoformat(value.replace('Z', '+00:00'))
    except ValueError:
        return None


def evidence(value):
    return (isinstance(value, dict) and timestamp(value.get('verified_at')) is not None
            and text(value.get('basis')))


def money(value):
    if isinstance(value, bool) or not isinstance(value, (int, float, Decimal)):
        raise ValueError('missing or invalid numeric amount')
    try:
        number = Decimal(str(value))
    except InvalidOperation as exc:
        raise ValueError('invalid numeric amount') from exc
    if not number.is_finite() or number < 0:
        raise ValueError('amount must be finite and nonnegative')
    return number


def money_context(values):
    """Budget exact finite arithmetic independently of the caller's decimal context."""
    # Addition/cancellation needs the full exponent span; products need the sum
    # of operand widths. This conservative budget covers the bounded expressions
    # below and in the scenario calculator. Refuse oversized input before work.
    width = sum(len(value.as_tuple().digits) + abs(value.as_tuple().exponent) + 2
                for value in values)
    if width > 10000:
        raise ValueError('monetary digit/exponent budget exceeds 10000')
    context = Context(prec=max(28, 2 * width + 16))
    context.traps[Inexact] = True
    return localcontext(context)


def expected_net(economics):
    if not isinstance(economics, dict):
        raise ValueError('economics must be an object')
    n = {key: money(economics.get(key)) for key in COST_FIELDS}
    if n['fee_rate'] > 1:
        raise ValueError('fee_rate must be a fraction from 0 to 1')
    with money_context(n.values()):
        return (n['item_price'] + n['shipping_collected'] - n['fee_base'] * n['fee_rate']
                - sum(n[key] for key in ('order_fee', 'postage', 'packing',
                                        'other_costs', 'risk_allowance')))


def check_private_orders(root, orders, errors):
    """Validate the optional, ignored private/orders.json map; never required, never tracked."""
    path = root / 'private/orders.json'
    if not path.exists():
        return
    try:
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError('must be a regular file inside the workspace')
        data = load(path)
    except (OSError, ValueError, TypeError) as exc:
        errors.append(f'private/orders.json: unreadable: {exc}')
        return
    entries = data.get('orders') if isinstance(data, dict) else None
    if type(data.get('schema_version') if isinstance(data, dict) else None) is not int or not isinstance(entries, dict):
        errors.append('private/orders.json: expected {"schema_version": 1, "orders": {<local order id>: {...}}}')
        return
    seen = {}
    for ref, entry in entries.items():
        if ref not in orders:
            errors.append(f'private/orders.json: unknown local order id {ref}')
        if not isinstance(entry, dict):
            errors.append(f'private/orders.json: {ref}: entry must be an object')
            continue
        if 'ricardo_order_id' in entry:
            rid = entry['ricardo_order_id']
            if not isinstance(rid, str) or not re.fullmatch(r'[0-9]{1,20}', rid):
                errors.append(f'private/orders.json: {ref}: ricardo_order_id must be a digit-only string')
            elif rid in seen:
                errors.append(f'private/orders.json: {ref}: ricardo_order_id also belongs to {seen[rid]}')
            else:
                seen[rid] = ref
        for field in ('tracking_number', 'tracking_url'):
            if field in entry and not text(entry[field]):
                errors.append(f'private/orders.json: {ref}: {field} must be a nonempty string')
        for field in PRIVATE_ORDER_AMOUNTS:
            if field in entry:
                try:
                    money(entry[field])
                except ValueError as exc:
                    errors.append(f'private/orders.json: {ref}: {field}: {exc}')


def validate(root, ledger, target=None, *, photo_scope=None):
    errors = []
    root = root.resolve()
    if not isinstance(ledger, dict):
        return ['ledger must be an object']
    if type(ledger.get('schema_version')) is not int or ledger.get('schema_version') != 1:
        errors.append('unsupported schema_version')
    config = ledger.get('config')
    if not isinstance(config, dict):
        errors.append('config: must be an object')
        config = {}
    if type(config.get('packing_estimates_authorized')) is not bool:
        errors.append('config: packing_estimates_authorized must be boolean')
    if config.get('marketplace') is not None and not one_of(config.get('marketplace'), MARKETPLACES):
        errors.append('config: marketplace must be ricardo or tutti')
    tables = {}
    for table in ('units', 'lots', 'photos', 'listings', 'orders'):
        rows = ledger.get(table)
        if not isinstance(rows, list):
            errors.append(f'{table}: must be a list')
            rows = []
        index = {}
        for row in rows:
            if not isinstance(row, dict) or not text(row.get('id')):
                errors.append(f'{table}: every row needs a nonempty string id')
                continue
            if row['id'] in index:
                errors.append(f'{table}: duplicate id {row["id"]}')
            index[row['id']] = row
        tables[table] = index
    units, lots, photos, listings, orders = (tables[t] for t in tables)
    for uid, unit in units.items():
        if 'excluded' in unit and type(unit['excluded']) is not bool:
            errors.append(f'{uid}: excluded must be boolean')
        if not one_of(unit.get('disposition', 'eligible'), {'eligible', 'held', 'excluded', 'disposed'}):
            errors.append(f'{uid}: invalid disposition')
    used = {}
    for lot_id, lot in lots.items():
        if not one_of(lot.get('state'), LOT_STATES):
            errors.append(f'{lot_id}: invalid lot state')
        members = lot.get('unit_ids')
        if not strings(members):
            errors.append(f'{lot_id}: nonempty unit_ids string list required')
            continue
        for uid in members:
            if uid not in units:
                errors.append(f'{lot_id}: unknown unit {uid}')
            elif units[uid].get('excluded') is True or one_of(units[uid].get('disposition'), {'excluded', 'disposed'}):
                errors.append(f'{lot_id}: excluded/disposed unit {uid} allocated')
            elif units[uid].get('disposition') == 'held' and lot.get('state') != 'held':
                errors.append(f'{lot_id}: held unit {uid} allocated to sale')
            if uid in used:
                errors.append(f'{lot_id}: duplicate allocation of {uid} also in {used[uid]}')
            used[uid] = lot_id
        link = lot.get('listing_ref')
        if link is not None and (not text(link) or link not in listings):
            errors.append(f'{lot_id}: unknown listing link')
        if one_of(lot.get('state'), {'draft_reserved', 'listed'}):
            linked = listings.get(link) if text(link) else None
            wanted = {'draft', 'local'} if lot['state'] == 'draft_reserved' else {'active'}
            if linked is None or not one_of(linked.get('status'), wanted):
                errors.append(f'{lot_id}: incompatible or missing listing link')
    photo_scope = target or photo_scope
    target_photos = listings.get(photo_scope, {}).get('photo_ids', []) if photo_scope else []
    for pid, photo in photos.items():
        # A held item's damaged derivative must not block unrelated launch work.
        if photo_scope and (not strings(target_photos) or pid not in target_photos):
            continue
        if type(photo.get('approved_for_upload')) is not bool:
            errors.append(f'{pid}: approved_for_upload must be boolean')
        if not strings(photo.get('listing_refs'), nonempty=False):
            errors.append(f'{pid}: listing_refs must be a string list')
        elif any(lid not in listings for lid in photo['listing_refs']):
            errors.append(f'{pid}: unknown authorized listing')
        if photo.get('approved_for_upload') is not True:
            continue
        relative = photo.get('path')
        if not text(relative) or Path(relative).is_absolute():
            errors.append(f'{pid}: photo path required')
            continue
        path = (root / relative).resolve()
        approved_root = (root / 'photos/listing').resolve()
        if not approved_root.is_relative_to(root) or not path.is_relative_to(approved_root):
            errors.append(f'{pid}: approved photo must remain inside photos/listing')
        elif not path.is_file():
            errors.append(f'{pid}: missing photo')
        # ubs:ignore -- Public product-photo integrity digest, not a secret authenticator.
        elif hashlib.sha256(path.read_bytes()).hexdigest() != photo.get('sha256'):
            errors.append(f'{pid}: reviewed photo hash changed')
    external_ids = {'draft_id': {}, 'listing_id': {}}
    for lid, listing in listings.items():
        status = listing.get('status')
        if not one_of(status, {'local', 'draft', 'active', 'ended', 'held'}):
            errors.append(f'{lid}: invalid listing status')
            continue
        stock_state = 'listed' if status == 'active' else 'draft_reserved'
        current = [lot for lot in lots.values() if lot.get('listing_ref') == lid
                   and lot.get('state') == stock_state]
        quantity = listing.get('quantity')
        if status in {'draft', 'active'} and (type(quantity) is not int or quantity != len(current)):
            errors.append(f'{lid}: quantity differs from reserved/listed purchasable lots')
        marketplace = listing.get('marketplace')
        if 'marketplace' in listing and not one_of(marketplace, MARKETPLACES):
            errors.append(f'{lid}: marketplace must be ricardo or tutti')
            marketplace = None
        elif status in {'draft', 'active'} and marketplace is None:
            errors.append(f'{lid}: draft/active listing needs marketplace (ricardo or tutti)')
        for field, seen in external_ids.items():
            if field in listing:
                value = listing[field]
                if not text(value):
                    errors.append(f'{lid}: {field} must be a nonempty string')
                    continue
                if field == 'listing_id' and marketplace is not None and not listing_identifier(marketplace, value):
                    errors.append(f'{lid}: listing_id is not a valid {marketplace} identifier '
                                  '(Ricardo: 6-15 digits; Tutti: canonical https://www.tutti.ch/de/vi/... URL)')
                    continue
                if field == 'listing_id' and marketplace is None:
                    errors.append(f'{lid}: listing_id requires a valid marketplace')
                    continue
                key = listing_key(marketplace, value) if field == 'listing_id' else (marketplace, value)
                if key in seen:
                    errors.append(f'{lid}: {field} also belongs to {seen[key]}')
                else:
                    seen[key] = lid
        if status == 'active' and not (marketplace is not None and listing_identifier(marketplace, listing.get('listing_id'))
                                       and timestamp(listing.get('active_verified_at'))):
            errors.append(f'{lid}: active listing lacks verified marketplace listing ID/time')
        if status == 'active' and not text(listing.get('active_evidence')):
            errors.append(f'{lid}: active account/page evidence missing')
        if 'seller_confirmed' in listing and type(listing['seller_confirmed']) is not bool:
            errors.append(f'{lid}: seller_confirmed must be boolean')
        elif status == 'active' and marketplace == 'tutti' and 'seller_confirmed' not in listing:
            # A Tutti reading of "listed" is page-declared availability; say whether the seller confirmed it.
            errors.append(f'{lid}: active tutti listing needs boolean seller_confirmed '
                          '(false when only the page reading is evidence)')
    ordered = {}
    check_private_orders(root, orders, errors)
    for oid, order in orders.items():
        for field in PRIVATE_ORDER_FIELDS:
            if field in order:
                errors.append(f'{oid}: {field} is private order data; keep it in private/orders.json '
                              'keyed by this local order id, not in the tracked ledger')
        if 'payment_due_at_handover' in order and (
                type(order['payment_due_at_handover']) is not bool
                or order.get('fulfillment_method') != 'pickup'):
            errors.append(f'{oid}: payment_due_at_handover must be boolean and is only valid for pickup orders')
        method = order.get('fulfillment_method', 'shipping')
        if not one_of(method, {'shipping', 'pickup'}):
            errors.append(f'{oid}: fulfillment_method must be shipping or pickup')
            continue
        allowed_states = PICKUP_STATES if method == 'pickup' else SHIPPING_STATES
        if not one_of(order.get('status'), allowed_states):
            errors.append(f'{oid}: invalid order status')
            continue
        incompatible_events = SHIPPING_EVENTS if method == 'pickup' else PICKUP_EVENTS
        incompatible_deadline = 'ship_by' if method == 'pickup' else 'pickup_by'
        incompatible_fields = [field + suffix for field in incompatible_events
                               for suffix in ('_at', '_evidence')]
        incompatible_fields.extend(incompatible_deadline + suffix for suffix in ('_at', '_timezone'))
        if any(field in order for field in incompatible_fields):
            errors.append(f'{oid}: {method} order contains incompatible fulfillment evidence or deadline')
        members = order.get('lot_ids', [])
        if not strings(members):
            errors.append(f'{oid}: nonempty lot_ids string list required')
            continue
        for lot_id in members:
            if lot_id not in lots:
                errors.append(f'{oid}: unknown lot {lot_id}')
                continue
            if order.get('status') != 'cancelled':
                if lot_id in ordered:
                    errors.append(f'{oid}: lot {lot_id} also in another noncancelled order')
                ordered[lot_id] = oid
                wanted = ('returned' if order.get('status') == 'returned' else
                          'collected' if order.get('status') == 'collected' else
                          'shipped' if order.get('status') in {'carrier_accepted','in_transit','delivered'} else 'sold')
                if lots[lot_id].get('state') != wanted:
                    errors.append(f'{oid}: {lot_id} must be {wanted} for this order state')
        status = order.get('status')
        # Cash (or other payment) at handover: arrangement may precede payment; payment is
        # then required at collection together with the handover report.
        at_handover = method == 'pickup' and order.get('payment_due_at_handover') is True
        if at_handover and status == 'paid':
            errors.append(f'{oid}: payment_due_at_handover contradicts status paid; '
                          'use pickup_arranged until the handover, or clear the flag if payment was received earlier')
        fields = (('pickup_arranged', 'paid', 'collected', 'returned') if at_handover else
                  ('paid', *PICKUP_EVENTS, 'returned') if method == 'pickup' else
                  ('paid', *SHIPPING_EVENTS, 'returned'))
        required = set()
        if status not in {'unpaid', 'cancelled'} and not (at_handover and status == 'pickup_arranged'):
            required.add('paid')
        if status in {'label_created', 'handoff_reported'}:
            required.add('label_created')
        if status == 'handoff_reported':
            required.add('handoff_reported')
        if method == 'shipping' and status in {'carrier_accepted', 'in_transit', 'delivered', 'returned'}:
            required.add('carrier_accepted')
        if status in {'in_transit', 'delivered', 'returned'}:
            required.add(status)
        if method == 'pickup' and status == 'pickup_arranged':
            required.add('pickup_arranged')
        if method == 'pickup' and status in {'collected', 'returned'}:
            required.add('collected')
        previous = None
        stamps = {}
        for field in fields:
            value = order.get(field + '_at')
            present = field + '_at' in order or field + '_evidence' in order
            if field in required or present:
                parsed = timestamp(value)
                if parsed is None or not text(order.get(field + '_evidence')):
                    errors.append(f'{oid}: {field} needs offset timestamp and source evidence')
                if parsed is not None:
                    stamps[field] = parsed
                    # Payment at handover happens with collection: it is not ordered against it.
                    if at_handover and field == 'paid':
                        continue
                    if previous is not None and parsed < previous:
                        errors.append(f'{oid}: {field} chronology is inconsistent')
                    previous = parsed
        if at_handover and 'paid' in stamps:
            if 'pickup_arranged' in stamps and stamps['paid'] < stamps['pickup_arranged']:
                errors.append(f'{oid}: paid chronology is inconsistent')
            if 'returned' in stamps and stamps['returned'] < stamps['paid']:
                errors.append(f'{oid}: returned chronology is inconsistent')
        def event_present(field):
            return field + '_at' in order or field + '_evidence' in order

        if status == 'unpaid' and any(event_present(field) for field in fields):
            errors.append(f'{oid}: unpaid order has later payment/shipment events')
        if status in fields:
            later = fields[fields.index(status) + 1:]
            if any(event_present(field) for field in later):
                errors.append(f'{oid}: order status predates recorded shipment events')
        deadline_field = 'pickup_by' if method == 'pickup' else 'ship_by'
        deadline_required = (status == 'pickup_arranged' if method == 'pickup'
                             else status not in {'unpaid', 'cancelled'})
        deadline_supplied = (method == 'pickup' and any(
            deadline_field + suffix in order for suffix in ('_at', '_timezone')))
        if deadline_required or deadline_supplied:
            if timestamp(order.get(deadline_field + '_at')) is None:
                errors.append(f'{oid}: {deadline_field}_at requires an offset timestamp')
            try:
                if not text(order.get(deadline_field + '_timezone')):
                    raise ValueError('missing timezone')
                zone = ZoneInfo(order[deadline_field + '_timezone'])
                deadline = timestamp(order.get(deadline_field + '_at'))
                if deadline is not None and deadline.utcoffset() != deadline.astimezone(zone).utcoffset():
                    errors.append(f'{oid}: deadline offset conflicts with {deadline_field}_timezone')
            except (ValueError, ZoneInfoNotFoundError):
                errors.append(f'{oid}: valid IANA {deadline_field}_timezone required')
        if status == 'cancelled' and not evidence(order.get('cancellation_disposition')):
            errors.append(f'{oid}: cancellation disposition needs dated evidence')
        if 'bank_arrived' in order and type(order['bank_arrived']) is not bool:
            errors.append(f'{oid}: bank_arrived must be boolean')
        if 'bank_arrived_at' in order and order.get('bank_arrived') is not True:
            errors.append(f'{oid}: deposit event requires bank_arrived true')
        financial = []
        for field in ('payout_sent', 'bank_arrived'):
            if (field + '_at' in order or field + '_evidence' in order
                    or (field == 'bank_arrived' and order.get('bank_arrived') is True)):
                parsed = timestamp(order.get(field + '_at'))
                if parsed is None or not text(order.get(field + '_evidence')):
                    errors.append(f'{oid}: {field} needs offset timestamp and separate evidence')
                if parsed is not None:
                    financial.append(parsed)
        if order.get('bank_arrived') is True and timestamp(order.get('payout_sent_at')) is None:
            errors.append(f'{oid}: bank arrival lacks payout-sent evidence')
        if len(financial) == 2 and financial[1] < financial[0]:
            errors.append(f'{oid}: bank arrival precedes payout')
    for lot_id, lot in lots.items():
        if one_of(lot.get('state'), {'sold', 'shipped', 'collected', 'returned'}) and lot_id not in ordered:
            errors.append(f'{lot_id}: sale/shipment/return state lacks a current order')
    if target:
        if target not in listings:
            errors.append(f'{target}: unknown listing')
        else:
            listing = listings[target]
            if not one_of(listing.get('status'), {'draft','active'}) or not text(listing.get('draft_id')):
                errors.append(f'{target}: requires a real saved draft')
            if type(listing.get('quantity')) is not int or listing['quantity'] < 1:
                errors.append(f'{target}: no purchasable stock')
            checks = listing.get('checks')
            if not isinstance(checks, dict):
                checks = {}
            for check in CHECKS:
                if not evidence(checks.get(check)):
                    errors.append(f'{target}: missing dated evidence for {check}')
            if listing.get('blockers') != []:
                errors.append(f'{target}: unresolved or unspecified blockers')
            ids = listing.get('photo_ids')
            if not strings(ids):
                errors.append(f'{target}: approved photo_ids required')
            else:
                if len(set(ids)) != len(ids):
                    errors.append(f'{target}: unique photo_ids required')
                for pid in ids:
                    if (pid not in photos or photos[pid].get('approved_for_upload') is not True
                            or not strings(photos[pid].get('listing_refs'))
                            or target not in photos[pid]['listing_refs']):
                        errors.append(f'{target}: photo {pid} not approved for this listing')
            economics = listing.get('economics', {})
            if not isinstance(economics, dict):
                economics = {}
            try:
                net = expected_net(economics)
                if net < money(config.get('min_net')):
                    errors.append(f'{target}: expected net {net} is below minimum')
            except (ValueError, TypeError, DecimalException) as exc:
                errors.append(f'{target}: incomplete economics: {exc}')
            if not text(config.get('currency')) or not one_of(config.get('marketplace'), MARKETPLACES):
                errors.append(f'{target}: currency and marketplace (ricardo or tutti) required')
            if not evidence(economics.get('fee_basis')) or not evidence(economics.get('packing_basis')):
                errors.append(f'{target}: fee and packing evidence basis required')
            if type(economics.get('packing_estimated')) is not bool:
                errors.append(f'{target}: packing_estimated must be boolean')
            if economics.get('packing_estimated') is True and config.get('packing_estimates_authorized') is not True:
                errors.append(f'{target}: estimates not authorized')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('project', type=Path)
    parser.add_argument('--listing', help='also check this listing for launch readiness')
    args = parser.parse_args()
    try:
        ledger = load(args.project / 'data/ledger.json')
        if not isinstance(ledger, dict):
            raise ValueError('ledger root must be an object')
        errors = validate(args.project, ledger, args.listing)
    except (OSError, ValueError, TypeError, AttributeError, DecimalException) as exc:
        parser.exit(1, f'Invalid workspace: {exc}\n')
    for error in errors:
        print(f'FAIL: {error}')
    if errors:
        raise SystemExit(1)
    print('PASS: structural checks only; visual, policy and live-account verification remain required.')


if __name__ == '__main__':
    main()
