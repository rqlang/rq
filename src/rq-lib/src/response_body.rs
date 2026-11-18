use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::RqError;
use crate::paths::{depth_below, join_lexically, normalize_lexically};
use crate::syntax::parse_result::OutputPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseBody {
    Text(String),
    Binary(BinaryBody),
    Output(OutputBody),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryBody {
    pub info: BodyFileInfo,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputBody {
    pub info: BodyFileInfo,
    pub bytes: Vec<u8>,
    pub path: PathBuf,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyFileInfo {
    pub media_type: Option<String>,
    pub file_name: String,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputTarget {
    pub path: String,
    pub is_folder: bool,
    pub applies_to_every_body: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutputSkipped {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedBody {
    pub body: ResponseBody,
    pub output_skipped: Option<OutputSkipped>,
}

const DOWNLOAD_DIR_NAME: &str = "rq";
const FALLBACK_FILE_STEM: &str = "response";
const FALLBACK_EXTENSION: &str = "bin";
const RESERVED_FILE_NAME_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
const MAX_FILE_NAME_BYTES: usize = 255;
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;
const WINDOWS_RESERVED_FILE_STEMS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

const TEXT_MEDIA_TYPES: &[&str] = &[
    "application/json",
    "application/xml",
    "application/javascript",
    "application/ecmascript",
    "application/x-www-form-urlencoded",
    "application/graphql",
    "application/x-ndjson",
    "application/yaml",
    "application/x-yaml",
    "application/toml",
    "application/sql",
];

const TEXT_MEDIA_TYPE_SUFFIXES: &[&str] = &["+json", "+xml", "+yaml"];

const SUCCESS_STATUSES: std::ops::Range<u16> = 200..300;

const BINARY_MEDIA_TYPE_PREFIXES: &[&str] = &["image/", "audio/", "video/", "font/"];

const BINARY_MEDIA_TYPES: &[&str] = &["application/octet-stream"];

const EXTENSIONS: &[(&str, &str)] = &[
    ("application/json", "json"),
    ("application/x-ndjson", "ndjson"),
    ("application/xml", "xml"),
    ("text/xml", "xml"),
    ("application/yaml", "yaml"),
    ("application/x-yaml", "yaml"),
    ("text/yaml", "yaml"),
    ("application/toml", "toml"),
    ("application/javascript", "js"),
    ("text/javascript", "js"),
    ("application/sql", "sql"),
    ("application/graphql", "graphql"),
    ("text/plain", "txt"),
    ("text/csv", "csv"),
    ("text/html", "html"),
    ("text/css", "css"),
    ("text/markdown", "md"),
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/avif", "avif"),
    ("image/bmp", "bmp"),
    ("image/tiff", "tiff"),
    ("image/x-icon", "ico"),
    ("image/vnd.microsoft.icon", "ico"),
    ("application/pdf", "pdf"),
    ("application/zip", "zip"),
    ("application/gzip", "gz"),
    ("application/x-tar", "tar"),
    ("application/x-7z-compressed", "7z"),
    ("application/wasm", "wasm"),
    ("application/msword", "doc"),
    ("application/vnd.ms-excel", "xls"),
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptx",
    ),
    ("audio/mpeg", "mp3"),
    ("audio/wav", "wav"),
    ("audio/ogg", "ogg"),
    ("video/mp4", "mp4"),
    ("video/webm", "webm"),
    ("font/woff", "woff"),
    ("font/woff2", "woff2"),
    ("font/ttf", "ttf"),
    ("font/otf", "otf"),
];

impl ResponseBody {
    pub fn from_response(
        headers: &HashMap<String, String>,
        bytes: Vec<u8>,
        request_name: &str,
    ) -> Self {
        match BodyFileInfo::of_response(headers, &bytes, request_name) {
            Some(info) => Self::Binary(BinaryBody { info, bytes }),
            None => Self::Text(
                String::from_utf8(bytes)
                    .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
            ),
        }
    }

    pub fn route(
        headers: &HashMap<String, String>,
        bytes: Vec<u8>,
        request_name: &str,
        status: u16,
        output: Option<&OutputTarget>,
    ) -> RoutedBody {
        let Some(output) = output.filter(|output| output.covers(headers, &bytes, request_name))
        else {
            return RoutedBody::unrouted(Self::from_response(headers, bytes, request_name));
        };
        if let Some(reason) = output.skip_reason(status, &bytes) {
            return RoutedBody {
                body: Self::from_response(headers, bytes, request_name),
                output_skipped: Some(OutputSkipped {
                    path: output.path.clone(),
                    reason,
                }),
            };
        }
        let binary = BodyFileInfo::of_response(headers, &bytes, request_name).is_some();
        let info = BodyFileInfo::describe(headers, &bytes, request_name);
        let path = output.destination(&info.file_name);
        RoutedBody::unrouted(Self::Output(OutputBody {
            info,
            bytes,
            path,
            binary,
        }))
    }
}

impl RoutedBody {
    fn unrouted(body: ResponseBody) -> Self {
        Self {
            body,
            output_skipped: None,
        }
    }
}

impl OutputBody {
    pub fn save(&self) -> Result<PathBuf, RqError> {
        if self.path.is_symlink() {
            return Err(RqError::Generic(format!(
                "{} is a symbolic link, which output never writes through",
                self.path.display()
            )));
        }
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, &self.bytes)?;
        Ok(self.path.clone())
    }
}

impl OutputTarget {
    pub fn resolve(chain: &[OutputPath], root: &str) -> Option<Self> {
        let last = chain.last()?;
        let composed = chain.iter().fold(None::<String>, |base, output| {
            let base = base.unwrap_or_else(|| output.base_dir.clone());
            Some(join_lexically(&base, &output.path))
        })?;
        let path = normalize_lexically(&composed);
        let error = match depth_below(&path, root) {
            Some(depth) if depth > 0 || last.is_folder => None,
            _ => Some(format!(
                "{path} is outside the source folder {}",
                normalize_lexically(root)
            )),
        };
        Some(Self {
            path,
            is_folder: last.is_folder,
            applies_to_every_body: last.applies_to_every_body,
            error,
        })
    }

    pub fn destination(&self, file_name: &str) -> PathBuf {
        if self.is_folder {
            Path::new(&self.path).join(file_name)
        } else {
            PathBuf::from(&self.path)
        }
    }

    fn covers(&self, headers: &HashMap<String, String>, bytes: &[u8], request_name: &str) -> bool {
        self.applies_to_every_body
            || BodyFileInfo::of_response(headers, bytes, request_name).is_some()
    }

    fn skip_reason(&self, status: u16, bytes: &[u8]) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        if !SUCCESS_STATUSES.contains(&status) {
            return Some(format!("the response status is {status}"));
        }
        bytes
            .is_empty()
            .then(|| "the response has no body".to_string())
    }
}

impl BinaryBody {
    pub fn save_in(&self, dir: &Path) -> Result<PathBuf, RqError> {
        create_private_dir_all(dir)?;
        let path = self.info.download_path(dir);
        if let Some(parent) = path.parent() {
            create_private_dir(parent)?;
        }
        write_private_file(&path, &self.bytes)?;
        Ok(path)
    }
}

impl BodyFileInfo {
    pub fn of_response(
        headers: &HashMap<String, String>,
        bytes: &[u8],
        request_name: &str,
    ) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }
        let media_type = media_type(headers);
        let binary = match media_type.as_deref() {
            Some(media_type) if is_text_media_type(media_type) => false,
            Some(media_type) if is_binary_media_type(media_type) => true,
            _ => !looks_like_text(bytes),
        };
        binary.then(|| Self::describe(headers, bytes, request_name))
    }

    pub fn describe(headers: &HashMap<String, String>, bytes: &[u8], request_name: &str) -> Self {
        let media_type = media_type(headers);
        Self {
            file_name: file_name(headers, media_type.as_deref(), request_name),
            media_type,
            size: bytes.len(),
        }
    }

    pub fn download_path(&self, dir: &Path) -> PathBuf {
        dir.join(Uuid::new_v4().to_string()).join(&self.file_name)
    }
}

pub fn temp_download_dir() -> PathBuf {
    std::env::temp_dir().join(DOWNLOAD_DIR_NAME)
}

#[cfg(unix)]
fn create_private_dir_all(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(std::io::Error::other(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(PRIVATE_DIR_MODE))
}

#[cfg(not(unix))]
fn create_private_dir_all(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(PRIVATE_DIR_MODE)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dir)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    options.open(path)?.write_all(bytes)
}

fn header<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn media_type(headers: &HashMap<String, String>) -> Option<String> {
    let content_type = header(headers, "content-type")?;
    let media_type = content_type.split(';').next()?.trim().to_ascii_lowercase();
    (!media_type.is_empty()).then_some(media_type)
}

fn is_text_media_type(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || TEXT_MEDIA_TYPES.contains(&media_type)
        || TEXT_MEDIA_TYPE_SUFFIXES
            .iter()
            .any(|suffix| media_type.ends_with(suffix))
}

fn is_binary_media_type(media_type: &str) -> bool {
    BINARY_MEDIA_TYPE_PREFIXES
        .iter()
        .any(|prefix| media_type.starts_with(prefix))
        || BINARY_MEDIA_TYPES.contains(&media_type)
        || EXTENSIONS.iter().any(|(known, _)| *known == media_type)
}

fn looks_like_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0)
        && std::str::from_utf8(bytes).map_or_else(|e| e.error_len().is_none(), |_| true)
}

