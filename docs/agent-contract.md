# Ricardo CLI agent contract

Use `ricardo --help` to discover the CLI and `ricardo capabilities --json` to
inspect its machine-readable command contract. These work without a browser.

## First live request

1. Run `ricardo doctor --json` and follow the returned setup hints.
2. Keep the configured browser extension connected and sign in on Ricardo in
   that browser. The CLI uses that session; it does not ask for your password.
3. Run `ricardo search Bambu P2S --max-price 350 --json`.
4. Run `ricardo inspect ARTICLE_ID --json` before recommending a particular
   auction. Check its status, closing time, price type, and observation time.

Copy article IDs and plain Ricardo URLs into recommendations so the user can
find the result without relying on clickable links.

## Output and failures

With `--json`, stdout contains one JSON object followed by a newline:

```json
{"schema_version":"1","ok":true,"data":{},"error":null}
```

On failure, `ok` is false, `data` is null, and `error` contains `code`, `message`,
`hint`, and `retryable`. JSON failures also go to stdout. Diagnostic messages
go to stderr. Always check both the process exit code and `ok`. Do not treat a
failed fetch, challenge, or unverified status as an empty successful search.
CLI usage errors also return this envelope when `--json` is requested.

| Exit | Meaning |
| --- | --- |
| 0 | Success, including an explicit mutation preview |
| 2 | Invalid command or input |
| 3 | Bridge unavailable or request timeout |
| 4 | Browser authentication required |
| 5 | Ricardo blocked the request or presented a challenge |
| 6 | Upstream response failed or interface shape changed |
| 7 | Unsafe or unsupported operation |
| 8 | Local file or I/O error |

Use `--timeout SECONDS` (1–300, default 30) to bound each browser request. The
broker's pacing wait (up to eight seconds) is added on top rather than charged
to it, so a multi-request command such as `selling status` takes roughly
requests × (8 s + request time). A browser that does not answer in time yields
`bridge_timeout` (exit 3). Do not
retry `retryable: false` errors without changing the condition described by
the hint. A missing browser session needs setup, not a tight polling loop.

## Search semantics

Search accepts quoted or unquoted words. `--limit` is bounded to 1–100 and
`--page` starts at 1. Sort values are `relevance`, `price`, and `ending`.
Filters and sorting apply within the requested page, not across the entire
result set. Use `--page 2` for the next page and inspect `partial`, `page_count`,
`filter_scope`, and `sort_scope`. A partial page cannot establish the globally
cheapest options. Each returned article is verified individually against its
own detail page; a failed verification fails the search rather than producing
a silently incomplete shortlist.
Active listings are requested by default; `--include-ended` opts out of that
filter. `--active` makes the default intent explicit. Unknown status must not
be represented as confirmed active.

Use `--product-type printer_3d` to filter Ricardo’s structured product attribute.
Sellers can miscategorize accessories; inspect titles and descriptions. `--category` sets
the category identifier. These are structured filters, not title heuristics.

`--max-price` is in CHF. Distinguish current bid, next minimum bid, and buy-now
price. An affordable next bid is not a guaranteed final purchase price.
Seller statements about working condition, hours, warranty, or maintenance
are claims, not independently verified facts.

`history [QUERY] --limit 20` reads earlier local observations. It is useful for
comparison but never establishes that an auction remains active. Live results
must preserve when they were observed; freshness timestamps are intentionally
variable across otherwise identical requests.

## Account and changes

Read account information with `account`, `favorites list`, `buying`, `selling`,
or `notifications`. `page URL` provides a read-only snapshot of a Ricardo page.
The `page` command is restricted to Ricardo; dedicated Tutti commands use the additional allowlisted site. This is not a general browser control API.
Account commands largely expose upstream data inside the versioned envelope.
Favorites and buying accept `--offset` (default 0) and `--limit` (default and
maximum 50 for favorites, 30 for buying). Use their `pagination.next_offset`
for the next request; null means there is no next offset to follow. Metadata
also includes `offset`, `limit`, `total`, and `partial`. Buying returns offers,
not completed orders; bare `selling` returns active offers only. Sold orders,
drafts and seller counts are separate commands: see "Seller views" below.

