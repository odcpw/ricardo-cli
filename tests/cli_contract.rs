use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::{Command, Output},
    thread,
};

fn run(args: &[&str], dir: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ricardo"))
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn discovery_and_input_errors_are_machine_readable_without_browser() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["capabilities", "--json"],
        vec!["robot-docs", "guide", "--json"],
    ] {
        let output = run(&args, dir.path());
        assert!(output.status.success());
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["schema_version"], "1");
        assert!(output.stderr.is_empty());
    }
    let output = run(
        &["search", "P2S", "--max-price", "NaN", "--json"],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(2));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["error"]["code"], "invalid_input");
    assert!(envelope["data"].is_null());
}

#[test]
fn disconnected_browser_is_an_error_not_an_empty_search() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(&["search", "P2S", "--json", "--timeout", "1"], dir.path());
    assert_eq!(output.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], false);
    assert!(envelope["data"].is_null());
}

#[test]
fn favorite_preview_does_not_require_browser_or_execute_write() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(&["favorites", "add", "1327242570", "--json"], dir.path());
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["dry_run"], true);
    assert!(!dir.path().join("bridge.sock").exists());
}

#[test]
fn closed_article_fixture_cannot_be_reported_as_available_by_process() {
    let dir = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(dir.path().join("bridge.sock")).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["op"], "fetch");
        let response = json!({"result": {"status":200,
            "url":"https://www.ricardo.ch/de/a/printer-1327242570/", "content_type":"text/html",
            "body":include_str!("fixtures/closed-p2s.html")}});
        writeln!(stream, "{response}").unwrap();
    });
    let output = run(
        &[
            "inspect",
            "https://www.ricardo.ch/de/a/printer-1327242570/",
            "--json",
        ],
        dir.path(),
    );
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["status"], "sold");
    assert!(envelope["data"]["next_bid"].is_null());
    assert!(envelope["data"]["buy_now"].is_null());
}

#[test]
fn shared_broker_refuses_parallel_requests_and_stops_after_rate_limit() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let mut broker = Command::new(env!("CARGO_BIN_EXE_ricardo"))
        .args([
            "--data-dir",
            dir.path().to_str().unwrap(),
            "bridge",
            "serve",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if dir.path().join("bridge.sock").exists() {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(10));
    }
    let first = Command::new(env!("CARGO_BIN_EXE_ricardo"))
        .args([
            "--data-dir",
            dir.path().to_str().unwrap(),
            "search",
            "P2S",
            "--json",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(broker.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let frame: Value = serde_json::from_str(&line).unwrap();
    let second = run(&["search", "P1S", "--json"], dir.path());
    let second: Value = serde_json::from_slice(&second.stdout).unwrap();
    let response = json!({"id":frame["id"],"result":{"status":429,"url":"https://www.ricardo.ch/de/s/P2S/","content_type":"text/html","body":"rate limited"}});
    writeln!(broker.stdin.as_mut().unwrap(), "{response}").unwrap();
    let first = first.wait_with_output().unwrap();
    let third = run(&["search", "P2S", "--json"], dir.path());
    let third: Value = serde_json::from_slice(&third.stdout).unwrap();
    drop(broker.stdin.take());
    let _ = broker.wait();
    assert_eq!(second["error"]["code"], "request_busy");
    assert_eq!(first.status.code(), Some(5));
    assert_eq!(third["error"]["code"], "rate_limit_cooldown");
    assert_eq!(third["error"]["retryable"], false);
}

#[test]
fn tutti_images_produce_local_bytes_and_manifest_without_private_data() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(dir.path().join("bridge.sock")).unwrap();
    let url = "https://www.tutti.ch/de/vi/zuerich/garten-handwerk/werkzeuge-maschinen/bambu-lab-p2s/82644754";
    let worker = thread::spawn(move || {
        for expected in ["fetch_tutti", "image"] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["op"], expected);
            let result = if expected == "fetch_tutti" {
                json!({"status":200,"url":url,"content_type":"text/html","body":include_str!("fixtures/tutti-p2s.html"),"checked_at":"2026-09-05T18:00:00Z"})
            } else {
                assert_eq!(request["url"], "https://c.tutti.ch/big/5804655919.jpg");
                json!({"status":200,"url":request["url"],"content_type":"image/jpeg","base64":"/9j/","checked_at":"2026-09-05T18:00:10Z"})
            };
            writeln!(stream, "{}", json!({"result":result})).unwrap();
        }
    });
    let output = run(
        &["tutti", "images", url, "--index", "1", "--json"],
        dir.path(),
    );
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let data = &value["data"];
    let path = std::path::Path::new(data["images"][0]["path"].as_str().unwrap());
    assert!(path.is_absolute());
    assert_eq!(std::fs::read(path).unwrap(), vec![255, 216, 255]);
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(path.parent().unwrap().join("manifest.json").is_file());
    assert_eq!(data["partial"], false);
    assert!(output.stderr.is_empty());
}

