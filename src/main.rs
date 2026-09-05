#![forbid(unsafe_code)]
mod bridge;
mod cli;
mod history;
mod media;
mod model;
mod output;
mod pacing;
mod tutti;
use chrono::Utc;
use clap::{CommandFactory, Parser};
use cli::{BridgeCommand, Command, ExtensionCommand, FavoritesCommand};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug)]
struct Error {
    exit: i32,
    code: &'static str,
    message: String,
    hint: String,
    retryable: bool,
}
impl Error {
    fn new(
        exit: i32,
        code: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            exit,
            code,
            message: message.into(),
            hint: hint.into(),
            retryable: matches!(exit, 3 | 5 | 6),
        }
    }
}
type Result<T> = std::result::Result<T, Error>;
fn state_dir(cli: &cli::Cli) -> Result<PathBuf> {
    cli.data_dir
        .clone()
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(|p| PathBuf::from(p).join("ricardo-cli")))
        .or_else(|| {
            std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share/ricardo-cli"))
        })
        .ok_or_else(|| {
            Error::new(
                8,
                "state_directory",
                "Cannot locate local state directory",
                "Pass --data-dir /absolute/private/path",
            )
        })
}
fn io_error(e: String) -> Error {
    Error::new(
        8,
        "local_io",
        e,
        "Check permissions on --data-dir; run ricardo doctor --json",
    )
}
fn upstream(e: String) -> Error {
    Error::new(
        6,
        "interface_changed",
        e,
        "Run ricardo doctor --json; inspect the live page in Chrome. Do not use cached results.",
    )
}
fn call(dir: &Path, req: Value, timeout: u64) -> Result<Value> {
    bridge::request(dir,req,Duration::from_secs(timeout)).map_err(|e| {
        if e.contains("Unsupported browser operation") {
            Error{exit:7,code:"extension_outdated",message:e,hint:"Reload the Ricardo CLI unpacked extension (0.2.0+) in Chrome, then click Connect bridge.".into(),retryable:false}
        } else if e.contains("rate_limit_cooldown") || e.contains("request_busy") {
            Error{exit:5,code:if e.contains("rate_limit_cooldown"){"rate_limit_cooldown"}else{"request_busy"},message:e,hint:"Wait for the cooldown or current command to finish. Do not run parallel commands or automatic retries.".into(),retryable:false}
        }else{Error::new(3,"bridge_unavailable",e,"Open the Ricardo CLI extension and Connect; run ricardo doctor --json.")}
    })
}
fn check_response(r: &Value, site: &str) -> Result<()> {
    let status = r["status"]
        .as_u64()
        .ok_or_else(|| upstream("Browser response missing HTTP status".into()))?;
    if matches!(status, 403 | 429) {
        return Err(Error {
            exit: 5,
            code: "upstream_blocked",
            message: format!("{site} returned HTTP {status}"),
            hint:
                "The bridge pauses all marketplace requests for two minutes. No automatic retries."
                    .into(),
            retryable: false,
        });
    }
    if status == 401 {
        return Err(Error::new(
            4,
            "authentication_required",
            format!("{site} requires sign-in"),
            "Sign in on the corresponding site in the connected Chrome profile",
        ));
    }
    if status == 404 {
        return Err(Error::new(
            6,
            "not_found",
            format!("{site} resource no longer exists"),
            "Check the current listing URL",
        ));
    }
    if !(200..300).contains(&status) {
        return Err(upstream(format!("{site} returned HTTP {status}")));
    }
    Ok(())
}
fn tutti_fetch(dir: &Path, path: &str, timeout: u64) -> Result<(String, String)> {
    let r = call(dir, json!({"op":"fetch_tutti","path":path}), timeout)?;
    check_response(&r, "Tutti")?;
    if r["url"] != format!("https://www.tutti.ch{path}") {
        return Err(upstream("Tutti response URL changed".into()));
    }
    if !r["content_type"]
        .as_str()
        .unwrap_or("")
        .contains("text/html")
    {
        return Err(upstream("Tutti response is not HTML".into()));
    }
    let body = r["body"]
        .as_str()
        .filter(|s| s.len() <= 4 * 1024 * 1024)
        .ok_or_else(|| upstream("Tutti response missing or too large".into()))?;
    let checked = r["checked_at"]
        .as_str()
        .ok_or_else(|| upstream("Tutti observation timestamp missing".into()))?;
    Ok((body.into(), checked.into()))
}
fn tutti_inspect(dir: &Path, input: &str, timeout: u64) -> Result<Value> {
    let u = tutti::listing_url(input)?;
    let (html, checked) = tutti_fetch(dir, u.path(), timeout)?;
    let data = tutti::parse_detail(&html, input, &checked)?;
    history::record(
        dir,
        "inspect",
        &format!("tutti:{}", data["id"].as_str().unwrap()),
        &data,
    )
    .map_err(io_error)?;
    Ok(data)
}
fn listing_images(dir: &Path, args: &cli::ImageArgs, timeout: u64) -> Result<Value> {
    let data = if args.article.starts_with("https://www.tutti.ch/") {
        tutti_inspect(dir, &args.article, timeout)?
    } else {
        json!(inspect(dir, &args.article, timeout)?)
    };
    media::download(dir, args, data, timeout)
}
fn fetch(dir: &Path, path: &str, timeout: u64) -> Result<(String, String, String)> {
    validate_read_path(path)?;
    let r = call(dir, json!({"op":"fetch","path":path}), timeout)?;
    let status = r["status"]
        .as_u64()
        .ok_or_else(|| upstream("Browser response missing HTTP status".into()))?;
    let url = r["url"]
        .as_str()
        .ok_or_else(|| upstream("Browser response missing URL".into()))?;
    let final_url = url::Url::parse(url).map_err(|e| upstream(e.to_string()))?;
    if final_url.scheme() != "https" || final_url.host_str() != Some("www.ricardo.ch") {
        return Err(Error::new(
            4,
            "authentication_required",
            "Ricardo redirected outside the expected site",
            "Sign in to Ricardo in the connected Chrome profile",
        ));
    }
    if matches!(status, 401) {
        return Err(Error::new(
            4,
            "authentication_required",
            "Ricardo session is not signed in",
            "Sign in to Ricardo in the connected Chrome profile",
        ));
    }
    if matches!(status, 403 | 429) {
        return Err(Error{exit:5,code:"upstream_blocked",message:format!("Ricardo returned HTTP {status}; search stopped"),hint:"The shared bridge pauses Ricardo requests for two minutes. Do not retry or reconnect to bypass the cooldown.".into(),retryable:false});
    }
    if status == 404 {
        return Err(Error::new(
            6,
            "not_found",
            "Ricardo article or endpoint no longer exists",
            "Check the article ID or current Ricardo interface",
        ));
    }
    if !(200..300).contains(&status) {
        return Err(upstream(format!("Ricardo returned HTTP {status}")));
    }
    let body = r["body"]
        .as_str()
        .ok_or_else(|| upstream("Missing response body".into()))?;
    let content_type = r["content_type"].as_str().unwrap_or("").to_owned();
    if body.len() > 4 * 1024 * 1024 {
        return Err(upstream("Response exceeds 4 MiB".into()));
    }
    Ok((body.to_owned(), content_type, url.to_owned()))
}
fn validate_read_path(path: &str) -> Result<()> {
    let bad = || {
        Error::new(
            7,
            "unsupported_route",
            "Only observed Ricardo read routes are supported",
            "Use ricardo search QUERY, ricardo inspect ID, or ricardo capabilities --json",
        )
    };
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.chars().any(|c| c.is_control() || c == '\\')
    {
        return Err(bad());
    }
    let u = url::Url::parse(&format!("https://www.ricardo.ch{path}")).map_err(|_| bad())?;
    if u.host_str() != Some("www.ricardo.ch") || u.fragment().is_some() {
        return Err(bad());
    }
    let p = u.path();
    if p == "/de/"
        || p.starts_with("/de/s/")
        || p.starts_with("/de/a/")
        || matches!(
            p,
            "/de/my-ricardo/saved/articles/"
                | "/de/my-ricardo/selling/offers/"
                | "/api/orders-spa/saved-articles"
                | "/api/orders-spa/buying/offers"
                | "/api/frontend/notifications"
                | "/api/mfa/notifications"
        )
    {
        Ok(())
    } else {
        Err(bad())
    }
}
fn article_input(s: &str) -> Result<(String, Option<String>)> {
    let bad = || {
        Error::new(
            2,
            "invalid_article",
            "Expected a numeric article ID or an https://www.ricardo.ch/de/a/... URL",
            "Example: ricardo inspect 1327242570 --json",
        )
    };
    if (6..=15).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()) {
        return Ok((s.into(), None));
    }
    let u = url::Url::parse(s).map_err(|_| bad())?;
    if u.scheme() != "https"
        || u.host_str() != Some("www.ricardo.ch")
        || u.port().is_some()
        || !u.username().is_empty()
        || u.password().is_some()
        || !u.path().starts_with("/de/a/")
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err(bad());
    }
    let tail = u
        .path()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .ok_or_else(bad)?;
    let id = tail.rsplit('-').next().ok_or_else(bad)?;
    if !(6..=15).contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    Ok((
        id.into(),
        if tail == id {
            None
        } else {
            Some(u.path().into())
        },
    ))
}
fn inspect(dir: &Path, input: &str, timeout: u64) -> Result<model::Listing> {
    let (id, path) = article_input(input)?;
    let path = match path {
        Some(p) => p,
        None => {
            let r = call(dir, json!({"op":"resolve","article":id}), timeout)?;
            let url = r["url"]
                .as_str()
                .ok_or_else(|| upstream("Article resolve returned no URL".into()))?;
            let (returned, p) = article_input(url)?;
            if returned != id {
                return Err(upstream("Resolved article ID mismatch".into()));
            }
            p.ok_or_else(|| upstream("Resolve did not return canonical URL".into()))?
        }
    };
    let (html, _, _) = fetch(dir, &path, timeout)?;
    model::detail(&html, &id, Utc::now()).map_err(upstream)
}
fn account(dir: &Path, timeout: u64) -> Result<Value> {
    let (h, _, _) = fetch(dir, "/de/", timeout)?;
    let mut a = model::account_data(&h).map_err(|e| {
        Error::new(
            4,
            "authentication_required",
            e,
            "Sign in in Chrome and retry ricardo account --json",
        )
    })?;
    a["checked_at"] = json!(Utc::now().to_rfc3339());
    Ok(a)
}
fn raw_api(dir: &Path, path: &str, timeout: u64) -> Result<Value> {
    let (body, ct, url) = fetch(dir, path, timeout)?;
    if !ct.contains("json") {
        return Err(Error::new(
            4,
            "authentication_required",
            "Expected JSON; got a page instead",
            "Sign in to Ricardo and retry",
        ));
    }
    let data: Value = serde_json::from_str(&body).map_err(|e| upstream(e.to_string()))?;
    Ok(
        json!({"source":"live_browser","checked_at":Utc::now().to_rfc3339(),"url":url,"upstream":data}),
    )
}
fn page(dir: &Path, input: &str, timeout: u64) -> Result<Value> {
    let path = if input.starts_with('/') {
        input.to_owned()
    } else {
        let u = url::Url::parse(input)
            .map_err(|e| Error::new(2, "invalid_url", e.to_string(), "Use a Ricardo URL"))?;
        if u.origin().ascii_serialization() != "https://www.ricardo.ch"
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
        {
            return Err(Error::new(
                7,
                "unsupported_origin",
                "URL must belong to www.ricardo.ch",
                "Use a Ricardo URL",
            ));
        }
        format!(
            "{}{}",
            u.path(),
            u.query().map(|q| format!("?{q}")).unwrap_or_default()
        )
    };
    let (body, ct, url) = fetch(dir, &path, timeout)?;
    if ct.contains("json") {
        return Ok(
            json!({"source":"live_browser","checked_at":Utc::now().to_rfc3339(),"url":url,"upstream":serde_json::from_str::<Value>(&body).map_err(|e|upstream(e.to_string()))?}),
        );
    }
    let doc = scraper::Html::parse_document(&body);
    let sel = scraper::Selector::parse("h1,h2,main,article").unwrap();
    let texts = doc
        .select(&sel)
        .map(|e| e.text().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>();
    let links=doc.select(&scraper::Selector::parse("a[href]").unwrap()).filter_map(|e|{let href=e.value().attr("href")?;if !href.starts_with("/de/"){return None}Some(json!({"text":e.text().collect::<Vec<_>>().join(" "),"url":format!("https://www.ricardo.ch{href}")}))}).take(300).collect::<Vec<_>>();
    if texts.is_empty() {
        return Err(upstream("Page has no recognized content".into()));
    }
    Ok(
        json!({"source":"live_browser_html","checked_at":Utc::now().to_rfc3339(),"url":url,"text":texts,"links":links,"limitation":"Server-rendered snapshot; client-loaded account data is available via named commands."}),
    )
}
fn run(c: &cli::Cli, dir: &Path) -> Result<Value> {
    match &c.command {
        Command::Capabilities => Ok(output::capabilities()),
        Command::RobotDocs { .. } => Ok(json!(output::agent_guide())),
        Command::Bridge {
            command: BridgeCommand::Status,
        } => call(dir, json!({"op":"status"}), c.timeout),
        Command::Bridge {
            command: BridgeCommand::Serve | BridgeCommand::Native,
        } => unreachable!(),
        Command::Extension {
            command: ExtensionCommand::Install { extension_id },
        } => bridge::install_native_host(dir, extension_id).map_err(io_error),
        Command::Images(args) => listing_images(dir, args, c.timeout),
        Command::Tutti { command } => match command {
            cli::TuttiCommand::Inspect { url } => tutti_inspect(dir, url, c.timeout),
            cli::TuttiCommand::Images(args) => {
                tutti::listing_url(&args.article)?;
                listing_images(dir, args, c.timeout)
            }
            cli::TuttiCommand::Search { query, limit } => {
                let query = query.join(" ");
                let path = tutti::search_path(&query)?;
                let (html, checked) = tutti_fetch(dir, &path, c.timeout)?;
                let (total, urls) = tutti::search_candidates(&html, &query)?;
                let mut listings = Vec::new();
                for url in urls.iter().take(*limit as usize) {
                    listings.push(tutti_inspect(dir, url, c.timeout)?);
                }
                Ok(
                    json!({"marketplace":"tutti","source":"live_browser","query":query,"checked_at":checked,"url":format!("https://www.tutti.ch{path}"),"total":total,"page":1,"partial":(listings.len() as u64)<total,"listings":listings,"scope":"First search page only, newest first; each returned listing independently inspected. Listed is not seller-confirmed availability."}),
                )
            }
        },
        Command::Account => account(dir, c.timeout),
        Command::Doctor => {
            let b = call(dir, json!({"op":"status"}), c.timeout)?;
            let a = account(dir, c.timeout)?;
            history::list(dir, None, 1).map_err(io_error)?;
            Ok(
                json!({"healthy":true,"bridge":b,"account":a,"state_directory":dir,"next_commands":["ricardo search Bambu P2S --json","ricardo inspect ARTICLE_ID --json"],"storage":"FrankenSQLite 0.3.16","runtime":"Asupersync 0.4.10"}),
            )
        }
        Command::Inspect { article } => {
            if article.starts_with("https://www.tutti.ch/") {
                return tutti_inspect(dir, article, c.timeout);
            }
            let l = inspect(dir, article, c.timeout)?;
            let data = json!(l);
            history::record(dir, "inspect", &l.id, &data).map_err(io_error)?;
            Ok(data)
        }
        Command::Search(s) => {
            let query = s.query.join(" ");
            let mut url = url::Url::parse("https://www.ricardo.ch/de/s/").unwrap();
            url.path_segments_mut()
                .unwrap()
                .pop_if_empty()
                .push(&query)
                .push("");
            // Observed via the Ricardo page-2 navigation button; filters and sorting are local to this page.
            if s.page > 1 {
                url.query_pairs_mut()
                    .append_pair("page", &s.page.to_string());
            }
            let path = format!(
                "{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default()
            );
            let (html, _, _) = fetch(dir, &path, c.timeout)?;
            let (mut items, total) = model::search_listings(&html, Utc::now()).map_err(upstream)?;
            let page_count = items.len();
            items.retain(|l| {
                s.category
                    .as_ref()
                    .is_none_or(|v| l.category_id.as_ref() == Some(v))
                    && s.product_type
                        .as_ref()
                        .is_none_or(|v| l.product_type.as_ref() == Some(v))
                    && s.max_price
                        .is_none_or(|max| l.entry_price().is_some_and(|p| p <= max))
                    && (s.include_ended || l.status == "active")
            });
            match s.sort {
                cli::Sort::Price => items.sort_by(|a, b| {
                    a.entry_price()
                        .unwrap_or(f64::INFINITY)
                        .total_cmp(&b.entry_price().unwrap_or(f64::INFINITY))
                }),
                cli::Sort::Ending => items.sort_by(|a, b| a.end_at.cmp(&b.end_at)),
                cli::Sort::Relevance => {}
            }
            let candidates = items.len();
            items.truncate(s.limit as usize);
            let mut verified = Vec::new();
            let warnings: Vec<Value> = Vec::new();
            for l in items {
                let d = inspect(dir, &l.url, c.timeout)?;
                if (s.include_ended || d.status == "active")
                    && s.max_price
                        .is_none_or(|p| d.entry_price().is_some_and(|v| v <= p))
                    && s.category
                        .as_ref()
                        .is_none_or(|v| d.category_id.as_ref() == Some(v))
                    && s.product_type
                        .as_ref()
                        .is_none_or(|v| d.product_type.as_ref() == Some(v))
                {
                    verified.push(d);
                }
            }
            match s.sort {
                cli::Sort::Price => verified.sort_by(|a, b| {
                    a.entry_price()
                        .unwrap_or(f64::INFINITY)
                        .total_cmp(&b.entry_price().unwrap_or(f64::INFINITY))
                }),
                cli::Sort::Ending => verified.sort_by(|a, b| a.end_at.cmp(&b.end_at)),
                cli::Sort::Relevance => {}
            }
            if !warnings.is_empty() {
                return Err(upstream(format!(
                    "{} article verification(s) failed; search is incomplete: {:?}",
                    warnings.len(),
                    warnings
                )));
            }
            let data = json!({"query":query,"source":"live_browser","checked_at":Utc::now().to_rfc3339(),"total":total,"page":s.page,"page_count":page_count,"candidates":candidates,"partial":total>page_count as u64||candidates>s.limit as usize,"filter_scope":"requested page; each returned article verified separately","sort":s.sort.as_str(),"sort_scope":"requested page","listings":verified,"warnings":warnings});
            history::record(dir, "search", &query, &data).map_err(io_error)?;
            Ok(data)
        }
        Command::Favorites {
            command: FavoritesCommand::List { offset, limit },
        } => {
            let mut data = raw_api(
                dir,
                &format!("/api/orders-spa/saved-articles?offset={offset}&limit={limit}"),
                c.timeout,
            )?;
            let count = data["upstream"]["savedArticles"]
                .as_array()
                .ok_or_else(|| upstream("Favorites response missing savedArticles array".into()))?
                .len() as u64;
            let total = data["upstream"]["totalCount"]
                .as_u64()
                .ok_or_else(|| upstream("Favorites response missing numeric totalCount".into()))?;
            let end = u64::from(*offset) + count;
            data["pagination"] = json!({"offset":offset,"limit":limit,"total":total,"partial":*offset>0||end<total,"next_offset":if count>0&&end<total{Some(end)}else{None}});
            Ok(data)
        }
        Command::Favorites { command } => {
            let (article, yes, action) = match command {
                FavoritesCommand::Add { article, yes } => (article, yes, "add"),
                FavoritesCommand::Remove { article, yes } => (article, yes, "remove"),
                _ => unreachable!(),
            };
            let (id, _) = article_input(article)?;
            if !yes {
                return Ok(
                    json!({"dry_run":true,"action":format!("favorites.{action}"),"article_id":id,"executable":true,"execute_with":format!("ricardo favorites {action} {id} --yes --json")}),
                );
            }
            let r=call(dir,json!({"op":"favorite","article":id,"action":action}),c.timeout).map_err(|e|Error{exit:6,code:"mutation_outcome_unknown",message:e.message,hint:"Check ricardo favorites list --json before retrying; the request may have succeeded".into(),retryable:false})?;
            let status = r["status"].as_u64().unwrap_or(0);
            if !(200..300).contains(&status) {
                return Err(Error {
                    exit: 6,
                    code: "favorite_failed",
                    message: format!("Ricardo returned HTTP {status}"),
                    hint: "Check favorites in Ricardo before retrying".into(),
                    retryable: false,
                });
            }
            Ok(
                json!({"dry_run":false,"action":format!("favorites.{action}"),"article_id":id,"acknowledged":true,"http_status":status,"checked_at":Utc::now().to_rfc3339(),"source":"live_browser","verification":"Ricardo HTTP acknowledgement"}),
            )
        }
        Command::Buying { offset, limit } => {
            let mut data = raw_api(
                dir,
                &format!("/api/orders-spa/buying/offers?offset={offset}&limit={limit}&locale=de"),
                c.timeout,
            )?;
            let count = data["upstream"]["offers"]
                .as_array()
                .ok_or_else(|| upstream("Buying response missing offers array".into()))?
                .len() as u64;
            let total = data["upstream"]["offersCount"]
                .as_u64()
                .ok_or_else(|| upstream("Buying response missing numeric offersCount".into()))?;
            let end = u64::from(*offset) + count;
            data["pagination"] = json!({"offset":offset,"limit":limit,"total":total,"partial":*offset>0||end<total,"next_offset":if count>0&&end<total{Some(end)}else{None}});
            data["scope"] = json!("buying offers; completed orders are not included");
            Ok(data)
        }
        Command::Selling => {
            let (html, _, url) = fetch(dir, "/de/my-ricardo/selling/offers/", c.timeout)?;
            let values = model::page_data(&html).map_err(upstream)?;
            let q = values
                .iter()
                .find_map(|v| model::find(v, &|x| x["queryKey"][0] == "selling-offers"))
                .ok_or_else(|| {
                    upstream("Selling offers query missing; session or interface changed".into())
                })?;
            Ok(
                json!({"source":"live_browser","checked_at":Utc::now().to_rfc3339(),"url":url,"upstream":q["state"]["data"]}),
            )
        }
        Command::Notifications => raw_api(dir, "/api/frontend/notifications?locale=de", c.timeout),
        Command::Page { url } => page(dir, url, c.timeout),
        Command::History { query, limit } => {
            history::list(dir, query.as_deref(), *limit).map_err(io_error)
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    if args.len() == 1 {
        let _ = cli::Cli::command().print_help();
        println!();
        return;
    }
    let c = match cli::Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                print!("{e}");
                return;
            }
            let _ = output::emit(
                &output::failure(
                    "invalid_input",
                    &e.to_string(),
                    "Run ricardo --help or ricardo help search for supported commands and flags",
                    false,
                ),
                json_mode,
            );
            std::process::exit(2)
        }
    };
    let result = state_dir(&c).and_then(|dir| {
        if let Command::Bridge {
            command: BridgeCommand::Serve | BridgeCommand::Native,
        } = &c.command
        {
            let native = matches!(
                c.command,
                Command::Bridge {
                    command: BridgeCommand::Native
                }
            );
            bridge::serve(&dir, native).map_err(io_error)?;
            Ok(Value::Null)
        } else {
            run(&c, &dir)
        }
    });
    if matches!(
        c.command,
        Command::Bridge {
            command: BridgeCommand::Serve | BridgeCommand::Native
        }
    ) {
        if let Err(e) = result {
            eprintln!("{}: {}", e.code, e.message);
            std::process::exit(e.exit)
        }
        return;
    }
    let (value, exit) = match result {
        Ok(v) => (output::success(v), 0),
        Err(e) => (
            output::failure(e.code, &e.message, &e.hint, e.retryable),
            e.exit,
        ),
    };
    if let Err(e) = output::emit(&value, c.json)
        && e.kind() != std::io::ErrorKind::BrokenPipe
    {
        eprintln!("Output failed: {e}");
        std::process::exit(8)
    }
    std::process::exit(exit)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_foreign_article_origins() {
        for s in [
            "https://evil.test/de/a/x-1327242570/",
            "https://www.ricardo.ch@evil.test/de/a/x-1327242570/",
            "https://www.ricardo.ch/de/a/x-1327242570/?token=secret",
        ] {
            assert!(article_input(s).is_err());
        }
    }
    #[test]
    fn accepts_article_id() {
        assert_eq!(article_input("1327242570").unwrap().0, "1327242570");
    }
}
