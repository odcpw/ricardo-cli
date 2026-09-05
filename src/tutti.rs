//! Observed Tutti Next.js data, scoped to the requested listing/query only.
use crate::{Error, Result, upstream};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use scraper::{Html, Selector};
use serde_json::{Value, json};

pub fn listing_url(input: &str) -> Result<url::Url> {
    let bad = || {
        Error::new(
            2,
            "invalid_tutti_url",
            "Expected a full https://www.tutti.ch/de/vi/.../ID listing URL",
            "Copy the listing URL from Tutti; numeric IDs alone are not supported",
        )
    };
    if input.chars().any(|c| c.is_control() || c == '\\')
        || ["%2e", "%2f", "%5c"]
            .iter()
            .any(|s| input.to_lowercase().contains(s))
    {
        return Err(bad());
    }
    let u = url::Url::parse(input).map_err(|_| bad())?;
    let parts: Vec<_> = u.path().split('/').collect();
    if u.origin().ascii_serialization() != "https://www.tutti.ch"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.path().starts_with("/de/vi/")
        || !(6..=10).contains(&parts.len())
        || parts
            .iter()
            .skip(1)
            .any(|s| s.is_empty() || *s == "." || *s == "..")
        || !parts.last().is_some_and(|s| {
            !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err(bad());
    }
    Ok(u)
}

pub fn search_path(query: &str) -> Result<String> {
    // Observed UI token: A + base64url(MessagePack [query, nil, [nil,nil,nil,nil]]).
    // Only unfiltered, first-page search is exposed; do not invent filter tokens.
    let bytes = query.as_bytes();
    if query.trim().is_empty() || bytes.len() > 200 || query.chars().any(char::is_control) {
        return Err(Error::new(
            2,
            "invalid_query",
            "Tutti query must contain 1–200 UTF-8 bytes without control characters",
            "Use ricardo tutti search Prusa Core --json",
        ));
    }
    let mut token = vec![0x93];
    if bytes.len() < 32 {
        token.push(0xa0 | bytes.len() as u8)
    } else {
        token.extend([0xd9, bytes.len() as u8]);
    }
    token.extend(bytes);
    token.extend([0xc0, 0x94, 0xc0, 0xc0, 0xc0, 0xc0]);
    Ok(format!("/de/q/suche/A{}", URL_SAFE_NO_PAD.encode(token)))
}
fn next(html: &str) -> Result<Value> {
    let doc = Html::parse_document(html);
    let script = doc
        .select(&Selector::parse("script#__NEXT_DATA__").unwrap())
        .next()
        .ok_or_else(|| {
            upstream("Tutti page has no Next.js data; it may be unavailable or challenged".into())
        })?;
    serde_json::from_str(&script.inner_html()).map_err(|e| upstream(e.to_string()))
}
fn queries(d: &Value) -> Result<&Vec<Value>> {
    d.pointer("/props/pageProps/dehydratedState/queries")
        .and_then(Value::as_array)
        .ok_or_else(|| upstream("Tutti query data changed".into()))
}
pub fn parse_detail(html: &str, input: &str, checked: &str) -> Result<Value> {
    let u = listing_url(input)?;
    let id = u.path().rsplit('/').next().unwrap();
    let d = next(html)?;
    let l = queries(&d)?
        .iter()
        .filter(|q| q["queryKey"][0] == "GetListingDetails" && q["queryKey"][1]["id"] == id)
        .filter_map(|q| q.pointer("/state/data/listing"))
        .find(|l| l["listingID"] == id)
        .ok_or_else(|| {
            upstream(
                "Requested Tutti listing missing; related listings are not a substitute".into(),
            )
        })?;
    if l["seoInformation"]["canonical"] != input {
        return Err(upstream(
            "Tutti canonical URL does not match requested listing".into(),
        ));
    }
    let title = l["localization"]["title"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| upstream("Tutti title missing".into()))?;
    let images = l["images"]
        .as_array()
        .ok_or_else(|| upstream("Tutti gallery missing".into()))?
        .iter()
        .map(|v| {
            v.pointer("/rendition/src")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| upstream("Tutti gallery rendition changed".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut unique = std::collections::HashSet::new();
    let images: Vec<_> = images
        .into_iter()
        .filter(|u| unique.insert(u.clone()))
        .collect();
    let doc = Html::parse_document(html);
    let mut availability = None;
    for e in doc.select(&Selector::parse("script[type='application/ld+json']").unwrap()) {
        let Ok(v) = serde_json::from_str::<Value>(&e.inner_html()) else {
            continue;
        };
        if v["@type"] == "Product" && v["url"] == input {
            availability = v
                .pointer("/offers/availability")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
    }
    // InStock is site evidence that the advert is listed, not seller confirmation.
    let status = match availability.as_deref() {
        Some("https://schema.org/InStock") => "listed",
        Some(
            "https://schema.org/SoldOut"
            | "https://schema.org/OutOfStock"
            | "https://schema.org/Discontinued",
        ) => "unavailable",
        _ => "unknown",
    };
    let price = l["seoInformation"]["numericPrice"]
        .as_f64()
        .filter(|p| p.is_finite() && *p >= 0.0);
    Ok(
        json!({"marketplace":"tutti","source":"live_browser","id":id,"url":input,"title":title,"checked_at":checked,"status":status,"status_evidence":{"schema_availability":availability,"seller_confirmed":false},"asking_price":price,"formatted_price":l["formattedPrice"],"currency":"CHF","description":l["localization"]["body"],"posted_at":l["timestamp"],"location":{"city":l["postcodeInformation"]["locationName"],"zip_code":l["postcodeInformation"]["postcode"]},"properties":l["properties"],"images":images}),
    )
}
pub fn search_candidates(html: &str, query: &str) -> Result<(u64, Vec<String>)> {
    let d = next(html)?;
    if d.pointer("/props/pageProps/initialReduxState/searchResultPage/query")
        .and_then(Value::as_str)
        != Some(query)
    {
        return Err(upstream("Tutti returned a different search query".into()));
    }
    let data = queries(&d)?
        .iter()
        .find(|q| q["queryKey"][0] == "SearchListingsByConstraints")
        .and_then(|q| q.pointer("/state/data/listings"))
        .ok_or_else(|| upstream("Tutti search result shape changed".into()))?;
    let total = data["totalCount"]
        .as_u64()
        .ok_or_else(|| upstream("Tutti search total missing".into()))?;
    let edges = data["edges"]
        .as_array()
        .ok_or_else(|| upstream("Tutti search edges missing".into()))?;
    let urls = edges
        .iter()
        .map(|e| {
            let n = &e["node"];
            let slug = n["seoInformation"]["deSlug"]
                .as_str()
                .ok_or_else(|| upstream("Tutti search slug missing".into()))?;
            let id = n["listingID"]
                .as_str()
                .ok_or_else(|| upstream("Tutti search ID missing".into()))?;
            let u = format!("https://www.tutti.ch/de/vi/{slug}/{id}");
            listing_url(&u)?;
            Ok(u)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((total, urls))
}

#[cfg(test)]
mod tests {
    use super::*;
    const URL: &str = "https://www.tutti.ch/de/vi/zuerich/garten-handwerk/werkzeuge-maschinen/bambu-lab-p2s/82644754";
    #[test]
    fn observed_search_token_roundtrip() {
        assert_eq!(
            search_path("Prusa Core").unwrap(),
            "/de/q/suche/Ak6pQcnVzYSBDb3JlwJTAwMDA"
        );
        assert!(search_path("").is_err());
        assert!(search_path(&"x".repeat(201)).is_err());
    }
    #[test]
    fn exact_listing_gallery_only() {
        let v = parse_detail(
            include_str!("../tests/fixtures/tutti-p2s.html"),
            URL,
            "2026-09-05T18:00:00Z",
        )
        .unwrap();
        assert_eq!(v["asking_price"], 420.0);
        assert_eq!(v["status"], "listed");
        assert_eq!(v["images"].as_array().unwrap().len(), 1);
        assert_eq!(v["images"][0], "https://c.tutti.ch/big/5804655919.jpg");
        assert!(v.get("sellerInfo").is_none());
        assert!(
            parse_detail(
                include_str!("../tests/fixtures/tutti-p2s.html"),
                &URL.replace("82644754", "12345"),
                "now"
            )
            .is_err()
        );
    }
    #[test]
    fn unknown_is_not_available() {
        let h = include_str!("../tests/fixtures/tutti-p2s.html")
            .replace("https://schema.org/InStock", "https://schema.org/Unknown");
        assert_eq!(parse_detail(&h, URL, "now").unwrap()["status"], "unknown");
    }
    #[test]
    fn search_is_scoped_to_search_query() {
        let (total, urls) = search_candidates(
            include_str!("../tests/fixtures/tutti-search.html"),
            "Prusa Core",
        )
        .unwrap();
        assert_eq!(total, 5);
        assert_eq!(urls.len(), 5);
        assert!(
            search_candidates(include_str!("../tests/fixtures/tutti-search.html"), "P2S").is_err()
        );
    }
    #[test]
    fn restrict_origins_and_routes() {
        for u in [
            "https://evil.test/de/vi/a/b/c/123",
            "https://www.tutti.ch/de/messages",
            "https://www.tutti.ch/de/vi/a/b/c/123?redirect=evil",
            "https://me@www.tutti.ch/de/vi/a/b/c/123",
            "https://www.tutti.ch/de/vi/a/%2e%2e/c/123",
        ] {
            assert!(listing_url(u).is_err(), "{u}");
        }
    }
}
