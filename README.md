# Ricardo CLI

A Rust CLI for searching current Ricardo auctions and reading account information
through your existing signed-in Chrome session. Results include article IDs,
plain URLs, separate bid and buy-now prices, status evidence, and timestamps.
Every returned search candidate is checked against its own article page.

This is an initial interface client, not a full transactional Ricardo API. It
exposes read operations (including your sold orders, drafts and seller counts)
and favorite changes; bidding, buying, payments, seller messages, and listing
drafting or publication are not implemented.

## Build

Linux is currently required for the Unix socket bridge and Chrome native host
registration. The repository selects its Rust toolchain and uses FrankenSQLite for
local observations and Asupersync in the application dependencies.

```bash
cargo build --release --locked
install -Dm755 target/release/ricardo "$HOME/.local/bin/ricardo"
ricardo capabilities --json
```

Ensure `$HOME/.local/bin` is on your PATH. With mise, use the checked-in toolchain
configuration. FrankenSQLite 0.3.16 requires nightly Rust intrinsics on x86;
the checked-in `rust-toolchain.toml` selects the required nightly. A stable
toolchain override will not build this dependency on that platform.

## Connect your browser

The following steps configure your browser; cloning or building does not install
the extension automatically.

1. Open `chrome://extensions` in the Chrome profile where you use Ricardo.
2. Enable Developer mode, choose **Load unpacked**, and select this repository's
   `extension` directory. Keep the directory in place.
3. Copy the extension's 32-character ID from that page.
4. Register the native host using the installed binary:

   ```bash
   ricardo extension install --extension-id YOUR_EXTENSION_ID --json
   ```

5. Open `https://www.ricardo.ch/de/` and sign in normally in that profile. Keep a
   Ricardo tab open. Click the extension icon, then **Connect bridge**.
6. Verify the connection and session:

   ```bash
   ricardo doctor --json
   ```

Registration writes a private wrapper and native host manifests for Google
Chrome and Chromium under the user configuration directory. The wrapper points
to the binary used for registration. Re-register if you move that binary or
load the extension under a different ID. Use the same `--data-dir` during
registration and subsequent CLI calls if you override the default state path.

Passwords and cookies remain in the browser. The extension handles approved
Ricardo requests, and the CLI receives responses through a private local socket.
Account responses may contain personal data. Only normalized public listing observations are saved to history.

## Search and inspect

```bash
ricardo search Bambu P2S --product-type printer_3d --max-price 350
ricardo search Bambu P1S --max-price 260 --sort ending --json
ricardo inspect ARTICLE_ID --json
ricardo inspect https://www.ricardo.ch/de/a/TITLE-ARTICLE_ID/ --json
```

Search accepts quoted or unquoted words. Active articles are the default;
`--include-ended` allows ended results encountered on the requested search page.
`--product-type printer_3d` filters Ricardo’s product attribute; sellers can miscategorize accessories, so
check the returned title and description. `--max-price` compares the next bid or buy-now amount, in CHF. An
affordable next bid does not guarantee an affordable auction outcome.

**Filters and sorting apply to the requested search page.** Use `--page 2` to
fetch the next page. `--limit` bounds verified candidates on that page (default
5, maximum 100).
The JSON response reports `partial`, `page_count`, `filter_scope`, and
`sort_scope`. A partial page cannot establish a globally cheapest shortlist;
cover all relevant pages before making that comparison. Failed detail
verification fails the request instead
of silently dropping unavailable data and claiming a complete search.

Unknown article status is never treated as confirmed active. Live requests do
not fall back to old indexed pages or local history. Seller descriptions remain
seller claims, including statements about condition, warranty, and print hours.

## Account and favorites

```bash
ricardo account --json
ricardo favorites list --offset 0 --limit 50 --json
ricardo buying --offset 0 --limit 30 --json
ricardo selling --json
ricardo selling sold --limit 30 --json
ricardo selling drafts --json
ricardo selling status --json
ricardo notifications --json
ricardo favorites add ARTICLE_ID --json
ricardo favorites add ARTICLE_ID --yes --json
ricardo favorites remove ARTICLE_ID --yes --json
```

Favorite changes preview by default. `--yes` executes the named change. Read the
response to distinguish a preview from a change acknowledged by Ricardo.