#[test]
fn older_extension_produces_reload_instruction() {
    let dir = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(dir.path().join("bridge.sock")).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        writeln!(stream,"{}",json!({"error":{"code":"browser_error","message":"Unsupported browser operation; only fetch, resolve and favorite are enabled"}})).unwrap();
    });
    let output = run(
        &[
            "tutti",
            "inspect",
            "https://www.tutti.ch/de/vi/a/b/printer/123",
            "--json",
        ],
        dir.path(),
    );
    worker.join().unwrap();
    assert_eq!(output.status.code(), Some(7));
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["error"]["code"], "extension_outdated");
    assert!(v["error"]["hint"].as_str().unwrap().contains("Reload"));
    assert_eq!(v["error"]["retryable"], false);
}

/// Fake browser worker on the bridge socket: serves one reply per CLI request,
/// records each request, and notes any request beyond the scripted ones.
fn fake_bridge(dir: &std::path::Path, replies: Vec<Value>) -> thread::JoinHandle<Vec<Value>> {
    let listener = UnixListener::bind(dir.join("bridge.sock")).unwrap();
    thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            requests.push(serde_json::from_str(&line).unwrap());
            writeln!(stream, "{reply}").unwrap();
        }
        listener.set_nonblocking(true).unwrap();
        thread::sleep(std::time::Duration::from_millis(300));
        if listener.accept().is_ok() {
            requests.push(json!({"unexpected_extra_request": true}));
        }
        requests
    })
}
fn ok_reply(url: &str, content_type: &str, body: &str) -> Value {
    json!({"result":{"status":200,"url":url,"content_type":content_type,"body":body,"checked_at":"2026-10-07T12:00:00Z"}})
}
fn status_reply(status: u64) -> Value {
    json!({"result":{"status":status,"url":"https://www.ricardo.ch/x","content_type":"text/plain","body":"","checked_at":"2026-10-07T12:00:00Z"}})
}
fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
const SEARCH: &str = "https://www.ricardo.ch/api/orders-spa/buying-selling/search";
const LIMITS: &str = "https://www.ricardo.ch/api/orders-spa/selling/selling-limits";
const PENDING: &str = "https://www.ricardo.ch/api/orders-spa/selling/has-orders-pending-actions";
const UNREADS: &str = "https://www.ricardo.ch/api/frontend/messaging/unreads";

#[test]
fn invalid_seller_paging_exits_2_with_a_json_correction() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        ["selling", "sold", "--limit", "31", "--json"],
        ["selling", "sold", "--limit", "0", "--json"],
        ["selling", "sold", "--offset", "10001", "--json"],
        ["selling", "sold", "--offset", "-1", "--json"],
    ] {
        let output = run(&args, dir.path());
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let v = envelope(&output);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "invalid_input");
        assert!(v["data"].is_null());
        let hint = v["error"]["hint"].as_str().unwrap();
        assert!(
            hint.contains("--limit 1-30") && hint.contains("selling sold"),
            "{hint}"
        );
    }
    assert!(!dir.path().join("bridge.sock").exists());
}

