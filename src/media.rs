//! Bounded gallery downloads via the shared paced bridge. No cookie export.
use crate::{Error, Result, call, io_error, upstream};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

pub fn validate_url(input: &str) -> Result<()> {
    let bad = || {
        Error::new(
            7,
            "unsupported_image_url",
            "Image URL is outside the observed Ricardo/Tutti CDN paths",
            "Inspect the listing again; only its supported gallery images may be downloaded",
        )
    };
    let u = url::Url::parse(input).map_err(|_| bad())?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || input.chars().any(|c| c.is_control() || c == '\\')
        || ["%2e", "%2f", "%5c"]
            .iter()
            .any(|p| input.to_lowercase().contains(p))
    {
        return Err(bad());
    }
    let p: Vec<_> = u.path().split('/').collect();
    let valid = match u.host_str() {
        Some("img.ricardostatic.ch") => {
            p.len() == 5
                && p[1] == "images"
                && uuid::Uuid::parse_str(p[2]).is_ok()
                && p[3]
                    .strip_prefix("t_")
                    .and_then(|s| s.split_once('x'))
                    .is_some_and(|(a, b)| {
                        !a.is_empty()
                            && !b.is_empty()
                            && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit())
                    })
                && !p[4].is_empty()
        }
        Some("c.tutti.ch") => {
            p.len() == 3
                && matches!(p[1], "big" | "thumbnail")
                && p[2].rsplit_once('.').is_some_and(|(id, ext)| {
                    !id.is_empty()
                        && id.bytes().all(|c| c.is_ascii_digit())
                        && matches!(ext, "jpg" | "jpeg" | "png" | "webp")
                })
        }
        _ => false,
    };
    if valid { Ok(()) } else { Err(bad()) }
}
fn image_extension(bytes: &[u8], mime: &str) -> Result<&'static str> {
    match mime {
        "image/jpeg" if bytes.starts_with(&[0xff, 0xd8, 0xff]) => Ok("jpg"),
        "image/png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Ok("png"),
        "image/webp" if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => {
            Ok("webp")
        }
        _ => Err(upstream(
            "Image signature does not match a supported JPEG/PNG/WebP content type".into(),
        )),
    }
}
pub fn download(
    dir: &Path,
    args: &crate::cli::ImageArgs,
    listing: Value,
    timeout: u64,
) -> Result<Value> {
    let urls = listing["images"]
        .as_array()
        .ok_or_else(|| upstream("Listing gallery missing".into()))?;
    if args.index.is_some_and(|i| i as usize > urls.len()) {
        return Err(Error::new(
            2,
            "image_index",
            "Photo index exceeds gallery size",
            format!("This listing has {} photos; indices start at 1", urls.len()),
        ));
    }
    let selected: Vec<_> = urls
        .iter()
        .enumerate()
        .filter(|(i, _)| args.index.is_none_or(|n| n as usize == i + 1))
        .take(args.limit as usize)
        .map(|(i, u)| {
            u.as_str()
                .map(|s| (i + 1, s))
                .ok_or_else(|| upstream("Gallery URL is not a string".into()))
        })
        .collect::<Result<_>>()?;
    for (_, url) in &selected {
        validate_url(url)?;
    }
    let parent = args
        .output_dir
        .clone()
        .unwrap_or_else(|| dir.join("images"));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&parent)
        .map_err(|e| io_error(e.to_string()))?;
    let folder = parent.join(uuid::Uuid::new_v4().to_string());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&folder)
        .map_err(|e| io_error(e.to_string()))?;
    let folder = fs::canonicalize(folder).map_err(|e| io_error(e.to_string()))?;
    let mut images = Vec::new();
    for (index, url) in selected {
        let r = call(dir, json!({"op":"image","url":url}), timeout)?;
        crate::check_response(&r, "Image")?;
        if r["url"] != url {
            return Err(upstream("Image response URL changed".into()));
        }
        let mime = r["content_type"].as_str().unwrap_or("");
        let encoded = r["base64"].as_str().ok_or_else(|| {
            upstream("Image response missing bytes; reload the browser extension".into())
        })?;
        if encoded.len() > 5_592_408 {
            return Err(upstream("Encoded image exceeds 4 MiB limit".into()));
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|e| upstream(e.to_string()))?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(upstream("Image exceeds 4 MiB limit".into()));
        }
        let ext = image_extension(&bytes, mime)?;
        let path = folder.join(format!("{index:03}.{ext}"));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| io_error(e.to_string()))?;
        file.write_all(&bytes)
            .map_err(|e| io_error(e.to_string()))?;
        images.push(json!({"index":index,"url":url,"path":path,"bytes":bytes.len(),"content_type":mime,"checked_at":r["checked_at"]}));
    }
    let result = json!({"listing_id":listing["id"],"listing_url":listing["url"],"listing_checked_at":listing["checked_at"],"total_images":urls.len(),"partial":images.len()<urls.len(),"images":images,"output_dir":folder});
    let mut manifest = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(folder.join("manifest.json"))
        .map_err(|e| io_error(e.to_string()))?;
    serde_json::to_writer_pretty(&mut manifest, &result).map_err(|e| io_error(e.to_string()))?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gallery_origin_allowlist() {
        for u in [
            "https://evil.test/a.jpg",
            "https://c.tutti.ch/big/1.jpg?x=1",
            "https://c.tutti.ch/big/1.svg",
            "http://c.tutti.ch/big/1.jpg",
            "https://c.tutti.ch/big/../private",
        ] {
            assert!(validate_url(u).is_err());
        }
        assert!(validate_url("https://c.tutti.ch/big/5804655919.jpg").is_ok());
        assert!(validate_url("https://img.ricardostatic.ch/images/1d6df0c6-fe2a-4534-868a-9f78939156f7/t_1000x750/prusa-core-one").is_ok());
    }
    #[test]
    fn reject_html_masquerading_as_photo() {
        assert!(image_extension(b"<html>challenge</html>", "image/jpeg").is_err());
        assert_eq!(
            image_extension(b"\xff\xd8\xffdata", "image/jpeg").unwrap(),
            "jpg"
        );
        assert!(image_extension(b"\xff\xd8\xffdata", "image/png").is_err());
    }
}
