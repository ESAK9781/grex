//! Lightweight stdio-to-HTTP MCP shim binary (`grex-mcp`).
//! Reads newline-delimited JSON-RPC 2.0 requests from stdin and forwards them
//! to the embedded Grex MCP server at `http://127.0.0.1:${FACTORY_MCP_PORT}/mcp`.

use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;

fn main() {
    let port = std::env::var("FACTORY_MCP_PORT").unwrap_or_else(|_| "4739".to_string());
    let addr = format!("127.0.0.1:{port}");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match post_json_rpc(&addr, trimmed) {
            Ok(body) => {
                if !body.trim().is_empty() {
                    let _ = writeln!(stdout, "{}", body.trim());
                    let _ = stdout.flush();
                }
            }
            Err(err) => {
                let fallback = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": serde_json::Value::Null,
                    "error": {
                        "code": -32000,
                        "message": format!("grex-mcp failed to reach Grex at {addr}: {err}")
                    }
                });
                let _ = writeln!(stdout, "{fallback}");
                let _ = stdout.flush();
            }
        }
    }
}

fn post_json_rpc(addr: &str, body: &str) -> io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;

    if let Some((_, payload)) = raw.split_once("\r\n\r\n") {
        Ok(payload.to_string())
    } else {
        Ok(raw)
    }
}