fn file_name(
    headers: &HashMap<String, String>,
    media_type: Option<&str>,
    request_name: &str,
) -> String {
    header(headers, "content-disposition")
        .and_then(disposition_file_name)
        .and_then(|name| sanitize_file_name(&name))
        .or_else(|| {
            let extension = extension_of(media_type);
            sanitize_file_name(&format!("{}.{extension}", request_name.replace('/', ".")))
        })
        .unwrap_or_else(|| format!("{FALLBACK_FILE_STEM}.{FALLBACK_EXTENSION}"))
}

fn extension_of(media_type: Option<&str>) -> &'static str {
    media_type
        .and_then(|media_type| {
            EXTENSIONS
                .iter()
                .find(|(known, _)| *known == media_type)
                .map(|(_, extension)| *extension)
                .or_else(|| suffix_extension(media_type))
        })
        .unwrap_or(FALLBACK_EXTENSION)
}

fn suffix_extension(media_type: &str) -> Option<&'static str> {
    TEXT_MEDIA_TYPE_SUFFIXES
        .iter()
        .find(|suffix| media_type.ends_with(*suffix))
        .and_then(|suffix| suffix.strip_prefix('+'))
}

fn disposition_file_name(value: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for (key, value) in disposition_parameters(value)
        .into_iter()
        .skip(1)
        .filter_map(|p| p.split_once('='))
    {
        match key.trim().to_ascii_lowercase().as_str() {
            "filename*" => extended = extended_value(value.trim()),
            "filename" => plain = Some(unquote(value.trim())),
            _ => {}
        }
    }
    extended.or(plain)
}

