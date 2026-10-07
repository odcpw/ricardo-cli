# Choose the next useful action

This is a decision procedure for an agent inheriting a real, unfinished sale. Use it instead of restarting intake or growing a backlog. It applies the authority, economics and outcome principles of the skill.

## Resume in one pass

1. Read the latest seller decision and its scope, then the canonical ledger, current Beads and last verified account coverage. Find the most recent evidence **per fact**, not the newest file overall.
2. Reconcile active marketplace commitments before creating more: sold orders awaiting dispatch or payment checks, unpaid stock reservations, open cases, drafts, active quantity and ambiguous submissions.
3. Run `sales-assistant.py next-actions PROJECT` for a local queue. Refresh live obligations: `ricardo selling status --json` then `ricardo selling sold --json` (both if listed by `ricardo capabilities --json`), otherwise the seller's Ricardo pages and Tutti messages in the browser. The queue and the CLI cannot see message contents or hidden cases.
4. Select one outcome: remove an urgent obligation, unlock a valuable ready listing, or resolve a high-value uncertainty. Perform it through its verification step.
5. Update only the affected records and tasks. End with achieved external state, remaining uncertainty and the smallest seller action.

Do not require a fresh intake answer when session evidence already supplies it. A new spreadsheet is a source, not a second inventory. A previous assistant's recommendation is not user authorization. A failed or blocked live read means the state is unknown, not that nothing is pending.

## Authority as a small decision table

Record `decision_id`, `decided_at`, `source`, `action`, `scope`, `limits`, `supersedes` and `review_condition` in the selling plan. The record points to the actual user decision; it cannot manufacture permission.

