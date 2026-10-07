#!/usr/bin/env python3
"""Deterministic monitor transitions; no browser, account or notification calls."""
import copy
import json
import runpy
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('monitor-diff.py')
MODULE = runpy.run_path(str(SCRIPT), run_name='monitor_diff_tests')
compare = MODULE['compare']
timestamp = MODULE['timestamp']
FIXTURES = Path(tempfile.mkdtemp(prefix='sales-monitor-tests-'))


def snapshot(at='2031-05-01T09:00:00Z', status='checked', events=None):
    return {'seller_ref': 'seller-local', 'observed_at': at, 'mode': 'scheduled',
            'required_surfaces': ['orders'],
            'surfaces': {'orders': {'status': status, 'checked_at': at, 'evidence_ref': 'obs-local'}},
            'events': events or [], 'notified': []}


def event(at='2031-05-01T09:00:00Z', **updates):
    row = {'id': 'order-local', 'surface': 'orders', 'kind': 'order',
           'status': 'paid', 'actionable': True, 'observed_at': at, 'evidence_ref': 'order-proof'}
    row.update(updates)
    return row


def delivered(result):
    baseline = copy.deepcopy(result['effective_baseline'])
    baseline['notified'].extend(row['fingerprint'] for row in result['notifications'])
    return baseline


