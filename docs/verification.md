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
