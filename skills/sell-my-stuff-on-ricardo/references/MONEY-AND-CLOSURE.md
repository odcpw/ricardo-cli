# Reconcile real money and finish the sale

Use once orders, labels, refunds or payouts exist. Forecast economics and cash movements are different records. This reference does not provide tax advice or assert a universal fee schedule or payment flow (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (fees, payments)).

## Four separate questions

1. **What did the buyer pay?** Item price, shipping and discounts as distinct amounts. In `ricardo selling sold` (if listed): `unit_price_chf`, `shipping_cost_chf`, `total_price_chf`.
2. **What did selling cost?** The marketplace success fee (`seller_success_fee_chf`, rate `seller_success_fee_rate`), postage, packing, adjustments, refunds and any authorized options or other costs.
3. **Where are the funds?** Whether payment was received (`payment.status`, `buyer_has_paid`), whether Ricardo released funds (`payout_unlocked_at`), whether a payout or transfer was sent, and separately verified bank or cash receipt. For cash at pickup the seller's own receipt is the evidence.
4. **What remains owed or at risk?** Open disputes, cancellations, returns, refunds, unpaid success fee or other unresolved debit.

An available balance or an "unlocked" flag is a point-in-time account fact, not lifetime revenue or profit. A zero or blank overview card cannot override itemized order evidence. Two payment regimes exist: protected integrated payment (funds held, then paid out) and direct payment (bank-transfer prepayment or cash on pickup, with no payout). Read the order's `transaction_type` to see which applies; for direct payment the seller's bank or cash receipt is the evidence of money received. How the success fee is collected (deducted from a payout, or billed in periods and invoiced) and its deadlines changed recently: verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (payments, success fee, billing).

## Replace the fee assumption with the recorded fee

The fee in the forecast was an assumption; after a sale, the sold order carries Ricardo's own per-order fee record.

1. Read the order's `seller_success_fee_chf` and `seller_success_fee_rate`. The rate differs between orders: use each order's own value, never one order's rate for another, and never to back-fill a quote for different goods.
2. Infer the fee base: `fee_chf ÷ rate`. Compare it with `unit_price_chf` and `total_price_chf` (item plus shipping), allowing for rounding: RICARDO-PLATFORM.md (live observations) records one order where the fee equalled the rate times the item price excluding shipping, another where it was rounded up to the next five Rappen, and a conflict with the relayed terms (which say shipping is included). Record which base the order shows as the `fee_basis` for later forecasts, labeled as one observation, and re-check on the first sale after the 2026-10-01 terms.
3. Put the actual fee into the order's reconciliation and the tracked `fee_basis` evidence; keep the forecast beside it and note the difference by cause (rate, base, shipping inclusion, a combined order).
4. For the remaining stock, rerun `sales-assistant.py economics` with the observed rate as a **scenario**, alongside the earlier assumption, rather than overwriting it. Confirm the next order's fee when it exists.

## Minimal private reconciliation ledger

Keep marketplace financial records and bank records local-only. For tracked analysis, use opaque order/event IDs and sanitized amounts:

```text
event_id | local_order_ref | kind | amount | currency | occurred_at
state = pending/posted/reversed | evidence_ref | supersedes_event_id
```

Keep a separate `funds_observations` collection: `observed_at`, `funds_state` (`processing`, `held`, `available`, `payout_sent`, `bank_arrived`, or `cash_received` for in-person payment), `amount`, `currency` and `evidence_ref`, optionally linked to order/payout references. These are observations of balances and transfers, not additive profit transactions. `posted` describes a transaction's ledger status, not whether its proceeds are available.

Kinds can include item revenue, shipping revenue, success fee, postage charge, packing cost, carrier adjustment, refund, fee credit, payout or transfer confirmation, cash received. Choose one sign convention and document it. Link refunds and credits to their original events so retries and duplicate exports cannot double count them.

