# Authenticated browser bridge

The bridge reads Ricardo through an already signed-in `https://www.ricardo.ch` tab. It does not read, export, or store cookies. Native messaging connection does not prove account authentication. Authentication is verified separately using Ricardo responses.

## Chrome / Chromium extension

1. Build the CLI and keep the executable at a stable path.
2. Open `chrome://extensions`, enable Developer mode, and **Load unpacked** the repository's `extension` directory.
3. Copy its extension ID. Run the CLI's `extension install --extension-id ID` command to register the native host for your profile. See `ricardo extension install --help` for the exact accepted syntax.
4. Open Ricardo and sign in manually in that Chrome profile.
5. Click the Ricardo CLI extension and **Connect bridge**. Then run the CLI's bridge status and account commands.

Installation writes the native host manifest in the Linux Google Chrome and Chromium default configuration locations and creates a private launcher in the CLI data directory. Other browser profiles may require copying the manifest into their configuration's `NativeMessagingHosts` directory. The launcher refers to the current executable: install again if you move it. Launcher and manifest updates replace complete files atomically and refuse symbolic-link targets. Extension installation and account login are user actions; the tool does not alter login state.

Reads use explicitly allowlisted GET routes. Saved-article changes use a separate fixed operation. The extension accepts neither JavaScript supplied by CLI callers nor arbitrary URLs or HTTP methods. The extension currently supports search/detail pages, the home page used to check login, saved-article and buying-offer endpoints, and the selling-offers page. Additional account capabilities require observed endpoint verification and an explicit route addition. A numeric-only `resolve` operation fetches the fixed article URL inside an existing Ricardo tab with a 10-second deadline. It follows redirects with `mode: "same-origin"`, which rejects external redirects, then validates HTTP status, origin, and the exact article ID in the canonical URL. It cancels the response body after reading the URL and never creates or navigates browser tabs. Other reads and saved-article mutations continue to reject redirects.

## Agent worker transport

`ricardo bridge serve` exposes the same private Unix socket and uses newline-delimited JSON on its standard streams. An agent with an authorized browser integration can service requests without installing the extension:

```json
{"id":"correlation-id","request":{"op":"fetch","path":"/de/s/bambu%20p2s/"}}
```

The worker must perform an authenticated same-origin GET in the Ricardo browser tab and send exactly one response on the broker's stdin:

```json
{"id":"correlation-id","result":{"status":200,"url":"https://www.ricardo.ch/de/s/bambu%20p2s/","content_type":"text/html","body":"...","checked_at":"2026-09-05T12:00:00Z"}}
```

Failures use `{"id":"...","error":{"code":"browser_error","message":"..."}}`. Standard output is exclusively protocol traffic; send diagnostics to standard error. EOF disconnects the worker and shuts down the socket. Requests have 45-second deadlines, with at most 16 concurrent clients and 8 MiB protocol frames. Native extension responses are capped at 4 MiB before JSON encoding.

The Unix socket lives in the selected CLI data directory as `bridge.sock`, with mode 0600 inside a 0700 directory. A persistent advisory lock serializes startup. An existing live broker is never replaced; a stale connection-refused socket is removed. Socket cleanup runs on normal shutdown and error returns, and worker output failures disconnect pending clients promptly. The broker's `ping`/`status` reports transport availability and explicitly marks authentication unverified.

`ricardo bridge native` is the Chrome native messaging variant: four-byte little-endian frame lengths, followed by JSON. Run it only via the native host launcher.

## Saved articles

The browser accepts `{"op":"favorite","action":"add","article":"1327242570"}` or the same request with `"action":"remove"`. Article IDs must contain only digits. Additional request fields are rejected. Add uses exactly `POST /marketplace-spa/api/saved-articles` with JSON `{"articleId":"1327242570"}`; remove uses exactly `DELETE /marketplace-spa/api/saved-articles/1327242570` with no body. Both endpoints were observed in Ricardo’s own interface.

These calls execute inside the existing Ricardo tab with browser-held credentials. The bridge returns the HTTP status and response body and never retries a mutation. A network failure leaves the outcome uncertain; read saved articles before deciding whether another action is needed. This operation cannot submit bids, purchases, seller messages, or other account changes.

The extension uses only `nativeMessaging`, `scripting`, and the Ricardo host permission. Chrome permits querying matching tab metadata through a host permission, so broad `tabs` access is unnecessary: [Chrome Tabs API permissions](https://developer.chrome.com/docs/extensions/reference/api/tabs#permissions).

## Gentle request pacing

Run one CLI command at a time. The shared native bridge permits only one
Ricardo request in flight and waits eight seconds after completion before the
next request. Concurrent commands fail with `request_busy` without contacting
Ricardo. Search checks articles sequentially and stops at its first failure.
The default search limit is five candidates. There are no automatic retries.
HTTP 429 or 403 pauses requests for two minutes; that cooldown survives reconnects.
A connection failure also delays subsequent requests. These limits govern this
CLI, not manual browsing or other programs, and cannot guarantee site acceptance.
