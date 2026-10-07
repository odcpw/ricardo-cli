---
name: ricardo-cli
description: Search live Ricardo auctions and inspect account pages through the user's logged-in browser using the local Rust ricardo CLI. Use for auction searches, listing verification, favorites, and Ricardo interface snapshots.
---

# Ricardo CLI

Read `../../docs/agent-contract.md` for the complete invocation and output
contract. Use `ricardo capabilities --json` for the installed version's actual
surface; do not assume a command is implemented merely because a newer guide
mentions it.

## Connection

Run `ricardo doctor --json`. Follow its exact hint if the browser extension,
bridge, or logged-in session is missing. Login happens in the user's browser.
Never request a password or export session credentials. If setup requires a
browser action you cannot perform, explain the specific step and continue any
independent local work.

## Finding an auction

```bash
ricardo search Bambu P2S --product-type printer_3d --max-price 350 --json
ricardo inspect ARTICLE_ID --json
```

Use structured filters for category and product type where available. Read the
result's status and observation timestamp; inspect promising articles before
presenting a shortlist. Distinguish next minimum bid from current bid and
buy-now price. An auction within budget may sell above that budget.

Search filters and sorting apply only to the requested page. Use `--page 2`
and subsequent pages to cover more results. Inspect `partial`, `page_count`,
`filter_scope`, and `sort_scope`; do not claim a globally cheapest option from
one partial page. A small filtered result count can mean other matches remain
on later pages. Account buying and favorites lists use `--offset` and `--limit`,
with `pagination.next_offset` guiding the next call; their maximum limits are
30 and 50 respectively. Buying returns offers, not completed orders, and bare
selling returns active offers only (sold orders: see Seller views).

Return title, article ID, plain URL, observed price type, status, closing time,
and when checked. Keep seller claims about condition or warranty attributed.
Never call an unknown or ended auction available. A blocked live request is a
failure, not an empty search or permission to use cached pages as current.

## Account interface

Use `account`, `favorites list`, `buying`, `selling`, `notifications`, or
`page URL` for supported read paths. Inspect the output rather than inferring
the existence of an endpoint. Known routes only: report unsupported operations
clearly and do not guess private API requests.

`favorites add ARTICLE_ID` and `favorites remove ARTICLE_ID` produce a preview;
`--yes` executes the requested change. Use explicit authorization already given
in the conversation; do not ask again for an already authorized favorite edit.
Search and account access do not authorize purchases, bids, publications,
payments, or seller messages. Those actions are not provided by this CLI.

## Seller views

For the user's own sales: `ricardo selling sold --limit 30 --json` (sold orders,
`--offset` from `pagination.next_offset`), `ricardo selling drafts --json`, and
`ricardo selling status --json` (listing limit, `orders_pending_action`, unread
count; three paced requests, 16 seconds or more). Bare `ricardo selling` still
lists active offers. These need extension 0.3.0+; `extension_outdated` means
reload it and Connect bridge. Check `capabilities` first and do not guess other
seller commands.

Read each order's own success-fee rate and amounts; they differ per order.
`payout_unlocked_at` is a release by Ricardo, not a bank deposit, and
`parcel_pickup` is a booking, not proof of handover. Buyer identity, addresses,
payment references and pickup addresses are omitted by design; send the user to
the order in Ricardo for those, and keep order data out of commits and shared
logs. Draft item fields are unverified: read the returned objects. Replying to
messages, listing, editing and publishing stay in the user's browser.

## Automation

Always pass `--json` and check both exit status and `ok`. Follow structured
error hints. Retry only when the reported condition permits it; avoid rapid
polling. Use `--timeout` to bound requests. `history` contains past local
observations and must never establish live availability.

Keep credentials and private results out of commits and public logs. If a
command's live behavior has not been exercised, say so rather than describing
local unit-test success as successful account integration.

## Gentle request pacing

Run one CLI command at a time. The shared native bridge permits only one
Ricardo request in flight and waits eight seconds after completion before the
next request. Concurrent commands fail with `request_busy` without contacting
Ricardo. Search checks articles sequentially and stops at its first failure.
The default search limit is five candidates. There are no automatic retries.
HTTP 429 or 403 pauses requests for two minutes; that cooldown survives reconnects.
A connection failure also delays subsequent requests. These limits govern this
CLI, not manual browsing or other programs, and cannot guarantee site acceptance.

## Tutti and photographs

Use `ricardo tutti search QUERY --limit 3 --json`, `ricardo tutti inspect URL
--json`, and `ricardo tutti images URL --json`. Only full canonical German Tutti
listing URLs are accepted. Search covers the first page only; check `partial`.
Tutti `listed` means site-declared availability, not seller confirmation.

For either marketplace, `ricardo images ID_OR_URL --json` downloads the gallery
and returns local `images[].path` files. `--index 3` downloads only the third
photo; `--limit 5` bounds a gallery batch. Use your image viewer on those paths:
listing text can omit hours that are visible in photographs. Quote displayed
counters as photographic evidence at the time the seller took the picture,
not a live reading from the machine. Do not infer wear or warranty from an image.

Requires extension 0.2.0 or newer with Tutti and both image-CDN permissions.
The existing paced bridge governs photos too; do not parallelize commands.

## Ricardo questions and seller answers

`inspect` (and verified search results) includes `questions_and_answers` from
that exact listing's embedded page query, without an extra network request.
Read `items[].question.text` and `items[].answer.text` before concluding that
hours, warranty, faults or accessories are unspecified. Answers preserve their
dates and are seller claims. An unanswered question has `answer: null`.
`status: not_loaded` means unavailable data, not zero questions. `loaded`
refers only to the question data embedded in the page; additional pagination
is not fetched. User account IDs and nicknames are excluded from this output.
