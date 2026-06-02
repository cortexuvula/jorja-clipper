use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct VideoServer {
    port: u16,
    video_path: Arc<Mutex<Option<PathBuf>>>,
}

impl VideoServer {
    pub fn new() -> Self {
        Self {
            port: 0,
            video_path: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&mut self, video_path: PathBuf) -> Result<u16, String> {
        // Always update the video path (for both new and existing servers)
        *self
            .video_path
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))? = Some(video_path.clone());

        // If server already running, return existing port
        if self.port != 0 {
            return Ok(self.port);
        }

        // Find an available port
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind to port: {}", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?
            .port();

        self.port = port;

        let video_path = Arc::clone(&self.video_path);

        thread::spawn(move || {
            for request in listener.incoming() {
                match request {
                    Ok(stream) => {
                        let video_path = match video_path.lock() {
                            Ok(guard) => guard.clone(),
                            Err(e) => {
                                eprintln!("Mutex poisoned: {}", e);
                                continue;
                            }
                        };
                        // Handle each request in its own thread to support
                        // concurrent range requests from the HTML5 video element
                        thread::spawn(move || {
                            let mut stream = stream;
                            if let Some(path) = video_path {
                                if let Err(e) = handle_request(&mut stream, &path) {
                                    eprintln!("Error handling request: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Connection failed: {}", e);
                    }
                }
            }
        });

        Ok(port)
    }
}

fn handle_request(
    stream: &mut std::net::TcpStream,
    video_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, BufReader};

    let mut reader = BufReader::new(&*stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the request line
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let _path = parts[1];

    // Read headers
    let mut headers = std::collections::HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
    }

    if method != "GET" && method != "HEAD" {
        let response = "HTTP/1.1 405 Method Not Allowed\r\n\r\n";
        stream.write_all(response.as_bytes())?;
        return Ok(());
    }

    // Check if file exists before opening
    if !video_path.exists() {
        let body = "Video file not found";
        let response = format!(
            "HTTP/1.1 404 Not Found\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n\
             {}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes())?;
        return Ok(());
    }

    // Open the file
    let mut file = File::open(video_path)?;
    let file_size = file.metadata()?.len();

    // Handle empty files
    if file_size == 0 {
        let body = "Empty file";
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes())?;
        return Ok(());
    }

    // Determine MIME type from file extension
    let mime_type = match video_path.extension().and_then(|e| e.to_str()) {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("ogg") | Some("ogv") => "video/ogg",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        Some("ts") => "video/mp2t",
        _ => "application/octet-stream",
    };

    // Check for Range header
    let (start, end, status_code) = if let Some(range) = headers.get("range") {
        if let Some(range_value) = range.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_value.split('-').collect();
            let start: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let end: u64 = parts
                .get(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(file_size - 1)
                .min(file_size - 1);
            (start, end, 206)
        } else {
            (0, file_size - 1, 200)
        }
    } else {
        (0, file_size - 1, 200)
    };

    let content_length = end - start + 1;

    // Build response headers
    let mut response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Accept-Ranges: bytes\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n",
        status_code,
        if status_code == 206 {
            "Partial Content"
        } else {
            "OK"
        },
        mime_type,
        content_length
    );

    if status_code == 206 {
        response.push_str(&format!(
            "Content-Range: bytes {}-{}/{}\r\n",
            start, end, file_size
        ));
    }

    response.push_str("\r\n");

    // Write headers
    stream.write_all(response.as_bytes())?;

    // Write body (if not HEAD request)
    if method == "GET" {
        file.seek(SeekFrom::Start(start))?;

        let mut remaining = content_length;
        let mut buffer = vec![0u8; 8192];

        while remaining > 0 {
            let to_read = std::cmp::min(buffer.len() as u64, remaining) as usize;
            match file.read(&mut buffer[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    stream.write_all(&buffer[..n])?;
                    remaining -= n as u64;
                }
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                    break;
                }
            }
        }
    }

    stream.flush()?;
    println!(
        "[video-server] {} {} ({}-{}) {} bytes",
        method, status_code, start, end, content_length
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpStream;
    use tempfile::NamedTempFile;

    fn send_request(port: u16, request: &str) -> (String, Vec<u8>) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(&stream);

        // Read status line
        let mut status_line = String::new();
        reader.read_line(&mut status_line).unwrap();

        // Read headers
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            if line.to_lowercase().starts_with("content-length:") {
                content_length = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
            }
        }

        // For HEAD requests, no body is sent even if content-length is set
        if request.starts_with("HEAD") {
            return (status_line.trim().to_string(), Vec::new());
        }

        // Read body - use read_to_end for robustness
        let mut body = Vec::new();
        if content_length > 0 {
            body.resize(content_length, 0);
            let _ = reader.read_exact(&mut body);
        }

