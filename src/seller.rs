//! Read-only seller views (sold orders, drafts, account signals).
//!
//! Orders are projected field by field from the shapes recorded in
//! `docs/seller-routes.md`. The output is built from an allow-list: personal
//! data is never copied and then removed, it is simply never read. Unknown
//! upstream fields are ignored. Missing required fields are interface errors,
//! never zeros.
use crate::model;
use serde_json::{Map, Value, json};

pub const SOLD_SCOPE: &str = "Sold orders for the signed-in seller. Buyer identity, contact details, addresses, payment references and pickup addresses are deliberately omitted; open the order in Ricardo to see them.";
pub const STATUS_SCOPE: &str = "Counts only. Message contents are not read; reply in Ricardo.";
pub const DRAFT_ITEM_SHAPE: &str =
    "unverified: no draft items had been observed when this command was written";
/// Upstream fields that are never read, emitted or stored.
pub const OMITTED_FIELDS: [&str; 12] = [
    "buyer_id",
    "seller_id",
    "orderCounterpart",
    "shipping_address",
    "invoice_address",
    "facets.buyers",
    "reference_number",
    "logistic.pickup.pickup_address",
    "logistic.pickup.pickup_infos",
    "logistic.pickup.order_key",
    "logistic.pickup.additional_one",
    "logistic.pickup.additional_two",
];

fn id(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => n.as_u64().map(|n| n.to_string()),
        _ => None,
    }
}
fn id_or_null(v: &Value) -> Value {
    id(v).map_or(Value::Null, Value::String)
}
fn text(v: &Value) -> Value {
    match v {
        Value::String(s) => json!(s),
        _ => Value::Null,
    }
}
fn flag(v: &Value) -> Value {
    v.as_bool().map_or(Value::Null, Value::Bool)
}
/// Ratings keep numbers and short enum-like codes only; free text (which can
/// carry names or messages) and anything structured are dropped.
fn rating_value(v: &Value) -> Value {
    match v {
        Value::Number(_) => v.clone(),
        Value::String(s)
            if !s.is_empty()
                && s.len() <= 40
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') =>
        {
            v.clone()
        }
        _ => Value::Null,
    }
}
/// Presence of an upstream object whose inner shape was never observed
/// populated; its contents are not read.
fn reported(v: &Value) -> Value {
    match v {
        Value::Object(o) => json!(!o.is_empty()),
        Value::Null => Value::Null,
        _ => json!(true),
    }
}
fn number_or_text_id(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.is_u64() => v.clone(),
        Value::String(s) if !s.is_empty() => v.clone(),
        _ => Value::Null,
    }
}
fn uint(v: &Value, name: &str) -> Result<Option<u64>, String> {
    match v {
        Value::Null => Ok(None),
        _ => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("Sold order field {name} must be a non-negative integer")),
    }
}
/// Optional integer cents; absent or null is `None`, a non-integer is an error.
fn cents(v: &Value, name: &str) -> Result<Option<i64>, String> {
    match v {
        Value::Null => Ok(None),
        _ => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("Sold order field {name} must be integer cents")),
    }
}
fn required_cents(o: &Value, parent: &str, key: &str) -> Result<i64, String> {
    let name = format!("{parent}.{key}");
    cents(&o[parent][key], &name)?
        .ok_or_else(|| format!("Sold order missing required field {name}"))
}
/// Integer cents to CHF through exact integer arithmetic and a two-decimal
/// literal, so no floating-point division or multiplication drift is possible.
pub fn chf(cents: i64) -> f64 {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    // A formatted decimal always parses; NaN would serialize as null, never 0.
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
        .parse()
        .unwrap_or(f64::NAN)
}
fn optional_chf(v: &Value, name: &str) -> Result<Value, String> {
    Ok(cents(v, name)?.map_or(Value::Null, |c| json!(chf(c))))
}
fn image_url(v: &Value) -> Value {
    let Some(s) = v.as_str() else {
        return Value::Null;
    };
    let on_ricardo_cdn =
        url::Url::parse(s).is_ok_and(|u| u.host_str() == Some("img.ricardostatic.ch"));
    if on_ricardo_cdn && crate::media::validate_url(s).is_ok() {
        json!(s)
    } else {
        Value::Null
    }
}
fn tracking_url(v: &Value) -> Value {
    let Some(u) = v.as_str().and_then(|s| url::Url::parse(s).ok()) else {
        return Value::Null;
    };
    if u.scheme() == "https"
        && matches!(u.host_str(), Some("www.swisspost.ch" | "www.post.ch"))
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
    {
        json!(u.as_str())
    } else {
        Value::Null
    }
}
fn parcel_pickup(p: &Value) -> Result<Value, String> {
    if !p.is_object() {
        return Ok(Value::Null);
    }
    Ok(json!({
        "date": text(&p["pickup_date"]),
        "hours": text(&p["pickup_hours"]),
        "place": text(&p["pickup_place"]),
        "status": text(&p["status"]),
        "parcel_size": text(&p["parcel_size"]),
        "weight_grams": uint(&p["weight"], "logistic.pickup.weight")?,
        "price_chf": optional_chf(&p["price"], "logistic.pickup.price")?,
    }))
}
fn carrier(service: &Value) -> Result<Value, String> {
    if !service.is_object() {
        return Ok(Value::Null);
    }
    Ok(json!({
        "provider": text(&service["provider"]),
        "service_type": text(&service["type"]),
        "receiver_options": text(&service["receiverOptions"]),
        "price_chf": optional_chf(&service["price"], "logistic.shipmentService.price")?,
    }))
}
fn ratings(r: &Value) -> Value {
    if !r.is_object() {
        return Value::Null;
    }
    let info = |v: &Value| json!({"rating": rating_value(&v["rating"]), "score": v["score"].as_f64().map_or(Value::Null, |_| v["score"].clone())});
    json!({"buyerRatingInfo": info(&r["buyerRatingInfo"]), "sellerRatingInfo": info(&r["sellerRatingInfo"])})
}

