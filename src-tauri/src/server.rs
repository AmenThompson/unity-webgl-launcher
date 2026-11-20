use std::{
    collections::VecDeque,
    future::Future,
    io::{self, SeekFrom},
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::{Request, State as HttpState},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    serve::Listener,
    Router,
};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::{
    fs::{self, File},
    io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
    task::JoinHandle,
};
use tokio_util::{
    io::ReaderStream,
    sync::{CancellationToken, WaitForCancellationFutureOwned},
};
use uuid::Uuid;

#[path = "compression.rs"]
pub(crate) mod compression;
pub use compression::Format;

const LOG_LIMIT: usize = 150;
const LOG_PATH_LIMIT: usize = 300;
const DIAGNOSTIC_LIMIT: usize = 32;
const RECENT_CHECK_LIMIT: usize = 32;
const RECENT_PATH_BYTES_LIMIT: usize = 32 * 1024;
pub(crate) const INDEX: &str = "index.html";

type EventLog = Arc<Mutex<VecDeque<String>>>;
type DiagnosticLog = Arc<Mutex<VecDeque<Diagnostic>>>;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    #[default]
    Auto,
    Br,
    Gzip,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub status: u16,
    pub path: String,
    pub message: &'static str,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub url: Option<String>,
    pub root: Option<String>,
    pub events: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub format: Option<Format>,
    pub fallback: Option<bool>,
}

#[derive(Default)]
pub struct Player {
    state: AsyncMutex<PlayerState>,
}

#[derive(Default)]
struct PlayerState {
    running: Option<Running>,
    last_events: Vec<String>,
    last_diagnostics: Vec<Diagnostic>,
}

struct Running {
    url: String,
    http: HttpProject,
    task: JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.http.cancel.cancel();
    }
}

#[derive(Clone)]
struct HttpProject {
    root: PathBuf,
    prefix: String,
    authority: HeaderValue,
    mode: Compression,
    detection: compression::Detection,
    threads: bool,
    events: EventLog,
    diagnostics: DiagnosticLog,
    cancel: CancellationToken,
}

impl PlayerState {
    fn snapshot(&self) -> Snapshot {
        match &self.running {
            Some(running) => Snapshot {
                url: Some(running.url.clone()),
                root: Some(display_root(&running.http.root)),
                events: read_events(&running.http.events),
                diagnostics: read_diagnostics(&running.http.diagnostics),
                format: Some(running.http.detection.format),
                fallback: running.http.detection.fallback,
            },
            None => Snapshot {
                events: self.last_events.clone(),
                diagnostics: self.last_diagnostics.clone(),
                ..Snapshot::default()
            },
        }
    }
}

impl Player {
    async fn start(
        &self,
        path: &Path,
        mode: Compression,
        threads: bool,
    ) -> Result<Snapshot, String> {
        self.start_scoped(path, mode, threads, None).await
    }

    async fn start_scoped(
        &self,
        path: &Path,
        mode: Compression,
        threads: bool,
        catalog_root: Option<&Path>,
    ) -> Result<Snapshot, String> {
        let mut state = self.state.lock().await;
        let (root, detection) = if let Some(catalog_root) = catalog_root {
            crate::catalog::preflight(path, catalog_root).await?
        } else {
            let root = project_root(path).await?;
            let detection = compression::detect(&root).await;
            (root, detection)
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|error| format!("无法启动本地服务：{error}"))?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let prefix = format!("/{}", Uuid::new_v4().simple());
        let url = format!("http://{address}{prefix}/");
        let http = HttpProject {
            root,
            prefix,
            authority: HeaderValue::from_str(&address.to_string())
                .map_err(|error| error.to_string())?,
            mode,
            detection,
            threads,
            events: EventLog::default(),
            diagnostics: DiagnosticLog::default(),
            cancel: CancellationToken::new(),
        };
        let app = Router::new().fallback(serve_asset).with_state(http.clone());
        let cancel = http.cancel.clone();
        let listener = StoppableListener {
            listener,
            cancel: cancel.clone(),
        };
        let fallback = match detection.fallback {
            Some(true) => "启用",
            Some(false) => "关闭",
            None => "无法确定",
        };
        push_event(
            &http.events,
            format!(
                "构建压缩：{}，解压回退：{fallback}",
                detection.format.label()
            ),
        );
        push_event(&http.events, "本地服务已启动".to_owned());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(cancel.cancelled_owned())
                .await;
        });
        // Prepare every fallible prerequisite before replacing the previous project.
        let old = state.running.replace(Running { url, http, task });
        state.last_events.clear();
        state.last_diagnostics.clear();
        if let Some(mut prior) = old {
            shutdown(&mut prior).await;
        }
        Ok(state.snapshot())
    }

    async fn stop(&self) -> Snapshot {
        let mut state = self.state.lock().await;
        if let Some(mut running) = state.running.take() {
            shutdown(&mut running).await;
            push_event(&running.http.events, "本地服务已停止".to_owned());
            state.last_events = read_events(&running.http.events);
            state.last_diagnostics = read_diagnostics(&running.http.diagnostics);
        }
        state.snapshot()
    }
}

