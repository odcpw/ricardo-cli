# Verification — 2026-09-05

The Rust CLI was exercised as a separate process through its Unix socket and
JSONL broker. The worker used this repository’s actual `handleRequest` extension
function, with the existing authorized Chrome browser integration implementing
its scripting calls. Ricardo requests ran in the already signed-in browser.
No cookies, passwords, or authentication tokens were exported or saved.

## Automated checks

- 29 Rust unit tests and 4 process integration tests passed.
- 6 JavaScript extension tests passed.
- Clippy passed with warnings denied; Rust formatting passed.
- Tests cover HTTP/bridge failures, bounded inputs, mutation previews, numeric
  resolution restrictions, socket recovery/shutdown, and persistent history.
- A default-thread stack overflow found in FrankenSQLite debug execution was
  fixed with a dedicated 32 MiB stack for database work; persistence tests pass.

## Live checks

- Doctor confirmed authentication and opened the FrankenSQLite history database.
- Search read React Flight results and verified each returned article against
  its own detail record. Page 2 was checked using Ricardo’s actual `?page=2` UI.
- Article **1327242570** is sold at CHF 385 and has no available buy-now price.
  This is the stale P2S listing that motivated this tool, and is a regression fixture.
- Active auction **1328424962** reported current bid CHF 179 and next bid CHF 184
  when observed. These are observations, not a current recommendation.
- Fixed-price article **1328009582** reported CHF 559, confirming its price unit.
- Account summary, favorites, buying offers, active selling offers, and
  notifications returned successful live responses. Private records were not
  written into this report or fixtures.
- Buying `--limit 1` and `--offset 1 --limit 1` returned the corresponding
  pagination metadata with a next offset.
- The UI’s favorite add/remove requests were observed using a reversible test;
  the article’s original unsaved state was restored. The CLI mutation protocol
  was tested without issuing bids, purchases, seller messages, or listing changes.

Search product attributes can contain miscategorized accessories. The CLI
reports the upstream attribute and documentation requires checking titles and
descriptions before recommending a printer. Price sorting applies within each
requested page; auction bids are not guaranteed final purchase prices.

## Standalone setup boundary

The unpacked extension has not yet been loaded into Chrome for standalone use.
The native transport has protocol tests; the live end-to-end tests used the
agent worker described above. Chrome’s browser-automation URL policy rejected
`chrome://extensions` and prohibited workarounds. Loading the extension therefore
requires the user’s manual action, followed by native-host registration and
pressing Connect bridge. `ricardo doctor --json` verifies that final setup.

## Installed release

`cargo build --release --locked -j 1` completed successfully. The optimized
executable is installed at `/home/oliver/.local/bin/ricardo` (39,395,200 bytes).
Ten `capabilities --json` runs had a median local process duration of 2.83 ms
on this machine; browser/network operations add their own latency. The installed
release passed a persistent-history smoke check and the closed-auction fixture
through its process entry point. The original Chrome connection became
unavailable during the final live release recheck, so that last recheck did not
complete. The earlier live checks above passed through the tested debug binary.

## Gentle pacing update

The shared broker now serializes requests with an eight-second completion gap,
refuses concurrent commands, and persists a two-minute pause after HTTP 429/403.
Search verifies sequentially and stops at the first failed request. 31 unit
tests and five process tests passed, including simultaneous-client refusal and
cooldown persistence. Clippy passed with warnings denied. The installed release
was built with `cargo rustc --release --locked -j 2 -- -C lto=off`; capability
output confirms the two-minute configuration. No live Ricardo requests were
sent while implementing or testing this change. Reconnect the extension bridge
to start the updated native process.

## 2026-09-05: Tutti and gallery downloads

Added Tutti first-page search and exact-URL listing inspection, plus paced
Ricardo/Tutti gallery downloads to private numbered files and a JSON manifest.
The observed Tutti search token for `Prusa Core` and the exact detail/query data
shapes were checked against current public pages. Fixtures retain only public
parser inputs, with synthetic descriptions and locations; no account/session
state or downloaded photos is committed.

Validation: 38 Rust unit tests and 7 process-contract tests passed (45 total),
10 Node extension tests passed, and Clippy passed with warnings denied.
Coverage includes exact-listing scoping, unknown availability, CDN/path
restrictions, credential-free image fetching, blocked/oversized responses,
image signatures, file permissions, manifest output and an actionable error
when the old extension receives a new operation. The image transport test uses
synthetic bytes; it is not a live authenticated-browser claim.

Release built with the existing cached dependencies and root LTO disabled for
this local iteration. The browser's currently loaded 0.1.0 extension must be
reloaded to activate 0.2.0 permissions and code. New native live Tutti/image
commands await that reload; previous Ricardo live reads were working.

Installed the release binary at `/home/oliver/.local/bin/ricardo`. A live
`ricardo tutti inspect` call returned the expected `extension_outdated` error
(exit 7, non-retryable) from the still-running older extension, with the reload
instruction. No Tutti request was sent by that old handler.

## 2026-09-05: Ricardo questions and answers

Inspection and verified search results now include the exact article's embedded
`get-questions-and-answers` query, without additional marketplace requests.
Public live reads of article 1328607492 confirmed two seller answers stating
590 hours, warranty until 19 February 2028, no returns, and paid repairs until
19 February 2031. These are seller statements, not independently verified
warranty or usage evidence. Fixtures omit account IDs and nicknames.

41 Rust unit tests and 7 process tests passed (48 total); Clippy passed with
warnings denied. Tests cover article scoping, missing data, unanswered questions,
malformed response shape, and preservation of answer text and dates.
The optimized release was built and installed. Its final live inspection could
not run because the browser bridge had disconnected (`bridge_unavailable`,
exit 3). Earlier live page reads verified the source shape; the installed
parser's live end-to-end verification remains pending reconnection.
