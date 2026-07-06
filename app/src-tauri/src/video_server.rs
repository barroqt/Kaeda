use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct VideoServer {
    port: u16,
    running: Arc<AtomicBool>,
}

impl VideoServer {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if !running_clone.load(Ordering::Relaxed) {
                    break;
                }
                match stream {
                    Ok(s) => {
                        std::thread::spawn(|| {
                            let _ = serve(s);
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self { port, running })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = TcpStream::connect(format!("127.0.0.1:{}", self.port));
    }
}

impl Drop for VideoServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(stream: TcpStream) -> Result<(), String> {
    let _peer = stream.peer_addr().map_err(|e| e.to_string())?.to_string();
    let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut writer = stream;

    let mut lines = reader.lines();

    let request_line = lines
        .next()
        .ok_or_else(|| "empty request".to_string())?
        .map_err(|e| e.to_string())?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "GET" {
        let _ = respond(&mut writer, 405, "Method Not Allowed", None, None, None);
        return Ok(());
    }

    let raw_path = parts[1];
    let encoded = raw_path.strip_prefix('/').unwrap_or(raw_path);
    let path = url_decode(encoded);

    if path.is_empty() {
        let _ = respond(&mut writer, 400, "Bad Request", None, None, None);
        return Ok(());
    }

    if path.split('/').any(|c| c == "..") {
        let _ = respond(&mut writer, 400, "Bad Request", None, None, None);
        return Ok(());
    }

    let file_path = &path;

    let mut range_value = None;
    loop {
        match lines.next() {
            Some(Ok(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(val) = trimmed.strip_prefix("Range: ") {
                    range_value = Some(val.to_string());
                }
            }
            Some(Err(e)) => return Err(e.to_string()),
            None => break,
        }
    }

    serve_file(&mut writer, file_path, range_value.as_deref())?;
    Ok(())
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn serve_file(writer: &mut TcpStream, path: &str, range: Option<&str>) -> Result<(), String> {
    let path = Path::new(path);
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            let _ = respond(writer, 404, "Not Found", None, None, None);
            return Ok(());
        }
    };
    if !meta.is_file() {
        let _ = respond(writer, 404, "Not Found", None, None, None);
        return Ok(());
    }
    let file_len = meta.len();
    let mime = mime_for(path);

    if let Some(range_str) = range {
        if let Some(range_val) = range_str.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_val.split('-').collect();
            let start: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let end: u64 = parts
                .get(1)
                .and_then(|s| {
                    let s = s.trim();
                    if s.is_empty() { None } else { s.parse().ok() }
                })
                .unwrap_or(file_len.saturating_sub(1));

            if start >= file_len {
                let _ = respond(writer, 416, "Range Not Satisfiable", None, None, None);
                return Ok(());
            }

            let content_len = end.saturating_sub(start).saturating_add(1);

            respond(
                writer,
                206,
                "Partial Content",
                Some(mime),
                Some(content_len),
                Some(&format!("bytes {}-{}/{}", start, end, file_len)),
            )?;

            let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            file.seek(SeekFrom::Start(start))
                .map_err(|e| e.to_string())?;

            copy_n(&mut file, writer, content_len)?;
        }
    } else {
        respond(writer, 200, "OK", Some(mime), Some(file_len), None)?;

        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, writer).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_n(reader: &mut impl Read, writer: &mut impl Write, mut n: u64) -> Result<(), String> {
    let mut buf = [0u8; 65536];
    while n > 0 {
        let to_read = buf.len().min(n as usize);
        let bytes = reader
            .read(&mut buf[..to_read])
            .map_err(|e| e.to_string())?;
        if bytes == 0 {
            break;
        }
        writer.write_all(&buf[..bytes]).map_err(|e| e.to_string())?;
        n -= bytes as u64;
    }
    Ok(())
}

fn respond(
    writer: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: Option<&str>,
    content_length: Option<u64>,
    content_range: Option<&str>,
) -> Result<(), String> {
    let mut headers = format!("HTTP/1.1 {} {}\r\n", status, reason);
    if let Some(ct) = content_type {
        headers.push_str(&format!("Content-Type: {}\r\n", ct));
    }
    if let Some(cl) = content_length {
        headers.push_str(&format!("Content-Length: {}\r\n", cl));
    }
    if let Some(cr) = content_range {
        headers.push_str(&format!("Content-Range: {}\r\n", cr));
    }
    headers.push_str("Access-Control-Allow-Origin: *\r\n");
    headers.push_str("Accept-Ranges: bytes\r\n");
    headers.push_str("Connection: close\r\n");
    headers.push_str("\r\n");
    writer
        .write_all(headers.as_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

    fn encode_path(p: &std::path::Path) -> String {
        p.to_str().unwrap().replace('/', "%2F")
    }

    fn temp_file(content: &[u8], label: &str) -> (std::path::PathBuf, std::fs::File) {
        let mut path = std::env::temp_dir();
        path.push(format!("kaeda_video_test_{label}_{}", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        std::io::Write::write_all(&mut &file, content).unwrap();
        (path, file)
    }

    fn send_request(port: u16, request: &str) -> Vec<u8> {
        let mut conn = TcpStream::connect(("127.0.0.1", port))
            .unwrap_or_else(|_| panic!("could not connect to server on port {port}"));
        conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        conn.write_all(request.as_bytes()).unwrap();
        let mut buf = Vec::new();
        conn.read_to_end(&mut buf).unwrap();
        buf
    }

    #[test]
    fn plain_get_returns_full_file() {
        let content = b"Hello, video server!";
        let (path, _f) = temp_file(content, "plain_get");
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let encoded = encode_path(&path);
        let response = send_request(
            port,
            &format!("GET /{encoded} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        );

        let (header, body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 200"),
            "expected 200, got: {header}"
        );
        assert_eq!(body, content);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn range_request_returns_partial_content() {
        let content = b"Hello, video server! This is a test.";
        let (path, _f) = temp_file(content, "range_partial");
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let encoded = encode_path(&path);
        let response = send_request(
            port,
            &format!("GET /{encoded} HTTP/1.1\r\nHost: 127.0.0.1\r\nRange: bytes=0-9\r\n\r\n"),
        );

        let (header, body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 206"),
            "expected 206, got: {header}"
        );
        assert!(
            header.contains("Content-Range: bytes 0-9/"),
            "missing Content-Range, header: {header}"
        );
        assert_eq!(
            body,
            &content[..10],
            "body='{:?}'",
            std::str::from_utf8(body)
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn range_request_past_end_returns_416() {
        let content = b"Short file.";
        let (path, _f) = temp_file(content, "range_past_end");
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let encoded = encode_path(&path);
        let response = send_request(
            port,
            &format!("GET /{encoded} HTTP/1.1\r\nHost: 127.0.0.1\r\nRange: bytes=100-200\r\n\r\n"),
        );

        let (header, _body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 416"),
            "expected 416, got: {header}"
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn nonexistent_path_returns_404() {
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let response = send_request(
            port,
            "GET /nonexistent/file.mp4 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        );

        let (header, _body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 404"),
            "expected 404, got: {header}"
        );
    }

    #[test]
    fn path_with_dotdot_returns_400() {
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let response = send_request(
            port,
            "GET /%2E%2E%2Fetc%2Fpasswd HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        );

        let (header, _body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 400"),
            "expected 400, got: {header}"
        );
    }

    #[test]
    fn non_get_method_returns_405() {
        let server = VideoServer::start().unwrap();
        let port = server.port();

        let response = send_request(port, "POST /file.mp4 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");

        let (header, _body) = split_http(&response);
        assert!(
            header.starts_with("HTTP/1.1 405"),
            "expected 405, got: {header}"
        );
    }

    fn split_http(response: &[u8]) -> (String, &[u8]) {
        let double_crlf = b"\r\n\r\n";
        if let Some(pos) = response
            .windows(double_crlf.len())
            .position(|w| w == double_crlf)
        {
            let header = String::from_utf8_lossy(&response[..pos]).to_string();
            let body = &response[pos + double_crlf.len()..];
            (header, body)
        } else {
            (String::from_utf8_lossy(response).to_string(), &[])
        }
    }
}
