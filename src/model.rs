use chrono::{DateTime, Utc};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub id: String,
    pub title: String,
    pub url: String,
    pub status: String,
    pub status_evidence: String,
    pub checked_at: String,
    pub end_at: Option<String>,
    pub currency: String,
    pub current_bid: Option<f64>,
    pub next_bid: Option<f64>,
    pub buy_now: Option<f64>,
    pub sold_price: Option<f64>,
    pub bids_count: Option<u64>,
    pub condition: Option<String>,
    pub category_id: Option<String>,
    pub product_type: Option<String>,
    pub location: Value,
    pub shipping: Value,
    pub seller: Value,
    pub description: Option<String>,
    pub images: Vec<String>,
}
impl Listing {
    pub fn entry_price(&self) -> Option<f64> {
        match (self.next_bid, self.buy_now) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}
fn string(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
}
fn field(v: &Value, k: &str) -> Option<String> {
    string(&v[k])
}
fn cents(v: &Value) -> Option<f64> {
    v.as_f64().map(|n| n / 100.0)
}
pub fn plain(html: &str) -> String {
    Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decode data, never execute scripts. Flight strings may be split between script tags.
pub fn page_data(html: &str) -> Result<Vec<Value>, String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("script").unwrap();
    let mut values = Vec::new();
    let mut flight = String::new();
    for script in doc.select(&sel) {
        let text = script.inner_html();
        if script.value().attr("id") == Some("__NEXT_DATA__") {
            values.push(
                serde_json::from_str(&text).map_err(|_| "Invalid Ricardo __NEXT_DATA__ JSON")?,
            );
        } else if let Some(raw) = text
            .strip_prefix("self.__next_f.push(")
            .and_then(|s| s.strip_suffix(')'))
            && let Ok(v) = serde_json::from_str::<Value>(raw)
            && v[0] == 1
            && let Some(s) = v[1].as_str()
        {
            flight.push_str(s);
        }
    }
    for line in flight.lines() {
        if let Some((_, body)) = line.split_once(':')
            && let Ok(v) = serde_json::from_str(body)
        {
            values.push(v);
        }
    }
    if values.is_empty() {
        return Err("Ricardo returned no recognized structured page data; login, challenge or interface change".into());
    }
    Ok(values)
}
pub fn find<'a>(v: &'a Value, predicate: &impl Fn(&Value) -> bool) -> Option<&'a Value> {
    if predicate(v) {
        return Some(v);
    }
    match v {
        Value::Array(a) => a.iter().find_map(|x| find(x, predicate)),
        Value::Object(o) => o.values().find_map(|x| find(x, predicate)),
        _ => None,
    }
}
pub fn search_data(html: &str) -> Result<Value, String> {
    for v in page_data(html)? {
        if let Some(data) = find(&v, &|x| {
            x.get("articles").is_some_and(Value::is_array) && x.get("totalArticlesCount").is_some()
        }) {
            return Ok(data.clone());
        }
    }
    Err("Search response has no articles/totalArticlesCount contract; refusing to report an empty result".into())
}
pub fn account_data(html: &str) -> Result<Value, String> {
    for v in page_data(html)? {
        let u = v
            .pointer("/props/currentUser")
            .filter(|u| u.is_object() && u.get("userId").is_some_and(|id| !id.is_null()))
            .or_else(|| {
                find(&v, &|x| {
                    x.get("user")
                        .is_some_and(|u| u.is_object() && u.get("accountType").is_some())
                })
                .and_then(|x| x.get("user"))
            });
        if let Some(u) = u {
            return Ok(
                json!({"authenticated":true,"nickname":u.get("nickname"),"id":u.get("userId"),"account_type":u.get("accountType")}),
            );
        }
    }
    Err("Ricardo session is not authenticated; sign in in the connected Chrome profile".into())
}
pub fn canonical_url(html: &str, id: &str) -> String {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("a[href]").unwrap();
    for a in doc.select(&sel) {
        let href = a.value().attr("href").unwrap_or("");
        if href.contains("/de/a/") && href.ends_with(&format!("-{id}/")) {
            return if href.starts_with('/') {
                format!("https://www.ricardo.ch{href}")
            } else {
                href.to_owned()
            };
        }
    }
    format!("https://www.ricardo.ch/de/a/{id}/")
}
pub fn search_listings(html: &str, checked: DateTime<Utc>) -> Result<(Vec<Listing>, u64), String> {
    let data = search_data(html)?;
    let total = data["totalArticlesCount"]
        .as_u64()
        .ok_or("Search result count is invalid")?;
    let mut seen = HashSet::new();
    let mut listings = Vec::new();
    for a in data["articles"]
        .as_array()
        .ok_or("Search articles are invalid")?
    {
        let id = field(a, "id").ok_or("Search article missing ID")?;
        if !seen.insert(id.clone()) {
            continue;
        }
        let end = field(a, "endDate");
        let end_date = end
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok());
        let status = match end_date {
            Some(d) if d <= checked => "ended",
            Some(_) => "active",
            None => "unknown",
        };
        let has_auction = a["hasAuction"].as_bool().unwrap_or(false);
        let bid = a["bidPrice"].as_f64();
        listings.push(Listing {
            id: id.clone(),
            title: field(a, "title").ok_or("Search article missing title")?,
            url: canonical_url(html, &id),
            status: status.into(),
            status_evidence: "live_search_end_date; detail verification required".into(),
            checked_at: checked.to_rfc3339(),
            end_at: end,
            currency: "CHF".into(),
            current_bid: None,
            next_bid: if has_auction { bid } else { None },
            buy_now: if a["hasBuyNow"] == true {
                a["buyNowPrice"].as_f64()
            } else {
                None
            },
            sold_price: None,
            bids_count: a["bidsCount"].as_u64(),
            condition: field(a, "conditionKey"),
            category_id: field(a, "categoryId"),
            product_type: field(a, "productTypeKey"),
            location: a["shipping"]
                .as_array()
                .and_then(|s| s.first())
                .map(|s| json!({"city":s["city"],"zip_code":s["zipCode"]}))
                .unwrap_or(Value::Null),
            shipping: a["shipping"].clone(),
            seller: json!({"id":a["sellerId"]}),
            description: None,
            images: field(a, "image").into_iter().collect(),
        });
    }
    Ok((listings, total))
}
pub fn detail(html: &str, expected: &str, checked: DateTime<Utc>) -> Result<Listing, String> {
    let values = page_data(html)?;
    let a = values
        .iter()
        .find_map(|v| v.pointer("/props/pageProps/article"))
        .ok_or("Article response missing its own article data")?;
    if field(a, "id").as_deref() != Some(expected) {
        return Err("Article ID mismatch; refusing recommendation or related-article data".into());
    }
    let o = &a["offer"];
    let end = field(o, "end_date");
    let ed = end
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok());
    let upstream = a["status"].as_str().unwrap_or("UNKNOWN");
    let sold = o["remaining_quantity"].as_i64() == Some(0);
    let status = if upstream == "CLOSED" || ed.is_some_and(|d| d <= checked) {
        if sold { "sold" } else { "ended" }
    } else if upstream == "PUBLISHED"
        && ed.is_some_and(|d| d > checked)
        && o["remaining_quantity"].as_i64().is_some_and(|q| q > 0)
        && o["availability_key"] == "in_stock"
        && o["isSellerAccountClosed"] != true
        && o["start_date"]
            .as_str()
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
            .is_some_and(|d| d <= checked)
    {
        "active"
    } else {
        "unknown"
    };
    let ty = o["offer_type"].as_str().unwrap_or("");
    let auction = ty.contains("auction");
    let buy = ty.contains("buynow") || ty == "fixed_price";
    let last = cents(&o["last_bid"]);
    let title = field(a, "title").ok_or("Article missing title")?;
    let url = a
        .pointer("/seoMeta/alternateUrls")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|x| x["locale"] == "de"))
        .and_then(|x| field(x, "alternateUrl"))
        .unwrap_or_else(|| canonical_url(html, expected));
    Ok(Listing {
        id: expected.into(),
        title,
        url,
        status: status.into(),
        status_evidence: format!(
            "article.status={upstream}; offer.start_date/end_date; quantity; availability; seller account"
        ),
        checked_at: checked.to_rfc3339(),
        end_at: end,
        currency: "CHF".into(),
        current_bid: if auction && o["bids_count"].as_u64().is_some_and(|n| n > 0) {
            last
        } else {
            None
        },
        next_bid: if auction && status == "active" {
            cents(&o["next_minimum_bid"])
        } else {
            None
        },
        buy_now: if buy && status == "active" {
            o["price"].as_f64()
        } else {
            None
        },
        sold_price: if status == "sold" {
            if auction { last } else { o["price"].as_f64() }
        } else {
            None
        },
        bids_count: o["bids_count"].as_u64(),
        condition: field(a, "conditionKey"),
        category_id: field(a, "categoryId"),
        product_type: a.pointer("/productType/key").and_then(string),
        location: json!({"city":o["city"],"zip_code":o["zip_code"]}),
        shipping: json!(
            a["deliveryOptions"]
                .as_array()
                .map(|items| items
                    .iter()
                    .map(|v| json!({"method":v["id"],"cost":cents(&v["price"]),"currency":"CHF"}))
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        ),
        seller: json!({"nickname":a["seller"]["nickname"],"ratings_count":a["seller"]["ratingsCount"],"score":a["seller"]["score"]}),
        description: a
            .pointer("/description/html")
            .and_then(Value::as_str)
            .map(plain),
        images: a["images"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| field(x, "url")).collect())
            .unwrap_or_default(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn now() -> DateTime<Utc> {
        "2026-09-05T09:00:00Z".parse().unwrap()
    }
    #[test]
    fn closed_auction_with_buy_now_is_not_active() {
        let l = detail(
            include_str!("../tests/fixtures/closed-p2s.html"),
            "1327242570",
            now(),
        )
        .unwrap();
        assert_eq!(l.status, "sold");
        assert_eq!(l.sold_price, Some(385.));
        assert_eq!(l.next_bid, None);
        assert_eq!(l.buy_now, None);
    }
    #[test]
    fn recommendations_cannot_substitute_for_article() {
        assert!(
            detail(
                include_str!("../tests/fixtures/closed-p2s.html"),
                "1328424962",
                now()
            )
            .is_err()
        );
    }
    #[test]
    fn live_search_decodes_flight() {
        let (l, n) =
            search_listings(include_str!("../tests/fixtures/search-p2s.html"), now()).unwrap();
        assert_eq!(n, 9);
        assert_eq!(l.len(), 9);
        assert_eq!(l[0].next_bid, Some(600.));
        assert_eq!(l[0].current_bid, None);
    }
    #[test]
    fn challenge_is_not_empty_search() {
        assert!(search_listings("<h1>Just a moment...</h1>", now()).is_err());
    }
    #[test]
    fn future_end_is_not_enough_for_unknown_detail_status() {
        let h = include_str!("../tests/fixtures/closed-p2s.html")
            .replace("CLOSED", "REMOVED")
            .replace("2026-08-30T16:26:47.9Z", "2027-08-30T16:26:47.9Z");
        assert_eq!(detail(&h, "1327242570", now()).unwrap().status, "unknown");
    }
    #[test]
    fn published_auction_prices_are_normalized() {
        let l = detail(
            include_str!("../tests/fixtures/active-1328424962.html"),
            "1328424962",
            now(),
        )
        .unwrap();
        assert_eq!(l.status, "active");
        assert_eq!(l.current_bid, Some(179.));
        assert_eq!(l.next_bid, Some(184.));
        assert_eq!(l.buy_now, None);
    }
    #[test]
    fn published_fixed_price_is_in_francs() {
        let l = detail(
            include_str!("../tests/fixtures/active-1328009582.html"),
            "1328009582",
            now(),
        )
        .unwrap();
        assert_eq!(l.status, "active");
        assert_eq!(l.buy_now, Some(559.));
        assert_eq!(l.current_bid, None);
        assert_eq!(l.next_bid, None);
    }
    #[test]
    fn empty_current_user_is_not_authenticated() {
        assert!(
            account_data(r#"<script id="__NEXT_DATA__">{"props":{"currentUser":{}}}</script>"#)
                .is_err()
        );
    }
    #[test]
    fn authenticated_user_projects_only_public_account_summary() {
        let h = r#"<script id="__NEXT_DATA__">{"props":{"currentUser":{"userId":123,"nickname":"sample","accountType":"private","sessionId":"secret","email":"private@example.test"}}}</script>"#;
        let a = account_data(h).unwrap();
        assert_eq!(a["id"], 123);
        assert_eq!(a["authenticated"], true);
        assert!(a.get("sessionId").is_none());
        assert!(a.get("email").is_none());
    }
}
