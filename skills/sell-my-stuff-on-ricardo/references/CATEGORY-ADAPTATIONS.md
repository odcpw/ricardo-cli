# Adapt the method to the object

This is a routing guide, not a statement that every object is eligible for Ricardo, Tutti or shipment. Verify current marketplace and carrier rules for the actual category, jurisdiction and route (verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (prohibited and restricted items, shipping limits)). The design extends the skill's evidence, contents, economics and physical-handoff principles.

## Select the constraints that change the plan

| Object characteristics | Evidence buyers need | Sale/packing adaptation | Hold only when material |
|---|---|---|---|
| Clothing, footwear, textiles | Size label plus actual dimensions where needed, material, wear/stains, alterations | Separate labeled size (EU/UK/US labels differ) from fit; coherent lots by buyer/size, protected dry packing | Unknown authenticity or undisclosed material defect; do not invent measurements |
| Books, records, media, sets | Edition/printing, completeness, relevant condition surfaces | Preserve valuable sets; compare edition-specific comps; protect corners/discs | Missing edition/volume materially changes offer |
| Fragile ceramics, art, glass | Marks/signatures, dimensions, all damage/restoration, base/back | Model nested protection and carton size; pickup if shipment risk dominates | Attribution or fragility cannot be represented/handled credibly |
| Tools, appliances, instruments | Exact model, power/connection needs (plug type, voltage), working evidence, accessories | Isolate heavy parts; distinguish bare tool from kit; preserve matched accessories | Unknown hazardous state or incompatible essential parts |
| Collectibles and luxury goods | Variant/edition, provenance, distinguishing details, condition | Do not inherit authenticity claims from comps; research category-specific authentication rules | Unsupported attribution materially changes value or eligibility |
| Furniture, bulky/awkward goods | Dimensions, damage, disassembly, floor/lift/access constraints | Pickup (Tutti or Ricardo Abholung) or freight; packing and labor economics; no standard parcel fiction | Cannot establish a feasible authorized handoff |
| Battery-powered, liquid or otherwise restricted goods | Relevant composition, battery condition/type, leakage or damage facts | Verify route, service and packaging restrictions before listing delivery options | Safety or eligibility unresolved; no generic service assumption |
| Data-bearing or account-linked goods | Ownership, reset/unlink state, included licenses/accessories | Keep seller data private; plan a verified reset/unlink before transfer | Data/account condition unresolved; do not claim erased merely because power is off |
| Spare parts and incomplete assemblies | Exact included pieces, identifiers, defect/unknown function | Sell accurately as parts or incomplete (the form's defect condition) only if allowed; compatible lots with clear boundaries | Guessing compatibility or calling unknown function "working" |
| Vehicles, regulated or age-limited goods | Registration and documents, legal status | Usually outside this skill's lifecycle; verify rules before any listing | Eligibility, ownership or legal status unresolved |

These rows may overlap. Apply all material constraints. An unbranded object can be sold honestly without a maker if category, safety and contents permit it. Conversely, a confident model identification does not settle authenticity or condition.

## Choose a listing structure

- **Unique item:** exact-unit photos and condition. Stable unit ID follows it through fulfilment.
- **Identical repeated goods:** one purchasable lot specification with separate physical allocations; use a quantity listing only when the format supports it (RICARDO-PLATFORM.md (formats) notes fixed price only, with a price per unit, and a set at one total price is a single lot) and condition and contents fit the representation; verify live. Otherwise list each lot separately, staggering closings to match packing capacity.
- **Variations:** use only supported category attributes and exact stock per option; never disguise unrelated goods as cheap variations.
- **Coherent kit:** include every promised component and accessory; explain buyer use case, dependencies and missing necessities. Do not create value by implying completeness.
- **Mixed lot:** enumerated contents and condition range; clearly disclose whether the buyer receives the exact photographed lot. Avoid "assorted" where the actual allocation is known.

The ledger's `quantity` means purchasable lots, not pieces or photographs.

## Packing is an optimization problem

List product/accessory dimensions, orientation constraints, vulnerable surfaces, cushioning needs and available cartons. Compare plausible arrangements before choosing a carton: side-by-side, stacked with separation, removable supported parts, or multiple parcels when justified. Geometry can dominate size-based postage; never choose a smaller carton by omitting protection.

For each candidate, retain inner fit, required cushioning, outer dimensions, total packed mass, service eligibility against the carrier's current limits, estimated cost range and packing effort. Research specifications and use conservative estimates when authorized. Label measured and estimated values separately. A carton sold with one set of dimensions may describe the inside; postage needs the outside.

Score at least the original carton plan and one practical alternative when postage defeats an otherwise useful sale. Recompute whole-shipment net. Do not raise the buyer's shipping cost arbitrarily to conceal a bad plan, or dissolve a valuable matched set before checking a better arrangement.

## Local pickup (Abholung) is a first-class fulfilment branch

Use pickup when it suits the seller and object (Ricardo itself suggests pickup only for large heavy items), on Ricardo (the listing form offers pickup alongside or instead of shipping) or on Tutti. State the pickup area without publishing a home address; the listing page shows a location (`ricardo inspect` returns city and postcode), so check what a buyer can see before publishing. Keep scheduling and exact location private. Decide the payment method and its evidence before the buyer arrives; cash handled in person is the seller's own receipt to record, and the order then carries `payment_due_at_handover: true` so that the arrangement does not wait for a payment that happens at the handover.

Do not fabricate a carrier event for pickup or treat a message proposing a time as completed handoff. Use the canonical order's `fulfillment_method: "pickup"`, with `paid`, `pickup_arranged` and `collected` states and their evidence. Lots remain `sold` until the seller's handover report and payment evidence change them to `collected`. Returned lots remain quarantined. The workspace schema specifies deadline and financial fields; the validator rejects mixed carrier/pickup evidence. The next-action queue produces collection work instead of postage tasks.

Pickup arrangement requires a real agreed deadline in `pickup_by_at` and an IANA `pickup_by_timezone` (normally `Europe/Zurich`); a paid order awaiting arrangement need not invent one. Keep address and access details out of Git. Store only sanitized confirmation evidence. Pickup authority does not automatically authorize buyer messages: apply existing messaging scope, prepare the scheduling reply, and ask only if that actual send remains unauthorized. A local structural pass cannot prove that the handover occurred. Whether Ricardo or Tutti offers any collection-confirmation workflow, how pickup deadlines work and that cash-on-pickup sales sit outside the platform's seller protection: verify live; see [RICARDO-PLATFORM.md](RICARDO-PLATFORM.md) (shipping and pickup, payments).

## Alternative channels and disposal

When the user's constraints defeat parcel sales, compare pickup, Tutti, a relevant specialist venue, consignment, donation, recycling or holding stock. Include fees, transport, authenticity work, seller labor and uncertainty. Recommending a channel does not authorize opening accounts, donating or discarding anything. One unit is exposed on one channel at a time: simultaneous cross-listing (for example Ricardo and Tutti together) is unsupported, because stale synchronization creates overselling risk and the ledger allows one lot per unit. To move a unit, end the first listing and verify it ended, then reserve the unit under the second ([workspace schema](WORKSPACE-SCHEMA.md#one-channel-at-a-time)). Do not use Tutti's copy or auto-publish to Ricardo (RICARDO-PLATFORM.md (Tutti)) while the first ad is live; it creates exactly that double exposure.
