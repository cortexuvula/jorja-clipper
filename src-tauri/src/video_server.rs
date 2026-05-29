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
        // Find an available port
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind to port: {}", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?
            .port();

        self.port = port;
        *self.video_path.lock().unwrap() = Some(video_path.clone());

        let video_path = Arc::clone(&self.video_path);

        thread::spawn(move || {
            for request in listener.incoming() {
                match request {
                    Ok(mut stream) => {
                        let video_path = video_path.lock().unwrap().clone();
                        if let Some(path) = video_path {
                            if let Err(e) = handle_request(&mut stream, &path) {
                                eprintln!("Error handling request: {}", e);
                            }
                        }
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
    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
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

    // Open the file
    let mut file = File::open(video_path)?;
    let file_size = file.metadata()?.len();

    // Determine MIME type
    let mime_type = "video/mp4";

    // Check for Range header
    let (start, end, status_code) = if let Some(range) = headers.get("range") {
        if let Some(range_value) = range.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_value.split('-').collect();
            let start: u64 = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
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
    Ok(())
}
