#!/usr/bin/env python3
"""Compare sanitized observed snapshots. Read-only; never fetches or acts on Ricardo or Tutti."""
import argparse
import hashlib
import json
import math
import re
from datetime import datetime, timezone, timedelta
from pathlib import Path

SURFACES = {'orders', 'messages', 'offers', 'returns', 'tracking', 'account', 'payouts'}
DEFAULT_REQUIRED = sorted(SURFACES - {'payouts'})
KINDS = {'order', 'question', 'offer', 'return', 'shipment', 'account', 'payout', 'ship_by', 'response_by'}


def timestamp(value):
    if not isinstance(value, str) or not re.fullmatch(
            r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)', value):
        raise ValueError('timestamp must be a string with UTC offset')
    dt = datetime.fromisoformat(value.replace('Z', '+00:00'))
    if dt.tzinfo is None or dt.utcoffset() is None:
        raise ValueError('timestamp requires UTC offset')
    return dt


def token(value):
    if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z][A-Za-z0-9_.-]{0,79}', value):
        raise ValueError('use short opaque local IDs/status codes, not personal data')
    return value


def fields(obj, allowed, required):
    if not isinstance(obj, dict) or set(obj) - set(allowed) or set(required) - set(obj):
        raise ValueError(f'expected fields {sorted(required)}; allowed {sorted(allowed)}')