/// Project one upstream sold order onto the documented, allow-listed shape.
pub fn normalize_order(o: &Value) -> Result<Value, String> {
    if !o.is_object() {
        return Err("Sold order is not an object".into());
    }
    let order_id = id(&o["order_id"]).ok_or("Sold order missing required field order_id")?;
    let article_id = id(&o["article_id"]).ok_or("Sold order missing required field article_id")?;
    if article_id.len() > 20 || !article_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Sold order field article_id is not a numeric Ricardo ID".into());
    }
    let unit = required_cents(o, "pricing", "unit_price")?;
    let total = required_cents(o, "pricing", "total_price")?;
    let pricing = &o["pricing"];
    let fee = cents(
        &pricing["seller_fee"]["success_fee"],
        "pricing.seller_fee.success_fee",
    )?;
    let rate = uint(
        &pricing["seller_fee"]["success_fee_rate_pertenthousand"],
        "pricing.seller_fee.success_fee_rate_pertenthousand",
    )?;
    let shipment = &o["logistic"];
    let delivery = &o["selected_delivery"];
    let transaction = &o["transaction_method"];
    let tracking_number = o["logistic"]["trackingNumber"]
        .as_str()
        .filter(|s| !s.is_empty());
    Ok(json!({
        "order_id": order_id,
        "legacy_order_id": id_or_null(&o["legacy_order"]),
        "article_id": article_id,
        "offer_id": id_or_null(&o["offer_id"]),
        "article_title": text(&o["article_title"]),
        "article_url": format!("https://www.ricardo.ch/de/a/{article_id}/"),
        "image_url": image_url(&o["image_url"]),
        "category_id": number_or_text_id(&o["category_id"]),
        "offer_type": text(&o["offer_type"]),
        "quantity": uint(&o["quantity"], "quantity")?,
        "created_at": text(&o["creation_date"]),
        "current_status": text(&o["current_status"]),
        "unit_price_chf": chf(unit),
        "shipping_cost_chf": optional_chf(&pricing["shipping_cost"], "pricing.shipping_cost")?,
        "total_price_chf": chf(total),
        "seller_success_fee_chf": fee.map_or(Value::Null, |c| json!(chf(c))),
        "seller_success_fee_rate": rate.map_or(Value::Null, |r| json!(r as f64 / 10_000.0)),
        "seller_success_fee_rate_pertenthousand": rate,
        "buyer_fee_reported": reported(&pricing["buyer_fee"]),
        "payment": {
            "method": text(&o["payment_method"]),
            "methods": o["payment_method_values"].as_array().map_or(Value::Null, |a| json!(a.iter().filter_map(Value::as_str).collect::<Vec<_>>())),
            "status": text(&o["payment_status"]),
            "status_updated_at": text(&o["payment_status_updated_at"]),
            "buyer_has_paid": flag(&o["buyer_has_paid"]),
            "buyer_has_paid_updated_at": text(&o["buyer_has_paid_updated_at"]),
            "transaction_type": text(&transaction["type"]),
            "fasttrack_payout": flag(&transaction["fasttrack_payout"]),
            "payout_unlock_authorised": flag(&o["money_unlock_authorised"]),
            "payout_unlocked_at": text(&o["money_unlock_occurred_at"]),
            "secured_transaction_fee_reported": reported(&o["securedTransactionFeeInfo"]),
        },
        "delivery": {
            "method_label": text(&delivery["label"]),
            "method_id": id_or_null(&delivery["method_id"]),
            "buyer_paid_shipping_chf": optional_chf(&delivery["cost_in_cents"], "selected_delivery.cost_in_cents")?,
            "free_shipping": flag(&o["isFreeShipping"]),
            "shipping_status": text(&o["shipping_status"]),
            "shipping_status_updated_at": text(&o["shipping_status_updated_at"]),
            "delivery_status": text(&o["delivery_status"]),
            "delivery_status_updated_at": text(&o["delivery_status_updated_at"]),
            "delivery_completed_at": text(&o["delivery_completed_occurred_at"]),
            "fulfillment_starts_at": text(&o["fulfillment_starts_at"]),
            "carrier": carrier(&shipment["shipmentService"])?,
            "tracking_number": tracking_number,
            "tracking_url": tracking_url(&shipment["trackingUrl"]),
            "parcel_pickup": parcel_pickup(&shipment["pickup"])?,
        },
        "buyer_rating_status": text(&o["buyer_rating_status"]),
        "buyer_rating_status_updated_at": text(&o["buyer_rating_status_updated_at"]),
        "seller_rating_status": text(&o["seller_rating_status"]),
        "seller_rating_status_updated_at": text(&o["seller_rating_status_updated_at"]),
        "ratings": ratings(&o["ratings"]),
        "contact_status": text(&o["contact_status"]),
        "contact_status_updated_at": text(&o["contact_status_updated_at"]),
        "dispute_state": o["dispute_state"].as_str().filter(|s| !s.is_empty()),
        "offer_closed": flag(&o["isOfferClosed"]),
        "offer_archived": flag(&o["isOfferArchived"]),
    }))
}

