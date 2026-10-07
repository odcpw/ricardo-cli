# Local decision tools

## Table of contents

- [Command map](#command-map)
- [Model base and downside economics](#model-base-and-downside-economics)
- [Resume with a bounded queue](#resume-with-a-bounded-queue)
- [Produce a listing worksheet](#produce-a-listing-worksheet)
- [Compare real draft and preview receipts](#compare-real-draft-and-preview-receipts)
- [Test the helpers](#test-the-helpers)

These tools read seller-supplied local records. They do not browse Ricardo or Tutti, mutate the ledger, publish, message buyers, buy labels, accept offers or establish authority; live reads belong to the [`ricardo` CLI](RICARDO-CLI.md). Their output is advisory. Run from this skill's directory with Python 3.9 or newer (standard library only).

## Command map

| Need | Command | Proof boundary |
|---|---|---|
| Test sale economics | `./scripts/sales-assistant.py economics /absolute/path/scenarios.json` | Explicit linear fee assumptions; no current fee lookup |
| Resume useful work | `./scripts/sales-assistant.py next-actions /absolute/path/project` | Local queue, not a live account check |
| Inspect one listing's preparation | `./scripts/sales-assistant.py listing-packet /absolute/path/project --listing LIST-001` | Internal worksheet; no fabricated buyer copy |
| Compare saved form fields to intent | `./scripts/compare-draft.py /absolute/path/reopened-receipts.json` | Supplied, identity-bound receipts; not a browser check |
| Validate ledger and approved photo hashes | `./scripts/validate-project.py /absolute/path/project --listing LIST-001` | Structural checks plus separately required visual/account review |
| Compare monitor snapshots | `./scripts/monitor-diff.py before.json current.json` | Supplied, sanitized snapshots; see the [monitor protocol](MONITOR-PROTOCOL.md) |

All assistant commands emit JSON to stdout. Malformed inputs produce a nonzero exit and a concise stderr error. A successful queue/packet command may contain structural or readiness errors: it successfully **reported** those errors and does not confer readiness. Check those arrays before actions. No files are written. Keep reports local until their IDs, paths and fields have passed privacy review.

## Model base and downside economics

Supply one scenario for the ordinary destination and packing case and another for plausible downside costs. For an auction, add scenarios at the start price, at a realistic closing price and at the buy-now price: the start price is the lowest outcome you could be held to. Neither is a guaranteed result. All money and rate fields are finite, nonnegative JSON numbers, not strings or booleans. Rates are fractions. `currency` defaults to `CHF` when omitted (the output sets `currency_defaulted: true`); an empty currency is an error. `minor_unit_digits` is explicit.

The numbers below are **placeholders for arithmetic, not Ricardo fees or carrier prices**. Replace every value with fees verified live (see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (fees)) and a current carrier quote. Set `includes_shipping` from the evidence (the orders recorded in RICARDO-PLATFORM.md showed a base excluding shipping, against the relayed terms; verify on the first sale). After a sale, use that order's recorded success-fee rate and fee base ([money and closure](MONEY-AND-CLOSURE.md#replace-the-fee-assumption-with-the-recorded-fee)).

```json
{
  "currency": "CHF",
  "minor_unit_digits": 2,
  "min_net": 20,
  "scenarios": [
    {
      "name": "base",
      "basis": "PLACEHOLDER arithmetic; replace with live fee and quote evidence",
      "item_price": 60,
      "shipping_collected": 10,
      "postage": 9,
      "packing": 2,
      "other_costs": 0,
      "risk_allowance": 3,
      "fee_model": {
        "rate": 0.0625,
        "fixed_fee": 0,
        "tax_rate": 0,
        "includes_shipping": false,
        "includes_tax": false
      }
    },
    {
      "name": "downside",
      "basis": "PLACEHOLDER arithmetic; larger parcel and a lower closing price",
      "item_price": 45,
      "shipping_collected": 10,
      "postage": 14,
      "packing": 4,
      "other_costs": 0,
      "risk_allowance": 6,
      "fee_model": {
        "rate": 0.0625,
        "fixed_fee": 0,
        "tax_rate": 0,
        "includes_shipping": false,
        "includes_tax": false
      }
    }
  ]
}
```

The output gives modeled fee base, fees, net, margin above the floor and the minimum item price satisfying that floor in **each supplied model**. Monetary outputs are decimal strings to retain arithmetic precision. The minimum price rounds upward to the configured currency increment; net/fees are unrounded model outputs. Actual marketplace per-line rounding can differ. Use the highest `minimum_item_price_in_this_model` among the scenarios you must survive (normally the downside) as the lower bound for an auction start price or a buy-now price, never as the ask.

Finite monetary arithmetic uses an isolated precision budget and rejects any inexact intermediate result. The price floor uses rational division plus an integer ceiling, so a tiny positive remainder cannot round down to a currency increment. Inputs exceeding an aggregate 10,000-digit/exponent work budget are rejected before calculation; this is a local resource limit. Large finite values such as `1e999` remain supported without losing small offsetting costs.

For `P` item price, `S` shipping collected, proportional rate `r`, fixed fee `f`, cost sum `C`, shipping-in-fee-base switch `b` and tax multiplier `k`, the model is:

```text
fee base = k × (P + b × S)
net      = P + S − r × fee base − f − C
floor P  = max(0, (minimum net + C + f − S + r × k × b × S) / (1 − r × k))
```

`k = 1 + tax_rate` when `includes_tax` is true, otherwise `1`. This simplified model assumes the fee-bearing price and shipping share one proportional tax multiplier. **Do not use it for mixed taxable bases, nonlinear or tiered fees, caps, minimums, per-listing option fees that do not fit this equation, or channels whose fees are not proportional.** Calculate exact fees separately, or enter an extra charge in `other_costs`, and use the ledger validator's independently calculated `fee_base` for a known single scenario. A nonpositive marginal-proceeds denominator is rejected rather than producing a misleading minimum.

For offers or a different closing price, rerun scenarios at that price; for combined orders, enter the whole shipment's total revenue, packing and fee structure. A computed floor never overrides the seller's firm ask, establishes a fair market price, or authorizes accepting an offer. A quick sale does not prove the higher counterfactual price would have sold.

## Resume with a bounded queue

```bash
./scripts/sales-assistant.py next-actions /absolute/path/project --now 2031-06-12T09:00:00+02:00
```

Omit `--now` during actual work to use current UTC time. Supply it for deterministic review or testing, not to hide an expired deadline. The command validates the ledger and ranks overdue handoffs first, handoffs due within the upcoming deadline window next, then access gaps/returns, routine fulfilment, payments, tracking, reconciliation and drafts. `--deadline-hours` defaults to 24 and `--freshness-hours` defaults to 24; both must be finite positive numbers. Choose windows for the actual cadence and obligations. Seller-reported handoff produces urgent scan verification near its deadline, never another physical-handoff or label task. Label creation still produces physical-handoff work.

Optional `operations/monitor.json` uses the existing scaffold's `surfaces` object and optional `required_surfaces` list. Required surfaces default to orders, messages, offers, returns, tracking and account. Missing required surfaces, `not_checked`, legacy `failed`/`inaccessible`/`unknown`, and `access_failed` all produce gaps. A `checked` receipt needs a valid, nonfuture `checked_at` inside the freshness window and a nonempty `evidence_ref`; missing or stale proof becomes a gap. The `local_coverage` output describes those **local records**, never a fresh verification performed by this command. No monitor file means no configured coverage was assessed, not that monitoring is healthy.

This queue does not replace the full monitoring/delta procedure, discover message, question or case deadlines or assess actual packing capacity; refresh those directly (`ricardo selling status`/`selling sold` if listed, and the browser) before choosing work. Priorities are advisory numbers, with smaller numbers first, not Beads priority assignments. Explicit known case deadlines may outrank this coarse queue.

For `fulfillment_method: "pickup"`, the queue follows pickup states: unpaid payment verification (or, with `payment_due_at_handover: true`, arrangement of the exact lot and agreeing how the buyer will pay); paid arrangement of the exact lot; `pickup_arranged` deadline and handover evidence (with payment taken at handover, payment and handover recorded together); `collected` reconciliation; returned-unit quarantine. It uses the actual `pickup_by_at` when recorded and does not invent a deadline for merely paid pickup. No pickup state creates postage or tracking tasks. See the [workspace schema](WORKSPACE-SCHEMA.md) for complete evidence requirements.

## Produce a listing worksheet

The packet gathers exact local lot memberships, recorded listing quantity/status, the public `marketplace` and `listing_id`, declared photo allowlist fields, presence of dated readiness evidence and both structural and readiness errors. It intentionally omits arbitrary prose, customer/order details and claims copied from other listings. It does not treat a field called `verified_at` as independent truth.

Photo output follows the listing's intended `photo_ids` order, including the lead image, rather than the order of the project photo table. Malformed ledger tables remain reported as structural errors; they must not crash the advisory queue or cause an unknown fulfilment method to be treated as shipping. Both commands share the validator's duplicate-key-rejecting decimal JSON reader. Invalid fractional fields are reported without a serialization traceback.

The worksheet scopes photo validation to the selected listing, just like `validate-project.py --listing`. An unrelated held item's missing derivative must not block ready stock. Global identity, allocation and order-state checks remain enforced. A full-project validator/queue run still reports the unrelated derivative problem for later work.

Use the packet alongside the [listing template](../assets/listing-template.md) to write accurate copy from source observations and owner reports. Inspect the live draft separately. The listed photos are **candidates from recorded intent**, not an upload action: readiness errors such as changed hashes or missing approval must be resolved first. Historical lots can appear in allocations, so honor each lot's state and the listing's current quantity.

## Compare real draft and preview receipts

Historical browser-verifier failures exposed two reusable risks: a missing control being treated as disabled, and selecting a draft by its position in a changing list. This comparator rejects both shortcuts. Use it after collecting current evidence through supported visible controls; it makes no browser calls.

```bash
./scripts/compare-draft.py /absolute/path/reopened-receipts.json
./scripts/compare-draft.py /absolute/path/preview-receipts.json
```

Optional `--now 2031-05-01T10:00:00Z` supports reproducible review and `--max-age-hours 24` sets the positive freshness window. Live runs should use actual time. Freshness alone does not prove a receipt came after the latest edit: recapture changed fields and dependent settings after every revision.

Each packet has these fields; unknown keys are errors:

| Field | Contract |
|---|---|
| `listing_ref` | Exact local listing record `id`, not list position |
| `draft_id` | The draft's identifier as the account shows it, or the opaque reference you entered in the form, recorded as a string |
| `stage` | `reopened` or `preview` |
| `expected` | All material fields below, from the current intended listing |
| `identity` | `listing_ref` and `draft_id` receipts independently binding this observation to the intended target |
| `observed` | Material-field receipts actually collected in this stage; missing receipts fail readiness |
| `not_shown` | Optional preview-only field exemptions with dated evidence and specific reason |

The `listing_ref` receipt establishes the local inventory/lot-to-draft mapping; do not invent a local ID that the site never displays. The `draft_id` receipt comes from the actual opened page or observed navigation (for Ricardo, the opaque reference typed into the form is a practical anchor). Reacquire identity after navigation. An index such as "third draft" is not identity evidence.

The material-field set is `title`, `quantity`, `price`, `currency`, `format`, `buy_now_price`, `duration`, `condition`, `photos`, `description`, `category`, `category_attributes`, `shipping`, `handling`, `payment`, `returns` and `promotions`, plus `buyer_offers` **only when the format is `buy_now`** (fixed price). Ricardo's buyer price proposals (Preisvorschläge) exist for fixed-price listings only (RICARDO-PLATFORM.md; verify live) and are binding once accepted, so `buyer_offers: true` needs offer authority. For auction, `auction_with_buy_now` and `classified` formats there is no such control: `buyer_offers` must be absent from `expected` and `observed`, and supplying it is an input error. There is no `auto_accept` field, because the Ricardo form has no automatic-acceptance control. Expected values cannot be null. Quantity is a positive integer; `price` is the start price for auctions and the fixed price for buy-now and classified formats; `format` is `auction`, `buy_now`, `auction_with_buy_now` or `classified`; `buy_now_price` is a number exactly when the format is `auction_with_buy_now` (and above the start price), otherwise the string `"none"`; `duration` is nonempty text such as `not_applicable` or a positive integer. `promotions` (boosts and other paid options, which default to `false` without authority) and `buyer_offers` are booleans describing whether that feature is enabled; give a receipt for the control as actually observed. Photos are an ordered, nonempty list of reviewed photo identifiers. Category attributes are an object. Shipping, handling, payment and returns are explicit nonempty strings or objects capturing the intended settings, including pickup when applicable. Text fields must be nonempty. A fallback to an unintended default (a token start price, a different format) shows up as `value_mismatch`, which is the point of the field set.

Every present observation has this shape:

```json
{"present": true, "value": false, "evidence_ref": "reopened-offer-control", "observed_at": "2031-05-01T10:00:00Z"}
```

This means a control was actually observed **off**. A control genuinely absent from the inspected view is instead:

```json
{"present": false, "evidence_ref": "reopened-view", "observed_at": "2031-05-01T10:00:00Z"}
```

The absent control fails readiness even when the desired setting is `false`. An omitted receipt means unobserved and also fails. `present: false` with a supplied `value`, or `present: true` without a value, is malformed and rejected. IDs and evidence references must be opaque identifiers without spaces; use references to local evidence, not raw correspondence or screenshots embedded in the packet.

Comparison preserves types and array order: `false`, `0`, `null`, `"false"` and missing are distinct; numeric `100` and `100.00` are equal after exact decimal parsing. No trimming, HTML stripping, case folding or substring matching silently changes observations. Record the stage-appropriate representation deliberately; when the preview renders formatted text differently, inspect the substantive equivalence manually and provide the appropriate expected representation with that evidence. Do not manufacture matching receipts from desired settings.

Preview does not expose every internal seller control. For a material field actually not shown there, omit it from `observed` and record, for example:

```json
{"not_shown": {"promotions": {"reason": "Buyer preview does not expose seller paid options", "evidence_ref": "preview-layout", "observed_at": "2031-05-01T10:00:00Z"}}}
```

This exemption is invalid for the reopened stage, invalid alongside an observed receipt for the same field, and unavailable for the preview's core title, quantity, price, currency and photo checks. It does not waive the successful reopened check of seller settings. Every expected field remains in `expected` even when the preview does not show it.

Output includes exact identity/stage, `ready`, issue reasons (`unobserved`, `not_present`, `value_mismatch`, `identity_unobserved`, `identity_mismatch`, `stale_receipt`) and declared preview exemptions. Value/identity mismatches include expected and observed values. Structural errors, unknown fields, duplicate JSON keys, contradictory receipts, invalid/naive/future timestamps, incoherent format and buy-now intent, or nonfinite JSON constants exit **2**. A valid comparison reporting mismatches exits **0 with `ready: false`**: process success is not listing readiness.

Publication still requires successful reopened **and** preview comparisons against the same current intent, same draft and actual unit allocation, alongside account readiness and existing authority. These outputs cannot prove receipt truth, detect doctored photos, infer missing UI state or authorize publication. After publication, `ricardo inspect` covers the live page ([CLI mapping](RICARDO-CLI.md)). Keep comparison files local until reviewed for private details.

## Test the helpers

```bash
./scripts/test-skill.py
```

This runs package checks (links, frontmatter, shebangs and executable bits, documented commands against `ricardo capabilities --json` when the binary is installed) and each regression suite: `test-project.py`, `test-assistant.py`, `test-monitor.py`, `test-draft.py`, `test-pickup.py`. They cover algebraic floor boundaries, marketplace identifier formats, fee-base assumptions, base/downside outcomes, malformed inputs, overdue queues, handoff distinctions, access failures, privacy-oriented packet field selection and actual CLI execution. Fixtures are synthetic. These are software checks, not sales, shipping or account-access proof.
