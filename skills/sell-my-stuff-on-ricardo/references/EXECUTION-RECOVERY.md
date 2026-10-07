# Recover the workflow without repeating external actions

Use on a resumed session, inconsistent form, ambiguous submit or access failure. Read only the row matching the current problem.

## A recoverable action record

Before a significant marketplace mutation (saving a draft, publishing, revising, ending), retain a compact sanitized record:

```text
action_id; local listing/lot reference; operation; current authority source
observed pre-state; intended field changes; draft/article ID if already known
submitted_at; outcome = prepared/submitted/uncertain/verified/failed
verification source and time; superseded action/draft if any
```

This can be a note in the existing listing record; do not create a distributed transaction system for one sale. Keep personal order details in the private map. On restart, resolve `submitted` or `uncertain` actions **before** another mutation of the same stock. A timeout says the result is unknown, not that the server rejected it.

## Three code-level checks that prevent false verification

**Presence is required before value comparison.** A missing offer or paid-option control does not mean it is off. A truncated page read lacking a checked option does not prove the option is absent. Record `observed`, actual value, source and time; unresolved presence remains unknown. Explicit `false`, an observed blank, unavailable and absent are four different observations. Verify control availability through the actual page before claiming a setting.

**Identity precedes every mutation.** Join browser work by verified draft or article ID (or the opaque reference you entered in the form) and local lot, not by a row's current position in a list. Recheck title and identity on the destination. Tabs, search results and draft order can change between reads. A valid selector on the wrong item is still wrong.

**Inspect the schema before writing a parser.** Discover actual field names and version from the real output; do not keep guessing keys until a command stops throwing. Preserve source data when adapting it to the canonical ledger. A broadened-search section must not contaminate exact comparables; parse dates without a hardcoded subset of months or one language. Validate meaning and units, not merely successful JSON parsing. When a CLI command is not in `ricardo capabilities --json`, it does not exist for this installation.

## Symptom-to-action table

| Symptom | Evidence to inspect | Safe next action | Completion proof |
|---|---|---|---|
| Publish timed out | Drafts (`ricardo selling drafts --json` if listed, or the Entwürfe page), active offers (`ricardo selling --json`), quantity and recent results | Retain reservation; identify whether the intended listing already exists | Active article ID and exact listing verified with `ricardo inspect`, or confirmed unsent draft |
| Repeated click appears ignored | Fresh page state, validation messages, modal/disabled state | Reacquire controls; address the actual validation error | Persisted state after reopening |
| Typed text is truncated or reverts | Visible value against the form's character limits, save result | Shorten to the limit deliberately; use supported field interaction; bounded chunks if needed | Entire saved text and buyer preview agree |
| Form cannot set a required feature | Available form controls and supported alternate flow | Save current work; migrate intentionally to a capable draft if necessary | New draft verified; old draft recorded as superseded and never published |
| Photos upload partially or reorder | Actual thumbnails per slot, count, full-resolution images | Reconcile successes; upload only missing approved derivatives | Exact image set/order and hashes bound to intent |
| Browser tab closed or selector stale | Current tab inventory and fresh DOM/screenshot | Re-select the observed account surface; use fresh locators | Correct seller/page and expected controls |
| Photo conversion exits successfully but image is blank | Decoded derivative at useful size | Try a faithful converter using the original; preserve the failed attempt separately | Correct pixels, orientation, crop and metadata review |
| Access works for listings but not orders/messages | Each requested surface independently | Complete accessible checks; identify the precise user-only challenge | Surface-specific access receipt, not a global login claim |
| CLI exits 5 | `error.code` (branch on it, not on the exit status alone) | `upstream_blocked`: stop CLI calls, tell the seller, wait out the two-minute pause. `rate_limit_cooldown`: wait until the stated time. `request_busy`: another local `ricardo` command is running; wait for it and run one command at a time (not a seller-facing problem). Continue browser-side work only if already authorized | A later successful read; never a cached result |
| Listing needs a correction after bids exist | Live listing and bid count | With a bid, only text and images can be added and the auction cannot be ended; add a clarifying note within revision authority and tell the seller | Added text verified on the live page; no promise that the sale can be undone |
| Unsold listing needs relisting | Ended state and edits made | Manual relist keeps watchers only when nothing is changed; any edit creates a new article number | New `listing_id` verified, old record `ended`, one allocation |
| Old local blocker conflicts with live page | Timestamp, entity and evidence of each assertion | Reconcile/update the stale note | Latest supported fact plus supersession explanation |
| Tracker says draft but listing is active | Account and local action history | Repair the record/task from evidence; do not publish again | One stock allocation and one intended active listing |
| Unit shown on Ricardo and Tutti at once | Both live pages, ledger allocation | End or hold the conflicting exposure per authority; reconcile the order that actually exists | One current allocation; the other exposure ended and verified |

Use supported browser and CLI controls. Do not read cookies, forge sessions, call undocumented hidden endpoints, bypass MFA or invent UI selectors. Before changing tools, distinguish a missing tool capability from a missing account permission. Switching forms or channels is not permission to change the sale's terms.

Do not set a universal number of retries or delay to "prove" an unsent draft, and never loop a CLI command. Check the account's actual validation or confirmation, matching drafts, active or scheduled listings and orders, allowing for any indicated processing. If still ambiguous, retain the reservation and the unresolved action. Finding an active listing is only the start of active verification: compare material fields, including disputed settings, before closing publication.

## Reconcile quantity after a sale or revision

Compare local lot count, purchasable listing quantity, active quantity, sold orders and cancelled/returned allocations. A pair is one purchasable lot of two components. Two sold lots can belong to one order. Do not decrement a component twice because both the listing and order changed. An auction that closes with no bids releases the lot only after the closed state is verified; automatic reactivation, if set, continues the exposure.

If they disagree, freeze new allocation of only the disputed units. Follow each physical ID through reservations and orders. An unpaid order can still reserve stock. Cancellation releases stock only with observed disposition; return receipt does not make it sellable until reconciled. Archive historical allocation events without erasing them.

## Draft migration procedure

1. Record the reason, old draft ID or reference and exact reserved lots.
2. Save the original content/settings and approved photo map locally.
3. Create one replacement draft through the supported flow; retain one stock reservation tied to the intended replacement, not duplicate availability.
4. Compare every material field; a new draft can reset category, format, start price, duration, shipping, payment, quantity or option settings to defaults.
5. Mark the old draft superseded in local history. Deleting it is a separate action subject to authority; avoiding publication is enough for bookkeeping.
6. Publish and verify only the selected draft within existing authority. Reconcile any uncertain result before retrying.

## Recover without burdening the seller

Distinguish agent-resolvable work from user-only actions. Recheck a misunderstood photograph, unsupported constraint or available UI path yourself first. If a seller action remains, give the exact destination, the one action needed and what can continue meanwhile. Do not ask them to repair a CLI tracker, create routine project files or redo facts already present in the archive.

Never remove a real blocker merely because the seller is frustrated. Explain the concrete evidence gap and its consequence, not a generic warning. Conversely, never preserve an invented blocker just because it was entered into Beads.
