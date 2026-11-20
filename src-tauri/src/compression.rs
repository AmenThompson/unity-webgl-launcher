//! Detect the published build's compression without modifying its files.

use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use serde::Serialize;
use tokio::{fs, io::AsyncReadExt};

use super::{open_contained, relative_asset, INDEX};

const TEXT_LIMIT: u64 = 1024 * 1024;
const PROBE_INPUT: u64 = 128 * 1024;
const MAX_PAYLOADS: usize = 24;
const MAX_DIRECTORY_ENTRIES: usize = 512;
const MAX_REFERENCED_SOURCES: usize = 8;
const BROTLI_MARKER: &[u8] = b"UnityWeb Compressed Content (brotli)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Brotli,
    Gzip,
    Disabled,
    Mixed,
    #[default]
    Unknown,
}

impl Format {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Brotli => "Brotli",
            Self::Gzip => "Gzip",
            Self::Disabled => "未压缩",
            Self::Mixed => "混合",
            Self::Unknown => "无法确定",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Detection {
    pub format: Format,
    pub fallback: Option<bool>,
}

#[derive(Default)]
pub(crate) struct ProjectInspection {
    pub detection: Detection,
    pub is_unity: bool,
    pub issues: Vec<String>,
}

struct References {
    payloads: BTreeSet<PathBuf>,
    sources: BTreeSet<PathBuf>,
    loader_stems: BTreeSet<String>,
    missing: BTreeSet<PathBuf>,
    saw_payload: bool,
    saw_loader: bool,
}

impl References {
    fn new() -> Self {
        Self {
            payloads: BTreeSet::new(),
            sources: BTreeSet::new(),
            loader_stems: BTreeSet::new(),
            missing: BTreeSet::new(),
            saw_payload: false,
            saw_loader: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadKind {
    Wasm,
    Data,
    Script,
    Memory,
}

#[derive(Clone, Copy)]
struct Payload {
    kind: PayloadKind,
    format: Format,
    fallback: bool,
}

fn payload(name: &str) -> Option<Payload> {
    let name = name.to_ascii_lowercase();
    let (name, format, fallback) = if let Some(name) = name.strip_suffix(".br") {
        (name, Format::Brotli, false)
    } else if let Some(name) = name.strip_suffix(".gz") {
        (name, Format::Gzip, false)
    } else if let Some(name) = name.strip_suffix(".unityweb") {
        // Existing Unity loaders rely on their embedded decompressor for this extension.
        (name, Format::Unknown, true)
    } else {
        (name.as_str(), Format::Disabled, false)
    };
    let kind = if name.ends_with(".wasm") || name.ends_with(".wasm.code") {
        PayloadKind::Wasm
    } else if name.ends_with(".data") {
        PayloadKind::Data
    } else if name.ends_with(".framework.js")
        || name.ends_with(".asm.js")
        || name.ends_with(".asm.code")
        || name.ends_with(".wasm.framework")
        || name.ends_with(".asm.framework")
    {
        PayloadKind::Script
    } else if name.ends_with(".mem") || name.ends_with(".asm.memory") {
        PayloadKind::Memory
    } else {
        return None;
    };
    Some(Payload {
        kind,
        format,
        fallback,
    })
}

/// Extract common template string literals without evaluating JavaScript.
struct Literals<'a>(&'a str);

impl<'a> Iterator for Literals<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let at = self.0.find(['\'', '"', '`', '/', '<'])?;
            let prefix = &self.0[..at];
            self.0 = &self.0[at..];
            if let Some(after) = self.0.strip_prefix("<!--") {
                self.0 = after.split_once("-->").map_or("", |(_, rest)| rest);
                continue;
            }
            if let Some(after) = self.0.strip_prefix("//") {
                self.0 = after.split_once('\n').map_or("", |(_, rest)| rest);
                continue;
            }
            if let Some(after) = self.0.strip_prefix("/*") {
                self.0 = after.split_once("*/").map_or("", |(_, rest)| rest);
                continue;
            }
            let quote = self.0.as_bytes()[0];
            self.0 = &self.0[1..];
            if !matches!(quote, b'\'' | b'"' | b'`') {
                continue;
            }
            let mut escaped = false;
            let end = self.0.bytes().position(|byte| {
                if escaped {
                    escaped = false;
                    false
                } else if byte == b'\\' {
                    escaped = true;
                    false
                } else {
                    byte == quote || (byte == b'\n' && quote != b'`')
                }
            })?;
            let (value, suffix) = self.0.split_at(end);
            self.0 = &suffix[1..];
            if suffix.as_bytes()[0] == quote && value.len() <= 4096 {
                return Some((prefix, value));
            }
        }
    }
}

