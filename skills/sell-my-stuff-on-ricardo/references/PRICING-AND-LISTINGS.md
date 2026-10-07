# Pricing and listings

## Find comparable sales

Use the `ricardo` CLI ([mapping](RICARDO-CLI.md)) or the authorized browser to inspect sold and ended items. Start with exact identifiers, then widen deliberately. For each useful comp record its URL, observation date, channel and sale format, model/variant, quantity, condition, included accessories, item price, shipping, currency, and material differences. Whether an ended Ricardo result is sold and whether it shows a final price is read from `status` and `sold_price` in the actual output; a result without a final price keeps that price unknown. Tutti shows asking prices only.

Compare delivered prices on a per-unit or per-set basis without pretending quantities are interchangeable. Separate auctions, defective goods, refurbished stock, retailer warranties, and accessory-rich bundles. An active asking price or a running auction's bid is a competitive reference, not evidence of a completed transaction. Weak or missing comps justify a labeled asking-price experiment; they do not establish the value of all remaining stock.

Delivered price is item price plus buyer-paid shipping. Other evidence supports a candidate asking range for the current item; do not backfill a guessed realized amount. An unknown maker is not automatically a blocker if the object can be described accurately and safely without attributing a maker; hold only material missing facts.

Inspect original successful listings where available. Borrow their useful structure and specificity, not their wording, branding, testing claims, photos or warranties. A high sold price does not establish that its formatting caused the result.

## Choose a sale format and worthwhile lot

Recommend an individual sale, coherent bundle, local pickup, Tutti or another suitable channel based on delivered economics, buyer demand, preparation effort, and the seller's urgency. Preserve valuable matched sets. Record the decision with the [format and start price worksheet](../assets/decision-worksheets.md#format-and-start-price) and the [channel worksheet](../assets/decision-worksheets.md#ricardo-vs-tutti-vs-pickup).