`favorites add ARTICLE_ID` and `favorites remove ARTICLE_ID` preview the intended
change. Add `--yes` to explicitly request the mutation. Preview success does
not establish that a change occurred. Unsupported interface operations must
return an error instead of claiming success.

Bidding, purchasing, publishing listings, sending seller messages, and payment
operations are not exposed. Account read access is not authorization to perform
those actions. Keep credentials in the browser session and keep local state
private. Account responses can contain personal information; persisted history
contains only normalized public listing observations.

## Seller views (extension 0.3.0+)

Read-only views of your own selling activity. Bare `ricardo selling` is
unchanged and returns active offers. Update the unpacked extension to 0.3.0,
reload it and click Connect bridge first; an older extension fails with
`extension_outdated` (exit 7) and a reload hint, never with partial data.

```bash
ricardo selling sold --limit 30 --json      # sold orders
ricardo selling sold --offset 30 --json     # next page (use pagination.next_offset)
ricardo selling drafts --json               # drafts tab
ricardo selling status --json               # listing limit, pending-action flag, unread count
```

`selling sold` accepts `--offset` 0-10000 and `--limit` 1-30 (default 30). It
sends one fixed seller-orders search through the browser session (a query, not a
change) and returns `source`, `checked_at`, `role`, `orders`, `status_counts`
(Ricardo's own buckets such as `sellerToSend` and `sellerToRate`), `pagination`
(`offset`, `limit`, `total`, `partial`, `next_offset`), `scope` and
`privacy.omitted_fields`. Each order carries identifiers (`order_id`,
`article_id`, `article_url`), `created_at`, CHF amounts converted exactly from
Ricardo's integer cents (`unit_price_chf`, `shipping_cost_chf`,
`total_price_chf`, `seller_success_fee_chf`, `seller_success_fee_rate` plus its
raw per-ten-thousand value), `payment`, `delivery` (including `carrier`,
`tracking_number`, `tracking_url` and `parcel_pickup`), rating and
`contact_status` fields, `dispute_state` (null when none) and `offer_closed`.
The success-fee rate differs between orders: read each order's own value. A
tracking URL is kept only for Swiss Post hosts. `payment.payout_unlocked_at` says
Ricardo released funds; it is not a bank deposit. `parcel_pickup` is a carrier
booking, not proof of handover. `delivery.method_label` `get_by_buyer` means
pickup.

**Privacy.** Buyer identity (names, nicknames, user IDs, email, phone),
shipping and invoice addresses, payment reference numbers, courier notes and the
pickup address are never read, emitted or stored; open the order in Ricardo for
them. Output is built field by field from a documented list, so new upstream
fields do not leak. Objects whose populated shape was never observed are
reduced to presence flags (`buyer_fee_reported`,
`payment.secured_transaction_fee_reported`), and ratings keep only numbers and
short codes, never free text. Sold orders are not written to local history. Order IDs,
amounts and tracking numbers are still private account data: keep them out of
commits and shared logs. A missing `order_id`, `article_id`, `pricing.unit_price`
or `pricing.total_price` is an `interface_changed` error that names the field,
never a zero.

`selling drafts` returns `drafts_count`, `drafts` (upstream draft objects as
Ricardo embeds them), `partial` and `item_shape`. No draft items had been
observed when the command was written, so read the returned objects rather than
assuming fields. The drafts page also embeds bank details for payout setup; only
the `selling-drafts` data is read and those details are never emitted. A page
without that data is an error, not zero drafts.

`selling status` makes three sequential, paced requests (expect 16 seconds or
more) and stops at the first failure. It returns `selling_limits`
(`current`, `limit`, `remaining`), `orders_pending_action`, `unread_messages`
(`count`, `last_updated_at`), `request_times`, `next_commands` and `scope`. When
`orders_pending_action` is true, run `ricardo selling sold --json`. Counts only:
message contents are never read; reply in Ricardo. Types are checked strictly;
an unexpected shape is `interface_changed`.

Drafting, editing, publishing and relisting are not supported and remain
browser tasks. Wrong guesses such as `ricardo sales`, `ricardo orders`,
`ricardo sold` or `ricardo drafts` exit 2 with a hint naming `ricardo selling
sold` or `ricardo selling drafts`.

## Examples