#[test]
fn likely_wrong_seller_commands_are_corrected_without_contacting_ricardo() {
    let dir = tempfile::tempdir().unwrap();
    for (args, expected) in [
        (vec!["sales", "--json"], "ricardo selling sold"),
        (vec!["sold", "--json"], "ricardo selling sold"),
        (vec!["orders", "--json"], "ricardo selling sold"),
        (vec!["drafts", "--json"], "ricardo selling drafts"),
        (vec!["selling", "orders", "--json"], "ricardo selling sold"),
        (vec!["selling", "draft", "--json"], "ricardo selling drafts"),
    ] {
        let output = run(&args, dir.path());
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let v = envelope(&output);
        assert_eq!(v["error"]["code"], "invalid_input");
        assert!(
            v["error"]["hint"].as_str().unwrap().contains(expected),
            "{args:?}"
        );
        assert_eq!(v["error"]["retryable"], false);
        assert!(
            !v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("similar subcommand"),
            "{args:?}: clap guess contradicts the correction"
        );
        assert!(output.stderr.is_empty());
    }
    let output = run(&["selling", "unsold", "--json"], dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        envelope(&output)["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("No separate unsold view")
    );
    let output = run(&["sales"], dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ricardo selling sold"));
    assert!(!dir.path().join("bridge.sock").exists());
}

#[test]
fn capabilities_list_seller_views_and_help_matches() {
    let dir = tempfile::tempdir().unwrap();
    let caps = envelope(&run(&["capabilities", "--json"], dir.path()));
    let commands = caps["data"]["commands"].as_array().unwrap();
    for name in [
        "selling",
        "selling sold",
        "selling drafts",
        "selling status",
    ] {
        let c = commands.iter().find(|c| c["command"] == name).unwrap();
        assert_eq!(c["access"], "read", "{name}");
    }
    let sold = commands
        .iter()
        .find(|c| c["command"] == "selling sold")
        .unwrap();
    assert_eq!(sold["requires_extension"], "0.3.0");
    assert_eq!(sold["pagination"]["limit_max"], 30);
    let help = run(&["selling", "--help"], dir.path());
    let help = String::from_utf8(help.stdout).unwrap();
    for word in [
        "sold",
        "drafts",
        "status",
        "ricardo selling sold --limit 30 --json",
    ] {
        assert!(help.contains(word), "{word}");
    }
    let status =
        String::from_utf8(run(&["selling", "status", "--help"], dir.path()).stdout).unwrap();
    assert!(status.contains("three paced requests"));
    let guide = envelope(&run(&["robot-docs", "guide", "--json"], dir.path()));
    let guide = guide["data"].as_str().unwrap();
    assert!(guide.contains("## Seller views") && guide.contains("ricardo selling sold"));
}

#[test]
fn bare_selling_still_reads_active_offers_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let html = r#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["iban-info",{}],"state":{"data":{"iban":"CH00 0000 0000 0000 0000 0"}}},{"queryKey":["selling-offers",{"limit":100}],"state":{"data":{"offers":[],"offersCount":0,"maxAllowedOffers":10}}}]}}}}</script>"#;
    let worker = fake_bridge(
        dir.path(),
        vec![ok_reply(
            "https://www.ricardo.ch/de/my-ricardo/selling/offers/",
            "text/html",
            html,
        )],
    );
    let output = run(&["selling", "--json"], dir.path());
    assert_eq!(
        worker.join().unwrap(),
        vec![json!({"op":"fetch","path":"/de/my-ricardo/selling/offers/"})]
    );
    assert!(output.status.success());
    let v = envelope(&output);
    let mut keys: Vec<_> = v["data"].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["checked_at", "source", "upstream", "url"]);
    assert_eq!(
        v["data"]["upstream"],
        json!({"offers":[],"offersCount":0,"maxAllowedOffers":10})
    );
    assert_eq!(v["data"]["source"], "live_browser");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("CH00"));
}

