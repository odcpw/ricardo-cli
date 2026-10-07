# Using the `ricardo` CLI in a sale

The CLI reads Ricardo and Tutti through the seller's signed-in browser. It is an **observation instrument**: it cannot create drafts, publish, revise, answer questions, message anyone, accept offers or touch payments. The agent fills the listing form in the seller's browser, saves a draft, has the seller review it, and publishes only under explicit authority. Authoritative contract: this repository's `docs/agent-contract.md` (live-observed seller routes: `docs/seller-routes.md`); installed surface: `ricardo capabilities --json`.

**Check `ricardo capabilities --json` first and use only what it lists.** The read-only seller commands `ricardo selling sold`, `ricardo selling drafts` and `ricardo selling status` exist in CLI versions that list them (marked *if listed* below); prefer them when listed, otherwise read the same pages in the seller's browser (My Ricardo, Mein Verkaufen). They need the browser extension at the version the contract names (0.3.0 at the time of writing); an outdated extension fails with `extension_outdated` (exit 7) and a reload hint, never with partial data. Further seller commands (for example unsold items) may appear in newer versions: prefer them when listed, never guess them. Guessed top-level names such as orders, sold or drafts exit 2 with a hint naming the correct `selling` subcommand.

## Rules for every call

- Start the session with `ricardo doctor --json`; follow its hint. Login, MFA and extension setup are the seller's actions.
- Always `--json`. Check the process exit code **and** `ok`. JSON failures arrive on stdout; diagnostics on stderr.
- One command at a time. No parallel calls, no shell loops, no retry loops. The bridge allows one request in flight and waits eight seconds after each; a concurrent call fails with `request_busy` without contacting Ricardo. A search verifies every returned article sequentially and `selling status` makes three paced requests, so budget time and keep `--limit` small (default 5); use `--page 2` for more.
- A failed, blocked or challenged read is **not an empty result**. Unknown status is not active. Never replace a failed live read with `history`, an indexed page or a remembered observation; `history` is old observations, never current availability.
- Keep every observation's `checked_at`. Copy article IDs and plain URLs into notes.
- Personal data: `selling sold` omits buyer identity, addresses, payment references and courier notes by design, but order IDs, amounts and tracking numbers remain private. They never go into the tracked ledger: keep them in the ignored `private/orders.json` keyed by the local order id ([workspace schema](WORKSPACE-SCHEMA.md#private-order-data)). Account output stays out of Git and shared logs; summarise it instead of pasting it. `ricardo images` writes private local files: use `--output-dir /absolute/workspace/private/comps` (ignored by the scaffold) and never commit or reuse another seller's photos or text.

| Exit | Meaning | Action |
|---|---|---|
| 0 | Success (including a mutation preview) | Still read `status`, `partial`, `checked_at` |
| 2 | Invalid input | Fix the invocation; the message corrects it |
| 3 | Bridge unavailable or timeout | Setup problem: `doctor`, reconnect extension; do not poll |
| 4 | Authentication required | Ask the seller to sign in in that browser |
| 5 | Blocked, cooling down or busy: **branch on `error.code`** (below) | Never switch to cached data |
| 6 | Upstream response or interface changed | Report as interface drift; no invented data |
| 7 | Unsafe or unsupported operation | Do it in the browser if authorized, or report the limit |
| 8 | Local file or I/O error | Fix the path or permissions |

Exit 5 carries three different codes; read `error.code`, not just the exit status:

| `error.code` | Meaning | Action |
|---|---|---|
| `upstream_blocked` | Ricardo or Tutti answered HTTP 403 or 429; the bridge now pauses all requests for two minutes | Stop CLI calls, tell the seller it was blocked, wait out the pause, then resume with one command; do not reconnect or retry in a loop |
| `rate_limit_cooldown` | A pause from an earlier block is still in effect (the message gives its end time); the request was not sent | Wait until the stated time; report the coverage gap meanwhile |
| `request_busy` | Another local `ricardo` command is still running; nothing was sent to Ricardo | Wait for that command to finish and run one command at a time; this is not a seller-facing problem and not an access failure |

Exit 7 also covers `extension_outdated` (reload the unpacked extension and reconnect the bridge). Do not retry `retryable: false` errors without changing the condition the hint names.

## Lifecycle stage to command

| Stage | Command | Read these fields | Does NOT establish |
|---|---|---|---|
| Preflight | `ricardo doctor --json`, `ricardo account --json` | Bridge/auth status; signed-in account summary (identity check, not for the repo) | That every surface (orders, messages) is accessible |
| Account readiness | `ricardo selling status --json` (if listed; three paced requests, expect 16 seconds or more) | `selling_limits.current/limit/remaining`, `orders_pending_action`, `unread_messages.count` and `last_updated_at`, `request_times` | Message contents (replies happen in the browser); payout setup; that listing is permitted for these goods |
| Ricardo comps | `ricardo search QUERY --include-ended --limit 10 --json`, then `ricardo inspect ID --json` for each comp that matters | `status`, `status_evidence`, `end_at`, `checked_at`, `bids_count`, `current_bid`, `buy_now`, `sold_price`, `condition`, `category_id`, `product_type`, `shipping`, `description`, `questions_and_answers`; page-level `partial`, `page_count`, `filter_scope`, `sort_scope` | See "Reading comparable results" |
| Tutti comps | `ricardo tutti search QUERY --limit 5 --json`, `ricardo tutti inspect URL --json` | `asking_price`, `status` (`listed`), `partial`, `total`, observation time | Any sale or realized price; seller-confirmed availability |
| Comp photos | `ricardo images ID_OR_URL --limit 5 --output-dir ... --json` | `images[].path`, `index`, `total_images`, `partial` | Condition, hours or authenticity (no OCR or visual inference); rights to reuse the photos |
| Buyer questions on similar items | `questions_and_answers.items[].question.text` / `.answer.text` from `inspect` | `status` (`loaded` / `not_loaded`) | Zero questions when `not_loaded`; answers are seller claims |
| Draft readback | `ricardo selling drafts --json` (if listed) | `drafts_count`, `drafts` (upstream objects), `partial`, `item_shape` (item fields were not observed when the command was written: read the actual JSON, do not assume fields) | That a draft's fields are correct; use reopened-draft receipts and the buyer preview. A page without draft data is an error, not zero drafts |
| Publication | none; seller's browser | | |
| Post-publication check, Ricardo | `ricardo inspect OWN_ARTICLE_ID --json` | `status == active`, `id`, `title`, `url`, `end_at`, price fields against the intended format and start price, `condition`, `category_id`, `shipping`, `location`, `images` count, `seller`, `checked_at` | Hidden settings, draft state, buyer preview rendering, account standing |
| Post-publication check, Tutti | `ricardo tutti inspect URL --json` (full canonical URL) | `status` (`listed`, `unavailable` or `unknown`), `status_evidence.schema_availability` and `status_evidence.seller_confirmed` (always false), `asking_price`, `currency`, `url`, `title`, `location.city` / `zip_code`, `images`, `checked_at` | Seller-confirmed availability, that the ad is the intended lot beyond title and price, messages. `unavailable` and `unknown` are **not verified**: do not record the ad as active |
| Live gallery check | `ricardo images OWN_ARTICLE_ID --json` | Count and order against the approved set, then view the files | Byte equality with local derivatives (the CDN re-encodes); privacy review |
| Active inventory | `ricardo selling --json` | Offers with open/closed `facets`, `plannedOffersCount`, `maxAllowedOffers` where present; match by article ID to the ledger | Orders, payments; absence means "not in this read" |
| Questions on own listing | `ricardo inspect OWN_ARTICLE_ID --json` | `questions_and_answers` items with dates; `answer: null` = unanswered | Completeness (embedded page data only, no pagination) |
| Alerts | `ricardo notifications --json` | Notification entries | A full inbox of orders, messages or offers; absence is not absence of a buyer request |
| Sold orders and fulfilment queue | `ricardo selling sold --limit 30 --json` (if listed; continue with `--offset` from `pagination.next_offset`) | See "Sold orders" | Carrier scan events, message contents, bank deposits, anything omitted by design |
| Money | `ricardo selling sold --json` (if listed) | `seller_success_fee_chf`, `seller_success_fee_rate`, `unit_price_chf`, `shipping_cost_chf`, `total_price_chf`, `payment` | Cash actually received, payout sent, bank arrival, tax treatment |

## Sold orders (if listed)

Top level: `source`, `checked_at`, `orders`, `status_counts`, `pagination` (`offset`, `limit`, `total`, `partial`, `next_offset`), `scope`, `privacy.omitted_fields`. Per order: `order_id`, `article_id`, `article_url`, `offer_type`, `quantity`, `created_at`, `unit_price_chf`, `shipping_cost_chf`, `total_price_chf`, `seller_success_fee_chf`, `seller_success_fee_rate`, `payment` (`method`, `status`, `buyer_has_paid`, `transaction_type`, `payout_unlocked_at`), `delivery` (`method_label`, `shipping_status`, `delivery_status`, `carrier.provider` / `service_type`, `tracking_number`, `tracking_url`, `parcel_pickup`), rating status, `contact_status`, fee details if present, `dispute_state` (null when none) and `offer_closed`. Buyer identity, addresses, payment references, courier notes and the pickup address are deliberately never read; open the order in Ricardo for them. Sold orders are not written to local history. A missing required field is an `interface_changed` error naming the field, never a zero. `status_counts` carries Ricardo's own seller buckets: `sellerToSend`, `sellerPaymentToCheck`, `sellerToRate`, `sellerAllDone`, `sellerPaused`, `sellerCancelationRequested`, `sellerCancelled`.

- The success fee and its rate come from Ricardo's per-order fee record and **differ between orders**: read each order's own value; never carry one order's rate to another or to a forecast.
- `payment.payout_unlocked_at` says Ricardo released the funds. It is not a transfer sent, and it is not a bank deposit.
- `parcel_pickup` is a carrier home-pickup booking: an arrangement, not proof that the parcel was handed over.
- `tracking_url` is kept only for Swiss Post hosts and is a link; the CLI does not read scan events. Open it in the seller's browser for the carrier's events.
- `delivery.method_label` identifies pickup (`get_by_buyer`) versus a parcel service; a pickup order never needs a label.
- Two payment regimes exist: integrated protected payment with a payout, and direct payment (bank-transfer prepayment or cash on pickup). `payment.transaction_type` such as `NOT_SECURED` indicates which one an order is in; read it per order. Only the protected regime has a payout to unlock; for direct payment the seller's own bank or cash receipt is the evidence. How the success fee is collected differs and changed recently: verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (payments, success fee, billing).

## Evidence ladder

| Claim | Minimum evidence |
|---|---|
| Listed | `ricardo inspect ID` shows `status: active` with the intended title, price, format and shipping; plus the seller's own account page. Unknown is not active |
| Sold | The order appears in `selling sold` (or the seller's Verkauft page) with its `order_id` |
| Paid | `payment.status` paid or `buyer_has_paid: true` on that order; for cash or bank transfer the seller's own receipt is the underlying evidence |
| Shipped (handed over) | `shipping_status: shipment_confirmed` **plus** a tracking number is the seller's confirmation, not a carrier scan. Carrier acceptance needs a scan event on the tracking page |
| Delivered | `delivery_status` delivered or received, with the carrier event when available |
| Collected (pickup) | The seller's handover report with payment evidence; `delivery_status` received/delivered is Ricardo's record. A scheduled time is not collection |
| Funds | Payout state and bank arrival come from separate evidence ([money and closure](MONEY-AND-CLOSURE.md)) |

Field-to-ledger mapping: [workspace schema](WORKSPACE-SCHEMA.md#ricardo-fields-to-ledger-states).

## Monitoring heartbeat

Cheapest useful check: `ricardo selling status --json` (if listed). If `orders_pending_action` is true (the CLI's own `next_commands` points to `selling sold`), the unread count rose, or `selling_limits.remaining` fell since the last snapshot, run `ricardo selling sold --json` (if listed) and then act in the browser (messages, payment, handoff). Run `selling sold` at the agreed daily cadence even when the heartbeat is quiet: rating-pending and cancellation buckets may not set a pending-action flag. A heartbeat covers only the surfaces it reads; record the others as `not_checked` in the monitor snapshot, never as quiet ([monitor protocol](MONITOR-PROTOCOL.md)). Respect pacing: do not shorten the interval to look for changes.

## Reading comparable results

`--include-ended` removes the default active-only filter; it does not promise that ended results are sold. Check the output rather than assuming:

- `status` values in the current output are `active`, `ended`, `sold` and `unknown`. In the source at the time of writing, `sold` requires a closed listing with no remaining quantity and `sold_price` is the last bid (auction) or the price (fixed price); confirm both fields exist in the JSON you received. If `sold_price` is null or `status` is `unknown`, the final price is **unknown**, not zero and not the last asking price.
- `ended` without `sold` is an unsold or unproven outcome, not a price. A winning bid is not proof that the buyer paid. Delivered price also needs the `shipping` options.
- Search sorting and filters apply within the requested page only; one page never proves a global minimum, maximum or median. Record query, filters, page and `checked_at` with each comp row.
- Seller descriptions, hours, warranty and condition are claims. `current_bid` and `next_bid` of an active auction are not final prices.
- An empty successful result means no matches in that scope; a failed read is a failure. Never convert one to the other.

## Tutti specifics

Only full canonical German URLs (`https://www.tutti.ch/de/vi/.../ID`, no query) are accepted; a bare ID is unsupported. Search is first page only, newest first, maximum 20. `listed` means the page declares in-stock, not that the seller confirmed availability; an asking price is not a bid or a sale. Keep a Tutti tab open for Tutti reads. There are no Tutti account or mutation commands: verify one's own Tutti ad through its public URL; messages and replies stay in the browser.

## Cross-checking against the ledger

After the seller publishes in the browser, the CLI result is an **independent readback**, recorded in `active_evidence` with its `checked_at`. The ledger's `active` status still needs the marketplace, the public `listing_id` and an `active_verified_at` timestamp. A Ricardo `active` reading maps to ledger `active`. A Tutti `listed` reading also maps to ledger `active`, but it is the ad page's declared availability only: record `seller_confirmed: false` on the listing and say so in `active_evidence`, and set it to true only when the seller has seen the ad live in their own Tutti account. A Tutti `unavailable` or `unknown` reading leaves the listing unverified (draft or held), never active. The Tutti `location` shows a city and postcode: check that is what the seller wants buyers to see. `compare-draft.py` covers pre-publication receipts; this covers the live page. A mismatch blocks that listing only.