async fn shutdown(running: &mut Running) {
    running.http.cancel.cancel();
    // The listener drops on cancellation and the socket wrapper interrupts
    // active connections, including transfers blocked on a slow client.
    let _ = (&mut running.task).await;
}

#[tauri::command]
pub async fn start_project(
    path: String,
    mode: Compression,
    threads: bool,
    catalog_root: Option<String>,
    state: State<'_, Player>,
) -> Result<Snapshot, String> {
    let path = Path::new(&path);
    if let Some(root) = catalog_root.as_deref() {
        state
            .start_scoped(path, mode, threads, Some(Path::new(root)))
            .await
    } else {
        state.start(path, mode, threads).await
    }
}

#[tauri::command]
pub async fn check_recent_projects(paths: Vec<String>) -> Vec<bool> {
    let mut valid = vec![false; paths.len()];
    for (output, input) in valid.iter_mut().zip(paths.iter().take(RECENT_CHECK_LIMIT)) {
        if input.len() <= RECENT_PATH_BYTES_LIMIT {
            *output = project_root(Path::new(input)).await.is_ok();
        }
    }
    valid
}

#[tauri::command]
pub async fn project_state(state: State<'_, Player>) -> Result<Snapshot, String> {
    Ok(state.state.lock().await.snapshot())
}

#[tauri::command]
pub async fn stop_project(state: State<'_, Player>) -> Result<Snapshot, String> {
    Ok(state.stop().await)
}

async fn project_root(selected: &Path) -> Result<PathBuf, String> {
    if selected.as_os_str().is_empty() {
        return Err("请选择 Unity WebGL 导出目录或 index.html".to_owned());
    }
    let canonical = fs::canonicalize(selected)
        .await
        .map_err(|error| format!("无法访问所选路径：{error}"))?;
    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|error| format!("无法读取所选路径：{error}"))?;
    let root = if metadata.is_dir() {
        canonical
    } else if metadata.is_file()
        && selected
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(INDEX))
    {
        let parent = selected
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::canonicalize(parent)
            .await
            .map_err(|error| format!("无法访问项目目录：{error}"))?
    } else {
        return Err("请选择 Unity WebGL 导出目录或 index.html".to_owned());
    };
    open_contained(&root, &root.join(INDEX))
        .await
        .map_err(|_| "所选目录缺少可读取且位于项目目录内的 index.html".to_owned())?;
    Ok(root)
}

pub(crate) fn display_root(root: &Path) -> String {
    let text = root.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            if rest.as_bytes().get(1) == Some(&b':') {
                return rest.to_owned();
            }
        }
    }
    text.into_owned()
}

fn push_event(log: &EventLog, line: String) {
    let mut events = log.lock().unwrap_or_else(|error| error.into_inner());
    if events.len() == LOG_LIMIT {
        events.pop_front();
    }
    events.push_back(line);
}

fn read_events(log: &EventLog) -> Vec<String> {
    log.lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .cloned()
        .collect()
}

fn read_diagnostics(log: &DiagnosticLog) -> Vec<Diagnostic> {
    log.lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .cloned()
        .collect()
}

fn record_diagnostic(log: &DiagnosticLog, status: StatusCode, relative: &str) {
    let missing = match status {
        StatusCode::NOT_FOUND => true,
        StatusCode::INTERNAL_SERVER_ERROR => false,
        _ => return,
    };
    let Some(path) = relative_asset(relative) else {
        return;
    };
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let lower = name.to_ascii_lowercase();
    let logical = [".br", ".gz", ".unityweb"]
        .iter()
        .find_map(|suffix| lower.strip_suffix(suffix))
        .unwrap_or(&lower);
    if logical.starts_with("favicon.") || logical.ends_with(".map") {
        return;
    }
    let is_streaming_asset = path.components().count() > 1
        && path.components().any(|component| {
            component
                .as_os_str()
                .eq_ignore_ascii_case(std::ffi::OsStr::new("StreamingAssets"))
        });
    let message = match (missing, logical) {
        (false, _) if is_streaming_asset || is_build_asset(logical) => {
            "本地服务读取资源失败。请检查文件的读取权限、占用状态和磁盘是否可用，再重试。"
        }
        (true, _) if is_streaming_asset => {
            "找不到或无法访问流式资源。请确认 StreamingAssets 完整，并核对项目请求的相对路径。"
        }
        (true, _) if is_build_asset(logical) => {
            "找不到或无法访问构建资源。请确认发布目录完整，并核对入口或加载脚本引用的路径及压缩后缀。"
        }
        _ => return,
    };
    let path = path
        .to_string_lossy()
        .replace('\\', "/")
        .chars()
        .take(LOG_PATH_LIMIT)
        .collect::<String>();
    let status = status.as_u16();
    let mut entries = log.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(index) = entries
        .iter()
        .position(|entry| entry.status == status && entry.path == path)
    {
        entries.remove(index);
    } else if entries.len() == DIAGNOSTIC_LIMIT {
        entries.pop_front();
    }
    entries.push_back(Diagnostic {
        status,
        path,
        message,
    });
}