Do not add order net proceeds and their later payout as two revenues. Do not subtract an already-netted fee twice. Distinguish estimated costs from posted transactions and seller-paid supplies from buyer shipping collected. Multiple orders can share one payout; one order may have multiple adjustments. Match many-to-many by actual statement evidence instead of forcing an order-to-payout equality.

Seller-paid packing and postage outside the marketplace affect economic contribution but may never debit an account balance. Therefore do not sum order profits, subtract holds and call that the payout. Reconcile account cash from actual account transactions, then separately reconcile seller costs and bank deposits. Do not store IBANs, account holders or statements in the tracked workspace.

## Reconciliation procedure

1. Verify the order's actual item and shipping amounts and currency (`ricardo selling sold`, or the order page).
2. Read the marketplace's order and transaction detail for the fee, credits, holds and adjustments.
3. Match postage purchases, voided labels and carrier adjustments. A Ricardo/Swiss Post label is billed to the seller's Ricardo account only when the Post scans the parcel and an unused label costs nothing, so a created label is not yet a cost; re-weighing can move a parcel into a higher bracket. Preserve a pending credit until posted.
4. Add actual packing and other seller costs and calculate realized contribution separately from the original forecast.
5. Compare forecast versus actual by cause: fee base, package geometry, destination, combined order, return or initial assumption. Feed the useful correction into remaining inventory plans.
6. Track payment received, funds released, payout sent and bank arrival independently. Confirm bank arrival only from appropriate separate evidence within account authority; otherwise retain unknown.
7. Close financial work only when material pending adjustments and cases are reconciled. Retain records consistent with the seller's applicable needs (and tax obligations, which this skill does not advise on); do not invent a universal retention period.

## Fee invoices and tax

Under the billing system in force from 2026-10-01 (RICARDO-PLATFORM.md (billing), verify live), fees, credits and refunds are bundled per period, an invoice follows, private accounts have a payment term, an overdue reminder carries a fee, and a negative balance becomes a credit note paid to the stored bank account. Treat an invoice as an obligation with a due date in the monitor, reconcile it against the per-order fees it contains, and never pay or dispute it without authority. Under the protected-payment terms, commission and fees are deducted before payout; whether that still holds under the new billing was not verified. Ricardo collects VAT on sales to Swiss and Liechtenstein buyers and private sellers are told they need not act (RICARDO-PLATFORM.md (fees)); the skill gives no tax advice, so keep the figures and let the seller or their adviser decide.

## Returns, cancellations and exceptions

For each case, capture case type, actual response deadline/timezone, last verified status (`dispute_state`, cancellation buckets, the case page), next required actor, authorized options and evidence. A non-delivery claim and a voluntary return can have different obligations; read the actual case (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (returns, disputes)). Delivery does not make a dispute impossible.

For cancellation, verify the marketplace outcome before releasing the lot. For a return, reconcile exact unit/accessories, return condition, deadline and refund outcome; keep it quarantined until disposition. A changed serial or missing accessory is an evidence discrepancy, not permission to accuse the buyer. Draft factual replies and recommendations; sending, refunds or dispute actions follow current authority and happen in the seller's browser.

A buyer who does not pay or collect is handled through the platform's own process: after the contact steps in the [platform clocks](DECISION-ENGINE.md#platform-clocks) the seller may cancel the sale within the window the file records, the commission is credited, and the at-fault party usually receives a negative rating. A goodwill return can be recorded as a cancellation to recover the commission, while the buyer's postage stays with the buyer; an item not matching the description means refunding price and declared shipping (RICARDO-PLATFORM.md (unpaid buyers, returns)). Record the outcome and release the lot only after it is verified. Cancelling, refunding and rating are the seller's actions in the browser.

Preserve private packing and condition evidence where useful. Never publish buyer correspondence or address-containing screenshots to the repository. Keep notification concise: what changed, which opaque lot, deadline, recommended next action and any action requiring the seller.
