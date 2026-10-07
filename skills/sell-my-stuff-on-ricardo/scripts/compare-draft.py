#!/usr/bin/env python3
"""Compare intended Ricardo/Tutti listing fields with fresh, identity-bound UI receipts; read-only."""
import argparse
import json
import math
import re
from datetime import datetime, timezone, timedelta
from decimal import Decimal, DecimalException
from pathlib import Path

# price is the start price for auction formats and the fixed asking price for buy_now/classified;
# buy_now_price is the extra buy-now price of auction_with_buy_now and the string "none" otherwise.
# buyer_offers (Preisvorschlag: buyer price proposals) exists only on fixed-price listings, so it is a
# material field exactly when the intended format is in FIXED_PRICE_FORMATS and is an unknown field otherwise.
BASE_MATERIAL = {'title', 'quantity', 'price', 'currency', 'format', 'buy_now_price', 'duration',
                 'condition', 'photos', 'description', 'category', 'category_attributes',
                 'shipping', 'handling', 'payment', 'returns', 'promotions'}
FIXED_PRICE_FORMATS = {'buy_now'}
PREVIEW_REQUIRED_VISIBLE = {'title', 'quantity', 'price', 'currency', 'photos'}
FORMATS = {'auction', 'buy_now', 'auction_with_buy_now', 'classified'}
TEXT_FIELDS = {'title', 'currency', 'condition', 'description', 'category'}
SETTING_FIELDS = {'shipping', 'handling', 'payment', 'returns'}
BOOLEAN_FIELDS = {'buyer_offers', 'promotions'}


def material_fields(expected):
    """The material field set for the intended format; shape errors surface in compare()."""
    fixed = isinstance(expected, dict) and expected.get('format') in FIXED_PRICE_FORMATS
    return BASE_MATERIAL | ({'buyer_offers'} if fixed else set())


def stamp(value):
    if not isinstance(value, str) or not re.fullmatch(
            r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)', value):
        raise ValueError('timestamps need ISO seconds and UTC offset')
    return datetime.fromisoformat(value.replace('Z', '+00:00'))


def identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,119}', value):
        raise ValueError('IDs and evidence references must be opaque nonempty identifiers')
    return value


def shape(value, allowed, required):
    if not isinstance(value, dict) or set(value) - allowed or required - set(value):
        raise ValueError('unknown or missing fields; required: ' + ', '.join(sorted(required)))