fn local_paths(base: &Path, value: &str) -> Vec<PathBuf> {
    let value = value.split(['?', '#']).next().unwrap_or_default().trim();
    if value.is_empty() || value.contains(':') || value.starts_with("//") {
        return Vec::new();
    }
    // Unity's standard template uses buildUrl + "/file". Some templates use
    // `${buildUrl}/file`; the name is static even when the directory is computed.
    let value = if value.starts_with("${") {
        let Some((_, suffix)) = value.split_once('}') else {
            return Vec::new();
        };
        suffix
    } else {
        value
    };
    let trailing = value.trim_start_matches("./").trim_start_matches('/');
    let Some(relative) = relative_asset(trailing) else {
        return Vec::new();
    };
    if trailing.is_empty() {
        return Vec::new();
    }
    let in_base = base.join(&relative);
    if relative.components().count() == 1 && base != Path::new("Build") {
        // A relative bare name is normal in legacy Build/foo.json. Also try the
        // standard Build directory for a concatenated path from index.html.
        if value.starts_with('/') {
            return vec![PathBuf::from("Build").join(relative), in_base];
        }
        return vec![in_base, PathBuf::from("Build").join(relative)];
    }
    vec![in_base]
}

async fn read_text(root: &Path, relative: &Path) -> Option<String> {
    let (file, _) = open_contained(root, &root.join(relative)).await.ok()?;
    let mut bytes = Vec::new();
    file.take(TEXT_LIMIT).read_to_end(&mut bytes).await.ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn name(path: &Path) -> &str {
    path.file_name()
        .and_then(|part| part.to_str())
        .unwrap_or_default()
}

fn loader_name(path: &Path) -> bool {
    name(path).to_ascii_lowercase().ends_with(".loader.js")
}

async fn scan_source(root: &Path, base: &Path, text: &str, refs: &mut References, required: bool) {
    let legacy = text.contains("UnityLoader.instantiate");
    let build_directory = Literals(text).find_map(|(prefix, value)| {
        if prefix.trim_end().strip_suffix('=').is_some_and(|lhs| {
            lhs.trim_end()
                .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                == Some("buildUrl")
        }) {
            let directory = value.trim_start_matches("./").trim_end_matches('/');
            if directory.is_empty() || directory == "." {
                Some(PathBuf::from(""))
            } else {
                relative_asset(directory)
            }
        } else {
            None
        }
    });
    let mut inspected = BTreeSet::new();
    for (prefix, literal) in Literals(text) {
        let uses_build_directory = literal.starts_with("${buildUrl}")
            || prefix.trim_end().strip_suffix('+').is_some_and(|lhs| {
                lhs.trim_end()
                    .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    == Some("buildUrl")
            });
        let paths =
            if let Some(directory) = build_directory.as_deref().filter(|_| uses_build_directory) {
                let relative = literal.strip_prefix("${buildUrl}").unwrap_or(literal);
                let relative = relative
                    .split(['?', '#'])
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches('/');
                if relative.is_empty() {
                    continue;
                }
                let Some(relative) = relative_asset(relative) else {
                    continue;
                };
                vec![base.join(directory).join(relative)]
            } else {
                local_paths(base, literal)
            };
        let Some(first) = paths.first() else { continue };
        let lower = name(first).to_ascii_lowercase();
        let is_payload = payload(&lower).is_some();
        let is_loader = loader_name(first) || lower == "unityloader.js";
        let is_manifest = legacy && lower.ends_with(".json");
        refs.saw_payload |= is_payload;
        refs.saw_loader |= is_loader;
        if !(is_payload || is_loader || is_manifest)
            || inspected.contains(first)
            || inspected.len() == MAX_PAYLOADS + MAX_REFERENCED_SOURCES
        {
            continue;
        }
        let requested = first.to_owned();
        inspected.insert(requested.clone());
        let mut found = false;
        for path in paths {
            if open_contained(root, &root.join(&path)).await.is_err() {
                continue;
            }
            if let Some(stem) = lower.strip_suffix(".loader.js") {
                refs.loader_stems.insert(stem.to_owned());
            }
            if is_payload {
                refs.payloads.insert(path);
            } else if lower != "unityloader.js" && refs.sources.len() < MAX_REFERENCED_SOURCES {
                refs.sources.insert(path);
            }
            found = true;
            break;
        }
        if !found && required && refs.missing.len() < MAX_PAYLOADS {
            refs.missing.insert(requested);
        }
    }
}

async fn directory_candidates(root: &Path) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    for directory in [Path::new("Build"), Path::new("")] {
        let Ok(canonical) = fs::canonicalize(root.join(directory)).await else {
            continue;
        };
        if !canonical.starts_with(root) {
            continue;
        }
        let Ok(mut entries) = fs::read_dir(canonical).await else {
            continue;
        };
        for _ in 0..MAX_DIRECTORY_ENTRIES {
            let Ok(Some(entry)) = entries.next_entry().await else {
                break;
            };
            let relative = directory.join(entry.file_name());
            if payload(name(&relative)).is_some() {
                paths.insert(relative);
            }
        }
    }
    paths
}

