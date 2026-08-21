use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn detect(path: &Path) -> Option<String> {
    if let Ok(mime) = detect_by_magic(path) {
        return Some(mime);
    }
    detect_by_extension(path)
}

pub fn matches_mime_pattern(detected: &str, pattern: &str) -> bool {
    if detected == pattern {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        detected.starts_with(prefix) && detected[prefix.len()..].starts_with('/')
    } else {
        false
    }
}

fn detect_by_magic(path: &Path) -> Result<String, ()> {
    let mut file = File::open(path).map_err(|_| ())?;
    let mut buf = [0u8; 33000];
    let n = file.read(&mut buf).map_err(|_| ())?;
    if n == 0 {
        return Err(());
    }
    let buf = &buf[..n];
    magic_match(buf, path)
}

fn magic_match(buf: &[u8], path: &Path) -> Result<String, ()> {
    if buf.starts_with(b"\x89PNG") {
        return Ok("image/png".into());
    }
    if buf.starts_with(b"\xFF\xD8\xFF") {
        return Ok("image/jpeg".into());
    }
    if buf.starts_with(b"GIF8") {
        return Ok("image/gif".into());
    }
    if buf.starts_with(b"RIFF") && buf.len() >= 12 && &buf[8..12] == b"WEBP" {
        return Ok("image/webp".into());
    }
    if buf.starts_with(b"II\x2A\x00") || buf.starts_with(b"MM\x00\x2A") {
        return Ok("image/tiff".into());
    }
    if buf.starts_with(b"BM") {
        return Ok("image/bmp".into());
    }
    if buf.starts_with(b"%PDF") {
        return Ok("application/pdf".into());
    }
    if buf.starts_with(b"PK\x03\x04") {
        return Ok(zip_subtype(path).into());
    }
    if buf.starts_with(b"7z\xBC\xAF") {
        return Ok("application/x-7z-compressed".into());
    }
    if buf.starts_with(b"Rar!") {
        return Ok("application/x-rar-compressed".into());
    }
    if buf.starts_with(b"\x1F\x8B") {
        return Ok(gzip_subtype(path).into());
    }
    if buf.starts_with(b"\xED\xAB\xEE\xDB") {
        return Ok("application/x-rpm".into());
    }
    if buf.starts_with(b"\x7FELF") {
        return Ok("application/x-executable".into());
    }
    if buf.starts_with(b"MZ") {
        return Ok(mz_subtype(path).into());
    }
    if buf.starts_with(b"ID3") {
        return Ok("audio/mpeg".into());
    }
    if buf.starts_with(b"fLaC") {
        return Ok("audio/flac".into());
    }
    if buf.starts_with(b"OggS") {
        return Ok("audio/ogg".into());
    }
    if buf.len() >= 8 && &buf[4..8] == b"ftyp" {
        return Ok(ftyp_subtype(buf).into());
    }
    if buf.len() > 32773 && &buf[32769..32774] == b"CD001" {
        return Ok("application/x-iso9660-image".into());
    }
    if buf.starts_with(b"<?xml") {
        return Ok(xml_subtype(path).into());
    }
    if buf.starts_with(b"<") {
        let head: Vec<u8> = buf.iter().take(512).copied().collect();
        if let Ok(s) = std::str::from_utf8(&head) {
            let lower = s.to_ascii_lowercase();
            if lower.contains("<!doctype html") || lower.contains("<html") {
                return Ok("text/html".into());
            }
            if lower.contains("<?xml") || lower.contains("<svg") {
                return Ok(xml_subtype(path).into());
            }
        }
        return Ok("text/xml".into());
    }
    Err(())
}

fn zip_subtype(path: &Path) -> &'static str {
    match ext_lower(path).as_deref() {
        Some("apk") => "application/vnd.android.package-archive",
        Some("apkm") => "application/vnd.android.package-archive",
        Some("xpi") => "application/x-xpinstall",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("pptx") => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("ods") => "application/vnd.oasis.opendocument.spreadsheet",
        Some("odp") => "application/vnd.oasis.opendocument.presentation",
        Some("jar") => "application/java-archive",
        Some("epub") => "application/epub+zip",
        _ => "application/zip",
    }
}