        (status_line.trim().to_string(), body)
    }

    #[test]
    fn test_video_server_start_and_serve() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test video content").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        assert!(port > 0);

        // Test GET request
        let (status, body) = send_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.contains("200"));
        assert_eq!(body, b"test video content");
    }

    #[test]
    fn test_video_server_range_request() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"0123456789ABCDEF").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Test range request (bytes 5-9)
        let (status, body) = send_request(
            port,
            "GET / HTTP/1.1\r\nHost: localhost\r\nRange: bytes=5-9\r\n\r\n",
        );

        assert!(status.contains("206"));
        assert_eq!(body, b"56789");
    }

    #[test]
    fn test_video_server_head_request() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test content").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Test HEAD request (should return headers but no body)
        let (status, body) = send_request(port, "HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n");

        assert!(status.contains("200"));
        assert_eq!(body.len(), 0); // HEAD should have no body
    }

    #[test]
    fn test_video_server_file_not_found() {
        let mut server = VideoServer::new();

        // Start with a path that doesn't exist
        let fake_path = PathBuf::from("/nonexistent/video.mp4");
        let port = server.start(fake_path).unwrap();

        let (status, body) = send_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.contains("404"));
        assert!(String::from_utf8_lossy(&body).contains("not found"));
    }

    #[test]
    fn test_video_server_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        // Don't write anything - file is empty

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        let (status, _) = send_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.contains("400"));
    }

    #[test]
    fn test_video_server_reuse_port() {
        let temp_file1 = NamedTempFile::new().unwrap();
        std::fs::write(temp_file1.path(), b"video1").unwrap();

        let temp_file2 = NamedTempFile::new().unwrap();
        std::fs::write(temp_file2.path(), b"video2").unwrap();

        let mut server = VideoServer::new();
        let port1 = server.start(temp_file1.path().to_path_buf()).unwrap();

        // Start again with different file - should reuse port
        let port2 = server.start(temp_file2.path().to_path_buf()).unwrap();
        assert_eq!(port1, port2);

        // Should now serve the second file
        let (status, body) = send_request(port2, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.contains("200"));
        assert_eq!(body, b"video2");
    }

    #[test]
    fn test_video_server_method_not_allowed() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        let (status, _) = send_request(port, "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.contains("405"));
    }

    #[test]
    fn test_video_server_invalid_request_line() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Send malformed request (no path)
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(b"INVALID\r\n\r\n").unwrap();
        stream.flush().unwrap();

        // Server should handle gracefully (connection closes)
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    #[test]
    fn test_video_server_different_mime_types() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Test various video formats
        let formats = vec![
            ("video.mp4", "video/mp4"),
            ("video.m4v", "video/mp4"),
            ("video.webm", "video/webm"),
            ("video.ogg", "video/ogg"),
            ("video.ogv", "video/ogg"),
            ("video.mkv", "video/x-matroska"),
            ("video.avi", "video/x-msvideo"),
            ("video.mov", "video/quicktime"),
            ("video.ts", "video/mp2t"),
            ("video.xyz", "application/octet-stream"),
        ];

        for (filename, _expected_mime) in formats {
            let video_file = temp_dir.path().join(filename);
            std::fs::write(&video_file, b"test video").unwrap();

            let mut server = VideoServer::new();
            let port = server.start(video_file).unwrap();

            let (status, body) = send_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
            assert!(status.contains("200"), "Failed for {}", filename);
            assert_eq!(body, b"test video");
        }
    }

    #[test]
    fn test_video_server_range_request_open_ended() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"0123456789ABCDEF").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Test open-ended range (bytes=5-)
        let (status, body) = send_request(
            port,
            "GET / HTTP/1.1\r\nHost: localhost\r\nRange: bytes=5-\r\n\r\n",
        );

        assert!(status.contains("206"));
        assert_eq!(body, b"56789ABCDEF");
    }

    #[test]
    fn test_video_server_invalid_range_header() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"0123456789ABCDEF").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Test invalid range header (not starting with "bytes=")
        let (status, body) = send_request(
            port,
            "GET / HTTP/1.1\r\nHost: localhost\r\nRange: items=0-4\r\n\r\n",
        );

        // Should return 200 OK (not 206) because the range header is invalid
        assert!(status.contains("200"));
        assert_eq!(body, b"0123456789ABCDEF");
    }

    #[test]
    fn test_video_server_multiple_requests() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test content").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Make multiple requests to the same server
        for _ in 0..3 {
            let (status, body) = send_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");

            assert!(status.contains("200"));
            assert_eq!(body, b"test content");
        }
    }

    #[test]
    fn test_video_server_large_file() {
        let temp_file = NamedTempFile::new().unwrap();

        // Create a large file (1MB)
        let large_content = vec![b'x'; 1024 * 1024];
        std::fs::write(temp_file.path(), &large_content).unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Request a range from the large file
        let (status, body) = send_request(
            port,
            "GET / HTTP/1.1\r\nHost: localhost\r\nRange: bytes=0-999\r\n\r\n",
        );

        assert!(status.contains("206"));
        assert_eq!(body.len(), 1000);
    }

    #[test]
    fn test_video_server_delete_request() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"test").unwrap();

        let mut server = VideoServer::new();
        let port = server.start(temp_file.path().to_path_buf()).unwrap();

        // Send a DELETE request (unsupported method)
        let (status, _) = send_request(port, "DELETE / HTTP/1.1\r\nHost: localhost\r\n\r\n");

        // Should return 405 Method Not Allowed
        assert!(status.contains("405"));
    }
}