pub(super) async fn detect(root: &Path) -> Detection {
    inspect(root).await.detection
}

pub(crate) async fn inspect(root: &Path) -> ProjectInspection {
    let Some(index) = read_text(root, Path::new(INDEX)).await else {
        return ProjectInspection::default();
    };
    let mut refs = References::new();
    scan_source(root, Path::new(""), &index, &mut refs, true).await;
    let is_unity = index.contains("createUnityInstance")
        || index.contains("UnityLoader.instantiate")
        || refs.saw_loader && refs.saw_payload;
    let sources = std::mem::take(&mut refs.sources);
    for path in sources {
        let manifest = name(&path).to_ascii_lowercase().ends_with(".json");
        if let Some(text) = read_text(root, &path).await {
            scan_source(
                root,
                path.parent().unwrap_or_else(|| Path::new("")),
                &text,
                &mut refs,
                manifest,
            )
            .await;
        } else if manifest {
            refs.missing.insert(path);
        }
    }

    let mut found = refs.payloads;
    // References outrank unrelated legacy artifacts found in the directory.
    if found.is_empty() && !refs.saw_payload {
        found = directory_candidates(root).await;
        if !refs.loader_stems.is_empty() {
            let matching: BTreeSet<_> = found
                .iter()
                .filter(|path| {
                    let lower = name(path).to_ascii_lowercase();
                    refs.loader_stems.iter().any(|stem| {
                        lower
                            .strip_prefix(stem)
                            .is_some_and(|suffix| suffix.starts_with('.'))
                    })
                })
                .cloned()
                .collect();
            if !matching.is_empty() {
                found = matching;
            }
        }
    }

    let mut issues: Vec<_> = refs
        .missing
        .into_iter()
        .map(|path| format!("缺少或无法读取构建资源：{}", path.display()))
        .collect();
    if is_unity && issues.is_empty() && found.is_empty() {
        issues.push("未找到 Unity WebGL 构建资源".to_owned());
    }
    ProjectInspection {
        is_unity,
        issues,
        detection: detect_files(root, found).await,
    }
}

async fn detect_files(root: &Path, found: BTreeSet<PathBuf>) -> Detection {
    let mut format = None;
    let mut fallback = None;
    let mut inconsistent_fallback = false;
    let mut unknown = false;
    for path in found.into_iter().take(MAX_PAYLOADS) {
        let Some(spec) = payload(name(&path)) else {
            continue;
        };
        let Ok((file, _)) = open_contained(root, &root.join(&path)).await else {
            continue;
        };
        let detected = if spec.fallback {
            probe_unityweb(file, spec.kind).await
        } else {
            spec.format
        };
        if detected == Format::Unknown {
            unknown = true;
        } else if let Some(previous) = format {
            if previous != detected {
                format = Some(Format::Mixed);
            }
        } else {
            format = Some(detected);
        }
        if let Some(previous) = fallback {
            inconsistent_fallback |= previous != spec.fallback;
        } else {
            fallback = Some(spec.fallback);
        }
    }
    let format = match format {
        Some(Format::Mixed) => Format::Mixed,
        Some(detected) if !unknown => detected,
        _ => Format::Unknown,
    };
    Detection {
        format,
        fallback: if inconsistent_fallback {
            None
        } else {
            fallback
        },
    }
}