def equal(left, right):
    # Python considers False == 0; marketplace switches must not inherit that rule.
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is bool and type(right) is bool and left == right
    if isinstance(left, (int, Decimal)) and isinstance(right, (int, Decimal)):
        return left == right
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(equal(left[key], right[key]) for key in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(equal(a, b) for a, b in zip(left, right))
    return left == right


def compare(packet, now, max_age_hours=24):
    shape(packet, {'listing_ref', 'draft_id', 'stage', 'expected', 'identity', 'observed', 'not_shown'},
          {'listing_ref', 'draft_id', 'stage', 'expected', 'identity', 'observed'})
    for key in ('listing_ref', 'draft_id'):
        identifier(packet[key])
    if packet['stage'] not in ('reopened', 'preview'):
        raise ValueError('stage must be reopened or preview')
    if not isinstance(now, datetime) or now.tzinfo is None or now.utcoffset() is None:
        raise ValueError('now needs a UTC offset')
    if isinstance(max_age_hours, bool) or not isinstance(max_age_hours, (int, float)) or not math.isfinite(max_age_hours) or max_age_hours <= 0:
        raise ValueError('max_age_hours must be positive and finite')
    expected, identity, observed = packet['expected'], packet['identity'], packet['observed']
    material = material_fields(expected)
    shape(expected, material, material)
    shape(identity, {'listing_ref', 'draft_id'}, set())
    shape(observed, material, set())
    not_shown = packet.get('not_shown', {})
    shape(not_shown, material - PREVIEW_REQUIRED_VISIBLE, set())
    if not_shown and packet['stage'] != 'preview':
        raise ValueError('not_shown exemptions are only valid for the preview stage')
    if set(not_shown) & set(observed):
        raise ValueError('a field cannot have both observed and not_shown receipts')
    for key, value in expected.items():
        if value is None:
            raise ValueError('expected values cannot be null')
        if key in TEXT_FIELDS and (not isinstance(value, str) or not value.strip()):
            raise ValueError(key + ': nonempty expected text required')
        if key == 'category_attributes' and not isinstance(value, dict):
            raise ValueError('category_attributes must be an object')
        if key in SETTING_FIELDS and (not isinstance(value, (str, dict)) or not value
                                      or isinstance(value, str) and not value.strip()):
            raise ValueError(key + ': explicit nonempty expected text or object required')
        if key in BOOLEAN_FIELDS and type(value) is not bool:
            raise ValueError(key + ': expected boolean required')
        if key == 'format' and value not in FORMATS:
            raise ValueError('format must be one of ' + ', '.join(sorted(FORMATS)))
        if key == 'duration' and not ((isinstance(value, str) and value.strip())
                                      or (type(value) is int and value > 0)):
            raise ValueError('duration: nonempty text (for example "not_applicable") or a positive integer required')
        if key == 'buy_now_price' and value != 'none' and (
                isinstance(value, bool) or not isinstance(value, (int, Decimal))
                or not Decimal(value).is_finite() or value < 0):
            raise ValueError('buy_now_price must be a finite nonnegative number or the string "none"')
        if key == 'quantity' and (type(value) is not int or value < 1):
            raise ValueError('quantity must be a positive integer')
        if key == 'price' and (isinstance(value, bool) or not isinstance(value, (int, Decimal)) or not Decimal(value).is_finite() or value < 0):
            raise ValueError('price must be a finite nonnegative number')
        if key == 'photos' and (not isinstance(value, list) or not value or any(not isinstance(x, str) or not x for x in value)):
            raise ValueError('photos must be a nonempty ordered list of photo identifiers')
        if key == 'photos':
            for photo in value:
                identifier(photo)
            if len(set(value)) != len(value):
                raise ValueError('photos must contain unique photo identifiers')
    # Intent coherence: the format decides whether a separate buy-now price exists.
    if (expected['format'] == 'auction_with_buy_now') != (expected['buy_now_price'] != 'none'):
        raise ValueError('buy_now_price must be a number exactly when format is auction_with_buy_now, otherwise "none"')
    if expected['buy_now_price'] != 'none' and expected['buy_now_price'] <= expected['price']:
        raise ValueError('buy_now_price must exceed the start price')
    issues, exemptions = [], []

    def receipt(value, field, absence=False):
        allowed = {'evidence_ref', 'observed_at', 'reason'} if absence else {'present', 'value', 'evidence_ref', 'observed_at'}
        required = allowed if absence else {'present', 'evidence_ref', 'observed_at'}
        shape(value, allowed, required)
        identifier(value['evidence_ref'])
        observed_at = stamp(value['observed_at'])
        if observed_at > now:
            raise ValueError(field + ': receipt is in the future')
        if now - observed_at > timedelta(hours=max_age_hours):
            issues.append({'field': field, 'reason': 'stale_receipt'})
        if absence:
            if not isinstance(value['reason'], str) or not value['reason'].strip():
                raise ValueError('not_shown receipt requires a specific reason')
        else:
            if type(value['present']) is not bool:
                raise ValueError('present must be boolean')
            if value['present'] and 'value' not in value:
                raise ValueError('present receipt requires an actual value')
            if not value['present'] and 'value' in value:
                raise ValueError('absent receipt cannot assert a value')
        return value

    for key in ('listing_ref', 'draft_id'):
        if key not in identity:
            issues.append({'field': key, 'reason': 'identity_unobserved'})
            continue
        row = receipt(identity[key], key)
        if not row['present'] or not equal(packet[key], row.get('value')):
            issues.append({'field': key, 'reason': 'identity_mismatch',
                           'expected': packet[key], 'observed': row.get('value')})
    for key in sorted(material):
        if key in not_shown:
            receipt(not_shown[key], key, absence=True)
            exemptions.append(key)
        elif key not in observed:
            issues.append({'field': key, 'reason': 'unobserved'})
        else:
            row = receipt(observed[key], key)
            if not row['present']:
                issues.append({'field': key, 'reason': 'not_present'})
            elif not equal(expected[key], row['value']):
                issues.append({'field': key, 'reason': 'value_mismatch',
                               'expected': expected[key], 'observed': row['value']})
    return {'listing_ref': packet['listing_ref'], 'draft_id': packet['draft_id'],
            'stage': packet['stage'], 'ready': not issues, 'issues': issues,
            'preview_not_shown': exemptions,
            'proof_limit': 'Only this stage and supplied receipts were compared. Publication requires successful reopened and preview checks against the same current intent and draft, plus existing authority. This tool does not open Ricardo or Tutti, check live fees or verify receipt truth.'}


def load(path):
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key: ' + key)
            result[key] = value
        return result

    def reject_constant(value):
        raise ValueError('nonfinite JSON constant: ' + value)
    return json.loads(path.read_text(encoding='utf-8'), parse_float=Decimal,
                      parse_constant=reject_constant, object_pairs_hook=object_pairs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('packet', type=Path)
    parser.add_argument('--now', help='ISO timestamp with offset; defaults to actual UTC time')
    parser.add_argument('--max-age-hours', type=float, default=24)
    args = parser.parse_args()
    try:
        result = compare(load(args.packet), stamp(args.now) if args.now else datetime.now(timezone.utc),
                         args.max_age_hours)
        print(json.dumps(result, indent=2, default=lambda value: str(value) if isinstance(value, Decimal) else value))
    except (OSError, ValueError, TypeError, OverflowError, DecimalException) as exc:
        parser.exit(2, f'Invalid draft comparison input: {exc}\n')


if __name__ == '__main__':
    main()