```bash
ricardo capabilities --json
ricardo doctor --json
ricardo search Bambu P1S --max-price 260 --sort ending --json
ricardo search Bambu P2S --product-type printer_3d --json
ricardo inspect ARTICLE_ID --json
ricardo selling sold --limit 30 --json
ricardo selling drafts --json
ricardo selling status --json
ricardo favorites list --json
ricardo favorites add ARTICLE_ID --json
ricardo favorites add ARTICLE_ID --yes --json
ricardo history "Bambu P1S" --limit 10 --json
```

`bridge serve` exchanges JSONL with a browser worker over stdin/stdout. The
extension uses the hidden native messaging transport. Ordinary consumers call
the read commands and do not need to implement either transport. `--no-color`
is accepted globally; output has no ANSI styling by default.

## Gentle request pacing

Run one CLI command at a time. The shared native bridge permits only one
Ricardo request in flight and waits eight seconds after completion before the
next request. Concurrent commands fail with `request_busy` without contacting
Ricardo. Search checks articles sequentially and stops at its first failure.
The default search limit is five candidates. There are no automatic retries.
HTTP 429 or 403 pauses requests for two minutes; that cooldown survives reconnects.
A connection failure also delays subsequent requests. These limits govern this
CLI, not manual browsing or other programs, and cannot guarantee site acceptance.

## Tutti and listing photos (extension 0.2.0+)

After updating, reload the unpacked extension and connect the bridge again.
The extension needs host access to www.tutti.ch, img.ricardostatic.ch and
c.tutti.ch, in addition to www.ricardo.ch. Keep a Tutti tab open for its reads.
Login stays in Chrome. Public image requests omit credentials entirely.

```bash
ricardo tutti search Prusa Core --limit 3 --json
ricardo tutti inspect TUTTI_LISTING_URL --json
ricardo images RICARDO_ARTICLE_ID --json
ricardo images RICARDO_ARTICLE_ID --index 3 --json
ricardo tutti images TUTTI_LISTING_URL --output-dir /tmp/printer-photos --json
```

`inspect` and `images` also recognize a full Tutti listing URL. Tutti IDs alone
are unsupported: use the canonical German `/de/vi/.../ID` URL with no query
parameters. Search currently inspects the first result page, newest first,
with a default limit of five and maximum twenty. Each returned listing is
individually fetched; the first failed inspection fails the command. `partial`
and `total` disclose incomplete coverage. Pagination, advanced filters and
Tutti account/mutation operations are not implemented.

Tutti `asking_price` is an asking price, not a bid or guaranteed checkout price.
`status: listed` means the exact listing page declares InStock in its structured
data. It is not seller-confirmed availability. Missing availability is unknown;
missing/mismatched listing data is an error. Seller descriptions are claims.
History keys are namespaced `tutti:ID` to avoid marketplace ID collisions.

`images` first inspects the listing, then downloads its own gallery sequentially.
Related-listing photos are excluded. By default it downloads up to twenty photos;
`--limit` accepts 1–50, or `--index N` fetches one one-based gallery position.
Each invocation creates a new private directory with numbered JPEG/PNG/WebP
files and a JSON manifest. JSON output provides absolute `images[].path`, source
URL, index, byte count and observation time, along with `total_images` and
`partial`. Open these paths with the agent's image-viewing tool to inspect them.
The CLI does not perform OCR or infer condition/hours from photographs.

Only observed Ricardo/Tutti image CDN paths are accepted. Redirects are rejected,
responses are capped at 4 MiB, and file signatures must match the image type.
Existing files are not overwritten. If a later download fails, earlier files may
remain in the new directory, but the command reports failure rather than complete
success. Photos are local artifacts: do not commit them or publish them.

All new requests share the bridge's eight-second gap, single-request concurrency
and two-minute 403/429 cooldown. There are no automatic retries.

## Ricardo questions and seller answers

`inspect` (and verified search results) includes `questions_and_answers` from
that exact listing's embedded page query, without an extra network request.
Read `items[].question.text` and `items[].answer.text` before concluding that
hours, warranty, faults or accessories are unspecified. Answers preserve their
dates and are seller claims. An unanswered question has `answer: null`.
`status: not_loaded` means unavailable data, not zero questions. `loaded`
refers only to the question data embedded in the page; additional pagination
is not fetched. User account IDs and nicknames are excluded from this output.
