# Monitor observations, obligations and meaningful changes

This protocol extends the fulfilment reference. It makes local comparisons reproducible without pretending the script accesses an account. Existing `operations/monitor.json` summaries remain valid; adopt these optional snapshot files when a structured comparison is useful.

## Acquire evidence before comparing

1. Confirm the authorized seller identity. An opaque local seller reference joins snapshots; do not store the account name in a generic example.
2. Check required surfaces independently: orders, messages, offers, returns/disputes, tracking and account alerts. Add payouts if authorized. Record actual access and observation times. What the CLI can read per surface (CLI versions that list the seller commands; otherwise the browser):

   | Surface | Cheap CLI read | Needs the browser |
   |---|---|---|
   | account | `ricardo selling status --json`: limits, pending-action flag, unread count; `ricardo notifications --json` | Account standing, payout setup |
   | orders | `ricardo selling sold --json`: orders, payment, delivery, Ricardo buckets | Buyer details, addresses |
   | messages | unread **count** only | Message contents, Tutti chats, replies |
   | offers | `ricardo selling --json`: open/closed offers | Buyer offers and bids detail |
   | returns | `dispute_state` and cancellation buckets in `selling sold` | Case pages and deadlines |
   | tracking | `tracking_url` and number from `selling sold` | Carrier scan events (open the tracking page) |
   | payouts | `payout_unlocked_at` only | Transfer status, bank arrival |

   A surface the CLI only partly covers is `checked` for the part actually read and the gap is recorded; use `not_checked` or `access_failed` rather than a quiet pass.
3. Extract only actionable entities and status transitions with stable **local** IDs. Keep actual order, message and tracking identifiers and private correspondence in the ignored map.
4. Represent a shipping, pickup or response deadline as a separate event with `due_at`. Do not derive carrier acceptance from a seller-confirmed shipment, a label or a booked parcel pickup.
5. Compare to the previous retained baseline. Keep inaccessible state unknown, and keep earlier evidence visibly historical.

Schedule health, interactive account access and access during a scheduled run are three different facts. Associate the run receipt with the actual current automation identity outside this compact snapshot. A replacement job needs its own execution proof. Never copy an old "verified" flag forward indefinitely. A CLI call that exits 3 or 4, or exits 5 with `upstream_blocked` or `rate_limit_cooldown`, is an access failure for the surface it would have read; the pause after a block lasts two minutes, so the run reports the failure rather than retrying. Exit 5 with `request_busy` means another local `ricardo` command was running: nothing reached Ricardo, so it is not an access failure; wait for that command and rerun once, sequentially.

## Snapshot contract

Use `scripts/monitor-diff.py BEFORE CURRENT [--now OFFSET_TIMESTAMP] [--freshness-hours N] [--deadline-hours N]`. Windows default to 24 hours as software defaults, not marketplace grace periods; choose them to fit the seller's cadence and obligations. Both must be positive integers. Input timestamps require UTC offsets. The script rejects unknown fields to reduce accidental copying of buyer data, and rejects duplicate JSON keys or nonfinite constants instead of allowing a later duplicate value to silently replace earlier evidence.

```json
{
  "seller_ref": "SELLER-LOCAL",
  "observed_at": "2031-06-12T09:00:00+02:00",
  "mode": "scheduled",
  "required_surfaces": ["orders", "messages", "offers", "returns", "tracking", "account"],
  "surfaces": {
    "orders": {
      "status": "checked",
      "checked_at": "2031-06-12T08:58:00+02:00",
      "evidence_ref": "OBS-ORDERS-001"
    },
    "returns": {
      "status": "access_failed",
      "checked_at": "2031-06-12T08:59:00+02:00",
      "evidence_ref": "OBS-RETURNS-001"
    }
  },
  "events": [
    {
      "id": "ORDER-A-HANDOFF",
      "surface": "orders",
      "kind": "ship_by",
      "status": "label_created",
      "actionable": true,
      "observed_at": "2031-06-12T08:58:00+02:00",
      "due_at": "2031-06-12T23:59:00+02:00",
      "evidence_ref": "OBS-ORDER-A-001"
    }
  ],
  "notified": []
}
```

This is intentionally incomplete coverage: omitted required surfaces will be reported as not checked. Do not fill them with fictitious successful checks to obtain a quiet result. A first-run baseline can contain the same seller reference and required-surface set, prior observation time, empty surfaces/events and no notification receipts; the first real snapshot then establishes actual state.