async fn probe_unityweb(file: fs::File, kind: PayloadKind) -> Format {
    let mut prefix = Vec::new();
    if file
        .take(PROBE_INPUT)
        .read_to_end(&mut prefix)
        .await
        .is_err()
    {
        return Format::Unknown;
    }
    if prefix.starts_with(&[0x1f, 0x8b, 0x08]) {
        return Format::Gzip;
    }
    if brotli_unity_marker(&prefix) {
        return Format::Brotli;
    }
    if payload_signature(&prefix, kind) {
        return Format::Disabled;
    }
    // RFC 7932 Brotli has no fixed magic. Demand a Unity payload signature
    // after decoding, so a random decodable stream isn't reported as a match.
    tokio::task::spawn_blocking(move || {
        let mut decoder = brotli_decompressor::Decompressor::new(Cursor::new(prefix), 4096);
        let mut decoded = [0u8; 256];
        let mut size = 0;
        loop {
            match decoder.read(&mut decoded[size..]) {
                Ok(0) | Err(_) => return Format::Unknown,
                Ok(count) => size += count,
            }
            if payload_signature(&decoded[..size], kind) {
                return Format::Brotli;
            }
            if size == decoded.len() {
                return Format::Unknown;
            }
        }
    })
    .await
    .unwrap_or(Format::Unknown)
}

fn payload_signature(bytes: &[u8], kind: PayloadKind) -> bool {
    match kind {
        PayloadKind::Wasm => bytes.starts_with(b"\0asm\x01\0\0\0"),
        PayloadKind::Data => bytes.starts_with(b"UnityWebData"),
        PayloadKind::Script => {
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
            let start = bytes
                .iter()
                .position(|byte| !byte.is_ascii_whitespace())
                .unwrap_or(bytes.len());
            let trimmed = &bytes[start..];
            [
                b"var unityFramework".as_slice(),
                b"function unityFramework",
                b"let unityFramework",
                b"const unityFramework",
            ]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
                || trimmed
                    .windows(b"unityFramework".len())
                    .any(|window| window == b"unityFramework")
        }
        PayloadKind::Memory => false,
    }
}

fn brotli_unity_marker(bytes: &[u8]) -> bool {
    let Some(&first) = bytes.first() else {
        return false;
    };
    // UnityLoader.js recognizes a leading Brotli metadata block before the
    // compressed bytes. WBITS uses 1, 4, or 7 bits of the first byte.
    let wbits_length = if first & 1 == 0 {
        1
    } else if first & 0x0e != 0 {
        4
    } else {
        7
    };
    let wbits = first & ((1 << wbits_length) - 1);
    if wbits == 0x11 {
        return false;
    }
    let skip_length = BROTLI_MARKER.len() - 1;
    let skip_octets = (usize::BITS - skip_length.leading_zeros()).div_ceil(8) as usize;
    let comment_offset = (wbits_length + 1 + 2 + 1 + 2 + (skip_octets << 3)).div_ceil(8);
    // ISLAST=0, MNIBBLES=3 (metadata), reserved=0, then MSKIPBYTES
    // and MSKIPLEN (metadata length minus one), least-significant bit first.
    let header =
        usize::from(wbits) | (((3 << 1) | (skip_octets << 4) | (skip_length << 6)) << wbits_length);
    bytes.starts_with(&header.to_le_bytes()[..comment_offset])
        && bytes.get(comment_offset..comment_offset + BROTLI_MARKER.len()) == Some(BROTLI_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brotli_marker_requires_the_full_unity_metadata_header() {
        // The 2022.3 loader expects a 22-bit leading metadata header for
        // a four-bit WBITS value of 11. The marker length is 36 octets.
        let mut bytes = vec![0x6b, 0x8d, 0x00];
        bytes.extend_from_slice(b"UnityWeb Compressed Content (brotli)");
        assert!(brotli_unity_marker(&bytes));
        assert!(!brotli_unity_marker(&bytes[3..]));
        bytes[1] ^= 0x80;
        assert!(!brotli_unity_marker(&bytes));
    }
}
