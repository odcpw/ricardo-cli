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