#[test]
fn sold_orders_use_one_fixed_search_and_emit_no_personal_data() {
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![ok_reply(
            SEARCH,
            "application/json",
            include_str!("fixtures/seller-orders.json"),
        )],
    );
    let output = run(&["selling", "sold", "--json"], dir.path());
    assert_eq!(
        worker.join().unwrap(),
        vec![json!({"op":"orders","role":"seller","offset":0,"limit":30})]
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
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
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
    let v = envelope(&output);
    let data = &v["data"];
    assert_eq!(data["role"], "seller");
    assert_eq!(data["checked_at"], "2026-10-07T12:00:00Z");
    assert_eq!(data["orders"].as_array().unwrap().len(), 2);
    assert_eq!(data["orders"][0]["total_price_chf"], 210.5);
    assert_eq!(data["orders"][1]["seller_success_fee_chf"], 78.45);
    assert_eq!(data["status_counts"]["sellerToSend"], 1);
    assert_eq!(data["pagination"]["total"], 2);
    assert_eq!(data["pagination"]["partial"], false);
    assert!(data["pagination"]["next_offset"].is_null());
    assert!(
        data["scope"]
            .as_str()
            .unwrap()
            .starts_with("Sold orders for the signed-in seller.")
    );
    // Account data is never written to local history.
    assert!(!dir.path().join("observations.sqlite3").exists());
    let paged = {
        let dir = tempfile::tempdir().unwrap();
        let worker = fake_bridge(
            dir.path(),
            vec![ok_reply(
                SEARCH,
                "application/json",
                include_str!("fixtures/seller-orders.json"),
            )],
        );
        let output = run(
            &[
                "selling", "sold", "--offset", "10", "--limit", "2", "--json",
            ],
            dir.path(),
        );
        assert_eq!(
            worker.join().unwrap(),
            vec![json!({"op":"orders","role":"seller","offset":10,"limit":2})]
        );
        envelope(&output)
    };
    assert_eq!(paged["data"]["pagination"]["offset"], 10);
    assert_eq!(paged["data"]["pagination"]["partial"], true);
}

#[test]
fn sold_orders_failures_map_to_the_documented_exit_codes() {
    let broken = {
        let mut v: Value =
            serde_json::from_str(include_str!("fixtures/seller-orders.json")).unwrap();
        v["orders"][0]["pricing"]
            .as_object_mut()
            .unwrap()
            .remove("total_price");
        v.to_string()
    };
    for (reply, exit, code) in [
        (status_reply(401), 4, "authentication_required"),
        (status_reply(429), 5, "upstream_blocked"),
        (status_reply(403), 5, "upstream_blocked"),
        (status_reply(404), 6, "not_found"),
        (status_reply(500), 6, "interface_changed"),
        (
            ok_reply(SEARCH, "text/html", "<h1>Sign in</h1>"),
            4,
            "authentication_required",
        ),
        (
            ok_reply(SEARCH, "application/json", "not json"),
            6,
            "interface_changed",
        ),
        (
            ok_reply(SEARCH, "application/json", "{\"orders\":[]}"),
            6,
            "interface_changed",
        ),
        (
            ok_reply(SEARCH, "application/json", &broken),
            6,
            "interface_changed",
        ),
        (
            ok_reply("https://www.ricardo.ch/de/", "application/json", "{}"),
            6,
            "interface_changed",
        ),
        (
            ok_reply("https://evil.test/api", "application/json", "{}"),
            4,
            "authentication_required",
        ),
        (
            json!({"error":{"code":"browser_error","message":"Unsupported browser operation; only fetch, resolve and favorite are enabled"}}),
            7,
            "extension_outdated",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let worker = fake_bridge(dir.path(), vec![reply.clone()]);
        let output = run(&["selling", "sold", "--json"], dir.path());
        worker.join().unwrap();
        let v = envelope(&output);
        assert_eq!(output.status.code(), Some(exit), "{reply}: {v}");
        assert_eq!(v["error"]["code"], code, "{reply}");
        assert!(v["data"].is_null());
        if code == "extension_outdated" {
            let hint = v["error"]["hint"].as_str().unwrap();
            assert!(
                hint.contains("0.3.0") && hint.contains("Connect bridge"),
                "{hint}"
            );
        }
    }
    // The failure that named a missing field reaches the agent.
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![ok_reply(SEARCH, "application/json", &broken)],
    );
    let v = envelope(&run(&["selling", "sold", "--json"], dir.path()));
    worker.join().unwrap();
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("pricing.total_price")
    );
}

