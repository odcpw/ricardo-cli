use serde_json::{Value, json};
use std::io::{self, Write};

pub const SCHEMA_VERSION: &str = "1";

pub fn success(data: Value) -> Value {
    json!({"schema_version": SCHEMA_VERSION, "ok": true, "data": data, "error": null})
}

pub fn failure(code: &str, message: &str, hint: &str, retryable: bool) -> Value {
    json!({"schema_version": SCHEMA_VERSION, "ok": false, "data": null,
        "error": {"code": code, "message": message, "hint": hint, "retryable": retryable}})
}

/// JSON always uses stdout, including errors. Human errors use stderr.
pub fn emit(envelope: &Value, json_mode: bool) -> io::Result<()> {
    if json_mode {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer(&mut stdout, envelope)?;
        writeln!(stdout)
    } else if envelope["ok"] == false {
        let error = &envelope["error"];
        writeln!(
            io::stderr().lock(),
            "{}: {}\nHint: {}",
            error["code"].as_str().unwrap_or("error"),
            error["message"].as_str().unwrap_or("Request failed"),
            error["hint"]
                .as_str()
                .unwrap_or("Run ricardo doctor --json")
        )
    } else {
        let data = &envelope["data"];
        let mut stdout = io::stdout().lock();
        if let Some(text) = data.as_str() {
            writeln!(stdout, "{text}")
        } else if let Some(table) = search_table(data) {
            write!(stdout, "{table}")
        } else {
            serde_json::to_writer_pretty(&mut stdout, data)?;
            writeln!(stdout)
        }
    }
}

fn clean(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("?")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn search_table(data: &Value) -> Option<String> {
    use std::fmt::Write as _;
    let listings = data.get("listings")?.as_array()?;
    let mut text = format!(
        "{} result(s) · {} · checked {}\n",
        listings.len(),
        clean(&data["query"]),
        clean(&data["checked_at"])
    );
    text.push_str("ID              STATUS     NEXT CHF    BUY CHF  TITLE\n");
    let price = |v: &Value| {
        v.as_f64()
            .map(|n| format!("{n:.2}"))
            .unwrap_or_else(|| "—".into())
    };
    for listing in listings {
        let _ = writeln!(
            text,
            "{:<15} {:<9} {:>9} {:>10}  {}",
            clean(&listing["id"]),
            clean(&listing["status"]),
            price(&listing["next_bid"]),
            price(&listing["buy_now"]),
            clean(&listing["title"])
        );
        let _ = writeln!(text, "  {}", clean(&listing["url"]));
        let _ = writeln!(text, "  Ends: {}", clean(&listing["end_at"]));
    }
    if data["partial"] == true {
        text.push_str("Partial results: filters and sorting apply to the requested page only. Use --page to inspect another page.\n");
    }
    Some(text)
}

pub fn capabilities() -> Value {
    json!({
        "name": "ricardo", "version": env!("CARGO_PKG_VERSION"),
        "schema_version": SCHEMA_VERSION,
        "transport": "logged-in browser extension and local native messaging bridge",
        "commands": [
            {"command":"search", "access":"read", "example":"ricardo search Bambu P2S --max-price 350 --json"},
            {"command":"inspect", "access":"read", "example":"ricardo inspect ARTICLE_ID --json"},
            {"command":"account", "access":"read"},
            {"command":"favorites list", "access":"read", "pagination":{"offset_default":0,"limit_default":50,"limit_max":50}},
            {"command":"favorites add", "access":"write", "confirmation":"--yes; otherwise preview"},
            {"command":"favorites remove", "access":"write", "confirmation":"--yes; otherwise preview"},
            {"command":"buying", "access":"read", "scope":"offers, not completed orders", "pagination":{"offset_default":0,"limit_default":30,"limit_max":30}},
            {"command":"selling", "access":"read", "scope":"active offers"},
            {"command":"notifications", "access":"read"},
            {"command":"page", "access":"read"},
            {"command":"history", "access":"local-read"},
            {"command":"doctor", "access":"diagnostic"},
            {"command":"bridge serve", "access":"local-service"},
            {"command":"bridge status", "access":"diagnostic"},
            {"command":"extension install", "access":"local-configuration"},
            {"command":"capabilities", "access":"local-read"},
            {"command":"robot-docs guide", "access":"local-read"}
        ],
        "global_flags": ["--json", "--no-color", "--timeout SECONDS", "--data-dir PATH", "--help", "--version"],
        "exit_codes": {"0":"success", "2":"invalid input", "3":"bridge unavailable or timeout", "4":"authentication required", "5":"blocked or challenge", "6":"upstream response or interface changed", "7":"unsafe or unsupported operation", "8":"local I/O failure"},
        "envelope": {"schema_version":"string", "ok":"boolean", "data":"value or null", "error":"null or {code,message,hint,retryable}"},
        "environment": {"NO_COLOR":"Output has no ANSI styling by default", "XDG_DATA_HOME":"Default local state root when configured"},
        "limitations": [
            "No bids, purchases, seller messages, listing publication, or payment operations are exposed.",
            "Ricardo interface changes can make an operation unsupported; errors do not fall back to stale success.",
            "History is a record of earlier observations, not proof of current availability.",
            "An auction price within budget is not a guaranteed purchase price."
            ,"Search filters and sorting apply within the requested page, not across all results. Returned articles are individually verified."
            ,"Account API commands preserve upstream data; buying returns offers rather than completed orders, and selling returns active offers only."
        ]
    })
}

pub fn agent_guide() -> &'static str {
    include_str!("../docs/agent-contract.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelopes_are_unambiguous_and_versioned() {
        let ok = success(json!([]));
        assert_eq!(ok["schema_version"], "1");
        assert_eq!(ok["ok"], true);
        assert!(ok["error"].is_null());
        let err = failure(
            "bridge_unavailable",
            "No browser connected",
            "Run ricardo doctor --json",
            true,
        );
        assert_eq!(err["ok"], false);
        assert!(err["data"].is_null());
        assert_eq!(err["error"]["retryable"], true);
    }

    #[test]
    fn human_search_distinguishes_prices_and_preserves_plain_url() {
        let data = json!({"query":"P2S", "checked_at":"2026-09-05T12:00:00Z", "partial":true,
            "listings":[{"id":"1234567890", "status":"active", "next_bid":250.5, "buy_now":null,
                "title":"Printer\u{001b}[31m", "url":"https://www.ricardo.ch/de/a/printer-1234567890/", "end_at":"2026-09-06T12:00:00Z"}]});
        let text = search_table(&data).unwrap();
        assert!(text.contains("250.50"));
        assert!(text.contains("NEXT CHF"));
        assert!(text.contains("https://www.ricardo.ch/de/a/printer-1234567890/"));
        assert!(text.contains("Partial results"));
        assert!(!text.contains('\u{001b}'));
    }
}
