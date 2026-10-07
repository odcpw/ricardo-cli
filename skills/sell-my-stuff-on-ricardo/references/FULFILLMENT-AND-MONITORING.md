# Fulfilment and monitoring

## Make the packing plan usable

Use measured packed weight and outside dimensions when available. If the seller delegates estimates, provide conservative estimates explicitly labeled as such: product and accessory weights, carton weight, cushioning, and a plausible arrangement in a standard box. Record specification sources and distinguish inside carton dimensions from the outside dimensions used for postage. Do not silently turn an estimate into a measurement.

Compare eligible carrier services using the actual destination assumptions, delivery expectations, package size and weight limits, and any signature or insurance needs. Domestic shipping within Switzerland is the default; the services, limits and prices are the carrier's current ones (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (shipping)). The seller sets the shipping cost in the listing form, so a wrong estimate is the seller's loss: keep a downside case. Do not infer package eligibility from a product's bare weight alone.

Specify cushioning, immobilization, protected fragile parts, and separation of heavy components. Preserve useful original packaging inside an appropriate outer carton. Keep each item's correct accessories together. Identify special constraints for the actual goods, such as liquids, batteries, perishables, fragile surfaces, or unusually large loads, using current carrier guidance.

If reality differs when packed, update the cost calculation and the shipping declaration. Delegated estimation does not require repeated pre-listing measurement requests, but a known incorrect package declaration must be corrected before buying postage or booking a pickup.

## Handoff for each paid order

