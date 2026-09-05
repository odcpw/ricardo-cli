//! Private local browser broker. Browser credentials never cross this boundary.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_CLIENTS: usize = 16;
type Pending = Arc<Mutex<HashMap<String, mpsc::SyncSender<Value>>>>;
fn private_dir(dir: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder
        .create(dir)
        .map_err(|e| format!("Cannot create bridge directory: {e}"))?;
    let meta = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Bridge directory must be a real directory".into());
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())
}
fn line(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut data = Vec::new();
    let n = reader
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut data)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(None);
    }
    if n > MAX_FRAME || data.last() != Some(&b'\n') {
        return Err("Invalid or oversized bridge frame".into());
    }
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|e| format!("Invalid bridge JSON: {e}"))
}
pub fn request(data_dir: &Path, request: Value, timeout: Duration) -> Result<Value, String> {
    let mut socket = UnixStream::connect(data_dir.join("bridge.sock")).map_err(|_| "Browser bridge unavailable. Run `ricardo bridge serve` with an agent worker or connect the browser extension.".to_string())?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if bytes.len() >= MAX_FRAME {
        return Err("Request exceeds bridge frame limit".into());
    }
    socket
        .write_all(&bytes)
        .and_then(|_| socket.write_all(b"\n"))
        .map_err(|e| e.to_string())?;
    let response = line(&mut BufReader::new(socket))?.ok_or("Bridge closed without a response")?;
    if let Some(error) = response.get("error") {
        return Err(error.to_string());
    }
    response
        .get("result")
        .cloned()
        .ok_or("Bridge response has no result".into())
}
fn write_worker(writer: &mut impl Write, value: &Value, native: bool) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Worker frame exceeds limit".into());
    }
    if native {
        writer
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    writer.write_all(&bytes).map_err(|e| e.to_string())?;
    if !native {
        writer.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}
fn worker_writer(
    writer: &mut impl Write,
    outgoing: mpsc::Receiver<Value>,
    pending_writer: Pending,
    writer_connected: Arc<AtomicBool>,
    native: bool,
) {
    while let Ok(frame) = outgoing.recv() {
        // Expired queued requests must never run later, especially mutations.
        if !frame
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| pending_writer.lock().unwrap().contains_key(id))
        {
            continue;
        }
        if write_worker(writer, &frame, native).is_err() {
            break;
        }
    }
    writer_connected.store(false, Ordering::Release);
    pending_writer.lock().unwrap().clear();
}
struct SocketGuard {
    path: std::path::PathBuf,
    inode: u64,
    _lock: fs::File,
}
impl Drop for SocketGuard {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.ino() == self.inode && m.file_type().is_socket())
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn bind_socket(data_dir: &Path) -> Result<(UnixListener, SocketGuard), String> {
    private_dir(data_dir)?;
    // The persistent advisory lock serializes stale-socket cleanup and binding.
    // Removing the lock file on exit would allow a second lock inode to race it.
    let lock_path = data_dir.join("bridge.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err("Bridge lock path must be a regular file".into());
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&lock_path)
        .map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|_| {
        "A browser bridge is already running or its lock is unavailable".to_string()
    })?;
    let path = data_dir.join("bridge.sock");
    match fs::symlink_metadata(&path) {
        Ok(meta) => {
            if !meta.file_type().is_socket() {
                return Err("Existing bridge path is not a socket".into());
            }
            match UnixStream::connect(&path) {
                Ok(_) => return Err("A browser bridge is already running".into()),
                Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(&path).map_err(|e| e.to_string())?
                }
                Err(e) => return Err(format!("Cannot inspect existing bridge socket: {e}")),
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    let inode = fs::metadata(&path).map_err(|e| e.to_string())?.ino();
    let guard = SocketGuard {
        path,
        inode,
        _lock: lock,
    };
    fs::set_permissions(&guard.path, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok((listener, guard))
}
pub fn serve(data_dir: &Path, native: bool) -> Result<(), String> {
    let (listener, _socket_guard) = bind_socket(data_dir)?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let connected = Arc::new(AtomicBool::new(true));
    let (pending_reader, worker_connected) = (pending.clone(), connected.clone());
    thread::spawn(move || {
        let mut input = BufReader::new(io::stdin());
        loop {
            let frame = if native {
                let mut header = [0u8; 4];
                if input.read_exact(&mut header).is_err() {
                    break;
                }
                let length = u32::from_le_bytes(header) as usize;
                if length == 0 || length > MAX_FRAME {
                    break;
                }
                let mut bytes = vec![0; length];
                if input.read_exact(&mut bytes).is_err() {
                    break;
                }
                serde_json::from_slice::<Value>(&bytes).ok()
            } else {
                line(&mut input).ok().flatten()
            };
            let Some(frame) = frame else {
                break;
            };
            if let Some(id) = frame.get("id").and_then(Value::as_str)
                && let Some(sender) = pending_reader.lock().unwrap().remove(id)
            {
                let _ = sender.try_send(frame);
            }
        }
        worker_connected.store(false, Ordering::Release);
        pending_reader.lock().unwrap().clear();
    });
    let (output, outgoing) = mpsc::sync_channel::<Value>(MAX_CLIENTS);
    let (pending_writer, writer_connected) = (pending.clone(), connected.clone());
    thread::spawn(move || {
        worker_writer(
            &mut io::stdout(),
            outgoing,
            pending_writer,
            writer_connected,
            native,
        );
    });
    let clients = Arc::new(AtomicUsize::new(0));
    let pacing = Arc::new(Mutex::new(crate::pacing::Pacing::open(data_dir)?));
    while connected.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .map_err(|e| e.to_string())?;
                socket
                    .set_write_timeout(Some(Duration::from_secs(10)))
                    .map_err(|e| e.to_string())?;
                if clients.fetch_add(1, Ordering::AcqRel) >= MAX_CLIENTS {
                    clients.fetch_sub(1, Ordering::AcqRel);
                    let _ = socket.write_all(b"{\"error\":{\"code\":\"bridge_busy\"}}\n");
                    continue;
                }
                let (pending, output, clients) = (pending.clone(), output.clone(), clients.clone());
                let pacing = pacing.clone();
                thread::spawn(move || {
                    let result: Result<Value, String> = (|| {
                        let mut reader =
                            BufReader::new(socket.try_clone().map_err(|e| e.to_string())?);
                        let req = line(&mut reader)?.ok_or("Empty request")?;
                        if matches!(
                            req.get("op").and_then(Value::as_str),
                            Some("ping" | "status")
                        ) {
                            return Ok(
                                json!({"transport":if native {"native_messaging"} else {"jsonl"},"connected":true,"authentication":"unverified"}),
                            );
                        }
                        let mut pace = pacing.try_lock().map_err(|_| "request_busy: another browser request is in progress; run CLI commands sequentially")?;
                        let delay = pace.delay(crate::pacing::now_ms())?;
                        thread::sleep(delay);
                        pace.started(crate::pacing::now_ms())?;
                        let id = uuid::Uuid::new_v4().to_string();
                        let (tx, rx) = mpsc::sync_channel(1);
                        pending.lock().unwrap().insert(id.clone(), tx);
                        let sent = output
                            .try_send(json!({"id":id,"request":req}))
                            .map_err(|_| {
                                "Browser worker output queue is full or disconnected".to_string()
                            });
                        if let Err(e) = sent {
                            pending.lock().unwrap().remove(&id);
                            return Err(e);
                        }
                        let reply = rx.recv_timeout(Duration::from_secs(45));
                        pending.lock().unwrap().remove(&id);
                        pace.finished(reply.as_ref().ok(), crate::pacing::now_ms())?;
                        let reply =
                            reply.map_err(|_| "Browser worker timed out or disconnected")?;
                        if let Some(error) = reply.get("error") {
                            return Err(error.to_string());
                        }
                        reply
                            .get("result")
                            .cloned()
                            .ok_or("Worker response missing result".into())
                    })();
                    let response = match result {
                        Ok(value) => json!({"result":value}),
                        Err(error) => json!({"error":{"code":"bridge_error","message":error}}),
                    };
                    if let Ok(bytes) = serde_json::to_vec(&response) {
                        let _ = socket
                            .write_all(&bytes)
                            .and_then(|_| socket.write_all(b"\n"));
                    }
                    clients.fetch_sub(1, Ordering::AcqRel);
                });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err("Native host installation target must be a regular file".into());
    }
    let parent = path.parent().ok_or("Installation target has no parent")?;
    let temporary = parent.join(format!(".ricardo-install-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
pub fn install_native_host(data_dir: &Path, extension_id: &str) -> Result<Value, String> {
    if extension_id.len() != 32 || !extension_id.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
        return Err("Extension ID must contain exactly 32 letters a–p".into());
    }
    private_dir(data_dir)?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let data_dir = fs::canonicalize(data_dir).map_err(|e| e.to_string())?;
    let wrapper = data_dir.join("native-host");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    atomic_write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec {} --data-dir {} bridge native\n",
            quote(&executable.to_string_lossy()),
            quote(&data_dir.to_string_lossy())
        )
        .as_bytes(),
        0o700,
    )?;
    let manifest = json!({"name":"com.odcpw.ricardo_cli","description":"Ricardo CLI authenticated browser bridge","path":wrapper,"type":"stdio","allowed_origins":[format!("chrome-extension://{extension_id}/")]});
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(&home).join(".config"));
    let mut paths = Vec::new();
    for browser in ["google-chrome", "chromium"] {
        let dir = config.join(browser).join("NativeMessagingHosts");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("com.odcpw.ricardo_cli.json");
        atomic_write(
            &path,
            &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
            0o600,
        )?;
        paths.push(path);
    }
    Ok(json!({"installed":true,"manifests":paths,"extension_id":extension_id,"wrapper":wrapper}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdout_failure_disconnects_worker_and_releases_waiters() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        pending
            .lock()
            .unwrap()
            .insert("request".into(), response_tx);
        let (output_tx, output_rx) = mpsc::sync_channel(1);
        output_tx
            .send(json!({"id":"request","request":{"op":"fetch"}}))
            .unwrap();
        drop(output_tx);
        let connected = Arc::new(AtomicBool::new(true));
        worker_writer(
            &mut BrokenWriter,
            output_rx,
            pending.clone(),
            connected.clone(),
            false,
        );
        assert!(!connected.load(Ordering::Acquire));
        assert!(pending.lock().unwrap().is_empty());
        assert!(matches!(
            response_rx.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
    #[test]
    fn socket_lock_refuses_live_broker_and_cleans_up_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let (listener, guard) = bind_socket(dir.path()).unwrap();
        assert!(bind_socket(dir.path()).is_err());
        assert!(UnixStream::connect(dir.path().join("bridge.sock")).is_ok());
        drop(listener);
        drop(guard);
        assert!(!dir.path().join("bridge.sock").exists());
        assert!(bind_socket(dir.path()).is_ok());
    }
    #[test]
    fn stale_socket_is_recovered_and_non_socket_paths_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bridge.sock");
        let external = UnixListener::bind(&path).unwrap();
        assert!(bind_socket(dir.path()).is_err());
        assert!(UnixStream::connect(&path).is_ok());
        drop(external);
        let (listener, guard) = bind_socket(dir.path()).unwrap();
        assert!(UnixStream::connect(&path).is_ok());
        drop(listener);
        drop(guard);
        fs::write(&path, b"keep this file").unwrap();
        assert!(bind_socket(dir.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep this file");
    }
    #[test]
    fn installer_replaces_complete_files_and_refuses_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native-host");
        atomic_write(&path, b"first", 0o700).unwrap();
        atomic_write(&path, b"second complete file", 0o700).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second complete file");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(atomic_write(&link, b"do not replace", 0o700).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"second complete file");
    }
    #[test]
    fn frame_parser_rejects_truncation_and_oversize() {
        assert!(line(&mut &b"{\"x\":1}"[..]).is_err());
        let huge = vec![b' '; MAX_FRAME + 1];
        assert!(line(&mut &huge[..]).is_err());
        assert_eq!(
            line(&mut &b"{\"x\":1}\n"[..]).unwrap(),
            Some(json!({"x":1}))
        );
    }
    #[test]
    fn request_round_trip_and_worker_errors() {
        for (response, success) in [
            (json!({"result":{"status":200}}), true),
            (json!({"error":{"code":"blocked"}}), false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let listener = UnixListener::bind(dir.path().join("bridge.sock")).unwrap();
            let worker = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let message = line(&mut BufReader::new(stream.try_clone().unwrap()))
                    .unwrap()
                    .unwrap();
                assert_eq!(message["op"], "fetch");
                writeln!(stream, "{response}").unwrap();
            });
            let result = request(
                dir.path(),
                json!({"op":"fetch","path":"/de/s/test/"}),
                Duration::from_secs(1),
            );
            assert_eq!(result.is_ok(), success);
            worker.join().unwrap();
        }
    }
    #[test]
    fn unresponsive_socket_obeys_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("bridge.sock")).unwrap();
        let worker = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(150));
        });
        let start = std::time::Instant::now();
        assert!(request(dir.path(), json!({"op":"ping"}), Duration::from_millis(20)).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
        worker.join().unwrap();
    }
}
