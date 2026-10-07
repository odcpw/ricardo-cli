# Listing worksheet — <local listing ID>

Status: local copy / saved draft / verified draft / active / ended
Evidence checked at: <timestamp and timezone>
Channel and format: <ricardo auction | ricardo buy-now | ricardo auction with buy-now | tutti classified> • Pickup/shipping: <both | pickup only | shipping only>
Reserved lot IDs: <IDs> • Quantity offered: <number of purchasable lots>
Listing language: <DE | FR | IT> • Seller-language gloss prepared: <yes/no>
Title (<= 60 characters, in the listing language): <searchable product type, brand if known, exact variant, quantity, defining trait>
Category and attributes: <verified Ricardo category/product type and attribute values; do not guess missing identity>
Condition selection: <form option matched to evidence; see condition note below>
Price and format: <currency CHF, start price, bid increment, buy-now price, duration/closing time, automatic reactivation (decided deliberately), offer policy>
Reference number (optional, <= 40 characters): <opaque local listing ID only; no names, addresses or contact details>

## Language note (DE / FR / IT)

Write the buyer-facing text once, in the language the seller chose for the likely buyers, using the vocabulary buyers search with (German, French and Italian differ in technical nouns and in size and unit conventions). Ricardo accepts German, French, Italian and English listing text and gives buyers a machine-translation button with no liability for its accuracy (RICARDO-PLATFORM.md (listing content rules); verify live). Do not mix languages by accident and do not machine-translate specifications without checking them against the evidence. Keep a short gloss in the seller's working language so they approve what buyers will read. If the seller wants a second language, treat it as a deliberate addition in the description, within the limits below. Draft buyer replies in the buyer's language.

## Buyer-facing description (<= 5000 characters, plain text)

No phone numbers, e-mail addresses, links, bank data or hints about selling elsewhere in any text field or photo (platform rule: [RICARDO-PLATFORM.md](../references/RICARDO-PLATFORM.md) (listing content rules)). Answers you later give to buyer questions are public and part of the listing's terms.

<One clear sentence identifying exactly what the buyer receives.>

**Included / Enthalten / Inclus / Incluso:** <exhaustive meaningful contents and quantities>.
**Not included / Nicht enthalten / Non inclus / Non incluso:** <commonly expected accessories absent from this sale>.

**Details:** <supported identifying specifications, size, material, edition, plug type or voltage, compatibility>.

**Condition:** <visible wear and defects, separately from functional condition>.
**History and verification:** <known history; owner report versus actual recent checks; limits>.
**Photos:** <actual-unit photos, or explicit representative-lot disclosure if justified>.

**Shipping or pickup:** <domestic parcel service and cost as set in the form; or pickup area (no street address) and how the buyer pays at handover>.
**Payment:** <methods exactly as the form offers them for the chosen delivery option>.
**Returns / Rückgabe:** <the seller's chosen statement; no false waiver, no promise the platform does not support>.

## Ricardo listing-form checklist (observed in the Ricardo listing form 2026-10-07)

Re-read the live form before relying on any item: labels, limits and defaults change. Policy facts (fees, formats, payment, returns) are in [RICARDO-PLATFORM.md](../references/RICARDO-PLATFORM.md); this list is only what the form showed.

- **Photos:** slots Hauptbild (main), three "Jede Seite" (each side), Mängel (defects), Details; QR upload from a phone exists. Upload approved derivatives only.
- **Titel:** maximum 60 characters. The form offers an AI title helper and chooses a product type (Produkttyp) after a photo or title: verify suggestions against the evidence.
- **Zustand (condition):** Gebraucht, Neu und originalverpackt, Neu (gemäss Beschreibung), Defekt. "Neu und originalverpackt" implies original packaging; never promote unused-but-opened or "owner says new" to it. Read the form's own definition of each option before choosing, and make the description say what is actually true (including for "Defekt").
- **Beschreibung:** maximum 5000 characters. **Referenz-Nr.:** optional, maximum 40 characters; put the opaque workspace listing or unit ID there so the live listing ties back to the ledger.
- **Angebot & Preis:** an auction toggle with Startpreis and Erhöhungsschritt (bid increment; the form defaulted to CHF 1.00), a Sofort-Kaufpreis (buy-now price), Laufzeit with start and end (scheduling is possible), and "automatic reactivation of your offer up to 9 times" (the terms summarised in RICARDO-PLATFORM.md state a different count for auctions: trust the live form and the terms read on the day). Choose each value deliberately; never accept the default start price or reactivation by omission. Confirm after saving that format, start price and buy-now price persisted.
- **Lieferung:** Versand (the seller sets the shipping cost) and/or Abholung (pickup at the account address; check what location the listing exposes before publishing).
- **Zahlung:** the form showed cash payment on pickup.
- **Publishing** accepts the platform terms (AGB) and notes that a success commission (Erfolgsprovision) applies when the item sells; the seller must accept. Buttons: ENTWURF SPEICHERN (save draft), VERÖFFENTLICHEN (publish). The CLI cannot save drafts or publish.

## Tutti notes

A Tutti ad is an asking-price classified: no bids, no auction format, and (per RICARDO-PLATFORM.md, verify live) free for a 60-day runtime with no sales commission and no integrated payment found. Price goes in the price field, not the title; no contact data, links or QR codes in title or description; one item per ad; the item must be in Switzerland. Use the same truthful contents, condition and language rules; state pickup area without a street address; arrange payment only through evidence the seller can see. A private seller's warranty exclusion works only if stated explicitly: that wording is the seller's decision. Tutti IDs are the full canonical `https://www.tutti.ch/de/vi/...` URL.

## Internal checks — never paste this section into the listing

- Approved derivative hashes and exact upload order.
- Comparable results (with unknown prices left unknown) and delivered-price normalization.
- Fee source, net calculation, packing basis and downside case, including the start-price floor.
- Account and seller identity; selling limits; quantity and stock allocation; draft persistence and the reference number.
- Buyer preview; offer settings; reactivation; paid options; current authority.
- Publication timestamp, live article ID and active-page verification (`ricardo inspect`).