Verify the order in the selling account (`ricardo selling sold --json` if listed, else the seller's sold-orders page) before allocating fulfilment; do not rely on a notification email alone. Provide one compact handoff:

- Exact unit or lot and storage reference, with an identifying approved photograph if useful.
- Every included accessory and explicit exclusions likely to cause confusion.
- Packing arrangement, chosen service, and whether dimensions/weight are measured or estimated.
- Ship-by date in the seller's timezone and the next required physical action (post-office drop-off or the booked carrier home pickup).
- Existing label access or the precise remaining label action; never buy a duplicate label because navigation failed. The Ricardo/Swiss Post label is obtained through the order, is for parcels only, is charged to the seller's Ricardo account when the Post scans it (an unused label costs nothing), and the carrier's home pickup (pick@home) is free with a Ricardo label but cannot be changed or cancelled once booked (RICARDO-PLATFORM.md (shipping)).

Buyer identities, addresses, full order identifiers, labels, tracking numbers and correspondence remain in Ricardo or a local-only private store. The tracked ledger uses only a local opaque order reference and non-identifying inventory, deadline, and status evidence. Ricardo's order ID, tracking numbers and order-level amounts live in the ignored `private/orders.json` keyed by that local reference ([private order data](WORKSPACE-SCHEMA.md#private-order-data)).

## Keep evidence states separate

Ricardo's own seller buckets (`sellerToSend`, `sellerPaymentToCheck`, `sellerToRate`, cancellation buckets in `status_counts` of `ricardo selling sold`, if listed) order the work queue; they are the marketplace's classification, not proof of physical events. Field-by-field mapping: [workspace schema](WORKSPACE-SCHEMA.md#ricardo-fields-to-ledger-states).

| State | Required evidence |
|---|---|
| Ordered | Order present in the sold list; payment may still be pending |
| Paid | `payment.status` paid or `buyer_has_paid: true`; for cash or bank transfer, the seller's own receipt |
| Label created / pickup booked | Purchased or generated label, or a carrier home-pickup booking; no physical handoff implied |
| Handoff reported | Seller reports handing over the parcel; carrier scan still unverified |
| Seller-confirmed shipment | `shipping_status: shipment_confirmed` with a tracking number; this is a seller-side confirmation |
| Carrier accepted | Carrier acceptance/collection scan read from the carrier's tracking page |
| In transit | Carrier movement event |
| Delivered | Carrier delivery event and/or `delivery_status` delivered or received; return obligations may remain |
| Payout unlocked / sent | Ricardo's `payout_unlocked_at`, or the payment service's own transfer status; neither is a deposit |
| Bank arrived | Deposit confirmed separately; a payout estimate or unlock alone is insufficient |

An overview status that says shipped or confirmed may follow the seller's own click or label creation. Read the detailed tracking before declaring carrier acceptance. Maintain handoff-reported separately if appropriate; move stock according to the documented shipment rule and preserve its evidence basis. Do not close fulfilment at publication, label purchase, or delivery while unresolved return work remains.

Keep source event time, observation time and predicted delivery date separate. Seeing "delivered" today does not prove delivery occurred today. Store the actual carrier event timestamp when available, and a separate verified-at timestamp for your observation. If the actual event time is unknown, leave that uncertainty explicit rather than backfilling today's date; the event-chronology validator needs the corresponding actual-event evidence before it can certify that transition. Never relabel an observation as an event.

When there is only a label or a pickup booking and no handoff report, the next seller action is packing and physical carrier handoff, not merely waiting for a scan. If the seller already reports handoff, preserve that report without pretending it is carrier evidence. Late dispatch can cancel a protected-payment sale and refund the buyer, so escalate approaching or missed ship-by deadlines immediately (the window differs between protected and direct payment; clocks: [decision engine](DECISION-ENGINE.md#platform-clocks); the order shows the actual dates); otherwise use the carrier's stated scan-processing window and the reported handoff time to decide the next check. Do not invent a universal grace period.

For a return, dispute or cancellation, track the response deadline, the case state in Ricardo (`dispute_state`, cancellation buckets), authorization, parcel receipt, condition reconciliation, and refund outcome. Keep returned units unavailable until inspected/reconciled; do not automatically restore them to a live listing. Follow the actual case and current marketplace obligations rather than assuming a stated return policy governs every dispute (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (returns, disputes)).

For pickup orders (`delivery.method_label: get_by_buyer` on Ricardo, or any Tutti sale), use the separate canonical pickup states and fields in the workspace schema. The seller's handover report with payment evidence changes the lot to `collected`; arranged time, payment and buyer intent do not. If the buyer pays in cash (or otherwise) at the handover, set `payment_due_at_handover: true`: the arrangement may precede payment, and payment plus handover are recorded together at collection. No shipping label or carrier timestamp is required or permitted in that branch. Keep the pickup address and access details private, and apply the same return and financial reconciliation discipline after collection.

## Operate the watch

Set cadence, timezone, scope, and notification preferences with the user. Prefer the current thread's supported automation mechanism. Verify an actual scheduled execution can access the signed-in selling account and the bridge; an automation card or successful interactive session does not prove scheduled access. Until then, report "checked interactively; scheduled monitoring unverified," not "being monitored."

Use a two-step cadence. The **heartbeat** is `ricardo selling status --json` (if listed): selling limits, a pending-action flag and an unread-message count. When the pending-action flag is set, the unread count rose, or a limit moved, run `ricardo selling sold --json` (if listed) and then use the browser for message contents and case details. The **daily pass** runs `selling sold` regardless, because rating-pending and cancellation states may not set the flag. Run one command at a time; the bridge is paced and a retry loop is a failure of the monitor.

Each run compares fresh observations with the previous verified state:

1. Orders/payment and approaching or missed shipping and pickup deadlines.
2. Carrier acceptance, delivery, and exceptions for open shipments (tracking pages in the browser).
3. Buyer questions on Ricardo and messages on Ricardo and Tutti, received offers and expirations, returns/disputes and response deadlines.
4. Account alerts (`ricardo notifications --json`) and access failures; payouts only when authorized and accessible.
5. Active quantities (`ricardo selling --json`, `ricardo inspect`) against inventory reservations and sales, including any listing that ended unsold or reactivated.

Record time, checked surfaces, evidence, and gaps. Access to one account page does not verify every dashboard. A missing dedicated returns view means incomplete coverage, not "no returns." A heartbeat that reads only counts covers the account surface, not message content. Report failed access instead of claiming a successful check. Reconcile external state before retrying any ambiguous mutation.

For MFA or challenge prompts, identify the exact page requiring the seller's sign-in and continue accessible checks. Do not change security settings, bypass authentication, invent integrations or silently reduce promised monitoring coverage to call the watch verified. Report scheduled execution proof and per-surface access separately. A CLI `upstream_blocked` or `rate_limit_cooldown` (exit 5) pauses requests for two minutes: report it, do not poll. `request_busy` only means another local command is running.

Default scheduled watches are read-only and send no buyer messages, answers to questions, offers, discounts, refunds, purchases, or listing changes. Notify only on actionable changes, deadlines, or access failures. Avoid repeated unchanged alerts unless the approaching deadline makes another reminder useful. Review pricing on the agreed schedule without automatically reducing it.

## Buyer communication and workload

Draft concise replies in the buyer's language (German, French or Italian) grounded in facts, with the exact intended recipient and order identified privately. Explicit send authority can persist for a narrowly defined message or scope; do not repeatedly request the same permission, and do not expand it to future automated messages. Before sending, confirm the correct thread, check for an already-sent copy, and ensure any supplied shipping day has not passed or become known to be infeasible. Verify the sent result once. Replies and public answers to listing questions are sent from the seller's browser; the CLI cannot send them.

Listings, questions and ratings may not carry contact details, and sales outside Ricardo are not allowed; IBANs, pickup details and tracking numbers are exchanged only in Ricardo's post-sale messages, where they stay (RICARDO-PLATFORM.md (listing content rules)). A price proposal on a fixed-price listing expires after a short window if nobody answers, so record each as a `response_by` event the moment it appears and alert before it lapses; answering is the seller's decision under offer authority. Treat requests to move off the platform, pay through an unfamiliar link, send a code, or ship before payment is visible as unverified: report them to the seller instead of acting. If illness or another disruption affects fulfilment (a holiday mode with Ricardo Plus extends deadlines only for items sold during the absence: RICARDO-PLATFORM.md (payments)), surface the concrete deadlines and prepare an honest delay message. Never promise a shipping day the seller has not supplied or authorized. Prioritize pending orders over more listings when packing capacity is the bottleneck. Monitor buyer responses without inventing a need to reply.