fn disposition_parameters(value: &str) -> Vec<&str> {
    let mut parameters = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, c) in value.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parameters.extend(value.get(start..index));
                start = index + 1;
            }
            _ => {}
        }
    }
    parameters.extend(value.get(start..));
    parameters
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .map_or_else(|| value.to_string(), |inner| inner.replace("\\\"", "\""))
}

fn extended_value(value: &str) -> Option<String> {
    let encoded = value.splitn(3, '\'').nth(2)?;
    String::from_utf8(percent_decode(encoded)?).ok()
}

fn percent_decode(value: &str) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            decoded.push(byte);
            continue;
        }
        let high = char::from(bytes.next()?).to_digit(16)?;
        let low = char::from(bytes.next()?).to_digit(16)?;
        decoded.push(u8::try_from(high * 16 + low).ok()?);
    }
    Some(decoded)
}

fn sanitize_file_name(name: &str) -> Option<String> {
    let last_segment = name.rsplit(['/', '\\']).next()?;
    let cleaned: String = last_segment
        .chars()
        .map(|c| {
            if c.is_control() || RESERVED_FILE_NAME_CHARS.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.');
    (!trimmed.is_empty()).then(|| truncate_file_name(&avoid_reserved_stem(trimmed)))
}

fn avoid_reserved_stem(name: &str) -> String {
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if WINDOWS_RESERVED_FILE_STEMS
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        format!("_{name}")
    } else {
        name.to_string()
    }
}

fn truncate_file_name(name: &str) -> String {
    if name.len() <= MAX_FILE_NAME_BYTES {
        return name.to_string();
    }
    let (stem, suffix) = match name.rsplit_once('.') {
        Some((stem, extension)) if extension.len() + 1 < MAX_FILE_NAME_BYTES => {
            (stem, format!(".{extension}"))
        }
        _ => (name, String::new()),
    };
    let budget = MAX_FILE_NAME_BYTES - suffix.len();
    let mut truncated = String::with_capacity(MAX_FILE_NAME_BYTES);
    for c in stem.chars() {
        if truncated.len() + c.len_utf8() > budget {
            break;
        }
        truncated.push(c);
    }
    truncated + &suffix
}

#[cfg(test)]
mod tests {
    use super::{BodyFileInfo, OutputSkipped, OutputTarget, ResponseBody};
    use crate::syntax::parse_result::OutputPath;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    const PNG_BYTES: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0xff];

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    fn info(pairs: &[(&str, &str)], bytes: &[u8]) -> Option<BodyFileInfo> {
        BodyFileInfo::of_response(&headers(pairs), bytes, "download")
    }

    fn file_name(pairs: &[(&str, &str)]) -> Option<String> {
        info(pairs, PNG_BYTES).map(|info| info.file_name)
    }

    #[test]
    fn an_image_media_type_is_binary() {
        let target = info(&[("content-type", "image/png")], PNG_BYTES);
        assert_eq!(
            target,
            Some(BodyFileInfo {
                media_type: Some("image/png".to_string()),
                file_name: "download.png".to_string(),
                size: PNG_BYTES.len(),
            })
        );
    }

    #[test]
    fn a_text_media_type_is_text_even_with_invalid_utf8() {
        let target = info(&[("content-type", "text/plain; charset=latin1")], PNG_BYTES);
        assert_eq!(target, None);
    }

    #[test]
    fn json_xml_and_their_suffixes_are_text() {
        let target = [
            "application/json",
            "application/problem+json",
            "application/xml",
            "application/atom+xml",
            "application/x-www-form-urlencoded",
        ]
        .iter()
        .find(|media_type| info(&[("content-type", media_type)], b"{}").is_some());
        assert_eq!(target, None);
    }

    #[test]
    fn the_content_type_header_name_is_case_insensitive() {
        let target = info(&[("Content-Type", "Application/PDF")], b"%PDF-1.7");
        assert_eq!(
            target.and_then(|info| info.media_type),
            Some("application/pdf".to_string())
        );
    }

    #[test]
    fn without_content_type_valid_utf8_is_text() {
        let target = info(&[], "héllo".as_bytes());
        assert_eq!(target, None);
    }

    #[test]
    fn without_content_type_invalid_utf8_is_binary() {
        let target = info(&[], PNG_BYTES).map(|info| info.file_name);
        assert_eq!(target, Some("download.bin".to_string()));
    }

    #[test]
    fn without_content_type_a_nul_byte_is_binary() {
        let target = info(&[], b"abc\0def");
        assert!(target.is_some());
    }

    #[test]
    fn without_content_type_text_cut_mid_character_is_text() {
        let target = info(&[], &"héllo".as_bytes()[..2]);
        assert_eq!(target, None);
    }

    #[test]
    fn an_unknown_media_type_with_utf8_text_is_text() {
        let target = info(
            &[("content-type", "application/jwt")],
            b"eyJhbGciOiJIUzI1NiJ9",
        );
        assert_eq!(target, None);
    }

    #[test]
    fn an_unknown_media_type_with_invalid_utf8_is_binary() {
        let target = info(&[("content-type", "application/x-custom")], PNG_BYTES);
        assert!(target.is_some());
    }

    #[test]
    fn an_octet_stream_is_binary_even_when_it_is_utf8() {
        let target = info(&[("content-type", "application/octet-stream")], b"abc");
        assert!(target.is_some());
    }

    #[test]
    fn an_image_media_type_is_binary_even_when_it_is_utf8() {
        let target = info(&[("content-type", "image/x-portable-graymap")], b"P2 1 1");
        assert!(target.is_some());
    }

    #[test]
    fn an_empty_body_is_never_binary() {
        let target = info(&[("content-type", "image/png")], b"");
        assert_eq!(target, None);
    }

    #[test]
    fn content_disposition_names_the_file() {
        let target = file_name(&[
            ("content-type", "image/png"),
            ("content-disposition", "attachment; filename=\"logo.png\""),
        ]);
        assert_eq!(target, Some("logo.png".to_string()));
    }

    #[test]
    fn an_extended_file_name_wins_and_is_percent_decoded() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"plain.pdf\"; filename*=UTF-8''informe%20a%C3%B1o.pdf",
        )]);
        assert_eq!(target, Some("informe año.pdf".to_string()));
    }

    #[test]
    fn a_file_name_cannot_leave_the_download_directory() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"../../etc/passwd\"",
        )]);
        assert_eq!(target, Some("passwd".to_string()));
    }

    #[test]
    fn a_windows_path_keeps_only_its_last_segment() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"..\\..\\evil.exe\"",
        )]);
        assert_eq!(target, Some("evil.exe".to_string()));
    }

    #[test]
    fn a_dot_dot_file_name_falls_back_to_the_request_name() {
        let target = file_name(&[
            ("content-type", "application/zip"),
            ("content-disposition", "attachment; filename=\"..\""),
        ]);
        assert_eq!(target, Some("download.zip".to_string()));
    }

    #[test]
    fn reserved_and_control_characters_are_replaced() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"a:b*c?\u{7}.png\"",
        )]);
        assert_eq!(target, Some("a_b_c__.png".to_string()));
    }

    #[test]
    fn a_quoted_semicolon_stays_in_the_file_name() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"informe;2024.pdf\"; size=10",
        )]);
        assert_eq!(target, Some("informe;2024.pdf".to_string()));
    }

    #[test]
    fn an_escaped_quote_in_the_file_name_is_replaced() {
        let target = file_name(&[("content-disposition", "attachment; filename=\"a\\\"b.pdf\"")]);
        assert_eq!(target, Some("a_b.pdf".to_string()));
    }

    #[test]
    fn a_windows_reserved_name_is_prefixed() {
        let target = ["CON.pdf", "nul", "Lpt1 .txt"].map(|name| {
            file_name(&[(
                "content-disposition",
                &format!("attachment; filename=\"{name}\""),
            )])
        });
        assert_eq!(
            target,
            [
                Some("_CON.pdf".to_string()),
                Some("_nul".to_string()),
                Some("_Lpt1 .txt".to_string())
            ]
        );
    }

    #[test]
    fn a_long_file_name_is_truncated_keeping_its_extension() {
        let name = format!("{}.pdf", "ñ".repeat(200));
        let target = file_name(&[(
            "content-disposition",
            &format!("attachment; filename=\"{name}\""),
        )])
        .unwrap_or_default();
        assert!(target.len() <= 255 && target.len() > 250 && target.ends_with("ñ.pdf"));
    }

    #[test]
    fn a_long_extension_is_kept_when_the_name_is_truncated() {
        let name = format!("{}.application-long", "a".repeat(300));
        let target = file_name(&[(
            "content-disposition",
            &format!("attachment; filename=\"{name}\""),
        )])
        .unwrap_or_default();
        assert!(target.len() == 255 && target.ends_with("a.application-long"));
    }

    #[test]
    fn an_extension_longer_than_the_cap_is_truncated_with_the_name() {
        let name = format!("a.{}", "b".repeat(300));
        let target = file_name(&[(
            "content-disposition",
            &format!("attachment; filename=\"{name}\""),
        )])
        .unwrap_or_default();
        assert!(target.len() == 255 && target.starts_with("a.b"));
    }

    #[test]
    fn a_malformed_percent_escape_falls_back_to_the_plain_name() {
        let target = file_name(&[(
            "content-disposition",
            "attachment; filename=\"plain.pdf\"; filename*=UTF-8''bad%G1.pdf",
        )]);
        assert_eq!(target, Some("plain.pdf".to_string()));
    }

    #[test]
    fn an_endpoint_request_name_becomes_a_dotted_file_name() {
        let target = BodyFileInfo::of_response(
            &headers(&[("content-type", "application/pdf")]),
            b"%PDF",
            "reports/monthly",
        )
        .map(|info| info.file_name);
        assert_eq!(target, Some("reports.monthly.pdf".to_string()));
    }

    #[test]
    fn a_text_body_is_decoded_lossily() {
        let target = ResponseBody::from_response(
            &headers(&[("content-type", "text/plain")]),
            vec![b'a', 0xff],
            "r",
        );
        assert_eq!(target, ResponseBody::Text("a\u{fffd}".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn saved_files_are_private_to_the_user() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("rq");
        std::fs::create_dir(&root)?;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))?;
        let ResponseBody::Binary(body) = ResponseBody::from_response(
            &headers(&[("content-type", "image/png")]),
            PNG_BYTES.to_vec(),
            "logo",
        ) else {
            return Err("expected a binary body".into());
        };
        let path = body.save_in(&root)?;
        let parent = path.parent().ok_or("no parent")?;
        let target = [root.as_path(), parent, path.as_path()]
            .map(|p| std::fs::metadata(p).map(|m| m.permissions().mode() & 0o777));
        assert_eq!(
            target.into_iter().collect::<Result<Vec<_>, _>>()?,
            vec![0o700, 0o700, 0o600]
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_download_root_that_is_a_symlink_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir(&elsewhere)?;
        let root = dir.path().join("rq");
        std::os::unix::fs::symlink(&elsewhere, &root)?;
        let ResponseBody::Binary(body) = ResponseBody::from_response(
            &headers(&[("content-type", "image/png")]),
            PNG_BYTES.to_vec(),
            "logo",
        ) else {
            return Err("expected a binary body".into());
        };
        assert!(body.save_in(&root).is_err());
        Ok(())
    }

    #[test]
    fn saving_writes_the_exact_bytes_in_a_fresh_directory() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let ResponseBody::Binary(body) = ResponseBody::from_response(
            &headers(&[("content-type", "image/png")]),
            PNG_BYTES.to_vec(),
            "logo",
        ) else {
            return Err("expected a binary body".into());
        };
        let first = body.save_in(dir.path())?;
        let second = body.save_in(dir.path())?;
        assert_eq!(std::fs::read(&first)?, PNG_BYTES);
        assert_ne!(first, second);
        assert_eq!(first.file_name(), second.file_name());
        Ok(())
    }

    fn segment(path: &str, base_dir: &str, applies_to_every_body: bool) -> OutputPath {
        OutputPath {
            path: path.to_string(),
            base_dir: base_dir.to_string(),
            is_folder: path.ends_with(['/', '\\']),
            applies_to_every_body,
        }
    }

    fn output_target(path: &str, is_folder: bool, applies_to_every_body: bool) -> OutputTarget {
        OutputTarget {
            path: path.to_string(),
            is_folder,
            applies_to_every_body,
            error: None,
        }
    }

    fn native(path: &str) -> String {
        path.replace('/', std::path::MAIN_SEPARATOR_STR)
    }

    fn route(
        pairs: &[(&str, &str)],
        bytes: &[u8],
        status: u16,
        output: &OutputTarget,
    ) -> (ResponseBody, Option<OutputSkipped>) {
        let routed = ResponseBody::route(
            &headers(pairs),
            bytes.to_vec(),
            "report",
            status,
            Some(output),
        );
        (routed.body, routed.output_skipped)
    }

    fn output_path(body: &ResponseBody) -> Option<&Path> {
        match body {
            ResponseBody::Output(output) => Some(output.path.as_path()),
            _ => None,
        }
    }

    #[test]
    fn json_and_its_suffix_types_get_a_json_extension() {
        let json = BodyFileInfo::describe(
            &headers(&[("content-type", "application/json")]),
            b"{}",
            "users",
        );
        let problem = BodyFileInfo::describe(
            &headers(&[("content-type", "application/problem+json")]),
            b"{}",
            "users",
        );
        assert_eq!(
            (json.file_name, problem.file_name),
            ("users.json".to_string(), "users.json".to_string())
        );
    }

    #[test]
    fn output_resolves_a_request_file_against_the_folder_of_its_file() {
        let target = OutputTarget::resolve(&[segment("out/x.pdf", "api", true)], "api");
        assert_eq!(
            target,
            Some(output_target(&native("api/out/x.pdf"), false, true))
        );
    }

    #[test]
    fn output_appends_each_level_to_the_folder_it_inherits() {
        let chain = [
            segment("downloads/", "api/shared", false),
            segment("reports/", "api", false),
            segment("report.pdf", "api", true),
        ];
        let target = OutputTarget::resolve(&chain, "api");
        assert_eq!(
            target.map(|t| t.path),
            Some(native("api/shared/downloads/reports/report.pdf"))
        );
    }

    #[test]
    fn an_absolute_level_ignores_the_folder_it_inherits() {
        let chain = [
            segment("downloads/", "/repo", false),
            segment("/repo/out/x.pdf", "/repo", true),
        ];
        let target = OutputTarget::resolve(&chain, "/repo");
        assert_eq!(target.map(|t| t.path), Some(native("/repo/out/x.pdf")));
    }

    #[test]
    fn only_endpoint_folders_apply_to_binary_bodies_only() {
        let target = OutputTarget::resolve(&[segment("downloads/", "api", false)], "api");
        assert_eq!(
            target,
            Some(output_target(&native("api/downloads"), true, false))
        );
    }

    #[test]
    fn output_that_climbs_out_of_the_source_folder_carries_an_error() {
        let target =
            OutputTarget::resolve(&[segment("../secrets/x", "/repo/api", true)], "/repo/api");
        assert_eq!(
            target.and_then(|t| t.error),
            Some(format!(
                "{} is outside the source folder {}",
                native("/repo/secrets/x"),
                native("/repo/api")
            ))
        );
    }

    #[test]
    fn output_file_that_is_the_source_folder_itself_carries_an_error() {
        let target = OutputTarget::resolve(&[segment("api", "", true)], "api");
        assert!(target.and_then(|t| t.error).is_some());
    }

    #[test]
    fn request_without_output_has_no_target() {
        assert_eq!(OutputTarget::resolve(&[], "api"), None);
    }

    #[test]
    fn request_output_writes_a_text_body_to_its_file() {
        let (body, skipped) = route(
            &[("content-type", "application/json")],
            b"{}",
            200,
            &output_target("out/users.json", false, true),
        );
        assert_eq!(
            (output_path(&body), skipped),
            (Some(Path::new("out/users.json")), None)
        );
    }

    #[test]
    fn request_output_folder_keeps_the_automatic_file_name() {
        let (body, _) = route(
            &[("content-type", "application/json")],
            b"{}",
            200,
            &output_target("out", true, true),
        );
        assert_eq!(
            output_path(&body),
            Some(PathBuf::from("out").join("report.json").as_path())
        );
    }

    #[test]
    fn endpoint_output_leaves_a_text_body_to_be_printed() {
        let (body, skipped) = route(
            &[("content-type", "application/json")],
            b"{}",
            200,
            &output_target("out", true, false),
        );
        assert_eq!(
            (body, skipped),
            (ResponseBody::Text("{}".to_string()), None)
        );
    }

    #[test]
    fn endpoint_output_takes_a_binary_body() {
        let (body, _) = route(
            &[("content-type", "image/png")],
            PNG_BYTES,
            200,
            &output_target("out", true, false),
        );
        assert_eq!(
            output_path(&body),
            Some(PathBuf::from("out").join("report.png").as_path())
        );
    }

    #[test]
    fn output_body_keeps_the_exact_bytes_of_invalid_utf8_text() {
        let bytes = [b'a', 0xe9, b'b'];
        let (body, _) = route(
            &[("content-type", "text/plain; charset=latin1")],
            &bytes,
            200,
            &output_target("x.txt", false, true),
        );
        let ResponseBody::Output(output) = body else {
            panic!("expected an output body");
        };
        assert_eq!(output.bytes, bytes);
    }

    #[test]
    fn an_error_response_is_not_written_and_says_why() {
        let (body, skipped) = route(
            &[("content-type", "application/json")],
            b"{}",
            404,
            &output_target("x.json", false, true),
        );
        assert_eq!(
            (body, skipped),
            (
                ResponseBody::Text("{}".to_string()),
                Some(OutputSkipped {
                    path: "x.json".to_string(),
                    reason: "the response status is 404".to_string()
                })
            )
        );
    }

    #[test]
    fn an_error_response_to_an_endpoint_output_is_silent_when_its_body_is_text() {
        let (_, skipped) = route(
            &[("content-type", "application/json")],
            b"{}",
            500,
            &output_target("out", true, false),
        );
        assert_eq!(skipped, None);
    }

    #[test]
    fn a_binary_error_response_to_an_endpoint_output_says_why() {
        let (body, skipped) = route(
            &[("content-type", "image/png")],
            PNG_BYTES,
            500,
            &output_target("out", true, false),
        );
        assert!(matches!(body, ResponseBody::Binary(_)));
        assert_eq!(
            skipped.map(|s| s.reason),
            Some("the response status is 500".to_string())
        );
    }

    #[test]
    fn an_empty_success_is_not_written_and_says_why() {
        let (_, skipped) = route(&[], b"", 204, &output_target("x.json", false, true));
        assert_eq!(
            skipped.map(|s| s.reason),
            Some("the response has no body".to_string())
        );
    }

    #[test]
    fn an_output_outside_the_source_is_not_written_and_says_why() {
        let output = OutputTarget {
            error: Some("outside".to_string()),
            ..output_target("x.json", false, true)
        };
        let (_, skipped) = route(&[], b"{}", 200, &output);
        assert_eq!(skipped.map(|s| s.reason), Some("outside".to_string()));
    }

    fn saved_output(file: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let output = output_target(&file.to_string_lossy(), false, true);
        let (body, _) = route(&[("content-type", "application/json")], b"{}", 200, &output);
        let ResponseBody::Output(body) = body else {
            return Err("expected an output body".into());
        };
        body.save()?;
        Ok(std::fs::read(file)?)
    }

    #[test]
    fn saving_an_output_body_creates_its_folders() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        assert_eq!(saved_output(&dir.path().join("a/b/x.json"))?, b"{}");
        Ok(())
    }

    #[test]
    fn saving_an_output_body_overwrites_an_existing_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("x.json");
        std::fs::write(&file, "old")?;
        assert_eq!(saved_output(&file)?, b"{}");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn saving_an_output_body_never_writes_through_a_link() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, "old")?;
        let link = dir.path().join("x.json");
        std::os::unix::fs::symlink(&outside, &link)?;
        assert!(saved_output(&link).is_err());
        assert_eq!(std::fs::read_to_string(&outside)?, "old");
        Ok(())
    }
}