The required-surface **set must match** between baseline and current snapshot; list order does not matter. Adding, removing or changing required surfaces otherwise fails comparison. An unavailable returns page cannot be made healthy by dropping returns from the policy. For a genuinely authorized policy change, preserve the old baseline and authority decision, explicitly initialize the new baseline with the revised required set, and carry forward outstanding events, notification receipts and applicable gap episodes. Compare subsequent observations against that new policy; do not invent successful observations or erase existing obligations during migration.

Allowed surface statuses: `checked`, `access_failed`, `not_checked`; stale observations are derived from their age. Allowed event kinds: `order`, `question`, `offer`, `return`, `shipment`, `account`, `payout`, `ship_by`, `response_by`. Use `response_by` for pickup deadlines, case responses and the short expiry of a buyer price proposal. `status` is a short local code, `actionable` a real boolean; deadline kinds require `due_at`. IDs and evidence references are short opaque tokens, never raw messages or addresses.

Optional `gap_episodes` is retained baseline state, keyed by surface, with `{ "id": "<SHA256>", "opened_at": "<offset timestamp>" }`. Let the differ produce it. A fresh successful check clears that surface's episode, so a later failure is new actionable access loss. Merely running again does not create a new episode.

Optional `event_epochs` maps retained event IDs to nonnegative integer revisions produced by the differ. Preserve it with the baseline. A freshly observed change in status, actionability or deadline advances the revision, so a resolved action that reopens can alert again without erasing prior delivery receipts. Merely refreshing an observation timestamp does not advance it. Distinct buyer questions still need distinct event IDs. Baseline notification/episode/revision metadata is authoritative; do not populate the current observation with invented acknowledgments.

## Interpret and persist the result

The result separates `current_snapshot` from `effective_baseline`, includes coverage, notifications and their fingerprints, and indicates whether required surfaces were freshly checked in this scheduled run. Coverage can remain within its age window while the scheduled-run flag is false: replayed checks do not establish new access. To support the flag, each required check must be later than the baseline snapshot (equal is allowed only for a surface absent from that baseline). Regressing a surface observation is rejected. The flag still cannot prove the scheduler ran if the caller falsely labels the input `scheduled`.

- `actionable`: newly actionable or still unnotified semantic state; not every new tracking scan. An order or question with a distant `due_at` remains immediately actionable; only dedicated `ship_by`/`response_by` events wait for the deadline window. An imminent general event emits one urgent alert instead of duplicating it with an ordinary alert.
- `due_soon` / `overdue`: deadline escalation categories. A known obligation can remain actionable even if its surface is now inaccessible; notification evidence marks its age/currentness.
- `coverage_gap`: unavailable, missing or stale required coverage, with a stable failure episode.

Keep `effective_baseline` after reviewing the comparison. **Only after the user-facing alert is actually delivered**, append its fingerprint to the baseline's `notified` list. Computing an alert is not sending it. If delivery fails, leave it unacknowledged so it is eligible again. Existing notifications are suppressed until semantic change, deadline escalation or a new failure episode warrants another alert.

Missing events remain in the effective baseline; absence from a dashboard does not prove resolution. Observe a resolved event explicitly with the same ID and `actionable: false`. If a later question or case arises on the same order, assign a new event ID rather than recycling an old question identity. A fresh scan can resolve the physical-handoff reminder; it does not erase the historically missed deadline in the order record.

The effective baseline also retains each surface's latest receipt across omitted/partial checks, preserving the timestamp needed to reject later replay of older evidence. Those retained receipts are historical: current coverage and scheduled-run verification still use only the current snapshot. Do not copy the baseline's merged surfaces into a new observation as if they had just been checked.

## Human-facing alert contract

Say what changed, exact local unit/lot, the actual deadline and the next necessary action. Give direct verified account navigation (for example the seller's sold-orders page) when useful. No names, addresses, full tracking IDs, bank data or message bodies go into Git. Draft replies separately in the private workflow, in the buyer's language. A buyer acknowledgment may require no response.

Keep asking price and offer policy unchanged during a default read-only watch. A displayed banner or suggestion is not automatically a seller-funded discount; distinguish base ask, displayed promotion, known/unknown funding and actual proceeds effect. Report a meaningful configuration discrepancy and investigate within authority before changing it.

## Limits and recovery

The differ cannot discover an order, prove a no-case count, infer whether an event should be actionable, verify credentials, send alerts, persist data or perform transactions. Its strict snapshot is an optional comparison format, not a replacement stock ledger. The seller's account and carrier remain the sources for actual facts. Run `scripts/test-monitor.py` when changing its logic.