/// Build the `selling sold` data object from the upstream search response.
pub fn sold_data(resp: &Value, offset: u32, limit: u32, checked_at: &str) -> Result<Value, String> {
    let rows = resp["orders"]
        .as_array()
        .ok_or("Sold orders response missing orders array")?;
    let total = resp["totalCount"]
        .as_u64()
        .ok_or("Sold orders response missing numeric totalCount")?;
    let mut orders = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        orders.push(normalize_order(row).map_err(|e| format!("{e} (order index {index})"))?);
    }
    // Only integer bucket counts are copied; facets.buyers is never read.
    let status_counts = match &resp["facets"]["status"] {
        Value::Null => Value::Null,
        Value::Object(buckets) => Value::Object(
            buckets
                .iter()
                .filter(|(_, v)| v.is_u64())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Map<_, _>>(),
        ),
        _ => return Err("Sold orders response facets.status is not an object".into()),
    };
    let count = orders.len() as u64;
    let end = u64::from(offset) + count;
    Ok(json!({
        "source": "live_browser",
        "checked_at": checked_at,
        "role": "seller",
        "orders": orders,
        "status_counts": status_counts,
        "pagination": {
            "offset": offset, "limit": limit, "total": total,
            "partial": offset > 0 || end < total,
            "next_offset": if count > 0 && end < total { Some(end) } else { None },
        },
        "scope": SOLD_SCOPE,
        "privacy": {"omitted_fields": OMITTED_FIELDS},
    }))
}

/// Select only the `selling-drafts` dehydrated query. The same page embeds an
/// `iban-info` query with bank details, which is never read.
pub fn drafts_data(html: &str, url: &str, checked_at: &str) -> Result<Value, String> {
    let values = model::page_data(html)?;
    let query = values
        .iter()
        .find_map(|v| model::find(v, &|x| x["queryKey"][0] == "selling-drafts"))
        .ok_or("Selling drafts query missing; session or interface changed")?;
    let data = &query["state"]["data"];
    let drafts = data["drafts"]
        .as_array()
        .ok_or("Selling drafts query has no drafts array; interface changed")?;
    let count = data["draftsCount"]
        .as_u64()
        .ok_or("Selling drafts query has no numeric draftsCount; interface changed")?;
    Ok(json!({
        "source": "live_browser",
        "checked_at": checked_at,
        "url": url,
        "drafts_count": count,
        "partial": count > drafts.len() as u64,
        "drafts": drafts,
        "item_shape": DRAFT_ITEM_SHAPE,
    }))
}

