//! Shared browser-request pacing. Only timestamps are persisted.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const GAP_MS: u64 = 8_000;
const COOLDOWN_MS: u64 = 120_000;
#[derive(Default, Serialize, Deserialize)]
struct State {
    next_allowed_ms: u64,
    blocked_until_ms: u64,
}
pub struct Pacing {
    path: PathBuf,
    state: State,
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Pacing {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let path = dir.join("request-pacing.json");
        let state = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Cannot read request pacing state: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self { path, state })
    }
    fn save(&self) -> Result<(), String> {
        let tmp = self.path.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_vec(&self.state).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(tmp, &self.path).map_err(|e| e.to_string())
    }
    pub fn delay(&self, now: u64) -> Result<Duration, String> {
        if now < self.state.blocked_until_ms {
            return Err(format!(
                "rate_limit_cooldown: Ricardo requests paused until Unix time {} ms; do not retry before then",
                self.state.blocked_until_ms
            ));
        }
        Ok(Duration::from_millis(
            self.state.next_allowed_ms.saturating_sub(now),
        ))
    }
    pub fn started(&mut self, now: u64) -> Result<(), String> {
        // If the broker crashes mid-request, a restarted broker still waits.
        self.state.next_allowed_ms = now + 60_000;
        self.save()
    }
    pub fn finished(&mut self, reply: Option<&serde_json::Value>, now: u64) -> Result<(), String> {
        let blocked = reply.is_some_and(|r| {
            matches!(r["result"]["status"].as_u64(), Some(403 | 429))
                || r.get("error").is_some_and(|e| {
                    let s = e.to_string();
                    s.contains("HTTP 429") || s.contains("HTTP 403")
                })
        });
        self.state.next_allowed_ms = now + if reply.is_some() { GAP_MS } else { 60_000 };
        if blocked {
            self.state.blocked_until_ms = now + COOLDOWN_MS;
        }
        self.save()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn completion_gap_and_crash_reservation_survive_restart() {
        let d = tempfile::tempdir().unwrap();
        let mut p = Pacing::open(d.path()).unwrap();
        p.started(1000).unwrap();
        assert_eq!(
            Pacing::open(d.path())
                .unwrap()
                .delay(1000)
                .unwrap()
                .as_millis(),
            60_000
        );
        p.finished(Some(&json!({"result":{"status":200}})), 2000)
            .unwrap();
        assert_eq!(p.delay(2001).unwrap().as_millis(), 7999);
        assert!(p.delay(10_000).unwrap().is_zero());
    }
    #[test]
    fn rate_limit_blocks_without_sleep_or_retry_and_survives_restart() {
        for reply in [
            json!({"result":{"status":429}}),
            json!({"result":{"status":403}}),
            json!({"error":{"message":"Article resolution returned HTTP 429"}}),
        ] {
            let d = tempfile::tempdir().unwrap();
            let mut p = Pacing::open(d.path()).unwrap();
            p.finished(Some(&reply), 1000).unwrap();
            let p = Pacing::open(d.path()).unwrap();
            assert!(
                p.delay(120_999)
                    .unwrap_err()
                    .contains("rate_limit_cooldown")
            );
            assert!(p.delay(121_000).unwrap().is_zero());
        }
    }
}
