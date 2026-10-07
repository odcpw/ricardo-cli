# Optional decision worksheets

Use only the worksheet needed for a current decision. These are internal, sanitized artifacts; never paste them wholesale into buyer-facing copy. Platform facts referenced here (fees, formats, payment, returns, pickup, Tutti) come from [RICARDO-PLATFORM.md](../references/RICARDO-PLATFORM.md): verify live and record the date.

## Evidence and blocker adjudication

```text
claim_id / affected units:
Proposition and why it changes the sale:
Observed / owner-reported / manufacturer specification / inference / unknown:
Sources with dates:
Contradictory evidence:
Cheapest independent check:
What would falsify the current blocker:
Outcome / supersedes / dependent records updated:
```

## Transfer useful ideas from a successful listing

```text
comp_id / exact match quality:
Transferable presentation: useful sections, photo roles, explicit contents
Nontransferable advantages: warranty, testing, refurbishment, provenance
Our supported facts:
Our unsupported or unknown claims to omit:
Original title/description angle:
```

## Format and start price

Decide before the draft is filled. Use `sales-assistant.py economics` for the floors; the highest floor among the scenarios you must survive is the lowest defensible start price.

```text
lot / units / listing language:
Comparable evidence: exact sold (final price known) / ended (price unknown) / active asks, dates, channel; unknowns left unknown:
Seller goal: speed / price / certainty / effort (rank):
Net floor (min_net) and the highest minimum price across base and downside:
Options considered (price, duration, closing day/time, expected net, downside net):
  A  Ricardo auction: start price __, bid increment __, buy-now price __ or none, duration __ (binding and closing rules verified live)
  B  Ricardo buy-now only: price __
  C  Ricardo auction with buy-now: start __, buy-now __ (a price I would accept immediately)
  D  Tutti classified: asking price __, negotiation room authorized __
Chosen option / start price / buy-now price / duration / closing time and why:
Start price below the floor?  no / yes -> seller's explicit risk decision, wording, date, source:
Automatic reactivation: off / on (count __; the live form showed up to 9, the relayed terms say 3: record which applied) -> authority and what happens to the allocation on relisting:
Buyer price proposals (Preisvorschläge; fixed price and unsold ended listings only, per RICARDO-PLATFORM.md, verify live): not allowed / allowed with floor __, who answers within the short response window, authority wording:
Paid options (boosts Bronze/Silber/Gold; priced only in the form; billed per run and again on every reactivation): none (default) / authorized limit __:
Review date and what would change the choice (no silent markdown):
Fee evidence, per RICARDO-PLATFORM.md and the form's own figure on the day (verify live): commission rate for this product type (the file records 8 to 12 percent by type, minimum CHF 0.10, maximum CHF 290 per sale), stackable discounts of 10 percent of the commission amount (for example an auction start of CHF 1 or less, every fifth sale), base (item only / item + shipping; the observed orders showed item only), observation date, unknowns. A discount is a small saving and never a reason to go below the floor:
Saved-draft check: format, start price, buy-now price and duration persisted after reopening (compare-draft receipt ids):
```

## Ricardo vs Tutti vs pickup

```text
lot / size and weight / fragility / value band:
Buyer pool: national (parcel) vs local (hand over); Ricardo and Tutti comps compared on the same basis:
Option 1  Ricardo, shipped: delivered net (base/downside), commission for the product type, packing effort, post-office trip or pick@home, dispute exposure; payment regime (protected payment needs Ricardo Plus and the seller's own Swiss bank account; direct payment means prepayment or cash) and its dispatch clock
Option 2  Ricardo with Abholung (pickup) also offered: what the listing shows publicly of the location, payment at handover (cash on pickup sits outside the platform's seller protection per the file), pickup expected within the platform's window
Option 3  Tutti, pickup: free 60-day listing, no sales commission and no integrated payment found (per RICARDO-PLATFORM.md, verify live); asking price, expected net, image limit, safety of the handover (time, place, who is present), payment method and evidence
Option 4  Donate or recycle: where, effort, receipt not required unless the seller wants one
Option 5  Hold: reason, review date
One channel at a time: if the unit is moving from one channel to another, the first listing is ended and verified ended (inspect result, date) before the unit is reserved under the second
Chosen channel and fulfilment method (shipping | pickup):
Pickup details to prepare: area (no street address), days/times, payment evidence the seller will record, who may collect
Remaining uncertainty and the smallest seller action:
```

## Packing alternatives

```text
lot / contents / fragile orientations:
Layout A: arrangement, inner fit with padding, outer size, total mass, basis
Layout B: arrangement, inner fit with padding, outer size, total mass, basis
Carrier service and limits checked live (date):
Base/downside cost and net for each:
Seller labor/supplies required (post-office trip or home pickup booking):
Selected plan / remaining uncertainty / estimate authorization:
```

## Reopened-draft comparison

| Field | Intended source | Form value | Reopened value | Buyer preview | Result |
|---|---|---|---|---|---|
| Title / condition / category attributes | | | | | |
| Included contents / exclusions | | | | | |
| Quantity of lots / pieces per lot | | | | | |
| Photos and order (by slot) | | | | | |
| Format / start price / buy-now price / duration | | | | | |
| Offers / reactivation / paid options | | | | | |
| Shipping cost / pickup / payment / returns statement | | | | | |
| Reference number and draft identity | | | | | |

## Demand experiment

```text
experiment_id / listing / exact stock exposed:
Buyer hypothesis and evidence:
Baseline observation window and metric definition:
Smallest intervention / settings held fixed:
Review date and stop condition:
Independent orders versus units sold:
Packing/handoff capacity:
Observed result and attribution limits:
Next recommendation within authority:
```

## Short seller handoff

```text
Next action: <physical/account task>
For: <opaque order and exact unit/lot; storage reference>
Include / exclude:
Pack:
Label or pickup booking: <verified private account destination, no public copy>
Deadline: <actual date/time and timezone>
Evidence now: <paid / label or booking / reported handoff / carrier scan>
```