| Action | Scope to preserve | Typical boundary |
|---|---|---|
| Research, photo preparation, local copy | Selected goods and privacy rules | Proceed under selling task |
| Conservative package estimates | Which goods/routes, acceptable uncertainty | Later delegation can supersede earlier measurement-only plan |
| Publish/revise (in the seller's browser) | Exact listing/batch or defined autonomous policy | Apply existing authority after concrete verification |
| Format, start price, buy-now price, reactivation | Floors, firm asks, review dates, discretion | A net floor is not an instruction to accept it; a start price is a price commitment |
| Change prices; accept or reject a price proposal (Preisvorschlag) | Floors, firm asks, review dates, discretion | Never implied by research or monitoring authority. Accepting is binding and silence expires after a short window (see clocks below), so decide the policy in advance |
| Paid listing options (boosts), automatic reactivation | Amount limit, which listings | Default none; boosts are billed again on every reactivation, including automatic ones |
| Spend | Postage, packing, accessories, paid listing options, amount limits | Do not infer a purchase budget from research authority |
| Message a buyer / answer a question | Recipient/topic and wording or defined message policy | One approved delay message is not unlimited sending authority |
| Monitoring | Surfaces, schedule, notifications | A read-only watch does not authorize commercial mutations |

An explicit correction wins over a stale checklist. Keep the superseded record so future agents understand why behavior changed. A tool-enforced approval (such as accepting platform terms) cannot be bypassed; explain its source precisely. Otherwise do the already authorized work, not another permission ceremony.

## Rank by effect on the seller's outcome

Use this order, adjusting for actual deadlines:

1. **Protect existing obligations:** paid orders to send, payments to check, price proposals about to expire, pickups agreed, case or dispute deadlines, fee invoices, carrier exceptions, account actions blocking those obligations. Use the [platform clocks](#platform-clocks) and the dates shown on the order.
2. **Remove false or cheap blockers:** inspect the disputed photograph, reopen a draft, fix an unsupported claim, restore the correct category, reconcile already-completed work.
3. **Launch worthwhile ready stock:** strong identity and photos, credible economics, manageable physical work.
4. **Buy information with agent effort:** identify a label, research a scarce variant, compare packing layouts, resolve a category mismatch.
5. **Expand or improve demand:** controlled additional stock, useful description details, new buyer vocabulary, evidence-backed experiments.

Within a tier, compare plausible net unlocked, probability of resolution, agent time, seller minutes and consequences of delay. Use ordinal estimates when probabilities are unknown. Do not manufacture precision or rank a speculative expensive item above a paid order.

**Value-of-information worksheet:** uncertainty → decision it changes → best existing source → cheapest discriminating check → possible outcomes → next action for each → stop condition. Example: a label close-up may distinguish two valuable variants; another glamour photo may not change any decision. Ask the seller for the former only if the archive cannot resolve it.

## Platform clocks

Per [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (payment, shipping deadlines, price proposals, billing), observed 2026-10-07. Verify live and prefer the date shown on the order or listing; rules changed on 2026-10-01. Which regime an order is in is read from `payment.transaction_type` and the listing.

| Clock | Protected payment (Käuferschutz+) | Direct payment |
|---|---|---|
| Payment | Buyer pays by TWINT or card; Ricardo holds the money | Both sides make contact within 7 days of the end (not needed for prepayment when bank details are stored); buyer pays within 14 days of receiving payment details |
| Dispatch | Ship within 7 days after payment confirmation or the sale is cancelled and refunded; cancelled if neither shipment nor receipt is confirmed within 10 days of payment | Ship within 14 days after payment; pickup within 14 days |
| Buyer check | With a Ricardo label, 3 days after delivery to confirm or report, silence completes the sale; with the seller's own label, 6 days to confirm receipt then 3 days to report | Buyer protection claims run 30 to 60 days after the end |
| Money | Payout after the inspection period without complaint; faster payout only for qualifying accounts | Direct from the buyer; the success fee is billed separately |
| Buyer does not pay | Not applicable | After 7 or 14 days: contact twice by e-mail and once by phone, then cancel (possible 7 to 60 days after the end, commission credited) or insist |

Price proposals on fixed-price listings: 48 hours to accept or reject, silence expires, acceptance is binding. Fee invoices under the billing system in force from 2026-10-01: bundled per 30-day period with a payment term for private accounts and a reminder fee when overdue. With Ricardo Plus a holiday mode can extend dispatch deadlines only for items sold during the absence.

Turn each clock into a dated `ship_by_at`, `pickup_by_at` or `response_by` event with its timezone only after reading the real date; never derive a deadline from this table alone.

## Challenge the agent's own blockers

For each hold, state the exact claim, evidence, counterevidence and what observation would resolve it. Inspect full image and close-up; an empty slot, shadow or molded packaging recess is not proof of a missing accessory. A feature absent from one form view is not proof that the account cannot sell the item. A missing dashboard is not proof that there are no cases.

Try a bounded independent recheck before involving the seller. Release a false hold when evidence supports release and record why. Never reverse a genuine safety, identity or allocation uncertainty just to increase throughput. Hold the affected listing, not the whole project.

## Control the physical workload

The bottleneck is often seller energy, packing space or the post-office trip, not listing creation. Keep a visible limit on simultaneously exposed units and parcels that could sell before the next handoff; a short auction closing on a weekend is a dated commitment. Ask for capacity only if unknown and material; infer it conservatively from recent missed commitments.

Expand after observing both demand and operational capacity. Several units bought by one customer are one demand event and may become one packing event. A booked parcel pickup proves neither packing nor capacity. If illness disrupts fulfilment, prioritize the real obligations and prepare authorized logistics/message options before adding more inventory exposure.

## Creative alternatives with constraints

For an unattractive standalone shipment, generate at least three **feasible** alternatives: a coherent buyer-use-case bundle, a better packing layout or service, and pickup, Tutti or another appropriate channel, or hold. Compare costs, accessory ownership, demand evidence and seller labor. Additional accessories or repairs are hypotheses, not permission to spend or conceal defects.

For scarce goods with thin comps, test a bounded ask on a limited batch. Define the buyer hypothesis, observable signal, review date and reversible next step. An experiment can reveal interest without proving a liquidation value for all stock. The objective is better choices with less seller work, not more records.
