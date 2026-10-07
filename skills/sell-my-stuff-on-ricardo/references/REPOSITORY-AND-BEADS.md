# Repository and Beads

## Create one private sales workspace

Inspect the requested folder and existing Git state first. Reuse a relevant repository rather than nesting repositories or overwriting work. Read its governing instructions. For a new workspace, initialize Git, create the requested private GitHub repository using an available supported tool, and verify the remote owner/name and privacy before the first push. Do not assume `private` because a creation command requested it.

Use a compact layout:

```text
README.md                     current workflow and record locations
PLAN.md                       latest seller preferences and authorization
data/ledger.json              canonical units, lots, photos, listings, orders
data/*.csv                    optional exported views; never a second truth
photos/listing/               reviewed, cleaned upload derivatives
listings/                     copy, draft references, public article IDs/URLs, verified settings
evidence/                    sanitized comparable notes and sources (URL, date, channel)
operations/                  packing, economics, monitoring, handoffs
.beads/                      tasks and dependency graph
originals/                   LOCAL ONLY, ignored
private/                     LOCAL ONLY, ignored (also comp photos from `ricardo images`)
private/orders.json          order IDs, tracking numbers, order amounts, keyed by local order id
```

Before adding anything, ignore originals, private records, browser/session exports, labels, tracking numbers, credentials, IBANs and bank details, temporary downloads (including photos downloaded by `ricardo images`, which belong to other sellers), SQLite databases/locks, and tool caches. Keep a separate local map for original names/paths that reveal personal information. Hash manifests and cleaned derivatives can be tracked; consider Git LFS for large derivatives before their first commit. LFS is storage, not a privacy boundary.

Review tracked filenames and staged content for buyer data, correspondence, credentials, addresses, tracking numbers, location metadata, personal surroundings, and original images. A private remote is not permission to upload them. Stage explicit reviewed paths. Check the full staged set before committing so unrelated work is not swept in. Verify push success and remote commit identity; never imply that a local commit proves backup.

Review the actual **staged blob** when scanning staged content (`git show :relative/path` or an equivalent index reader), not just the working-tree file. The working copy may have been sanitized after sensitive bytes were staged. Also inspect previously tracked files: ignore rules do not untrack them. If sensitive data has already reached history, stop further propagation and handle containment explicitly; a later clean file does not erase previous disclosure.

## Separate records by purpose

- The ledger is the physical stock truth; derive optional inventory/allocation/photo CSV views from it. Every unit is allocated once; representative photos do not create additional stock.
- Listings map exact reservations to real draft references and active article IDs (Ricardo) or listing URLs (Tutti). Preserve superseded draft references to prevent accidental reuse.
- Evidence records what was observed, where, and when. Estimates and experiments remain labeled.
- Beads records work, dependencies, and completion evidence. It does not replace the stock ledger.
- Monitoring records last verified state and coverage. Old notes do not override fresher direct evidence.

Use stable local references to join these records without buyer identity. When a bundle changes, release/reserve its exact membership atomically in the ledger before altering a listing. If marketplace state is uncertain, keep the units reserved until reconciled.

Before committing a canonical transition, assert the expected prior unit/lot states, preserve a dated action receipt, and verify readback. A sequence of separate JSON/CSV writes is not one atomic transaction. Derive views after the canonical record succeeds and reconcile any interrupted export; do not let multiple files independently define current quantity.

If the seller wants a workbook, generate it from the canonical ledger. Size ranges from current data; check source-ID joins, formulas/totals and rendered sheets separately. A valid spreadsheet archive does not prove correct formulas or readable layout. Treat edits in the workbook as proposed changes to reconcile back, not an uncontrolled second source of truth.

## Build a small dependency graph

Use the installed `br` CLI. Check `br --help` and relevant subcommand help when syntax differs; do not install an unwanted package manager or rebuild the tracker merely to start selling. Initialize only if this workspace has no Beads store:

```bash
br init --prefix sale --json
br create 'Reconcile inventory and reviewed photos' --type task --priority 1 --json
br create 'Prepare one listing draft' --type task --priority 1 --json
```

Capture returned IDs rather than inventing them. For actual IDs held in task-specific variables:

```bash
br dep add "$draft_task" "$inventory_task" --json
br dep add "$draft_task" "$pricing_task" --json
br dep add "$draft_task" "$packing_task" --json
br dep add "$publish_task" "$draft_task" --json
br dep add "$fulfillment_task" "$publish_task" --json
br dep cycles --json
```

The first argument depends on the second. Include account/preview checks before publication. A publication task closes only with verified live evidence (`ricardo inspect` showing the article active, plus the seller's own page); fulfillment continues through shipment and return resolution. Prefer one useful task per listing stage or coherent batch over a task for every click. Block only affected stock.

Make task creation restartable: use a stable local stage/listing key, look for an existing matching task, and record each returned ID immediately. A bulk creation loop interrupted halfway must reconcile those results before rerunning. Do not infer idempotency from a successful CLI invocation.

## Work and close on evidence

```bash
br ready --json
br list --json --limit 0
br list --status closed --json --limit 0
br show "$task_id" --json
br update "$task_id" --status in_progress --json
br update "$task_id" --append-notes 'Verified result and evidence path.' --json
br close "$task_id" --reason 'Acceptance criteria met; evidence recorded.' --json
br sync --flush-only --json
```

Replace example note/reason text with the actual proof. Inspect counts and pagination fields; a truncated list is not a complete review. After incorporating upstream tracker changes, use `br sync --import-only --json` as appropriate before editing; reconcile conflicting work rather than blindly forcing either side. `br` does not run Git for you.

Keep completed prerequisites current instead of forcing publication past stale tasks. If the marketplace already changed, reconcile the facts and task graph; do not perform the action again to make the tracker appear consistent. Back up and diagnose a broken tracker without deleting history or unrelated work. A tracker repair must not become a substitute for ready sales work.

For schema/tool failure, preserve database, journal/WAL and exported history before a supported repair; inspect binary path/version and use the relevant tracker repair skill. An issue export may omit event history, so rebuilding from JSONL is not automatically lossless. Respect the seller's preferred installation method. During the outage, capture a minimal sanitized action journal and continue independent authorized work; do not call unrecorded external actions unperformed.

At session end, record verified outcomes, the smallest real blockers, and the next physical or account action. Export Beads, inspect explicit staged files, commit/push authorized work, and preserve unrelated edits. Do not declare a sale completed merely because its draft, listing, or administrative task is complete.
