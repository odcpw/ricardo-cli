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
30 and 50 respectively. Buying returns offers, not completed orders, and selling
returns active offers only.

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

## Automation

Always pass `--json` and check both exit status and `ok`. Follow structured
error hints. Retry only when the reported condition permits it; avoid rapid
polling. Use `--timeout` to bound requests. `history` contains past local
observations and must never establish live availability.

Keep credentials and private results out of commits and public logs. If a
command's live behavior has not been exercised, say so rather than describing
local unit-test success as successful account integration.