fn is_build_asset(logical: &str) -> bool {
    [
        ".wasm",
        ".js",
        ".mjs",
        ".data",
        ".mem",
        ".wasm.code",
        ".wasm.framework",
        ".asm.code",
        ".asm.framework",
        ".asm.memory",
    ]
    .iter()
    .any(|suffix| logical.ends_with(suffix))
}

async fn serve_asset(HttpState(project): HttpState<HttpProject>, request: Request) -> Response {
    let (request, _) = request.into_parts();
    // Require the actual loopback authority and the unescaped random prefix.
    if request.headers.get(header::HOST) != Some(&project.authority)
        || request
            .uri
            .authority()
            .is_some_and(|value| value.as_str().as_bytes() != project.authority.as_bytes())
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(path) = request.uri.path().strip_prefix(&project.prefix) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let relative = if path.is_empty() {
        None
    } else if let Some(relative) = path.strip_prefix('/') {
        Some(relative)
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let method = &request.method;
    let response = if *method != Method::GET && *method != Method::HEAD {
        let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        response
    } else if let Some(relative) = relative {
        tokio::select! {
            biased;
            _ = project.cancel.cancelled() => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            response = file_response(&project, relative, method, &request.headers) => response,
        }
    } else {
        let mut response = StatusCode::TEMPORARY_REDIRECT.into_response();
        let location = format!("{}/", project.prefix);
        response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).expect("ASCII prefix"),
        );
        response
    };

    let mut response = response;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    if project.threads {
        headers.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            "cross-origin-embedder-policy",
            HeaderValue::from_static("require-corp"),
        );
    }
    if let Some(relative) = relative {
        record_diagnostic(&project.diagnostics, response.status(), relative);
    }
    let logged_method: String = method.as_str().chars().take(16).collect();
    let logged_path: String = path.chars().take(LOG_PATH_LIMIT).collect();
    push_event(
        &project.events,
        format!(
            "{} {} {}",
            response.status().as_u16(),
            logged_method,
            logged_path
        ),
    );
    response
}

fn relative_asset(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() {
        return Some(PathBuf::from(INDEX));
    }
    let mut path = PathBuf::new();
    for segment in raw.split('/') {
        let segment = percent_decode_str(segment).decode_utf8().ok()?;
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.ends_with(['.', ' '])
            || segment.chars().any(|c| {
                c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            })
            || windows_device(&segment)
        {
            return None;
        }
        path.push(segment.as_ref());
    }
    Some(path)
}

fn windows_device(segment: &str) -> bool {
    let base = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].into_iter().any(|prefix| {
        base.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

async fn open_contained(root: &Path, requested: &Path) -> io::Result<(File, u64)> {
    let target = fs::canonicalize(requested).await?;
    if !target.starts_with(root) || !fs::metadata(&target).await?.is_file() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let file = File::open(&target).await?;
    // Check the opened handle on Windows and Linux as well, so a concurrent
    // replacement with an external symlink cannot cause the body to escape.
    verify_handle(root, &file).await?;
    let metadata = file.metadata().await?;
    if !metadata.is_file() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok((file, metadata.len()))
}

#[cfg(windows)]
async fn verify_handle(root: &Path, file: &File) -> io::Result<()> {
    use std::{
        ffi::OsString,
        os::windows::{
            ffi::OsStringExt,
            io::{AsRawHandle, RawHandle},
        },
    };

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(file: RawHandle, path: *mut u16, size: u32, flags: u32)
            -> u32;
    }

    let mut path = vec![0u16; 512];
    loop {
        // The borrowed file keeps the handle live. The buffer holds `path.len()` UTF-16 units.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                path.as_mut_ptr(),
                path.len() as u32,
                0,
            )
        } as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length < path.len() {
            let opened = PathBuf::from(OsString::from_wide(&path[..length]));
            return if opened.starts_with(root) {
                Ok(())
            } else {
                Err(io::ErrorKind::PermissionDenied.into())
            };
        }
        path.resize(length + 1, 0);
    }
}