def validate(snapshot):
    fields(snapshot, {'seller_ref', 'observed_at', 'mode', 'surfaces', 'events', 'notified', 'required_surfaces', 'gap_episodes', 'event_epochs'},
           {'seller_ref', 'observed_at', 'mode', 'surfaces', 'events'})
    token(snapshot['seller_ref'])
    observed = timestamp(snapshot['observed_at'])
    if snapshot['mode'] not in {'interactive', 'scheduled'}:
        raise ValueError('mode must be interactive or scheduled')
    required = snapshot.get('required_surfaces', DEFAULT_REQUIRED)
    if not isinstance(required, list) or not required or any(s not in SURFACES for s in required) or len(set(required)) != len(required):
        raise ValueError('required_surfaces must be unique known surface names')
    if not isinstance(snapshot['surfaces'], dict) or set(snapshot['surfaces']) - SURFACES:
        raise ValueError('unknown account surface')
    for surface in snapshot['surfaces'].values():
        fields(surface, {'status', 'checked_at', 'evidence_ref'}, {'status', 'checked_at', 'evidence_ref'})
        if surface['status'] not in {'checked', 'access_failed', 'not_checked'}:
            raise ValueError('surface status must be checked, access_failed or not_checked')
        if timestamp(surface['checked_at']) > observed:
            raise ValueError('surface observation cannot be later than snapshot')
        token(surface['evidence_ref'])
    if not isinstance(snapshot['events'], list):
        raise ValueError('events must be a list')
    ids = set()
    for event in snapshot['events']:
        fields(event, {'id', 'surface', 'kind', 'status', 'actionable', 'observed_at', 'due_at', 'evidence_ref'},
               {'id', 'surface', 'kind', 'status', 'actionable', 'observed_at', 'evidence_ref'})
        if token(event['id']) in ids:
            raise ValueError('duplicate event ID')
        ids.add(event['id'])
        token(event['status'])
        token(event['evidence_ref'])
        if event['surface'] not in SURFACES or event['kind'] not in KINDS or type(event['actionable']) is not bool:
            raise ValueError('invalid event surface, kind or actionable boolean')
        if timestamp(event['observed_at']) > observed:
            raise ValueError('event observation cannot be later than snapshot')
        if 'due_at' in event:
            timestamp(event['due_at'])
        if event['kind'] in {'ship_by', 'response_by'} and 'due_at' not in event:
            raise ValueError('deadline event requires due_at')
    notified = snapshot.get('notified', [])
    if not isinstance(notified, list) or any(not isinstance(x, str) or not re.fullmatch('[0-9a-f]{64}', x) for x in notified):
        raise ValueError('notified must contain only notification fingerprint hashes')
    gaps = snapshot.get('gap_episodes', {})
    if not isinstance(gaps, dict) or set(gaps) - SURFACES:
        raise ValueError('gap_episodes must map known surfaces to episode records')
    for episode in gaps.values():
        fields(episode, {'id', 'opened_at'}, {'id', 'opened_at'})
        if not isinstance(episode['id'], str) or not re.fullmatch('[0-9a-f]{64}', episode['id']):
            raise ValueError('episode ID must be a SHA256 fingerprint')
        if timestamp(episode['opened_at']) > observed:
            raise ValueError('gap episode cannot start after snapshot')
    epochs = snapshot.get('event_epochs', {})
    if (not isinstance(epochs, dict) or set(epochs) - ids
            or any(type(v) is not int or v < 0 for v in epochs.values())):
        raise ValueError('event_epochs must map retained event IDs to nonnegative integers')
    return snapshot


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def compare(before, current, now, freshness_hours=24, deadline_hours=24):
    validate(before)
    validate(current)
    if before['seller_ref'] != current['seller_ref']:
        raise ValueError('seller identity mismatch; do not compare accounts')
    if set(before.get('required_surfaces', DEFAULT_REQUIRED)) != set(current.get('required_surfaces', DEFAULT_REQUIRED)):
        raise ValueError('required_surfaces policy changed; record authority and explicitly rebaseline the new policy without dropping outstanding events')
    if timestamp(current['observed_at']) < timestamp(before['observed_at']):
        raise ValueError('current snapshot predates baseline')
    if not isinstance(now, datetime) or now.tzinfo is None or now.utcoffset() is None:
        raise ValueError('now must be an offset-aware datetime')
    if timestamp(current['observed_at']) > now:
        raise ValueError('snapshot is in the future')
    if any(isinstance(value, bool) or not isinstance(value, (int, float))
           or not math.isfinite(value) or value <= 0 for value in (freshness_hours, deadline_hours)):
        raise ValueError('freshness and deadline windows must be positive')
    alerts, coverage = [], {}
    # Only caller-persisted delivery receipts suppress notifications. Computation is not delivery.
    notified = set(before.get('notified', []))
    prior_events = {event['id']: event for event in before['events']}
    effective = dict(prior_events)
    epochs = dict(before.get('event_epochs', {}))
    gap_episodes = {}
    accepted_current = set()

    def alert(reason, identity, status, extra=None, semantic=None):
        item = {'reason': reason, 'id': identity, 'status': status}
        if extra:
            item.update(extra)
        key = fingerprint(semantic if semantic is not None else item)
        if key not in notified:
            alerts.append(dict(item, fingerprint=key))

    required = current.get('required_surfaces', DEFAULT_REQUIRED)
    for name in sorted(set(required) | set(current['surfaces'])):
        value = current['surfaces'].get(name)
        previous = before['surfaces'].get(name)
        if value and previous and timestamp(value['checked_at']) < timestamp(previous['checked_at']):
            raise ValueError('surface observation predates its baseline')
        prior_gap = before.get('gap_episodes', {}).get(name)
        if (value and value['status'] == 'checked' and prior_gap
                and timestamp(value['checked_at']) < timestamp(prior_gap['opened_at'])):
            raise ValueError('surface observation predates its unresolved coverage gap')
        status = value['status'] if value else 'not_checked'
        if value and now - timestamp(value['checked_at']) > timedelta(hours=freshness_hours):
            status = 'stale'
        coverage[name] = status
        if status != 'checked' and name in required:
            episode = before.get('gap_episodes', {}).get(name)
            if episode is None:
                episode = {'id': fingerprint({'seller_ref': current['seller_ref'],
                           'surface': name, 'opened_at': current['observed_at']}),
                           'opened_at': current['observed_at']}
            gap_episodes[name] = episode
            alert('coverage_gap', name, status, {'episode': episode['id']},
                  {'reason': 'coverage_gap', 'surface': name, 'episode': episode['id']})

    for event in current['events']:
        previous = prior_events.get(event['id'])
        if previous and timestamp(event['observed_at']) < timestamp(previous['observed_at']):
            raise ValueError('event observation predates its baseline')
        if previous and (event['kind'], event['surface']) != (previous['kind'], previous['surface']):
            raise ValueError('stable event ID changed kind or surface')
        if coverage.get(event['surface']) != 'checked':
            continue  # failed/old surface cannot overwrite prior known events
        if now - timestamp(event['observed_at']) > timedelta(hours=freshness_hours):
            continue
        def state(row):
            due = timestamp(row['due_at']).astimezone(timezone.utc) if 'due_at' in row else None
            return row['status'], row['actionable'], due

        if previous and state(event) != state(previous):
            epochs[event['id']] = epochs.get(event['id'], 0) + 1
        effective[event['id']] = event
        accepted_current.add(event['id'])

    for event in effective.values():
        if not event['actionable']:
            continue
        extra = {'kind': event['kind'], 'evidence_current': event['id'] in accepted_current,
                 'last_observed_at': event['observed_at']}
        semantic = {'id': event['id'], 'kind': event['kind'], 'status': event['status'],
                    'seller_ref': current['seller_ref']}
        if epochs.get(event['id'], 0):
            semantic['epoch'] = epochs[event['id']]
        if 'due_at' in event:
            remaining = timestamp(event['due_at']) - now
            urgency = 'overdue' if remaining.total_seconds() <= 0 else 'due_soon'
            if remaining > timedelta(hours=deadline_hours):
                if event['kind'] not in {'ship_by', 'response_by'}:
                    alert('actionable', event['id'], event['status'], dict(extra, due_at=event['due_at']),
                          dict(semantic, reason='actionable'))
                continue
            alert(urgency, event['id'], event['status'], dict(extra, due_at=event['due_at']),
                  dict(semantic, reason=urgency,
                       due_at=timestamp(event['due_at']).astimezone(timezone.utc).isoformat()))
        else:
            alert('actionable', event['id'], event['status'], extra,
                  dict(semantic, reason='actionable'))

    # Missing events are retained, never silently resolved by a blank/partial dashboard.
    # Retain the latest per-surface receipt through partial runs as historical
    # evidence; current coverage still comes exclusively from current surfaces.
    baseline = dict(current, surfaces=dict(before['surfaces'], **current['surfaces']),
                    events=list(effective.values()), notified=sorted(notified),
                    gap_episodes=gap_episodes, event_epochs=epochs)
    return {
        'decision': 'NOTIFY' if alerts else 'DONT_NOTIFY',
        'notifications': alerts, 'coverage': coverage,
        'scheduled_access_verified_this_run': current['mode'] == 'scheduled' and all(
            coverage.get(s) == 'checked' and (
                timestamp(current['surfaces'][s]['checked_at']) > timestamp(before['observed_at']) or
                s not in before['surfaces'] and timestamp(current['surfaces'][s]['checked_at']) == timestamp(before['observed_at']))
            for s in required),
        'current_snapshot': current, 'effective_baseline': baseline,
        'limitations': ['Local comparison only; inputs require live evidence.',
                        'Persist notification fingerprints only after actual delivery.',
                        'Missing events are retained; explicitly observe resolution. Retained deadlines remain actionable even when their evidence is stale.',
                        'No messages, offers, prices, refunds, labels or listings are changed.']}


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
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=object_pairs,
                      parse_constant=reject_constant)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('current', type=Path)
    parser.add_argument('--now', help='Offset timestamp; defaults to UTC now')
    parser.add_argument('--freshness-hours', type=int, default=24)
    parser.add_argument('--deadline-hours', type=int, default=24)
    args = parser.parse_args()
    try:
        result = compare(load(args.before), load(args.current),
                         timestamp(args.now) if args.now else datetime.now(timezone.utc),
                         args.freshness_hours, args.deadline_hours)
    except (OSError, ValueError, TypeError, KeyError, OverflowError) as exc:
        parser.exit(2, f'Invalid monitor input: {exc}\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