fn whole(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| {
        v.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0 && f.fract() == 0.0 && *f < 1e15)
            .map(|f| f as u64)
    })
}
/// `{current, limit}` from `/api/orders-spa/selling/selling-limits`.
pub fn selling_limits(v: &Value) -> Result<(u64, u64), String> {
    let field = |k: &str| {
        whole(&v[k])
            .ok_or_else(|| format!("selling-limits field {k} is not a count; interface changed"))
    };
    Ok((field("current")?, field("limit")?))
}
/// Bare boolean from `/api/orders-spa/selling/has-orders-pending-actions`.
pub fn pending_actions(v: &Value) -> Result<bool, String> {
    v.as_bool()
        .ok_or_else(|| "has-orders-pending-actions is not a boolean; interface changed".into())
}
/// `{unread_count, last_updated_at}` from `/api/frontend/messaging/unreads`.
pub fn unreads(v: &Value) -> Result<(u64, String), String> {
    let count = whole(&v["unread_count"])
        .ok_or("unreads field unread_count is not a count; interface changed")?;
    let at = v["last_updated_at"]
        .as_str()
        .ok_or("unreads field last_updated_at is not a string; interface changed")?;
    Ok((count, at.to_owned()))
}
/// Combine the three validated account signals; `observed` holds each response time.
pub fn status_data(
    (current, limit): (u64, u64),
    pending: bool,
    (count, last_updated): (u64, String),
    observed: [&str; 3],
) -> Value {
    let mut next = Vec::new();
    if pending {
        next.push("ricardo selling sold --json");
    }
    next.push("ricardo selling --json");
    next.push("ricardo selling drafts --json");
    json!({
        "source": "live_browser",
        "checked_at": observed[2],
        "request_times": {"selling_limits": observed[0], "orders_pending_action": observed[1], "unread_messages": observed[2]},
        "selling_limits": {"current": current, "limit": limit, "remaining": limit.saturating_sub(current)},
        "orders_pending_action": pending,
        "unread_messages": {"count": count, "last_updated_at": last_updated},
        "next_commands": next,
        "scope": STATUS_SCOPE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/seller-orders.json")).unwrap()
    }
    fn sold() -> Value {
        sold_data(&fixture(), 0, 30, "2026-10-07T12:00:00Z").unwrap()
    }
    fn f(v: &Value) -> f64 {
        v.as_f64().unwrap()
    }

    #[test]
    fn cents_convert_exactly_to_francs() {
        for (cents, francs) in [
            (0, 0.0),
            (1, 0.01),
            (5, 0.05),
            (21050, 210.5),
            (7845, 78.45),
            (21049, 210.49),
            (100_000_001, 1_000_000.01),
            (-1050, -10.5),
        ] {
            assert_eq!(chf(cents), francs, "{cents}");
        }
        assert_eq!(json!(chf(21049)).to_string(), "210.49");
    }

    #[test]
    fn sold_orders_have_correct_money() {
        let data = sold();
        let a = &data["orders"][0];
        assert_eq!(f(&a["unit_price_chf"]), 200.0);
        assert_eq!(f(&a["shipping_cost_chf"]), 10.5);
        assert_eq!(f(&a["total_price_chf"]), 210.5);
        assert_eq!(f(&a["seller_success_fee_chf"]), 20.0);
        assert_eq!(a["seller_success_fee_rate"], json!(0.1));
        assert_eq!(a["seller_success_fee_rate_pertenthousand"], 1000);
        assert_eq!(f(&a["delivery"]["buyer_paid_shipping_chf"]), 10.5);
        assert_eq!(f(&a["delivery"]["carrier"]["price_chf"]), 10.5);
        assert_eq!(a["buyer_fee_reported"], false);
        let b = &data["orders"][1];
        assert_eq!(f(&b["total_price_chf"]), 713.0);
        assert_eq!(f(&b["unit_price_chf"]), 713.0);
        assert_eq!(f(&b["seller_success_fee_chf"]), 78.45);
        assert_eq!(b["seller_success_fee_rate"], json!(0.11));
        assert!(b["shipping_cost_chf"].is_null());
        assert_eq!(f(&b["delivery"]["buyer_paid_shipping_chf"]), 0.0);
        assert_eq!(b["delivery"]["method_label"], "get_by_buyer");
    }

    #[test]
    fn sold_orders_expose_identity_status_and_aftersale_fields() {
        let data = sold();
        let a = &data["orders"][0];
        assert_eq!(a["order_id"], "40000001");
        assert_eq!(a["legacy_order_id"], "1200000001");
        assert_eq!(a["article_url"], "https://www.ricardo.ch/de/a/1300000001/");
        assert_eq!(a["category_id"], 11111);
        assert_eq!(a["quantity"], 1);
        assert_eq!(a["created_at"], "2026-01-10T09:00:00.000Z");
        assert_eq!(a["payment"]["method"], "bank_transfer");
        assert_eq!(a["payment"]["methods"], json!(["bank_transfer"]));
        assert_eq!(a["payment"]["transaction_type"], "NOT_SECURED");
        assert_eq!(a["payment"]["buyer_has_paid"], true);
        assert!(a["payment"]["secured_transaction_fee_reported"].is_null());
        assert_eq!(a["delivery"]["shipping_status"], "pending");
        assert_eq!(a["delivery"]["carrier"]["provider"], "swisspost");
        assert_eq!(a["delivery"]["carrier"]["service_type"], "swisspost-parcel");
        assert_eq!(a["delivery"]["carrier"]["receiver_options"], "PRI");
        assert_eq!(a["delivery"]["tracking_number"], "990000000000000001");
        assert_eq!(a["buyer_rating_status"], "pending");
        assert_eq!(
            a["ratings"],
            json!({"buyerRatingInfo":{"rating":null,"score":null},"sellerRatingInfo":{"rating":null,"score":null}})
        );
        assert!(a["dispute_state"].is_null());
        assert_eq!(a["offer_closed"], true);
        assert_eq!(a["offer_archived"], false);
        assert_eq!(
            a["image_url"],
            "https://img.ricardostatic.ch/images/00000000-0000-0000-0000-000000000001/t_100x100/synthetic"
        );
        let b = &data["orders"][1];
        assert!(b["delivery"]["tracking_number"].is_null());
        assert!(b["delivery"]["tracking_url"].is_null());
        assert!(b["delivery"]["parcel_pickup"].is_null());
        assert_eq!(b["delivery"]["delivery_status"], "received");
        assert_eq!(b["seller_rating_status"], "done");
    }

    #[test]
    fn page_metadata_counts_and_scope_are_disclosed() {
        let data = sold();
        assert_eq!(data["role"], "seller");
        assert_eq!(data["source"], "live_browser");
        assert_eq!(data["checked_at"], "2026-10-07T12:00:00Z");
        assert_eq!(data["status_counts"]["sellerToSend"], 1);
        assert_eq!(data["status_counts"]["sellerAllDone"], 1);
        assert_eq!(data["status_counts"].as_object().unwrap().len(), 7);
        assert_eq!(
            data["pagination"],
            json!({"offset":0,"limit":30,"total":2,"partial":false,"next_offset":null})
        );
        assert_eq!(data["scope"], SOLD_SCOPE);
        assert!(
            data["privacy"]["omitted_fields"]
                .as_array()
                .unwrap()
                .contains(&json!("orderCounterpart"))
        );
        let page = sold_data(&fixture(), 0, 1, "t").unwrap();
        assert_eq!(page["pagination"]["partial"], false);
        let mut partial = fixture();
        partial["totalCount"] = json!(5);
        let page = sold_data(&partial, 2, 2, "t").unwrap();
        assert_eq!(
            page["pagination"],
            json!({"offset":2,"limit":2,"total":5,"partial":true,"next_offset":4})
        );
        let last = sold_data(&partial, 3, 2, "t").unwrap();
        assert!(last["pagination"]["next_offset"].is_null());
        assert_eq!(last["pagination"]["partial"], true);
    }

    #[test]
    fn personal_data_never_reaches_the_output() {
        // The omitted_fields list names fields without values; exclude its own entries.
        let text = serde_json::to_string(&sold())
            .unwrap()
            .replace(&serde_json::to_string(&OMITTED_FIELDS).unwrap(), "[]");
        assert!(!text.contains("omitted_fields\":[\"")); // the list was found and removed
        for forbidden in [
            "fake.buyer@example.invalid",
            "Musterstrasse",
            "synthetic_buyer_nick",
            "another_fake_nick",
            "+41000000000",
            "Beispielweg",
            "fake.seller@example.invalid",
            "Synthetic private note",
            "000000000000000000000000001",
            "900000001",
            "another.fake@example.invalid",
            "800000001",
            "00000000-0000-0000-0000-00000000abcd",
            "pickup_address",
            "pickup_infos",
            "order_key",
            "orderCounterpart",
        ] {
            assert!(!text.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn pickup_booking_keeps_only_logistics_fields() {
        let data = sold();
        let pickup = data["orders"][0]["delivery"]["parcel_pickup"]
            .as_object()
            .unwrap();
        let mut keys: Vec<_> = pickup.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "date",
                "hours",
                "parcel_size",
                "place",
                "price_chf",
                "status",
                "weight_grams"
            ]
        );
        assert_eq!(pickup["date"], "2026-01-12");
        assert_eq!(pickup["hours"], "FROM_7_30_AM");
        assert_eq!(pickup["place"], "STORAGE_BOX_DEPOSITED");
        assert_eq!(pickup["status"], "APPROVED");
        assert_eq!(pickup["parcel_size"], "SMALL");
        assert_eq!(pickup["weight_grams"], 2000);
        assert_eq!(f(&pickup["price_chf"]), 0.0);
    }

    #[test]
    fn tracking_links_are_limited_to_swiss_post() {
        let valid = [
            "https://www.swisspost.ch/track?formattedParcelCodes=1",
            "https://www.post.ch/track",
        ];
        for url in valid {
            assert!(tracking_url(&json!(url)).is_string(), "{url}");
        }
        for url in [
            "http://www.swisspost.ch/track",
            "https://swisspost.ch/track",
            "https://www.swisspost.ch.evil.test/track",
            "https://evil.test/www.swisspost.ch",
            "https://user@www.swisspost.ch/track",
            "https://www.swisspost.ch:8443/track",
            "javascript:alert(1)",
            "not a url",
            "",
        ] {
            assert!(tracking_url(&json!(url)).is_null(), "{url}");
        }
        assert!(tracking_url(&json!(7)).is_null());
        let data = sold();
        assert_eq!(
            data["orders"][0]["delivery"]["tracking_url"],
            "https://www.swisspost.ch/track?formattedParcelCodes=990000000000000001"
        );
        let mut hostile = fixture();
        hostile["orders"][0]["logistic"]["trackingUrl"] = json!("https://evil.test/track");
        let page = sold_data(&hostile, 0, 30, "t").unwrap();
        assert!(page["orders"][0]["delivery"]["tracking_url"].is_null());
    }

    #[test]
    fn images_must_pass_the_ricardo_cdn_validation() {
        for url in [
            "https://evil.test/images/00000000-0000-0000-0000-000000000001/t_100x100/x",
            "http://img.ricardostatic.ch/images/00000000-0000-0000-0000-000000000001/t_100x100/x",
            "https://img.ricardostatic.ch/other/x",
            "https://img.ricardostatic.ch/images/00000000-0000-0000-0000-000000000001/t_100x100/x?token=1",
            "https://c.tutti.ch/big/123.jpg",
        ] {
            assert!(image_url(&json!(url)).is_null(), "{url}");
        }
        assert!(image_url(&Value::Null).is_null());
    }

    #[test]
    fn required_fields_are_errors_not_zeros() {
        for (path, name) in [
            ("order_id", "order_id"),
            ("article_id", "article_id"),
            ("pricing/total_price", "pricing.total_price"),
            ("pricing/unit_price", "pricing.unit_price"),
        ] {
            let mut input = fixture();
            match path.split_once('/') {
                Some((parent, key)) => {
                    input["orders"][0][parent]
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                }
                None => {
                    input["orders"][0].as_object_mut().unwrap().remove(path);
                }
            }
            let err = sold_data(&input, 0, 30, "t").unwrap_err();
            assert!(err.contains(name), "{err}");
        }
        let mut input = fixture();
        input["orders"][0]
            .as_object_mut()
            .unwrap()
            .remove("pricing");
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("pricing.unit_price")
        );
    }

    #[test]
    fn malformed_values_are_interface_errors() {
        let mut input = fixture();
        input["orders"][0]["pricing"]["total_price"] = json!(210.5);
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("integer cents")
        );
        let mut input = fixture();
        input["orders"][0]["article_id"] = json!("12/../../x");
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("article_id")
        );
        let mut input = fixture();
        input["orders"] = json!({"not": "an array"});
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("orders")
        );
        let mut input = fixture();
        input.as_object_mut().unwrap().remove("totalCount");
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("totalCount")
        );
        let mut input = fixture();
        input["facets"]["status"] = json!("unexpected");
        assert!(
            sold_data(&input, 0, 30, "t")
                .unwrap_err()
                .contains("facets.status")
        );
    }

    #[test]
    fn unknown_upstream_fields_are_ignored() {
        let mut input = fixture();
        input["orders"][0]["brand_new_field"] = json!({"secret": "novel-private-value"});
        input["orders"][0]["pricing"]["new_pricing_field"] = json!(5);
        let text = serde_json::to_string(&sold_data(&input, 0, 30, "t").unwrap()).unwrap();
        assert!(!text.contains("novel-private-value"));
        assert!(!text.contains("brand_new_field"));
    }

    #[test]
    fn empty_result_is_a_valid_empty_page() {
        let input = json!({"orders":[],"totalCount":0,"pageSize":30,"totalPages":0});
        let data = sold_data(&input, 0, 30, "t").unwrap();
        assert_eq!(data["orders"], json!([]));
        assert!(data["status_counts"].is_null());
        assert_eq!(data["pagination"]["partial"], false);
    }

    fn drafts_page(drafts: Value, count: u64) -> String {
        let data = json!({"props":{"pageProps":{"dehydratedState":{"queries":[
            {"queryKey":["iban-info",{"locale":"de"}],"state":{"status":"success","data":{
                "holder":"Fake Holder","iban":"CH00 0000 0000 0000 0000 0","address":"Beispielweg 9"}}},
            {"queryKey":["selling-drafts",{"includeValidation":true,"limit":100,"locale":"de","offset":0}],
                "state":{"status":"success","data":{"drafts":drafts,"draftsCount":count}}}
        ]}}}});
        format!(
            r#"<html><body><script id="__NEXT_DATA__" type="application/json">{data}</script></body></html>"#
        )
    }

    #[test]
    fn drafts_select_only_their_query_and_never_emit_bank_details() {
        let html = drafts_page(json!([]), 0);
        assert!(html.contains("CH00 0000 0000 0000 0000 0"));
        let data = drafts_data(
            &html,
            "https://www.ricardo.ch/de/my-ricardo/selling/drafts/",
            "t",
        )
        .unwrap();
        assert_eq!(data["drafts_count"], 0);
        assert_eq!(data["drafts"], json!([]));
        assert_eq!(data["partial"], false);
        assert_eq!(data["source"], "live_browser");
        assert!(
            data["item_shape"]
                .as_str()
                .unwrap()
                .starts_with("unverified")
        );
        let text = serde_json::to_string(&data).unwrap();
        for secret in ["CH00", "Fake Holder", "iban", "Beispielweg"] {
            assert!(!text.contains(secret), "leaked {secret}");
        }
        let html = drafts_page(json!([{"id":"d1","title":"Synthetic draft"}]), 3);
        let data = drafts_data(&html, "u", "t").unwrap();
        assert_eq!(data["drafts"][0]["title"], "Synthetic draft");
        assert_eq!(data["partial"], true);
        assert!(!serde_json::to_string(&data).unwrap().contains("CH00"));
    }

    #[test]
    fn drafts_page_without_its_query_is_an_interface_error() {
        let iban_only = r#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["iban-info"],"state":{"data":{"iban":"CH00 0000 0000 0000 0000 0"}}}]}}}}</script>"#;
        let err = drafts_data(iban_only, "u", "t").unwrap_err();
        assert!(err.contains("Selling drafts query missing"));
        assert!(!err.contains("CH00"));
        assert!(drafts_data("<h1>Just a moment...</h1>", "u", "t").is_err());
        let bad = drafts_page(json!({"not":"array"}), 0);
        assert!(
            drafts_data(&bad, "u", "t")
                .unwrap_err()
                .contains("drafts array")
        );
        let html = r#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["selling-drafts"],"state":{"data":{"drafts":[]}}}]}}}}</script>"#;
        assert!(
            drafts_data(html, "u", "t")
                .unwrap_err()
                .contains("draftsCount")
        );
    }

    #[test]
    fn status_signals_validate_types_strictly() {
        assert_eq!(
            selling_limits(&json!({"current":3,"limit":10})).unwrap(),
            (3, 10)
        );
        assert_eq!(
            selling_limits(&json!({"current":3.0,"limit":10.0})).unwrap(),
            (3, 10)
        );
        for bad in [
            json!({"current":"3","limit":10}),
            json!({"current":3}),
            json!({"current":-1,"limit":10}),
            json!({"current":1.5,"limit":10}),
            json!({"current":null,"limit":10}),
            json!([3, 10]),
            json!(null),
        ] {
            assert!(selling_limits(&bad).is_err(), "{bad}");
        }
        assert!(pending_actions(&json!(true)).unwrap());
        assert!(!pending_actions(&json!(false)).unwrap());
        for bad in [json!("true"), json!(1), json!(null), json!({"value":true})] {
            assert!(pending_actions(&bad).is_err(), "{bad}");
        }
        assert_eq!(
            unreads(&json!({"unread_count":2,"last_updated_at":"2026-10-07T10:00:00Z"})).unwrap(),
            (2, "2026-10-07T10:00:00Z".to_owned())
        );
        for bad in [
            json!({"unread_count":"2","last_updated_at":"x"}),
            json!({"unread_count":2,"last_updated_at":5}),
            json!({"unread_count":2}),
            json!({"last_updated_at":"x"}),
            json!(false),
        ] {
            assert!(unreads(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn status_combines_counts_without_message_contents() {
        let limits = selling_limits(&json!({"current":4,"limit":10,"ignored":"x"})).unwrap();
        let unread = unreads(&json!({"unread_count":2,"last_updated_at":"2026-10-07T10:00:00Z","messages":["private text"]})).unwrap();
        let data = status_data(limits, true, unread.clone(), ["a", "b", "c"]);
        assert_eq!(
            data["selling_limits"],
            json!({"current":4,"limit":10,"remaining":6})
        );
        assert_eq!(data["orders_pending_action"], true);
        assert_eq!(
            data["unread_messages"],
            json!({"count":2,"last_updated_at":"2026-10-07T10:00:00Z"})
        );
        assert_eq!(data["next_commands"][0], "ricardo selling sold --json");
        assert_eq!(data["checked_at"], "c");
        assert_eq!(data["request_times"]["selling_limits"], "a");
        assert_eq!(data["scope"], STATUS_SCOPE);
        assert!(
            !serde_json::to_string(&data)
                .unwrap()
                .contains("private text")
        );
        let quiet = status_data(limits, false, unread.clone(), ["a", "b", "c"]);
        assert!(
            !quiet["next_commands"]
                .as_array()
                .unwrap()
                .contains(&json!("ricardo selling sold --json"))
        );
        let over = status_data((12, 10), false, unread, ["a", "b", "c"]);
        assert_eq!(over["selling_limits"]["remaining"], 0);
    }
    #[test]
    fn nested_fee_and_rating_content_never_leaks() {
        let mut resp = fixture();
        let o = &mut resp["orders"][0];
        o["pricing"]["buyer_fee"] =
            json!({"fee": 100, "buyer_id": "900000001", "email": "leak@example.invalid"});
        o["securedTransactionFeeInfo"] =
            json!({"payer": {"firstName": "Hans", "address": {"streetName": "Leakstrasse"}}});
        o["ratings"]["buyerRatingInfo"]["rating"] =
            json!("Super, danke Hans Muster aus Leakstrasse 5");
        o["ratings"]["sellerRatingInfo"] =
            json!({"rating": "POSITIVE", "score": 5, "comment": "Hans"});
        let out = sold_data(&resp, 0, 30, "2026-10-07T00:00:00Z").unwrap();
        let text = out.to_string();
        for leak in ["leak@example.invalid", "Hans", "Leakstrasse", "900000001"] {
            assert!(!text.contains(leak), "{leak} leaked");
        }
        let a = &out["orders"][0];
        assert_eq!(a["buyer_fee_reported"], true);
        assert_eq!(a["payment"]["secured_transaction_fee_reported"], true);
        assert!(a["ratings"]["buyerRatingInfo"]["rating"].is_null());
        assert_eq!(
            a["ratings"]["sellerRatingInfo"],
            json!({"rating": "POSITIVE", "score": 5})
        );
    }
}
