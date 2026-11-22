use std::io::{BufRead, BufReader, Read, Write};

pub struct Limits {
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError {
    pub status: u16,
    pub message: String,
}

fn bad(status: u16, message: impl Into<String>) -> HttpError {
    HttpError { status, message: message.into() }
}

pub fn read_request(stream: impl Read, limits: &Limits) -> Result<Request, HttpError> {
    let mut reader = BufReader::new(stream);
    let mut head_bytes = 0usize;
    let mut line = String::new();
    let mut read_line = |reader: &mut BufReader<_>, line: &mut String| -> Result<(), HttpError> {
        line.clear();
        let n = reader.by_ref().take((limits.max_header_bytes - head_bytes + 1) as u64).read_line(line).map_err(|e| bad(400, e.to_string()))?;
        head_bytes += n;
        if head_bytes > limits.max_header_bytes {
            return Err(bad(431, "request head too large"));
        }
        if n == 0 {
            return Err(bad(400, "connection closed before the request ended"));
        }
        Ok(())
    };

    read_line(&mut reader, &mut line)?;
    let request_line = line.trim_end_matches(['\r', '\n']).to_string();
    let mut parts = request_line.split(' ');
    let (method, target, version) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if method.is_empty() || !target.starts_with('/') || !version.starts_with("HTTP/1.") || parts.next().is_some() {
        return Err(bad(400, "malformed request line"));
    }
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let path = percent_decode(raw_path).ok_or_else(|| bad(400, "bad percent-encoding in path"))?;
    let query = raw_query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            Some((percent_decode(k)?, percent_decode(v)?))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| bad(400, "bad percent-encoding in query"))?;

    let mut headers = Vec::new();
    loop {
        read_line(&mut reader, &mut line)?;
        let l = line.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            break;
        }
        let (k, v) = l.split_once(':').ok_or_else(|| bad(400, "malformed header"))?;
        headers.push((k.trim().to_string(), v.trim().to_string()));
    }
    let header = |name: &str| headers.iter().find(|(k, _): &&(String, String)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
    if header("Transfer-Encoding").is_some() {
        return Err(bad(411, "send a Content-Length body; chunked transfer is not supported"));
    }
    let len = match header("Content-Length") {
        None => 0,
        Some(v) => v.parse::<usize>().map_err(|_| bad(400, "bad Content-Length"))?,
    };
    if len > limits.max_body_bytes {
        return Err(bad(413, format!("body larger than {} MB", limits.max_body_bytes / 1_000_000)));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).map_err(|_| bad(400, "body shorter than Content-Length"))?;
    Ok(Request { method: method.to_string(), path, query, headers, body })
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Response { status, content_type: "application/json", body: value.to_string().into_bytes() }
    }

    pub fn error(err: &HttpError) -> Self {
        Self::json(err.status, &serde_json::json!({ "error": err.message }))
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    }
}

pub fn write_response(mut stream: impl Write, res: &Response) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        res.status,
        reason(res.status),
        res.content_type,
        res.body.len()
    )?;
    stream.write_all(&res.body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: Limits = Limits { max_header_bytes: 1024, max_body_bytes: 64 };

    fn parse(raw: &str) -> Result<Request, HttpError> {
        read_request(raw.as_bytes(), &LIMITS)
    }

    #[test]
    fn parses_path_query_headers_and_body() {
        let r = parse("POST /api/jobs?bpm=128&name=a%20b%E3%81%82+c HTTP/1.1\r\nHost: 127.0.0.1:8977\r\nContent-Length: 3\r\n\r\nabc").unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/api/jobs"));
        assert_eq!(r.param("bpm"), Some("128"));
        assert_eq!(r.param("name"), Some("a bあ c"));
        assert_eq!(r.header("host"), Some("127.0.0.1:8977"));
        assert_eq!(r.body, b"abc");
    }

    #[test]
    fn rejects_oversize_chunked_short_and_malformed_requests() {
        let status = |raw: &str| parse(raw).unwrap_err().status;
        assert_eq!(status(&format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(2000))), 431);
        assert_eq!(status("POST / HTTP/1.1\r\nContent-Length: 65\r\n\r\n"), 413);
        assert_eq!(status("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"), 411);
        assert_eq!(status("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc"), 400);
        assert_eq!(status("GET / HTTP/1.1 extra\r\n\r\n"), 400);
        assert_eq!(status("GET nope HTTP/1.1\r\n\r\n"), 400);
        assert_eq!(status("GET /%zz HTTP/1.1\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\nno-colon\r\n\r\n"), 400);
    }

    #[test]
    fn writes_a_closed_response_with_length() {
        let mut out = Vec::new();
        write_response(&mut out, &Response::json(404, &serde_json::json!({"error": "x"}))).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(text.contains("Content-Length: 13\r\n") && text.ends_with("{\"error\":\"x\"}"));
    }
}