Account data is currently returned largely in Ricardo's upstream structure,
inside the CLI envelope. Favorites and buying support `--offset` and `--limit`
(defaults and maximums: 50 favorites, 30 buying offers). Their `pagination`
metadata includes `offset`, `limit`, `total`, `partial`, and `next_offset`.
Continue with `--offset NEXT_OFFSET` when it is non-null. Buying returns offers,
not completed orders; bare `selling` returns active offers only.
`page URL` offers a server-rendered snapshot of known Ricardo read routes, not
arbitrary browser control or a complete client-rendered page.

## Seller views

Extension 0.3.0 adds read-only seller views (reload the unpacked extension and
connect the bridge again after updating):

- `ricardo selling sold [--offset N] [--limit N]` lists sold orders (limit 1-30,
  default 30) with CHF amounts, the per-order success fee and rate, payment,
  delivery, tracking and ratings, plus Ricardo's own status buckets.
- `ricardo selling drafts` reads the drafts tab. No draft items had been observed
  when this was written, so the item shape is reported as unverified.
- `ricardo selling status` reads the listing limit, whether orders await action
  and the unread-message count. It makes three paced requests (about 16 seconds
  or more) and never reads message contents.

Buyer names, contact details, addresses, payment references and pickup addresses
are omitted by design; the output is built from an allow-list, and sold orders
are not saved to local history. Order IDs, amounts and tracking numbers are still
private. Creating, editing and publishing listings stays in the browser. The
observed routes are recorded in [docs/seller-routes.md](docs/seller-routes.md).

## Agent contract and local history

```bash
ricardo robot-docs guide
ricardo capabilities --json
ricardo history "Bambu P2S" --limit 10 --json
```

With `--json`, stdout contains one envelope:

```json
{"schema_version":"1","ok":true,"data":{},"error":null}
```

Errors use `ok:false`, `data:null`, and
`error:{code,message,hint,retryable}`. Check the process status and `ok`.
See [the agent contract](docs/agent-contract.md) for exit codes and examples,
and [the local skill](skills/ricardo-cli/SKILL.md) for agent usage instructions.

History stores earlier observations in the private data directory, by default
`$XDG_DATA_HOME/ricardo-cli` or `$HOME/.local/share/ricardo-cli`. History establishes
what was observed earlier, never current auction availability. There is no
scheduled watch daemon yet.

## Worker integration and verification

`ricardo bridge serve` provides a JSONL worker connection on stdin/stdout as an
alternative to native messaging. It must be attached to a worker; running it
alone does not connect a browser. Requests have an `id` and `request`; workers
reply with the same `id` and either `result` or `error`. The extension uses the
hidden `bridge native` command with length-prefixed Chrome messages.

```bash
cargo test --locked
node --test tests/
```

Fixture and protocol tests validate parsing and error behavior. They do not
establish that your extension is installed or your account is connected. Run
`doctor` and a real search to exercise your own logged-in path. Ricardo interface
changes can require parser or route updates; the CLI should then report an
actionable error rather than inventing successful results.

## Gentle request pacing

Run one CLI command at a time. The shared native bridge permits only one
Ricardo request in flight and waits eight seconds after completion before the
next request. Concurrent commands fail with `request_busy` without contacting
Ricardo. Search checks articles sequentially and stops at its first failure.
The default search limit is five candidates. There are no automatic retries.
HTTP 429 or 403 pauses requests for two minutes; that cooldown survives reconnects.
A connection failure also delays subsequent requests. These limits govern this
CLI, not manual browsing or other programs, and cannot guarantee site acceptance.

### Tutti and photos

The same browser bridge now supports Tutti first-page search, listing inspection,
and local gallery downloads for both marketplaces:

```bash
ricardo tutti search Prusa Core --limit 3 --json
ricardo tutti inspect TUTTI_LISTING_URL --json
ricardo images RICARDO_ID_OR_LISTING_URL --json
ricardo images RICARDO_ID_OR_LISTING_URL --index 3 --json
ricardo tutti images TUTTI_LISTING_URL --json
```

Open the returned `images[].path` files to inspect hours, accessories and condition.
Each run creates numbered files and a manifest in its own private directory.
Photos are fetched sequentially through the shared eight-second pacing gate.
Reload the unpacked extension after updating and reconnect it. Version 0.2.0
adds only Tutti and the Ricardo/Tutti image CDNs to the host permissions;
version 0.3.0 adds the seller views and no new permissions.
See [the agent contract](docs/agent-contract.md) for limits and freshness semantics.