class MonitorTests(unittest.TestCase):
    def run_compare(self, before, current, now=None, **kwargs):
        return compare(before, current, timestamp(now or current['observed_at']), **kwargs)

    def test_unchanged_nonactionable_is_quiet(self):
        state = snapshot(events=[event(actionable=False)])
        result = self.run_compare(state, state)
        self.assertEqual(result['decision'], 'DONT_NOTIFY')
        self.assertFalse(result['scheduled_access_verified_this_run'])

    def test_action_with_distant_deadline_is_not_suppressed(self):
        current = snapshot(events=[event(due_at='2031-05-10T10:00:00Z')])
        result = self.run_compare(snapshot(), current)
        self.assertEqual([n['reason'] for n in result['notifications']], ['actionable'])

    def test_action_near_deadline_emits_one_urgent_alert(self):
        current = snapshot(events=[event(due_at='2031-05-01T10:00:00Z')])
        result = self.run_compare(snapshot(), current)
        self.assertEqual([n['reason'] for n in result['notifications']], ['due_soon'])

    def test_replayed_checks_do_not_prove_scheduled_run_access(self):
        before = snapshot()
        current = snapshot('2031-05-01T10:00:00Z')
        current['surfaces'] = copy.deepcopy(before['surfaces'])
        result = self.run_compare(before, current)
        self.assertEqual(result['coverage']['orders'], 'checked')
        self.assertFalse(result['scheduled_access_verified_this_run'])
        current['surfaces']['orders']['checked_at'] = current['observed_at']
        self.assertTrue(self.run_compare(before, current)['scheduled_access_verified_this_run'])
        before['surfaces'] = {}
        current['surfaces']['orders']['checked_at'] = '2031-05-01T08:00:00Z'
        self.assertFalse(self.run_compare(before, current)['scheduled_access_verified_this_run'])

    def test_old_success_cannot_clear_new_failure_episode(self):
        failed = self.run_compare(snapshot(), snapshot('2031-05-01T10:00:00Z', 'access_failed'))
        current = snapshot('2031-05-01T11:00:00Z')
        current['surfaces']['orders']['checked_at'] = '2031-05-01T09:00:00Z'
        with self.assertRaisesRegex(ValueError, 'surface observation predates'):
            self.run_compare(delivered(failed), current)

    def test_old_success_cannot_clear_gap_from_an_omitted_surface(self):
        missing = snapshot('2031-05-01T10:00:00Z')
        missing['surfaces'] = {}
        gap = self.run_compare(snapshot(), missing)
        current = snapshot('2031-05-01T11:00:00Z')
        current['surfaces']['orders']['checked_at'] = '2031-05-01T09:00:00Z'
        with self.assertRaisesRegex(ValueError, 'unresolved coverage gap'):
            self.run_compare(delivered(gap), current)

    def test_partial_snapshot_retains_latest_surface_history(self):
        first = self.run_compare(snapshot(), snapshot('2031-05-01T10:00:00Z', 'access_failed'))
        failed = self.run_compare(delivered(first), snapshot('2031-05-01T11:00:00Z', 'access_failed'))
        missing = snapshot('2031-05-01T12:00:00Z')
        missing['surfaces'] = {}
        partial = self.run_compare(delivered(failed), missing)
        self.assertEqual(partial['coverage']['orders'], 'not_checked')
        self.assertEqual(partial['effective_baseline']['surfaces']['orders']['checked_at'],
                         '2031-05-01T11:00:00Z')
        self.assertEqual(partial['current_snapshot']['surfaces'], {})
        old_success = snapshot('2031-05-01T13:00:00Z')
        old_success['surfaces']['orders']['checked_at'] = '2031-05-01T10:30:00Z'
        with self.assertRaisesRegex(ValueError, 'predates its baseline'):
            self.run_compare(delivered(partial), old_success)

    def test_extreme_window_cli_returns_input_error_without_traceback(self):
        folder = Path(tempfile.mkdtemp(prefix='window-', dir=FIXTURES))
        path = folder / 'snapshot.json'
        path.write_text(json.dumps(snapshot()))
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(path), str(path),
            '--now', '2031-05-01T09:00:00Z', '--freshness-hours', '999999999999999999'],
            capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertNotIn('Traceback', result.stderr)

    def test_unsent_action_remains_eligible_and_observation_time_not_identity(self):
        initial = self.run_compare(snapshot(), snapshot(events=[event()]))
        later = '2031-05-01T10:00:00Z'
        repeated = self.run_compare(initial['effective_baseline'], snapshot(later, events=[event(later)]))
        self.assertEqual(initial['notifications'][0]['fingerprint'], repeated['notifications'][0]['fingerprint'])
        self.assertEqual(repeated['decision'], 'NOTIFY')
        quiet = self.run_compare(delivered(initial), snapshot(later, events=[event(later)]))
        self.assertEqual(quiet['decision'], 'DONT_NOTIFY')

    def test_changed_status_is_new_action(self):
        initial = self.run_compare(snapshot(), snapshot(events=[event()]))
        later = '2031-05-01T10:00:00Z'
        result = self.run_compare(delivered(initial), snapshot(later, events=[event(later, status='label_created')]))
        self.assertEqual(result['decision'], 'NOTIFY')
        self.assertNotEqual(initial['notifications'][0]['fingerprint'], result['notifications'][0]['fingerprint'])

    def test_reopened_action_and_return_to_prior_status_are_new_alerts(self):
        for middle in (dict(actionable=False, status='paid'), dict(actionable=True, status='label_created')):
            with self.subTest(middle=middle):
                initial = self.run_compare(snapshot(), snapshot(events=[event()]))
                later = '2031-05-01T10:00:00Z'
                changed = self.run_compare(delivered(initial), snapshot(later, events=[event(later, **middle)]))
                final_time = '2031-05-01T11:00:00Z'
                reopened = self.run_compare(delivered(changed), snapshot(final_time, events=[event(final_time)]))
                self.assertEqual(reopened['decision'], 'NOTIFY')
                self.assertNotEqual(initial['notifications'][0]['fingerprint'], reopened['notifications'][0]['fingerprint'])
                repeated = self.run_compare(delivered(reopened), snapshot('2031-05-01T12:00:00Z',
                    events=[event('2031-05-01T12:00:00Z')]))
                self.assertEqual(repeated['decision'], 'DONT_NOTIFY')

    def test_failed_surface_cannot_resolve_known_action(self):
        initial = snapshot(events=[event()])
        later = '2031-05-01T10:00:00Z'
        current = snapshot(later, 'access_failed', [event(later, actionable=False)])
        result = self.run_compare(initial, current)
        retained = result['effective_baseline']['events'][0]
        self.assertTrue(retained['actionable'])
        self.assertFalse(result['current_snapshot']['events'][0]['actionable'])
        action = next(row for row in result['notifications'] if row['reason'] == 'actionable')
        self.assertFalse(action['evidence_current'])
        self.assertFalse(result['scheduled_access_verified_this_run'])

    def test_missing_deadline_retained_and_escalates_even_stale(self):
        due = event(kind='ship_by', due_at='2031-05-02T10:00:00Z')
        prior = snapshot(events=[due])
        current = snapshot('2031-05-02T09:00:00Z')
        soon = self.run_compare(prior, current)
        self.assertEqual(soon['notifications'][0]['reason'], 'due_soon')
        after = self.run_compare(delivered(soon), snapshot('2031-05-02T11:00:00Z'))
        self.assertEqual(after['notifications'][0]['reason'], 'overdue')
        self.assertFalse(after['notifications'][0]['evidence_current'])
        self.assertEqual(len(after['effective_baseline']['events']), 1)

    def test_future_deadline_not_premature_notification(self):
        state = snapshot(events=[event(kind='ship_by', due_at='2031-05-10T10:00:00Z')])
        self.assertEqual(self.run_compare(state, state)['decision'], 'DONT_NOTIFY')

    def test_explicit_fresh_resolution_stops_deadline_alert(self):
        past = event(kind='ship_by', due_at='2031-05-01T08:00:00Z')
        current = snapshot(events=[dict(past, actionable=False, status='resolved')])
        result = self.run_compare(snapshot(events=[past]), current)
        self.assertEqual(result['decision'], 'DONT_NOTIFY')
        self.assertFalse(result['effective_baseline']['events'][0]['actionable'])

    def test_gap_episode_survives_runs_and_resets_after_recovery(self):
        failure = self.run_compare(snapshot(), snapshot('2031-05-01T10:00:00Z', 'access_failed'))
        baseline = delivered(failure)
        repeat = self.run_compare(baseline, snapshot('2031-05-01T11:00:00Z', 'access_failed'))
        self.assertEqual(repeat['decision'], 'DONT_NOTIFY')
        self.assertEqual(repeat['effective_baseline']['gap_episodes'], baseline['gap_episodes'])
        recovered = self.run_compare(repeat['effective_baseline'], snapshot('2031-05-01T12:00:00Z'))
        self.assertEqual(recovered['effective_baseline']['gap_episodes'], {})
        second_failure = self.run_compare(recovered['effective_baseline'], snapshot('2031-05-01T13:00:00Z', 'access_failed'))
        self.assertEqual(second_failure['decision'], 'NOTIFY')
        self.assertNotEqual(failure['notifications'][0]['fingerprint'], second_failure['notifications'][0]['fingerprint'])

    def test_gap_status_aging_does_not_generate_repeated_alert(self):
        failed = self.run_compare(snapshot(), snapshot('2031-05-01T10:00:00Z', 'access_failed'))
        stale = self.run_compare(delivered(failed), failed['current_snapshot'], now='2031-05-03T10:00:00Z')
        self.assertEqual(stale['coverage']['orders'], 'stale')
        self.assertEqual(stale['decision'], 'DONT_NOTIFY')

    def test_unsent_coverage_alert_remains_eligible(self):
        first = self.run_compare(snapshot(), snapshot('2031-05-01T10:00:00Z', 'not_checked'))
        again = self.run_compare(first['effective_baseline'], snapshot('2031-05-01T11:00:00Z', 'not_checked'))
        self.assertEqual(first['notifications'][0]['fingerprint'], again['notifications'][0]['fingerprint'])

    def test_stale_surface_and_stale_event_cannot_prove_resolution(self):
        before = snapshot(events=[event()])
        current = snapshot('2031-05-03T10:00:00Z', events=[event(actionable=False)])
        current['surfaces']['orders']['checked_at'] = before['observed_at']
        result = self.run_compare(before, current)
        self.assertTrue(result['effective_baseline']['events'][0]['actionable'])
        self.assertEqual(result['coverage']['orders'], 'stale')
        current['surfaces']['orders']['checked_at'] = current['observed_at']
        result = self.run_compare(before, current)
        self.assertTrue(result['effective_baseline']['events'][0]['actionable'])

    def test_identity_order_and_future_rejected(self):
        before = snapshot()
        cases = [dict(snapshot(), seller_ref='wrong-seller'), snapshot('2031-05-01T08:00:00Z'),
                 snapshot('2031-05-01T11:00:00Z')]
        for current in cases:
            with self.subTest(current=current['observed_at']):
                with self.assertRaises(ValueError):
                    self.run_compare(before, current, now='2031-05-01T10:00:00Z')
        current = snapshot(events=[event('2031-05-01T10:00:00Z')])
        with self.assertRaises(ValueError):
            self.run_compare(before, current)

    def test_event_history_cannot_regress_or_change_kind(self):
        before = snapshot(events=[event()])
        for altered in (event('2031-05-01T08:00:00Z'), event(kind='question')):
            with self.subTest(altered=altered):
                with self.assertRaises(ValueError):
                    self.run_compare(before, snapshot('2031-05-01T10:00:00Z', events=[altered]))

    def test_private_unknown_fields_and_bad_boolean_rejected(self):
        current = snapshot(events=[event()])
        current['events'][0]['buyer_name'] = 'PRIVATE_SENTINEL'
        with self.assertRaises(ValueError):
            self.run_compare(snapshot(), current)
        for invalid in ('true', 1, None, float('nan')):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    self.run_compare(snapshot(), snapshot(events=[event(actionable=invalid)]))

    def test_invalid_windows_and_naive_timestamps_rejected(self):
        for invalid in (True, 0, -1, float('nan'), float('inf')):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    self.run_compare(snapshot(), snapshot(), freshness_hours=invalid)
        with self.assertRaises(ValueError):
            timestamp('2031-05-01T09:00:00')
        with self.assertRaises(ValueError):
            timestamp(True)
        with self.assertRaises(ValueError):
            timestamp('2031-05-01T09:00:00+00:60')

    def test_epoch_metadata_cannot_name_missing_event_or_use_boolean(self):
        for epochs in ({'missing': 1}, {'order-local': True}, {'order-local': -1}):
            with self.subTest(epochs=epochs):
                data = snapshot(events=[event()])
                data['event_epochs'] = epochs
                with self.assertRaises(ValueError):
                    self.run_compare(data, data)

    def test_interactive_run_cannot_prove_scheduled_access(self):
        current = dict(snapshot(), mode='interactive')
        self.assertFalse(self.run_compare(snapshot(), current)['scheduled_access_verified_this_run'])

    def test_compare_does_not_mutate_inputs(self):
        before, current = snapshot(), snapshot(events=[event()])
        originals = copy.deepcopy((before, current))
        self.run_compare(before, current)
        self.assertEqual((before, current), originals)

    def test_required_surface_policy_cannot_silently_change(self):
        before = snapshot()
        before['required_surfaces'] = ['orders', 'returns']
        for required in (['orders'], ['orders', 'returns', 'messages']):
            with self.subTest(required=required):
                current = snapshot()
                current['required_surfaces'] = required
                with self.assertRaisesRegex(ValueError, 'policy changed'):
                    self.run_compare(before, current)
        current = snapshot()
        current['required_surfaces'] = ['returns', 'orders']
        self.assertEqual(self.run_compare(before, current)['coverage']['returns'], 'not_checked')

    def test_duplicate_json_and_nonfinite_constant_are_rejected(self):
        for body in ('{"seller_ref":"a","seller_ref":"b"}',
                     '{"events":[{"actionable":true,"actionable":false}]}',
                     '{"observed_at":NaN}'):
            with self.subTest(body=body):
                path = Path(tempfile.mkdtemp(prefix='invalid-', dir=FIXTURES)) / 'snapshot.json'
                path.write_text(body, encoding='utf-8')
                with self.assertRaises(ValueError):
                    MODULE['load'](path)

    def test_actual_cli(self):
        folder = Path(tempfile.mkdtemp(prefix='case-', dir=FIXTURES))
        before, current = folder / 'before.json', folder / 'current.json'
        before.write_text(json.dumps(snapshot()), encoding='utf-8')
        current.write_text(json.dumps(snapshot(events=[event()])), encoding='utf-8')
        result = subprocess.run([sys.executable, '-B', str(SCRIPT), str(before), str(current),
                                 '--now', '2031-05-01T09:00:00Z'], capture_output=True,
                                text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['decision'], 'NOTIFY')


if __name__ == '__main__':
    print(f'Retained synthetic fixtures: {FIXTURES}', flush=True)
    unittest.main(verbosity=2)
