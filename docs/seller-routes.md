# Observed seller routes — 2026-10-07

Observed in the signed-in Ricardo web interface (Chrome, `de` locale) by
navigating My Ricardo → Mein Verkaufen and the listing form, and recording the
interface's own requests. Nothing was submitted, saved, or changed; the drafts
count was confirmed unchanged afterwards. Only structure is recorded here: no
account values, buyer data, addresses, IBANs, or tracking numbers.

## Mein Verkaufen tabs

| Tab | Page | Data source |
|---|---|---|
| Entwürfe (drafts) | `GET /de/my-ricardo/selling/drafts/` | `__NEXT_DATA__` dehydrated query `["selling-drafts", {includeValidation: true, limit: 100, locale: "de", offset: 0}]` → `{drafts: [], draftsCount}` |
| Geplant (scheduled) | client-side view of selling offers | `plannedOffersCount` in the selling-offers query |
| Am Verkaufen (selling) | `GET /de/my-ricardo/selling/offers/` | dehydrated query `["selling-offers", {limit: 100, locale: "de", offset: 0, sort: "endingSoon"}]` → `{offers, offersCount, openOffersCount, plannedOffersCount, maxAllowedOffers, hasResultsWithNoFilter, facets: {status: {open, closed}, highestBidders}}` |
| Verkauft (sold) | `GET /de/my-ricardo/selling/orders/` (shell only) | client `POST /api/orders-spa/buying-selling/search` |

The account observed had no drafts and no open or closed offers, so draft and
offer **item** shapes were not observed. Both pages also embed an `iban-info`
query (holder name, address, account number); tools must select queries by
exact `queryKey[0]` and never emit other queries.

## Sold orders

`POST /api/orders-spa/buying-selling/search`, `Content-Type: application/json`,
body exactly `{"params":{"isSeller":true,"offset":0,"limit":30}}` (the interface
uses limit 30). A plain same-origin `fetch` with browser credentials returned
HTTP 200; `limit: 1` returned one order with `pageSize: 1` and `totalPages`
adjusted, so pagination is honoured.

Response: `{orders: [...], totalCount, pageSize, totalPages, hasResultsWithNoFilter, facets: {status: {...}, buyers: {...}}}`.

`facets.status` keys: `sellerToSend`, `sellerPaymentToCheck`, `sellerToRate`,
`sellerAllDone`, `sellerPaused`, `sellerCancelationRequested`, `sellerCancelled`
(integer counts). `facets.buyers` is keyed by **buyer nickname** (personal data).

Order fields observed (values below are types or observed enum values):

- Identity: `order_id`, `legacy_order`, `offer_id`, `article_id` (numeric strings), `article_title`, `image_url`, `category_id` (number), `offer_type` (`auction`), `quantity`, `creation_date`, `current_status` (`Open`).
- Money, CHF decimals: `unit_price`, `total_price`, `shipping_cost`.
- Money, cents: `pricing.{unit_price, total_price, shipping_cost?, quantity}`, `pricing.seller_fee.{success_fee, success_fee_rate_pertenthousand}`, `pricing.buyer_fee` (object, empty when observed). Observed seller rates 1000 and 1100 per ten-thousand on different orders, so the rate must be read per order, never assumed.
- Payment: `payment_method` (`cash`, `bank_transfer`), `payment_method_values[]`, `payment_status` (`paid`), `payment_status_updated_at`, `buyer_has_paid` (bool) + `_updated_at`, `transaction_method.{type: "NOT_SECURED", fasttrack_payout: bool}`, `securedTransactionFeeInfo` (null when observed), `sellerEligibleForOfflinePayment`, `hasIban`, `money_unlock_authorised`, `money_unlock_occurred_at`.
- Delivery: `selected_delivery.{label, method_id, cost_in_cents}` (labels observed: `get_by_buyer` for pickup, `parcel_a_2kg`), `shipping_method`, `isFreeShipping`, `shipping_status` (`pending`, `shipment_confirmed`) + `_updated_at`, `delivery_status` (`received`, `delivered`) + `_updated_at`, `delivery_completed_occurred_at`, `fulfillment_starts_at`.
- `logistic.{logisticId, orderId, type, trackingNumber, trackingUrl, shipmentService: {provider: "swisspost"|"none", type: "swisspost-parcel"|"none", price, basePrice, receiverOptions}, pickup}`. `logistic.pickup` is a Swiss Post parcel pickup booking: `{pickup_date, pickup_hours, pickup_place, status, parcel_size, weight, price, is_free_pick_at_home, bring_label_at_home, order_key, pickup_address, pickup_infos, …}`. Tracking URLs observed on `https://www.swisspost.ch/track?…`.
- After-sale: `buyer_rating_status`, `seller_rating_status` (`done`) + `_updated_at`, `ratings.{buyerRatingInfo, sellerRatingInfo}.{rating, score}`, `contact_status`, `dispute_state` (empty string when none), `isOfferClosed`, `isOfferArchived`, `withMatchingOffer`, `withMatchingCategory`, `isConsentedToReuseListingData`.
- **Personal data — must never be emitted or persisted:** `buyer_id`, `seller_id`, `orderCounterpart` (`userId, email, firstName, lastName, nickname, country, phone, address`), `shipping_address` (`firstName, lastName, zipCode, city, streetName, streetNumber, additionalAddress, country`), `invoice_address`, `facets.buyers`, `logistic.pickup.pickup_address`, `logistic.pickup.pickup_infos`, `logistic.pickup.order_key`, `reference_number`.

## Seller account signals (GET, JSON)

Loaded by the interface on every My Ricardo page:

- `/api/orders-spa/selling/selling-limits` → `{current: number, limit: number}`
- `/api/orders-spa/selling/has-orders-pending-actions` → bare boolean
- `/api/frontend/messaging/unreads` → `{unread_count: number, last_updated_at: string}`

Also observed but not adopted: `/api/frontend/users/{userId}/has-payout-problems`,
`/api/frontend/users/{userId}/mfa-enrollment-status` (user ID in path),
`/api/frontend/money-guard/user-status` (non-JSON),
`/api/frontend/user-tasks?taskType[]=ADYEN_BANKSTATEMENT_VERIFICATION&taskType[]=ADYEN_ID_VERIFICATION` (shape not recorded).

## Listing form (`/de/list/`) — structure only

Single page, sections: Fotos (slots: Hauptbild, three "Jede Seite", Mängel,
Details; QR upload from phone), Details (Titel max 60 characters, AI title
helper; Produkttyp chosen after a photo or title), Zustand & Beschreibung
(Zustand: Gebraucht, Neu und originalverpackt, Neu (gemäss Beschreibung),
Defekt; Beschreibung max 5000; optional Referenz-Nr. max 40), Angebot & Preis
(Auktion toggle with Startpreis and Erhöhungsschritt defaulting to CHF 1.00;
Sofort-Kaufpreis; Laufzeit Start/Ende; "Automatische Reaktivierung deines
Angebots bis zu 9 Mal"), Lieferung (Versand and/or Abholung from the account
address; Versandkosten; costs borne by buyer), Zahlung (cash on pickup shown for
pickup). Publishing accepts the AGB and notes that a success commission
(Erfolgsprovision) applies when the item sells. Buttons: ENTWURF SPEICHERN,
VERÖFFENTLICHEN. Loading the form did not create a draft. Draft save and
publish requests were **not** observed; CLI drafting/publishing is unsupported.
