# Workspace schema

`init-project.py` requires Python 3.9+ and a destination that does not exist. It creates local files only; use the repository reference for GitHub and Beads. Existing source folders must be adapted without overwriting their files. The canonical `data/ledger.json` may coexist with exported CSV views, but edits happen in only one canonical ledger.

## Ledger records

Both ledger consumers reject duplicate JSON keys and nonfinite constants, and preserve fractional JSON numbers as decimals. Repeated keys cannot silently replace authorization or economics. Supplying either side of an event time/evidence pair requires both sides; incomplete shipment or payout evidence is not ignored. Listing photo allowlists must contain unique references in their intended display order.

| Collection | Meaning and required fields |
|---|---|
| `config` | `marketplace` (the primary channel: `ricardo` or `tutti`), `currency` (scaffold default `CHF`), nonnegative numeric `min_net`, boolean `packing_estimates_authorized`. Marketplace and `min_net` may be null in an empty scaffold but are required for launch. Each listing also records its own marketplace. |
| `units` | Atomic physical components: unique `id`; `disposition` is `eligible` (default), `held`, `excluded`, or `disposed`. Optional boolean `excluded: true` also excludes goods. Add identity/evidence/condition fields. Held units can belong only to held lots; excluded/disposed units cannot be allocated. |
| `lots` | One purchasable lot: unique `id`, nonempty `unit_ids`, `state`, `listing_ref` when reserved/listed. Each atomic unit belongs to at most one current lot, including accessories. |
| `photos` | Unique `id`, path inside `photos/listing`, reviewed `sha256`, boolean `approved_for_upload`, authorized `listing_refs`; add source hash, crop, rotation, metadata and visual-review evidence. |
| `listings` | Unique local `id`, `status`, `marketplace` (`ricardo` or `tutti`; required for draft and active), integer `quantity` in purchasable lots, string `draft_id`, public `listing_id`, offset `active_verified_at`, nonempty `active_evidence`, boolean `seller_confirmed` (required on an active Tutti listing); photo IDs, economics, blockers and dated check evidence. |
| `orders` | Local opaque `id`, nonempty string-list `lot_ids`, `status`, optional `fulfillment_method` (`shipping` by default, or `pickup`), optional boolean `payment_due_at_handover` (pickup only), and state evidence below. **No Ricardo order ID, tracking number, amount or buyer data**: see [private order data](#private-order-data). Shipping requires `ship_by_at` and `ship_by_timezone` for paid onward. Pickup uses its own arrangement and collection fields. |

**Naming.** `id` is the local record. `listing_ref` and `listing_refs` (on lots and photos) point to a local listing `id`. `listing_id` is only the marketplace's public identifier: a Ricardo article ID is a string of 6 to 15 digits; a Tutti listing is identified by its full canonical `https://www.tutti.ch/de/vi/.../ID` URL (no query or fragment). The validator rejects a mismatched pair, a bare Tutti number, and the same ad on two records: uniqueness is keyed on the Ricardo article ID or, for Tutti, on the trailing numeric ad ID, so URLs that differ only in slug or percent-encoding case collide. An `active` listing needs `marketplace`, a valid `listing_id`, `active_verified_at` and `active_evidence`. For Tutti, `listed` means the ad page declares availability; it is not seller-confirmed availability. An active Tutti listing therefore also carries `seller_confirmed: false` when the only evidence is that page reading (`ricardo tutti inspect` always reports `seller_confirmed: false`), and `true` only when the seller has seen the ad live in their own Tutti account. A Tutti `unavailable` or `unknown` reading is not an active listing.

`draft_id` is the identifier the seller's account shows for the saved draft. If the account exposes no stable draft ID, bind identity with the opaque reference number you typed into the draft (the Ricardo form has an optional Referenz-Nr. field, maximum 40 characters, observed 2026-10-07) and say so in `checks.draft_reopened.basis`; never a list position. Where a channel offers no saved draft, record the reference of the pre-publication state you reviewed and say that no draft exists. Use only an opaque local ID in that field (for example the local listing `id`), never names, addresses or contact details.

### One channel at a time

Simultaneous cross-listing of one unit (an auction and a Tutti ad, or two listings of any kind) is unsupported. The ledger cannot represent it: a unit belongs to one lot and a lot to one listing, and the validator rejects a unit in two lots. To move a unit between channels, end the first listing and verify that it ended (`status: ended` on `ricardo inspect`, or the seller's own page; for Tutti the ad removed or deactivated by the seller), set that listing `ended` with its evidence and release or hold the lot, and only then reserve the unit in a new lot under the second listing. Never reserve under the second channel first to save time.

### Private order data

The tracked `data/ledger.json` identifies an order only by its opaque local `id`. Ricardo's `order_id`, tracking numbers and tracking URLs, and order-level amounts (unit price, shipping cost, total, success fee and rate) are account data that must stay out of commits. Keep them in the ignored `private/orders.json` that `init-project.py` creates, keyed by the local order id:

```json
{"schema_version": 1,
 "orders": {"ORD-001": {"ricardo_order_id": "1234567890", "tracking_number": "…",
                        "total_price_chf": 60, "seller_success_fee_rate": 0.1}}}
```

The validator reads this map when it is present and never requires it: `ricardo_order_id` must be a digit-only string and unique across entries, amounts must be finite nonnegative numbers, tracking fields nonempty text, and every key must name an order in the ledger. It also rejects those fields (and `iban`, buyer or address fields) inside a tracked ledger order, naming `private/orders.json`. The map is free to hold other private notes, but never copy it into Git, chat summaries or the tracked evidence. Do not store a buyer name, address or payment reference beside the order ID even there.

`lots.state`: `draft_reserved`, `listed`, `sold`, `shipped`, `collected`, `returned`, `held`.
`listings.status`: `local`, `draft`, `active`, `ended`, `held`.
Shipping `orders.status`: `unpaid`, `paid`, `label_created`, `handoff_reported`, `carrier_accepted`, `in_transit`, `delivered`, `returned`, `cancelled`.
Pickup `orders.status`: `unpaid`, `paid`, `pickup_arranged`, `collected`, `returned`, `cancelled`.

An unpaid order still reserves its lots as `sold`; payment state remains explicit. Purchased labels and seller-reported handoff keep lots `sold` until carrier acceptance proves `shipped`. Pickup lots remain `sold` through arrangement and become `collected` only with collection proof. Returned lots remain quarantined. Sold/shipped/collected/returned lots require exactly one appropriate current order. Before resale, archive the old completed allocation/order event in sanitized history and reconcile a fresh current allocation; never erase the history or leave the unit simultaneously allocated to both sales (including across Ricardo and Tutti). Cancelled orders do not retain current ownership, but require dated `cancellation_disposition` evidence before releasing stock.

Listing `quantity` equals current `draft_reserved` lots for a draft or current `listed` lots for an active listing. It is not the number of components in a bundle. Draft-reserved lots link to local/draft listings; listed lots link to active listings. Historical sold lots may keep the listing link without inflating current quantity. Unassigned eligible units are available; unassigned held/excluded/disposed units retain those dispositions. Derive state counts from atomic unit membership, never add lots and components together. If the live listing is configured to reactivate automatically (the form observed 2026-10-07 offers this), treat each reactivation as continued exposure of the same lot. A manual relist ("Erneut verkaufen") that changes anything creates a new article number and drops watchers (RICARDO-PLATFORM.md (formats)): record it as a new listing record with the same lot, mark the old one `ended`, and verify the new ID with `ricardo inspect` before closing publication. Whether an automatic reactivation changes the article number is unverified: check after the first one.

## Ricardo fields to ledger states

Read from `ricardo selling sold` in CLI versions that list it, or from the seller's Verkauft page. These are Ricardo's records; the ledger states below still need their dated evidence pairs. Verify field meanings live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (payments, shipping).

| Ricardo observation | Ledger state and evidence |
|---|---|
| Order present in `selling sold`; payment not yet recorded | `unpaid`, lot `sold`; `order_id` goes to `private/orders.json` under the local order id |
| `payment.status` paid or `buyer_has_paid: true` | `paid` with `paid_at`/`paid_evidence` (field names, order reference and `checked_at`; for cash or bank transfer, the seller's receipt) |
| `delivery.shipping_status: pending`, no tracking | `paid`; bucket `sellerToSend` means dispatch work remains |
| `shipment_confirmed` with a tracking number, no scan seen | `label_created` or `handoff_reported`: seller-side confirmation, lot stays `sold` |
| Carrier scan read from the tracking page opened in the browser | `carrier_accepted`, then `in_transit`; lot `shipped` |
| `delivery.shipping_status`/`delivery_status` delivered or received | `delivered`, with the carrier event time if known; actual event time versus observation time stay separate |
| `delivery.method_label: get_by_buyer` | Set `fulfillment_method: pickup`; `pickup_arranged` needs an agreed time; `collected` needs the seller's handover report with payment evidence. If `payment.method` is cash (payment at handover), set `payment_due_at_handover: true` |
| `parcel_pickup` booking present | Arrangement only (not a handoff) |
| `payment.transaction_type` (for example `NOT_SECURED`) | Which payment regime the order is in (protected payment with a payout, or direct payment with none); sets the clocks and the money evidence |
| `payment.payout_unlocked_at` | Funds released by Ricardo: neither `payout_sent_at` nor `bank_arrived_at` |
| `dispute_state` non-empty, or `sellerCancelationRequested` / `sellerCancelled` buckets | Case work; cancelled orders need `cancellation_disposition` before stock is released |
| `sellerToRate` bucket | Post-sale rating task; not a fulfilment blocker |

## Dates and state evidence

Timestamps use ISO 8601 with seconds and a UTC offset or `Z`; naive local datetimes and boolean flags are rejected. `ship_by_at` uses the seller's local offset, matching the IANA `ship_by_timezone` (normally `Europe/Zurich`) at that instant. State pairs are `<event>_at` plus nonempty `<event>_evidence` identifying the observed source. A timestamp alone is insufficient.

`ship_by_at` and `pickup_by_at` come from the dates the order and listing show, which depend on the payment regime (protected payment or direct payment: [platform clocks](DECISION-ENGINE.md#platform-clocks)); do not compute them from a table alone.

Paid and later orders require `paid_at`/`paid_evidence`. For shipping, label-created orders require `label_created_at`/`label_created_evidence`; handoff-reported orders additionally require `handoff_reported_at`/`handoff_reported_evidence`. Carrier-accepted, in-transit, delivered, and returned shipping orders require `carrier_accepted_at`/`carrier_accepted_evidence`; the latter three also require their own event pair. Later observed scans need not invent an earlier label or handoff event. Supplied events must follow payment → label → handoff → acceptance → transit → delivery → return chronology. Status must not lag a later recorded event; unpaid orders cannot carry later payment/shipment events.

Optional `payout_sent_at` and `bank_arrived_at` each require separate corresponding evidence, and apply only when the payment route involves a payout. Boolean `bank_arrived: true` requires both valid transfer and deposit records in chronological order; a marketplace notice does not establish a deposit. These financial checks and the cancellation-disposition requirement apply to both fulfillment methods.

## Local pickup (Abholung)

Use the same canonical ledger and validator with `orders.fulfillment_method: "pickup"`; do not create a sidecar allocation ledger, pretend carrier events exist, or skip validation. Omission of the method preserves shipping behavior. Pickup is a first-class path on both Ricardo and Tutti.

| Order status | Required evidence | Current lot state |
|---|---|---|
| `unpaid` | Verified order or agreed sale; no payment/collection events | `sold` |
| `paid` | `paid_at` and `paid_evidence`; no invented collection deadline (not used when `payment_due_at_handover` is true) | `sold` |
| `pickup_arranged` | Payment (unless payment is due at handover), `pickup_arranged_at` and `pickup_arranged_evidence`, plus `pickup_by_at` and `pickup_by_timezone` | `sold` |
| `collected` | Payment, `collected_at` and `collected_evidence` | `collected` |
| `returned` | Payment, collection proof, `returned_at` and `returned_evidence`; condition/disposition still unresolved | `returned` |
| `cancelled` | Dated `cancellation_disposition`; preserve any historical events | Released or held after reconciliation; cancelled order owns no current allocation |

**Payment at handover.** Pickups on Ricardo and Tutti are often cash (or another payment) taken when the goods change hands. Set `payment_due_at_handover: true` on such an order (pickup only; it must be boolean). Then `pickup_arranged` is valid before any payment evidence and the lot stays `sold`; the status path is `unpaid` → `pickup_arranged` → `collected`, and a `paid` status is rejected as contradictory (if payment arrived earlier, clear the flag). At `collected` (and `returned`) both `paid_at`/`paid_evidence` and `collected_at`/`collected_evidence` are required: record the seller's report of the payment actually received and of the handover together. Payment is not ordered against collection (they happen at the same moment), but it cannot precede the arrangement or follow a return. A `paid_at` recorded while the status is still `pickup_arranged` means the status lags the evidence. Without the flag the default stays pay-first, as in the table.

Collection evidence describes what the seller actually observed: the seller's own report of the handover, payment received (cash or otherwise), and any marketplace status. A buyer promising to arrive, an appointment or payment alone does not prove collection, and a channel may offer no collection confirmation at all (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md), pickup). Keep addresses, access details, participant identities and private arrangements outside Git. The validator checks structure and sequence; it cannot authenticate the physical handoff.

Events use offset timestamps and nonempty source evidence, ordered payment → pickup arranged → collected → returned. Observed collection does not require inventing an earlier arrangement event. `pickup_by_at` uses the seller's local offset matching the IANA `pickup_by_timezone` at that instant; both are mandatory while `pickup_arranged`, and if either is recorded at another stage the pair must remain valid. A late collection may truthfully follow the deadline; retain the history.

Pickup rejects shipping-only statuses, `ship_by_*` fields, and label/handoff/carrier/in-transit/delivery event fields. Shipping rejects pickup arrangement/collection events and `pickup_by_*` fields. Remove inappropriate empty template fields rather than inventing evidence. Collected lots are not available for resale; returned lots remain quarantined until condition/disposition and the historical allocation are reconciled.

For launch readiness, the existing check names remain stable. Record the actual pickup arrangements under `shipping_returns_handling` and the safe handling/load plan under `packing_supplies`; explain why postage is inapplicable. Use zero postage and zero shipping collected when true, while retaining actual handling/packing costs and verified fee rules. These documented zeroes are valid inputs, not an exemption from economic or account checks. Pickup authority does not authorize refunds or prove payout/bank arrival.

## Readiness checks

`--listing LIST-001` also requires a saved draft, positive matching quantity, a valid marketplace, no blockers, photos approved for that listing, complete economics and evidence for each check below. Every check value is an object with `verified_at` (offset timestamp) and `basis` (nonempty sanitized observation/source), not a checkbox set to make the validator pass. This same evidence object is used for `fee_basis`, `packing_basis`, and `cancellation_disposition`.

```json
{
  "checks": {
    "identity_contents": {"verified_at": "<ISO timestamp with offset>", "basis": "<observed contents and source>"},
    "condition_claims": {"verified_at": "<ISO timestamp with offset>", "basis": "<claim evidence>"},
    "photo_match": {"verified_at": "<ISO timestamp with offset>", "basis": "<visual review>"},
    "category_attributes": {"verified_at": "<ISO timestamp with offset>", "basis": "<saved category and attribute fields>"},
    "account_limits": {"verified_at": "<ISO timestamp with offset>", "basis": "<selling status or account surface>"},
    "notification_access": {"verified_at": "<ISO timestamp with offset>", "basis": "<notification surface>"},
    "packing_supplies": {"verified_at": "<ISO timestamp with offset>", "basis": "<available supplies or feasible preparation plan>"},
    "shipping_returns_handling": {"verified_at": "<ISO timestamp with offset>", "basis": "<saved settings>"},
    "offers_promotions": {"verified_at": "<ISO timestamp with offset>", "basis": "<saved settings>"},
    "draft_reopened": {"verified_at": "<ISO timestamp with offset>", "basis": "<reopened draft>"},
    "buyer_preview": {"verified_at": "<ISO timestamp with offset>", "basis": "<preview comparison>"},
    "publication_authority": {"verified_at": "<ISO timestamp with offset>", "basis": "<current scope and decision>"}
  }
}
```

Economics fields are nonnegative finite JSON numbers (not strings or booleans) in the configured currency: `item_price`, `shipping_collected`, `fee_base`, fractional `fee_rate`, `order_fee`, `postage`, `packing`, `other_costs`, `risk_allowance`. For an auction, `item_price` is a scenario price (start price, a realistic outcome or the buy-now price); record the scenario in `fee_basis`. `fee_base` is the independently determined fee-bearing transaction base, not always item price. If fees are tiered, either calculate a documented effective rate at this price or record the additional charge in `other_costs`; avoid double counting. After a sale, replace the assumption with that order's own recorded fee and rate. Include dated `fee_basis`, dated `packing_basis`, and required boolean `packing_estimated`. Estimates require the config authorization flag to be true.

Recompute at the candidate offer or closing price and for combined orders. The script checks one provided scenario, not all destinations or future adjustments. A passing net does not authorize accepting an offer. Measured and estimated packing values remain distinct.

## Scope limits

Checks reject duplicate IDs/allocations, unknown members, held/excluded goods allocated improperly, changed approved photo hashes, quantity conflicts, missing active-listing proof, malformed or duplicated marketplace identifiers and order IDs, reused current order lots, inconsistent shipment states, unsupported bank-arrival flags and incomplete/below-floor launch economics. A full run checks all approved derivatives; `--listing` scopes photo-file and readiness checks to that listing while retaining global identity/allocation/state integrity checks. Unrelated held goods do not need photos or launch economics to pass a targeted check.

The checks do not recognize faces, inspect product condition, validate the truth or freshness of evidence, calculate current Ricardo or Tutti fees, read either site, buy labels, ship parcels or publish listings. Dates establish syntactic validity and sequence, not physical facts. Agent and seller review of actual evidence remains necessary.