- **Auction:** the start price is the lowest price you could be held to: the platform's terms bind the seller once a bid exists (the listing then accepts only added text and images, and cannot be ended), and a bid in the closing minutes extends the close (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (formats, obligations)). Choose it from the net floor and matched comps, never by form default. Ricardo's own guidance favors low start prices to attract bidders, and its commission has stackable discounts, one of which applies to auctions with a start price of CHF 1 or less; per the file each discount is 10% of the commission amount, a small saving that never compensates for a start below the floor. A token starting bid is allowed only as a recorded seller choice after the downside case is shown. Choose duration and closing day for when buyers bid and the seller can handle the handoff (Ricardo's guidance suggests evenings and weekend closings; the file records the details).
- **Sofort-Kaufpreis (buy-now, fixed price):** price control and speed; evidence for the price must be a matched comp range, not hope. A format cannot be converted after publishing.
- **Auction with a buy-now price:** both prices must pass the downside case; the buy-now price sits above the start price and is one you would accept immediately, because a buyer who takes it ends the auction.
- **Price proposals (Preisvorschläge), the nearest analogue of a best-offer feature:** per RICARDO-PLATFORM.md they exist on fixed-price listings (not auctions) and for 60 days on unsold ended listings, where the option is on by default; there is a minimum share of the price, a short response window with silence expiring, a limit per buyer, and an accepted proposal is binding. Enable them, and accept or reject, only with offer authority and a floor from the economics tool. Pending proposals are auto-rejected when a listing is reactivated.
- **Tutti:** an asking price with no bidding; listings are free for a 60-day runtime that can be renewed and no sales commission was found, and no integrated checkout or payment was found, so payment is arranged between the parties (RICARDO-PLATFORM.md (Tutti)). Leave negotiation room only if the seller has authorized it. It is the natural pickup channel.

Calculate each candidate sale:

```text
net = item proceeds + shipping collected
      - marketplace success fee and payment costs - postage - packing - other sale costs
      - chosen risk allowance
```

Model fees using the current account, category and channel rules and their applicable base. Cost lines per RICARDO-PLATFORM.md (fees), observed 2026-10-07, to be verified live before quoting any net: no listing fee for ordinary goods; a success commission of 8 to 12 percent depending on product type, with a minimum and a maximum per sale, shown in the listing form before publishing (declaring a wrong product type to lower it is prohibited); stackable commission discounts under stated conditions; paid visibility boosts (Bronze, Silber, Gold) priced only in the form, billed per listing run and again on every reactivation; Swiss Post label costs charged when the parcel is scanned; and fee invoices. Whether the commission base includes shipping is unsettled: the terms as relayed say yes and the observed order records say the item price only, so read the fee on the first sale and reconcile every sale against its own record ([money and closure](MONEY-AND-CLOSURE.md#replace-the-fee-assumption-with-the-recorded-fee)). Show estimated versus verified inputs, shipping undercollection risk, and the risk allowance (for example a dispute reserve) as separate lines. Do not reuse one observed rate across product types or orders: the recorded rate differs between orders.

The user's minimum net is a floor, not a universal asking price or an offer-acceptance instruction. Compare conservative scenarios before assigning prices. If a sale is uneconomic, improve the bundle or packing plan, choose pickup, or recommend another channel; do not manufacture an optimistic valuation to force it through.

Record format, start price, buy-now price, duration, rationale, confidence, offer authority, minimum economics, and review date. Default to no automatic markdowns, no acceptance or rejection of price proposals without offer authority, no seller-initiated discounts, boosts or other paid options, or automatic reactivation unless explicitly authorized; a boost is a cost that recurs on each reactivation. A quick sale is a signal to reassess remaining stock, not proof that a particular higher price would have sold.

## Write the listing

Use an accurate search title that leads with familiar product or category terms and meaningful attributes, followed by the exact brand/model, within the form's character limit ([listing template](../assets/listing-template.md) lists the limits observed). Keep contact details, links and hints about selling outside the platform out of every text field: the platform prohibits them (RICARDO-PLATFORM.md (listing content rules)). Choose the category by buyer intent and actual product identity. Before cutting price on an obscure item, investigate category, category attributes, title discoverability, language and delivered shipping cost. Do not invent popular use cases or compatibility to obtain traffic.

Write in the language the seller chose (German, French or Italian) so the likely buyers can read it; a mixed-language title is a deliberate choice, not an accident. Keep a short gloss in the seller's working language for their approval. Do not machine-translate technical claims without checking them against the evidence.

Use original, restrained, mobile-friendly copy:

1. Exact product and quantity in one clear opening sentence.
2. Key specifications supported by labels or authoritative documentation.
3. Explicit included and excluded contents.
4. Cosmetic condition, known history, and functional condition as separate facts.
5. Compatibility limitations or setup requirements material to a buyer (for example plug type or voltage).
6. Packing approach, delivery and pickup terms, and the return statement.

Describe every defect: under the terms as relayed, a private seller is liable for undescribed defects that significantly impair value or use, and the description, photos and public answers together form the contract. Whether a warranty-exclusion sentence is effective was not verified (Tutti says a private seller can exclude warranty only if stated explicitly): that wording is the seller's decision, never added by the agent. Owner-reported working condition is not fresh testing. Unused is not necessarily sealed. Photos and title must agree with the unit or lot being sold. Use the form's supported formatting as plain text; avoid links to outside sites, contact details that invite leaving the platform, policy walls, or terms inconsistent with the selected settings.

Choose the condition option from the form's current definitions and actual evidence. The form observed 2026-10-07 offers four: Gebraucht, Neu und originalverpackt, Neu (gemäss Beschreibung) and Defekt; its helper text for Gebraucht says used but functional, so choosing it makes a functional claim. "No fresh test" alone is not proof of either fully working or defective. Do not automatically downgrade every owner-reported item to defective, or promote uncertain function to tested working. "Unused" without sealed original packaging is not "original-packaged". When an option implies a material claim not supported here, resolve that gap or choose an accurate allowed representation for the affected item.

If generating text from structured facts, keep formatting minimal and review title, first photo, attributes, condition and description independently for likely misunderstandings: an accurate paragraph cannot cure a false attribute. Check all essential claims and sections after save, not merely the presence of one heading. Keep internal economics, authority and private evidence out of buyer-facing content. Treat the form's AI title or product-type helpers as suggestions to verify, never as evidence.

## Save, verify, and publish

1. Reserve the exact units. Reconcile existing drafts and listings first (`ricardo selling drafts --json` if listed, `ricardo selling --json`) so a resumed session cannot duplicate a sale allocation.
2. Fill the real listing form in the correct signed-in account using approved photos and supported visible controls, then save a draft. Derive controls from current UI evidence; never invent clicks, selectors, hidden APIs, or session workarounds. The CLI cannot create or edit drafts.
3. Verify category, condition, category attributes, title, description, photos/order, lot quantity versus available quantity, format, start price, buy-now price, duration, delivery and pickup, payment, returns statement, offers, reactivation and paid options. Adapt all settings to the seller's brief.
4. Save and reopen. Check persisted fields and the buyer preview against the local record (`compare-draft.py`). A local file is not a saved draft, and a preview is not publication.
5. Verify account readiness (`ricardo selling status --json` if listed: limits, pending actions), packing supplies, and order-notification access. Finish every independent preparation step before surfacing a user-only login, terms or payment action.
6. Publish within the existing authorization; the seller accepts the platform terms and commission notice in the form. Do not require redundant approval; if a tool or unresolved decision requires the user, identify the specific remaining action and the concrete draft ready for it.
7. Verify the active listing: `ricardo inspect ARTICLE_ID --json` for status, price fields, shipping, condition and seller, `ricardo images ARTICLE_ID --json` for the gallery. Record evidence and move the reservation to listed only after verification.

On timeout or ambiguous submission, retain the reservation and inspect drafts, active listings, and recent results before retrying. Launch a limited batch, confirm the listing and fulfilment workflow, then expand without overwhelming the seller's packing capacity.

## Ricardo and Tutti policy topics to re-read

Re-read the sourced, dated file at the time of use: [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md). It describes policy, not what a particular account will be charged or allowed. Expected topics:

- Fees: success fee, listing and option fees, payment costs, how and when they are charged (fees).
- Formats: auction, buy-now, combinations, duration, start-price rules, reactivation (formats).
- Photo and listing-content requirements, prohibited and restricted items (photo requirements, prohibited items).
- Payment methods and payout, unpaid-buyer handling (payments).
- Delivery services, pickup, returns, disputes and buyer protection (shipping, returns, disputes).
- Tutti: free versus paid options, messaging, safety guidance (Tutti).
