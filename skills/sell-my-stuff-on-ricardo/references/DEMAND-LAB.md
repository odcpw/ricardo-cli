# Research buyers, diagnose demand and design experiments

Use when choosing a sale proposition, interpreting weak traffic or deciding whether a fast sale justifies releasing more inventory. Seller price authority always outranks a tool's computed floor.

## Build a comparable-evidence funnel

1. Start exact: maker/model/edition/size/material/quantity and relevant condition.
2. Expand one dimension at a time: synonyms, adjacent variant, same buyer function, and the other language (German, French or Italian keywords). Keep exact and analogous evidence separate.
3. Inspect sold items and ended unsold items: `ricardo search QUERY --include-ended --json`, then `ricardo inspect ID --json`. Active asks describe competition; ended unsold listings reveal attempted prices, not proven demand. Whether an ended result is sold, and whether a final price is shown, is read from `status` and `sold_price` in the output ([CLI mapping](RICARDO-CLI.md#reading-comparable-results)), never assumed.
4. Open the strongest and anomalous results. Verify contents, defects, shipping, sale format, date and seller warranty. Read the seller questions and answers in `inspect` for what buyers actually asked. Do not assume the highest displayed result is the best matched sale.
5. Record rejected comps and the reason when they would materially change pricing. Stop when another search is unlikely to change the batch decision; sparse evidence should stay sparse. One search page is a sample of that page, not of the market.

Use a compact evidence row:

```text
comp_id | URL | observed_at | sold_at | exact/adjacent | channel | format
variant | quantity_basis | condition | contents | warranty
item_price | shipping | currency | final_price_known | exclusion_reason
```

Unknown shipping makes delivered cost unknown. An unknown final price stays unknown: do not substitute the last asking price or the last bid of an unfinished auction. A Tutti asking price is an ask, not a sale, and Tutti shows no realized price. Do not use a crossed-out price as a numeric sample. Do not mix currencies without a dated conversion basis, or divide a premium matched set into an assumed per-piece price.

For usable matched sales, show sample size, date span and range or median rather than a false precise valuation. Keep sold dates and observation dates separate. Note survivorship, seasonality, refurbished or warranty premiums, incomplete kits and auction urgency. No observed formatting pattern proves causal price uplift.

For a whole collection, separate sold-supported, analogous and experimental/unpriced segments. Show gross asks and plausible net scenarios separately, including time, quantities and sell-through assumptions. Identify which weakly evidenced segment dominates the apparent total. Multiplying a thinly supported experimental ask by every unit is not a reliable collection valuation. A small live experiment can improve evidence without assigning false certainty to the remaining stock.

## Turn specifications into a buyer proposition

Create a three-column map: **verified property → practical buyer benefit → qualification**. Example: measured storage dimensions → fits a particular space → buyer must check clearance. Manufacturer capability can support a use case, but does not prove this used unit was tested for it. A theoretical adapter or expansion slot does not establish complete-system compatibility.

Use familiar category nouns and buyer tasks before obscure branding when appropriate, while retaining exact identity. Populate truthful category attributes (the Ricardo form asks for a product type after a photo or title; accept a suggested type or title only after checking it against the evidence). Search synonyms should describe the actual object, not unrelated popular products. Retain defects and absent accessories prominently even when they reduce conversion.

Design a photo sequence to answer buyer questions: what is it, what is included, which exact variant, what condition, what defect, what size or connection. Each image should add evidence. A crowded table of unrelated goods creates uncertainty even if the title is correct.

## Diagnose the funnel before changing the ask

| Observation | First hypotheses | Next discriminating check |
|---|---|---|
| Listing absent in search | Category, restrictive filters, geography, title identity, wrong language | `ricardo inspect` the article ID, then search exact title and category with recorded filters |
| Views but few bids or questions | Main photo/title, irrelevant category, delivered price, start price too high | Compare search-result presentation with truly matched alternatives |
| Views without buyer action | Contents/condition uncertainty, compatibility, total cost, patience | Read the buyer preview and comparable details; inspect repeated questions |
| Watchers but no purchase | Timing, comparison shopping, seller's price experiment | Observe over the agreed period; watchers do not authorize price changes |
| Questions about the same detail | Missing or confusing information | Add a supported fact or photo within revision authority, not invented reassurance |
| Immediate purchase | Strong fit, limited supply, favorable price, or chance | Reassess remaining stock with additional comps and independent demand events |
| Auction closed far below comps | Start price or timing, thin audience, weak photos | Do not generalize one close; revisit start price, closing time and wording for the next lot |
| Units sold but delayed handoff | Capacity bottleneck | Finish obligations before exposing more stock |

A search failure is scoped to query, filters, language and observation time. Do not declare global invisibility from one personalized search. Record view/watch metrics with their window and definition. Counters can reset or be recalculated after revisions; a negative difference is not negative demand. Never add snapshots to make a lifetime total.

## Run a bounded listing experiment

Use `experiment_id`, listing reference, hypothesis, baseline date/window, intervention, fixed settings, expected signal, review date and stop condition. Change the smallest coherent factor set. If title and category must change together, call it a combined intervention rather than claiming attribution to one word.

Example: "Buyers search by object function rather than maker. Revise title vocabulary and truthful category attributes; hold ask and shipping constant; compare compatible observation windows." If the interface resets metrics, start a new baseline. Short windows and tiny samples support qualitative learning, not significance claims.

Do not invent A/B capability, duplicate live listings, or allocate the same unit to two test arms (including an auction and a Tutti ad). Use sequential revisions or genuinely separate stock. Promotions, discounts and paid listing options are separate authorized experiments with explicit cost limits.

## Decide whether to release more

Review independent transactions, questions, time to sale, net after real fees (read from the sold order, not assumed), return signals and physical throughput. Count units and customer/order events separately. Consider one quantity listing for identical goods only if the format supports it (RICARDO-PLATFORM.md (formats) notes quantity listings exist for fixed prices, not auctions; verify live); differing condition, included items or unrepresentative photos require individual treatment.

Release a bounded tranche, preserving the same stock ledger. Revise asking strategy only within authority. A quick sale may justify researching a higher next ask; it does not prove that the original sale was a mistake or that every remaining unit will clear at the new price.