#[cfg(target_os = "linux")]
async fn verify_handle(root: &Path, file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let opened = fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).await?;
    if opened.starts_with(root) {
        Ok(())
    } else {
        Err(io::ErrorKind::PermissionDenied.into())
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
async fn verify_handle(_root: &Path, _file: &File) -> io::Result<()> {
    Ok(())
}

fn content_headers(path: &Path, mode: Compression) -> (HeaderValue, Option<&'static str>) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let lowercase = name.to_ascii_lowercase();
    let (logical, encoding) = if let Some(logical) = lowercase.strip_suffix(".br") {
        (logical, Some("br"))
    } else if let Some(logical) = lowercase.strip_suffix(".gz") {
        (logical, Some("gzip"))
    } else if let Some(logical) = lowercase.strip_suffix(".unityweb") {
        let encoding = match mode {
            Compression::Br => Some("br"),
            Compression::Gzip => Some("gzip"),
            Compression::Auto | Compression::Fallback => None,
        };
        (logical, encoding)
    } else {
        (lowercase.as_str(), None)
    };
    let mime = match Path::new(logical).extension().and_then(|ext| ext.to_str()) {
        Some("wasm") => "application/wasm",
        Some("js" | "mjs") => "application/javascript",
        Some("data" | "mem" | "symbols") => "application/octet-stream",
        _ => mime_guess::from_path(logical)
            .first_raw()
            .unwrap_or("application/octet-stream"),
    };
    (HeaderValue::from_static(mime), encoding)
}

async fn file_response(
    project: &HttpProject,
    relative: &str,
    method: &Method,
    request_headers: &HeaderMap,
) -> Response {
    let Some(relative) = relative_asset(relative) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (mut file, length) =
        match open_contained(&project.root, &project.root.join(&relative)).await {
            Ok(opened) => opened,
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };
    // There are no validators, so If-Range always falls back to the full response.
    let range = if request_headers.contains_key(header::IF_RANGE) {
        None
    } else {
        request_headers.get(header::RANGE).map(|value| {
            if request_headers.get_all(header::RANGE).iter().count() != 1 {
                return Err(());
            }
            parse_range(value.to_str().map_err(|_| ())?, length)
        })
    };
    let (status, start, count, content_range) = match range {
        Some(Ok((start, end))) => (
            StatusCode::PARTIAL_CONTENT,
            start,
            end - start + 1,
            Some(format!("bytes {start}-{end}/{length}")),
        ),
        Some(Err(())) => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            0,
            0,
            Some(format!("bytes */{length}")),
        ),
        None => (StatusCode::OK, 0, length, None),
    };
    let body = if *method == Method::HEAD || count == 0 {
        Body::empty()
    } else {
        if start != 0 && file.seek(SeekFrom::Start(start)).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Body::from_stream(ReaderStream::with_capacity(file.take(count), 64 * 1024))
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(count));
    if let Some(content_range) = content_range {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&content_range).expect("ASCII range"),
        );
    }
    if status != StatusCode::RANGE_NOT_SATISFIABLE {
        let (mime, coding) = content_headers(&relative, project.mode);
        headers.insert(header::CONTENT_TYPE, mime);
        if let Some(coding) = coding {
            headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(coding));
        }
    }
    response
}

fn parse_range(value: &str, length: u64) -> Result<(u64, u64), ()> {
    let (unit, value) = value.split_once('=').ok_or(())?;
    if length == 0 || !unit.eq_ignore_ascii_case("bytes") {
        return Err(());
    }
    let (start, end) = value.split_once('-').ok_or(())?;
    let number = |text: &str| {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(());
        }
        // Oversized valid decimal bounds saturate, allowing end and suffix clamping.
        Ok(text.bytes().fold(0u64, |value, digit| {
            value
                .saturating_mul(10)
                .saturating_add(u64::from(digit - b'0'))
        }))
    };
    if start.is_empty() {
        let suffix = number(end)?;
        return if suffix == 0 {
            Err(())
        } else {
            Ok((length.saturating_sub(suffix), length - 1))
        };
    }
    let start = number(start)?;
    let end = if end.is_empty() {
        length - 1
    } else {
        number(end)?.min(length - 1)
    };
    if start <= end {
        Ok((start, end))
    } else {
        Err(())
    }
}

struct StoppableListener {
    listener: TcpListener,
    cancel: CancellationToken,
}

impl Listener for StoppableListener {
    type Io = StoppableSocket;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (stream, address) = Listener::accept(&mut self.listener).await;
        let cancelled = Some(Box::pin(self.cancel.clone().cancelled_owned()));
        (StoppableSocket { stream, cancelled }, address)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

struct StoppableSocket {
    stream: TcpStream,
    cancelled: Option<Pin<Box<WaitForCancellationFutureOwned>>>,
}

impl StoppableSocket {
    fn check_cancelled(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if let Some(cancelled) = &mut self.cancelled {
            if cancelled.as_mut().poll(cx).is_pending() {
                return Ok(());
            }
            self.cancelled = None;
        }
        Err(io::ErrorKind::ConnectionAborted.into())
    }
}

impl AsyncRead for StoppableSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.check_cancelled(cx)?;
        Pin::new(&mut self.stream).poll_read(cx, data)
    }
}

