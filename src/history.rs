//! Append-only public observations. History is never a live availability source.
use asupersync::runtime::RuntimeBuilder;
use fsqlite::{Connection, SqliteValue};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
};

// FrankenSQLite 0.3.16 overflows Rust's default 2 MiB test-thread stack in
// debug builds. Keep its runtime/futures entirely on one bounded larger-stack
// thread rather than depending on the caller's stack or RUST_MIN_STACK.
const DATABASE_STACK_BYTES: usize = 32 * 1024 * 1024;
fn on_database_thread<T: Send>(
    job: impl FnOnce() -> Result<T, String> + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("ricardo-history".into())
            .stack_size(DATABASE_STACK_BYTES)
            .spawn_scoped(scope, job)
            .map_err(|e| format!("Cannot start history worker: {e}"))?
            .join()
            .map_err(|_| "History worker panicked".to_string())?
    })
}

async fn open(dir: &Path) -> Result<Connection, String> {
    let mut builder = fs::DirBuilder::new();
    builder
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| e.to_string())?;
    let meta = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("History directory must be a real directory".into());
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let path = dir.join("observations.sqlite3");
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("History database must not be a symlink".into());
    }
    let conn = Connection::open(path.to_str().ok_or("History path is not valid UTF-8")?)
        .await
        .map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    conn.execute("CREATE TABLE IF NOT EXISTS observations (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, lookup_key TEXT NOT NULL, checked_at TEXT NOT NULL, payload TEXT NOT NULL)").await.map_err(|e|e.to_string())?;
    conn.execute("CREATE INDEX IF NOT EXISTS observations_lookup ON observations (lookup_key, id)")
        .await
        .map_err(|e| e.to_string())?;
    Ok(conn)
}
fn public_only(data: &Value) -> Result<(), String> {
    match data {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "body"
                        | "html"
                        | "raw"
                        | "cookies"
                        | "cookie"
                        | "authorization"
                        | "password"
                        | "token"
                        | "access_token"
                        | "refresh_token"
                        | "currentuser"
                        | "email"
                        | "phone"
                        | "account_data"
                ) {
                    return Err("History accepts normalized public listing data only".into());
                }
                public_only(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                public_only(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn record(dir: &Path, kind: &str, key: &str, data: &Value) -> Result<(), String> {
    if !matches!(kind, "search" | "detail" | "inspect") {
        return Err("Only public search and detail observations may be recorded".into());
    }
    public_only(data)?;
    on_database_thread(|| {
        let runtime = RuntimeBuilder::current_thread()
            .build()
            .map_err(|e| e.to_string())?;
        runtime.block_on(async {
        let conn = open(dir).await?;
        conn.execute_with_params("INSERT INTO observations (kind, lookup_key, checked_at, payload) VALUES (?1, ?2, ?3, ?4)", &[SqliteValue::from(kind),SqliteValue::from(key),SqliteValue::from(chrono::Utc::now().to_rfc3339()),SqliteValue::from(data.to_string())]).await.map_err(|e|e.to_string())?;
        Ok(())
    })
    })
}
pub fn list(dir: &Path, key: Option<&str>, limit: u32) -> Result<Value, String> {
    if limit > 1000 {
        return Err("History limit must be at most 1000".into());
    }
    on_database_thread(|| {
        let runtime = RuntimeBuilder::current_thread()
            .build()
            .map_err(|e| e.to_string())?;
        runtime.block_on(async {
        let conn = open(dir).await?;
        let rows = if let Some(key) = key {
            conn.query_with_params("SELECT kind, lookup_key, checked_at, payload FROM observations WHERE lookup_key = ?1 ORDER BY id DESC LIMIT ?2", &[SqliteValue::from(key),SqliteValue::Integer(i64::from(limit))]).await
        } else {
            conn.query_with_params("SELECT kind, lookup_key, checked_at, payload FROM observations ORDER BY id DESC LIMIT ?1", &[SqliteValue::Integer(i64::from(limit))]).await
        }.map_err(|e|e.to_string())?;
        let mut observations = Vec::new();
        for row in rows {
            let get = |i| -> Result<String,String> {match row.get(i){Some(SqliteValue::Text(s))=>Ok(s.to_string()),_=>Err("Invalid history database row".into())}};
            observations.push(json!({"kind":get(0)?,"key":get(1)?,"observed_at":get(2)?,"historical":true,"data":serde_json::from_str::<Value>(&get(3)?).map_err(|e|e.to_string())?}));
        }
        Ok(json!({"historical":true,"observations":observations}))
    })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observations_persist_across_connections_and_filter_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        record(
            dir.path(),
            "detail",
            "123",
            &json!({"id":"123","status":"active","next_bid":200}),
        )
        .unwrap();
        record(
            dir.path(),
            "detail",
            "999",
            &json!({"id":"999","status":"active"}),
        )
        .unwrap();
        record(
            dir.path(),
            "detail",
            "123",
            &json!({"id":"123","status":"ended","next_bid":250}),
        )
        .unwrap();
        let result = list(dir.path(), Some("123"), 10).unwrap();
        let rows = result["observations"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["data"]["status"], "ended");
        assert_eq!(rows[1]["data"]["next_bid"], 200);
        assert_eq!(rows[0]["historical"], true);
        assert_eq!(
            list(dir.path(), None, 1).unwrap()["observations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn account_and_raw_payloads_are_not_persisted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(record(dir.path(), "account", "me", &json!({})).is_err());
        assert!(
            record(
                dir.path(),
                "search",
                "test",
                &json!({"nested":{"cookies":"secret"}})
            )
            .is_err()
        );
        assert!(!dir.path().join("observations.sqlite3").exists());
    }
}