#[test]
fn selling_drafts_reads_only_the_drafts_query() {
    let dir = tempfile::tempdir().unwrap();
    let html = r#"<html><script id="__NEXT_DATA__">{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["iban-info",{}],"state":{"data":{"holder":"Fake Holder","iban":"CH00 0000 0000 0000 0000 0"}}},{"queryKey":["selling-drafts",{"includeValidation":true,"limit":100,"locale":"de","offset":0}],"state":{"data":{"drafts":[],"draftsCount":0}}}]}}}}</script></html>"#;
    let worker = fake_bridge(
        dir.path(),
        vec![ok_reply(
            "https://www.ricardo.ch/de/my-ricardo/selling/drafts/",
            "text/html",
            html,
        )],
    );
    let output = run(&["selling", "drafts", "--json"], dir.path());
    assert_eq!(
        worker.join().unwrap(),
        vec![json!({"op":"fetch","path":"/de/my-ricardo/selling/drafts/"})]
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!text.contains("CH00") && !text.contains("Fake Holder"));
    let v = envelope(&output);
    assert_eq!(v["data"]["drafts_count"], 0);
    assert_eq!(v["data"]["drafts"], json!([]));
    assert_eq!(
        v["data"]["url"],
        "https://www.ricardo.ch/de/my-ricardo/selling/drafts/"
    );
    assert!(
        v["data"]["item_shape"]
            .as_str()
            .unwrap()
            .starts_with("unverified")
    );
    // A page without the drafts query (e.g. a login page) is an error, not zero drafts.
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![ok_reply(
            "https://www.ricardo.ch/de/my-ricardo/selling/drafts/",
            "text/html",
            "<h1>Just a moment...</h1>",
        )],
    );
    let output = run(&["selling", "drafts", "--json"], dir.path());
    worker.join().unwrap();
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(envelope(&output)["error"]["code"], "interface_changed");
}

#[test]
fn selling_status_makes_three_sequential_reads_and_stops_at_the_first_failure() {
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![
            ok_reply(LIMITS, "application/json", r#"{"current":4,"limit":10}"#),
            ok_reply(PENDING, "application/json", "true"),
            ok_reply(
                UNREADS,
                "application/json",
                r#"{"unread_count":2,"last_updated_at":"2026-10-07T09:00:00Z"}"#,
            ),
        ],
    );
    let output = run(&["selling", "status", "--json"], dir.path());
    assert_eq!(
        worker.join().unwrap(),
        vec![
            json!({"op":"fetch","path":"/api/orders-spa/selling/selling-limits"}),
            json!({"op":"fetch","path":"/api/orders-spa/selling/has-orders-pending-actions"}),
            json!({"op":"fetch","path":"/api/frontend/messaging/unreads"}),
        ]
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let data = &envelope(&output)["data"];
    assert_eq!(
        data["selling_limits"],
        json!({"current":4,"limit":10,"remaining":6})
    );
    assert_eq!(data["orders_pending_action"], true);
    assert_eq!(data["unread_messages"]["count"], 2);
    assert_eq!(data["next_commands"][0], "ricardo selling sold --json");
    assert!(
        data["scope"]
            .as_str()
            .unwrap()
            .contains("Message contents are not read")
    );
    assert!(!dir.path().join("observations.sqlite3").exists());

    // A blocked first read stops everything: exactly one request, exit 5.
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(dir.path(), vec![status_reply(429)]);
    let output = run(&["selling", "status", "--json"], dir.path());
    assert_eq!(worker.join().unwrap().len(), 1);
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(envelope(&output)["error"]["code"], "upstream_blocked");

    // A wrong type on the second read stops before the third.
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![
            ok_reply(LIMITS, "application/json", r#"{"current":4,"limit":10}"#),
            ok_reply(PENDING, "application/json", r#""yes""#),
        ],
    );
    let output = run(&["selling", "status", "--json"], dir.path());
    assert_eq!(worker.join().unwrap().len(), 2);
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(envelope(&output)["error"]["code"], "interface_changed");

    // An older extension rejecting a new route gets the reload instruction.
    let dir = tempfile::tempdir().unwrap();
    let worker = fake_bridge(
        dir.path(),
        vec![
            json!({"error":{"code":"browser_error","message":"Route is not an approved read endpoint"}}),
        ],
    );
    let output = run(&["selling", "status", "--json"], dir.path());
    assert_eq!(worker.join().unwrap().len(), 1);
    assert_eq!(output.status.code(), Some(7));
    let v = envelope(&output);
    assert_eq!(v["error"]["code"], "extension_outdated");
    assert!(v["error"]["hint"].as_str().unwrap().contains("0.3.0"));
}