impl AsyncWrite for StoppableSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.check_cancelled(cx)?;
        Pin::new(&mut self.stream).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check_cancelled(cx)?;
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{io::AsyncWriteExt, time::timeout};

    struct TempProject(PathBuf);

    impl TempProject {
        fn new() -> Self {
            let temp = std::env::temp_dir().join(format!("unity-server-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(temp.join("Build")).unwrap();
            std::fs::write(temp.join(INDEX), "project index").unwrap();
            Self(temp)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn url_parts(url: &str) -> (&str, &str) {
        let url = url.strip_prefix("http://").unwrap();
        let slash = url.find('/').unwrap();
        url.split_at(slash)
    }

    async fn request(authority: &str, method: &str, path: &str, extra: &str) -> (String, Vec<u8>) {
        request_with_host(authority, authority, method, path, extra).await
    }

    async fn request_with_host(
        address: &str,
        host: &str,
        method: &str,
        path: &str,
        extra: &str,
    ) -> (String, Vec<u8>) {
        let mut socket = TcpStream::connect(address).await.unwrap();
        socket
            .write_all(
                format!(
                    "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{extra}\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut raw = Vec::new();
        timeout(Duration::from_secs(5), socket.read_to_end(&mut raw))
            .await
            .unwrap()
            .unwrap();
        let divider = raw.windows(4).position(|item| item == b"\r\n\r\n").unwrap();
        (
            String::from_utf8(raw[..divider].to_vec())
                .unwrap()
                .to_ascii_lowercase(),
            raw[divider + 4..].to_vec(),
        )
    }

    #[test]
    fn range_parser_bounds_and_suffixes() {
        assert_eq!(parse_range("bytes=2-4", 10), Ok((2, 4)));
        assert_eq!(parse_range("bytes=9-", 10), Ok((9, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Ok((7, 9)));
        assert_eq!(
            parse_range("bytes=1-99999999999999999999999", 10),
            Ok((1, 9))
        );
        for range in [
            "bytes=-0",
            "bytes=10-",
            "bytes=3-2",
            "bytes=+1-2",
            "bytes=0-2,4-6",
            "items=0-2",
        ] {
            assert_eq!(parse_range(range, 10), Err(()), "{range}");
        }
        assert_eq!(parse_range("bytes=0-0", 0), Err(()));
    }

    #[test]
    fn relative_paths_reject_escapes_and_windows_special_names() {
        for raw in [
            "../secret",
            "%2e%2e/secret",
            "a%2fb",
            "a%5Cb",
            "C%3Afile",
            "//file",
            "a.. ./file",
            "NUL.txt",
            "COM%C2%B2.html",
            "file%3Astream",
            "file%00.js",
        ] {
            assert_eq!(relative_asset(raw), None, "{raw}");
        }
        assert_eq!(relative_asset(""), Some(PathBuf::from(INDEX)));
        assert_eq!(
            relative_asset("Build/%E6%B5%8B%20test.js"),
            Some(PathBuf::from("Build").join("测 test.js"))
        );
    }

    #[test]
    fn unity_compression_mime_and_content_coding() {
        for (name, mode, mime, coding) in [
            (
                "game.wasm.br",
                Compression::Auto,
                "application/wasm",
                Some("br"),
            ),
            (
                "game.data.gz",
                Compression::Auto,
                "application/octet-stream",
                Some("gzip"),
            ),
            (
                "game.framework.js.gz",
                Compression::Br,
                "application/javascript",
                Some("gzip"),
            ),
            (
                "game.wasm.unityweb",
                Compression::Fallback,
                "application/wasm",
                None,
            ),
            (
                "game.data.unityweb",
                Compression::Auto,
                "application/octet-stream",
                None,
            ),
            (
                "game.wasm.unityweb",
                Compression::Br,
                "application/wasm",
                Some("br"),
            ),
            (
                "game.framework.js.unityweb",
                Compression::Gzip,
                "application/javascript",
                Some("gzip"),
            ),
            (
                "loader.js",
                Compression::Gzip,
                "application/javascript",
                None,
            ),
        ] {
            let (actual_mime, actual_coding) = content_headers(Path::new(name), mode);
            assert_eq!(actual_mime, mime, "{name}");
            assert_eq!(actual_coding, coding, "{name}");
        }
    }

    #[tokio::test]
    async fn auto_reports_referenced_artifacts_and_preserves_fallback_response_bytes() {
        let ws = TempProject::new();
        let player = Player::default();
        let cases: [(&str, &[u8], Format, bool, Option<&str>); 6] = [
            (
                "game.wasm",
                b"\0asm\x01\0\0\0",
                Format::Disabled,
                false,
                None,
            ),
            (
                "game.wasm.br",
                b"compressed",
                Format::Brotli,
                false,
                Some("br"),
            ),
            (
                "game.wasm.gz",
                b"compressed",
                Format::Gzip,
                false,
                Some("gzip"),
            ),
            (
                "game.wasm.unityweb",
                &[
                    31, 139, 8, 0, 0, 0, 0, 0, 0, 10, 99, 72, 44, 206, 101, 100, 96, 96, 0, 0, 206,
                    51, 75, 28, 8, 0, 0, 0,
                ],
                Format::Gzip,
                true,
                None,
            ),
            (
                "game.wasm.unityweb",
                &[139, 3, 128, 0, 97, 115, 109, 1, 0, 0, 0, 3],
                Format::Brotli,
                true,
                None,
            ),
            (
                "game.wasm.unityweb",
                b"invalid bytes",
                Format::Unknown,
                true,
                None,
            ),
        ];
        std::fs::write(ws.0.join("Build/old.data.br"), b"unused").unwrap();
        for (artifact, bytes, format, fallback, expected_encoding) in cases {
            std::fs::write(ws.0.join("Build").join(artifact), bytes).unwrap();
            std::fs::write(
                ws.0.join(INDEX),
                format!(
                    r#"
                <script>
                  const buildUrl = 'Build';
                  // codeUrl: buildUrl + '/old.data.br'
                  const config = {{ codeUrl: buildUrl + '/{artifact}?version=1' }};
                </script>"#
                ),
            )
            .unwrap();
            let snapshot = player.start(&ws.0, Compression::Auto, false).await.unwrap();
            assert_eq!(snapshot.format, Some(format));
            assert_eq!(snapshot.fallback, Some(fallback));
            let (authority, base) = url_parts(snapshot.url.as_deref().unwrap());
            let (head, body) =
                request(authority, "GET", &format!("{base}Build/{artifact}"), "").await;
            assert!(head.starts_with("http/1.1 200"), "{head}");
            assert!(head.contains("content-type: application/wasm"));
            assert_eq!(
                head.lines()
                    .find_map(|line| line.strip_prefix("content-encoding: ")),
                expected_encoding
            );
            assert_eq!(body, bytes);
        }
        let stopped = player.stop().await;
        assert_eq!(stopped.format, None);
        assert_eq!(stopped.fallback, None);
    }

    #[tokio::test]
    async fn legacy_manifest_references_and_mixed_payloads() {
        let ws = TempProject::new();
        std::fs::write(
            ws.0.join(INDEX),
            r#"<script>UnityLoader.instantiate('game','Build/game.json')</script>"#,
        )
        .unwrap();
        std::fs::write(
            ws.0.join("Build/game.json"),
            r#"{"wasmCodeUrl":"new.wasm.code.unityweb","dataUrl":"new.data.br"}"#,
        )
        .unwrap();
        std::fs::write(ws.0.join("Build/new.wasm.code.unityweb"), [31, 139, 8]).unwrap();
        std::fs::write(ws.0.join("Build/new.data.br"), b"example").unwrap();
        let player = Player::default();
        let snapshot = player.start(&ws.0, Compression::Auto, false).await.unwrap();
        assert_eq!(snapshot.format, Some(Format::Mixed));
        assert_eq!(snapshot.fallback, None);
        player.stop().await;
    }

    #[tokio::test]
    async fn http_head_range_headers_and_prefix() {
        let ws = TempProject::new();
        std::fs::write(ws.0.join("Build/测 test.wasm.br"), b"123456789").unwrap();
        let player = Player::default();
        let started = player
            .start(&ws.0.join(INDEX), Compression::Auto, true)
            .await
            .unwrap();
        let (authority, base) = url_parts(started.url.as_deref().unwrap());
        let asset = format!("{base}Build/%E6%B5%8B%20test.wasm.br");
        let (headers, body) = request(authority, "GET", base, "").await;
        assert!(headers.starts_with("http/1.1 200"), "{headers}");
        assert_eq!(body, b"project index");
        assert!(headers.contains("cross-origin-opener-policy: same-origin"));
        assert!(headers.contains("cross-origin-embedder-policy: require-corp"));
        let (headers, body) = request(authority, "GET", &asset, "Range: bytes=2-4\r\n").await;
        assert!(headers.starts_with("http/1.1 206"), "{headers}");
        assert!(headers.contains("content-range: bytes 2-4/9"));
        assert!(headers.contains("content-length: 3"));
        assert!(headers.contains("content-type: application/wasm"));
        assert!(headers.contains("content-encoding: br"));
        assert_eq!(body, b"345");
        let (headers, body) = request(authority, "HEAD", &asset, "").await;
        assert!(headers.starts_with("http/1.1 200"), "{headers}");
        assert!(headers.contains("content-length: 9"));
        assert!(body.is_empty());
        let (headers, _) = request(authority, "GET", &asset, "Range: bytes=9-\r\n").await;
        assert!(headers.starts_with("http/1.1 416"), "{headers}");
        assert!(headers.contains("content-range: bytes */9"));
        for path in [
            "/index.html".to_owned(),
            format!("{base}%2e%2e/index.html"),
            format!("{base}Build/%5csecret"),
        ] {
            let (headers, body) = request(authority, "GET", &path, "").await;
            assert!(headers.starts_with("http/1.1 404"), "{headers}");
            assert!(body.is_empty());
        }
        let (headers, _) = request_with_host(authority, "unrelated.example", "GET", base, "").await;
        assert!(headers.starts_with("http/1.1 404"));
        assert!(player
            .state
            .lock()
            .await
            .snapshot()
            .events
            .iter()
            .any(|line| line.starts_with("206 GET")));
        player.stop().await;
    }

    #[tokio::test]
    async fn invalid_replacement_preserves_server_and_stop_closes_active_transfer() {
        let ws = TempProject::new();
        let stream = std::fs::File::create(ws.0.join("large.data")).unwrap();
        stream.set_len(32 * 1024 * 1024).unwrap();
        drop(stream);
        let player = Player::default();
        let first = player.start(&ws.0, Compression::Auto, false).await.unwrap();
        assert!(player
            .start(&ws.0.join("Build"), Compression::Br, true)
            .await
            .is_err());
        assert_eq!(player.state.lock().await.snapshot().url, first.url);
        let (authority, base) = url_parts(first.url.as_deref().unwrap());
        let (headers, _) = request(authority, "GET", base, "").await;
        assert!(!headers.contains("cross-origin-opener-policy"));
        let mut socket = TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(
                format!("GET {base}large.data HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes(),
            )
            .await
            .unwrap();
        let mut first_byte = [0];
        timeout(Duration::from_secs(5), socket.read_exact(&mut first_byte))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_byte, [b'H']);
        let stopped = timeout(Duration::from_secs(5), player.stop())
            .await
            .unwrap();
        assert!(stopped.url.is_none() && stopped.root.is_none());
        assert!(TcpStream::connect(authority).await.is_err());
        let second = player
            .start(&ws.0, Compression::Fallback, false)
            .await
            .unwrap();
        assert_ne!(url_parts(second.url.as_deref().unwrap()).1, base);
        player.stop().await;
    }

    #[tokio::test]
    async fn catalog_replacement_preserves_running_service_when_resource_disappears() {
        let current = TempProject::new();
        let catalog = TempProject::new();
        let candidate = catalog.0.join("候选");
        std::fs::create_dir_all(candidate.join("Build")).unwrap();
        std::fs::write(
            candidate.join(INDEX),
            "<script>createUnityInstance(canvas, {codeUrl: 'Build/game.wasm'});</script>",
        )
        .unwrap();
        let asset = candidate.join("Build/game.wasm");
        std::fs::write(&asset, b"wasm").unwrap();
        let catalog_path = fs::canonicalize(&catalog.0).await.unwrap();
        let candidate_entry = fs::canonicalize(candidate.join(INDEX)).await.unwrap();
        assert!(crate::catalog::preflight(&candidate_entry, &catalog_path)
            .await
            .is_ok());
        let player = Player::default();
        let first = player
            .start(&current.0, Compression::Auto, false)
            .await
            .unwrap();
        std::fs::remove_file(&asset).unwrap();
        let result = player
            .start_scoped(
                &candidate_entry,
                Compression::Auto,
                false,
                Some(&catalog_path),
            )
            .await;
        assert!(result.unwrap_err().contains("game.wasm"));
        assert_eq!(player.state.lock().await.snapshot().url, first.url);
        let (authority, base) = url_parts(first.url.as_deref().unwrap());
        let (_, body) = request(authority, "GET", base, "").await;
        assert_eq!(body, b"project index");
        std::fs::write(&asset, b"wasm").unwrap();
        let replaced = player
            .start_scoped(
                &candidate_entry,
                Compression::Auto,
                false,
                Some(&catalog_path),
            )
            .await
            .unwrap();
        assert_ne!(replaced.url, first.url);
        assert!(TcpStream::connect(authority).await.is_err());
        player.stop().await;
    }

    #[cfg(any(windows, unix))]
    #[tokio::test]
    async fn rejects_outside_links_and_allows_inside_links() {
        let ws = TempProject::new();
        let outside = TempProject::new();
        #[cfg(unix)]
        use std::os::unix::fs::{symlink as symlink_dir, symlink as symlink_file};
        #[cfg(windows)]
        use std::os::windows::fs::{symlink_dir, symlink_file};
        if let Err(error) = symlink_dir(&outside.0, ws.0.join("external")) {
            assert!(
                cfg!(windows) && matches!(error.raw_os_error(), Some(1314 | 5)),
                "{error}"
            );
            // A directory junction exercises the same containment check without
            // requiring Windows developer mode or symlink privilege.
            #[cfg(windows)]
            {
                let status = std::process::Command::new("cmd.exe")
                    .args(["/C", "mklink", "/J"])
                    .arg(ws.0.join("external"))
                    .arg(&outside.0)
                    .output()
                    .unwrap();
                assert!(
                    status.status.success(),
                    "{}",
                    String::from_utf8_lossy(&status.stderr)
                );
            }
        }
        let root = project_root(&ws.0).await.unwrap();
        assert!(open_contained(&root, &root.join("external").join(INDEX))
            .await
            .is_err());
        let inside = ws.0.join("inside.html");
        if symlink_file(ws.0.join(INDEX), &inside).is_ok() {
            assert!(open_contained(&root, &inside).await.is_ok());
        }
        #[cfg(windows)]
        let _ = std::fs::remove_dir(ws.0.join("external"));
    }

    #[tokio::test]
    async fn recent_project_checks_preserve_order_and_cap_filesystem_work() {
        let ws = TempProject::new();
        let directory = ws.0.to_string_lossy().into_owned();
        let entry = ws.0.join(INDEX).to_string_lossy().into_owned();
        let missing = ws.0.join("missing").to_string_lossy().into_owned();
        let no_entry = ws.0.join("Build").to_string_lossy().into_owned();
        let other = ws.0.join("Build/game.wasm");
        std::fs::write(&other, b"wasm").unwrap();
        let paths = vec![
            directory.clone(),
            missing,
            entry,
            no_entry,
            other.to_string_lossy().into_owned(),
            String::new(),
            "x".repeat(RECENT_PATH_BYTES_LIMIT + 1),
        ];
        assert_eq!(
            check_recent_projects(paths).await,
            vec![true, false, true, false, false, false, false]
        );
        let mut expected = vec![true; RECENT_CHECK_LIMIT];
        expected.push(false);
        assert_eq!(
            check_recent_projects(vec![directory; RECENT_CHECK_LIMIT + 1]).await,
            expected
        );
    }

    #[tokio::test]
    async fn critical_http_failures_update_only_their_sessions_diagnostics() {
        let ws = TempProject::new();
        let player = Player::default();
        let first = player.start(&ws.0, Compression::Auto, false).await.unwrap();
        let (authority, base) = url_parts(first.url.as_deref().unwrap());
        for name in [
            "Build/%E6%B5%8B%20game.wasm.br?version=one",
            "Build/%E6%B5%8B%20game.wasm.br?version=two",
            "Build/game.framework.js.gz",
            "Build/game.data.unityweb",
            "StreamingAssets/bundles/terrain.bundle",
            "TemplateData/favicon.ico",
            "Build/game.js.map",
            "TemplateData/style.css",
            "%2e%2e/outsider.wasm",
        ] {
            let (headers, _) = request(authority, "GET", &format!("{base}{name}"), "").await;
            assert!(headers.starts_with("http/1.1 404"), "{name}: {headers}");
        }
        let snapshot = player.state.lock().await.snapshot();
        let paths = snapshot
            .diagnostics
            .iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "Build/测 game.wasm.br",
                "Build/game.framework.js.gz",
                "Build/game.data.unityweb",
                "StreamingAssets/bundles/terrain.bundle"
            ]
        );
        assert!(snapshot.diagnostics.iter().all(|item| item.status == 404));
        assert!(snapshot.diagnostics[0].message.contains("压缩后缀"));
        assert!(snapshot.diagnostics[3].message.contains("StreamingAssets"));
        assert_eq!(player.stop().await.diagnostics, snapshot.diagnostics);
        let next = player.start(&ws.0, Compression::Auto, false).await.unwrap();
        assert!(next.diagnostics.is_empty());
        player.stop().await;
    }

    #[test]
    fn diagnostic_log_is_bounded_and_deduplicated_by_status_and_path() {
        let log = DiagnosticLog::default();
        for i in 0..DIAGNOSTIC_LIMIT + 3 {
            record_diagnostic(&log, StatusCode::NOT_FOUND, &format!("Build/{i}.wasm"));
        }
        record_diagnostic(&log, StatusCode::INTERNAL_SERVER_ERROR, "Build/4.wasm");
        record_diagnostic(&log, StatusCode::INTERNAL_SERVER_ERROR, "Build/4.wasm");
        record_diagnostic(
            &log,
            StatusCode::INTERNAL_SERVER_ERROR,
            "StreamingAssets/favicon.png",
        );
        record_diagnostic(&log, StatusCode::SERVICE_UNAVAILABLE, "Build/stopping.wasm");
        let diagnostics = read_diagnostics(&log);
        assert_eq!(diagnostics.len(), DIAGNOSTIC_LIMIT);
        assert_eq!(diagnostics.first().unwrap().path, "Build/4.wasm");
        let error = diagnostics.last().unwrap();
        assert_eq!((error.status, error.path.as_str()), (500, "Build/4.wasm"));
        assert!(error.message.contains("读取权限"));
    }

    #[test]
    fn request_log_is_bounded_and_keeps_order() {
        let log = EventLog::default();
        for i in 0..LOG_LIMIT + 5 {
            push_event(&log, i.to_string());
        }
        let events = read_events(&log);
        assert_eq!(events.len(), LOG_LIMIT);
        assert_eq!(events.first().unwrap(), "5");
        assert_eq!(events.last().unwrap(), &(LOG_LIMIT + 4).to_string());
    }
}