fn gzip_subtype(path: &Path) -> &'static str {
    match ext_lower(path).as_deref() {
        Some("tgz") => "application/x-compressed-tar",
        Some("gz") => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.ends_with(".tar") {
                "application/x-compressed-tar"
            } else {
                "application/gzip"
            }
        }
        _ => "application/gzip",
    }
}

fn mz_subtype(path: &Path) -> &'static str {
    match ext_lower(path).as_deref() {
        Some("msi") => "application/x-msi",
        _ => "application/x-dosexec",
    }
}

fn ftyp_subtype(buf: &[u8]) -> &'static str {
    if buf.len() >= 12 {
        let brand = &buf[8..12];
        if brand == b"qt  " || brand == b"moov" {
            return "video/quicktime";
        }
    }
    "video/mp4"
}

fn xml_subtype(path: &Path) -> &'static str {
    match ext_lower(path).as_deref() {
        Some("svg") => "image/svg+xml",
        Some("drawio") => "application/xml",
        Some("kdenlive") => "application/xml",
        _ => "text/xml",
    }
}

fn ext_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

fn detect_by_extension(path: &Path) -> Option<String> {
    let ext = ext_lower(path)?;
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "heic" | "heif" => "image/heif",

        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "flv" => "video/x-flv",
        "wmv" => "video/x-ms-wmv",
        "ts" => "video/mp2t",

        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "wma" => "audio/x-ms-wma",
        "opus" => "audio/opus",
        "m3u" | "m3u8" => "audio/x-mpegurl",

        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tgz" => "application/x-compressed-tar",
        "bz2" => "application/x-bzip2",
        "xz" => "application/x-xz",
        "zst" | "zstd" => "application/zstd",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/x-rar-compressed",
        "tar" => "application/x-tar",
        "rpm" => "application/x-rpm",
        "deb" => "application/vnd.debian.binary-package",
        "iso" => "application/x-iso9660-image",
        "dmg" => "application/x-apple-diskimage",
        "appimage" => "application/x-executable",
        "exe" => "application/x-dosexec",
        "msi" => "application/x-msi",
        "apk" | "apkm" => "application/vnd.android.package-archive",
        "xpi" => "application/x-xpinstall",
        "jar" => "application/java-archive",
        "epub" => "application/epub+zip",

        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",

        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "xml" => "text/xml",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "md" | "markdown" => "text/markdown",
        "yml" | "yaml" => "text/x-yaml",
        "toml" => "text/x-toml",
        "ini" | "cfg" => "text/x-ini",
        "sql" => "text/x-sql",
        "sh" | "bash" | "zsh" | "fish" => "text/x-shellscript",
        "bat" | "cmd" => "text/x-batch",
        "ps1" => "text/x-powershell",
        "py" => "text/x-python",
        "rs" => "text/x-rust",
        "c" | "h" => "text/x-c",
        "cpp" | "cxx" | "cc" | "hpp" => "text/x-c++",
        "java" => "text/x-java",
        "go" => "text/x-go",
        "rb" => "text/x-ruby",
        "php" => "text/x-php",
        "lua" => "text/x-lua",
        "r" => "text/x-r",
        "swift" => "text/x-swift",
        "kt" | "kts" => "text/x-kotlin",
        "pl" | "pm" => "text/x-perl",
        "tex" | "latex" => "text/x-tex",
        "log" => "text/x-log",
        "conf" => "text/plain",

        "ass" | "ssa" => "text/x-ssa",
        "srt" => "text/x-subrip",
        "vtt" => "text/vtt",

        "desktop" => "application/x-desktop",
        "drawio" => "application/xml",
        "excalidraw" => "application/json",
        "kdenlive" => "application/xml",
        "torrent" => "application/x-bittorrent",
        "wasm" => "application/wasm",

        _ => return None,
    };
    Some(mime.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_file(name: &str, content: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("harbor_mime_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    #[test]
    fn pattern_exact_match() {
        assert!(matches_mime_pattern("image/png", "image/png"));
        assert!(!matches_mime_pattern("image/png", "image/jpeg"));
    }

    #[test]
    fn pattern_wildcard() {
        assert!(matches_mime_pattern("image/png", "image/*"));
        assert!(matches_mime_pattern("image/jpeg", "image/*"));
        assert!(matches_mime_pattern("text/plain", "text/*"));
        assert!(!matches_mime_pattern("application/pdf", "image/*"));
    }

    #[test]
    fn pattern_no_false_prefix() {
        assert!(!matches_mime_pattern("imageextra/png", "image/*"));
    }

    #[test]
    fn extension_known_types() {
        let cases = vec![
            ("test.m3u", "audio/x-mpegurl"),
            ("test.ass", "text/x-ssa"),
            ("test.svg", "image/svg+xml"),
            ("test.drawio", "application/xml"),
            ("test.excalidraw", "application/json"),
            ("test.kdenlive", "application/xml"),
            ("test.odt", "application/vnd.oasis.opendocument.text"),
            ("test.sql", "text/x-sql"),
            ("test.yml", "text/x-yaml"),
            ("test.toml", "text/x-toml"),
            ("test.md", "text/markdown"),
            ("test.csv", "text/csv"),
            ("test.bat", "text/x-batch"),
            ("test.sh", "text/x-shellscript"),
            ("test.desktop", "application/x-desktop"),
            ("test.pdf", "application/pdf"),
            ("test.rs", "text/x-rust"),
        ];
        for (name, expected) in cases {
            let got = detect_by_extension(Path::new(name));
            assert_eq!(got.as_deref(), Some(expected), "failed for {name}");
        }
    }

    #[test]
    fn extension_unknown_returns_none() {
        assert_eq!(detect_by_extension(Path::new("file.xyzzy123")), None);
    }

    #[test]
    fn magic_png() {
        let p = test_file("magic_png.bin", b"\x89PNG\r\n\x1a\n");
        assert_eq!(detect(&p).as_deref(), Some("image/png"));
    }

    #[test]
    fn magic_jpeg() {
        let p = test_file("magic_jpeg.bin", b"\xFF\xD8\xFF\xE0");
        assert_eq!(detect(&p).as_deref(), Some("image/jpeg"));
    }

    #[test]
    fn magic_pdf() {
        let p = test_file("magic_pdf.bin", b"%PDF-1.4");
        assert_eq!(detect(&p).as_deref(), Some("application/pdf"));
    }

    #[test]
    fn magic_zip_with_apk_ext() {
        let p = test_file("magic_zip.apk", b"PK\x03\x04");
        assert_eq!(
            detect(&p).as_deref(),
            Some("application/vnd.android.package-archive")
        );
    }

    #[test]
    fn magic_mp4_ftyp() {
        let p = test_file("magic_mp4.bin", b"\x00\x00\x00\x1cftypisom");
        assert_eq!(detect(&p).as_deref(), Some("video/mp4"));
    }

    #[test]
    fn magic_webp() {
        let p = test_file("magic_webp.bin", b"RIFF\x00\x00\x00\x00WEBP");
        assert_eq!(detect(&p).as_deref(), Some("image/webp"));
    }

    #[test]
    fn magic_elf() {
        let p = test_file("magic_elf.bin", b"\x7FELF\x02\x01\x01");
        assert_eq!(detect(&p).as_deref(), Some("application/x-executable"));
    }

    #[test]
    fn magic_gzip_tgz() {
        let p = test_file("archive.tar.gz", b"\x1F\x8B\x08");
        assert_eq!(detect(&p).as_deref(), Some("application/x-compressed-tar"));
    }

    #[test]
    fn empty_file_falls_back_to_ext() {
        let p = test_file("empty.md", b"");
        assert_eq!(detect(&p).as_deref(), Some("text/markdown"));
    }

    #[test]
    fn nonexistent_with_ext() {
        let p = Path::new("/tmp/does_not_exist_harbor_mime_test.pdf");
        assert_eq!(detect(p).as_deref(), Some("application/pdf"));
    }
}

