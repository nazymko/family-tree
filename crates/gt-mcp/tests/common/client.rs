#![allow(dead_code, clippy::unwrap_used, clippy::indexing_slicing)]
//! A client that speaks to the real listener over a plain socket, one connection per request.
//! Included by `#[path = "common/client.rs"] mod client;` next to `mod common;`.

use crate::common::{KEY, request};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct HttpClient {
    pub port: u16,
    pub key: String,
}

impl HttpClient {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            key: KEY.into(),
        }
    }

    /// POSTs one JSON-RPC request to `/mcp` and returns the parsed body.
    pub fn rpc(&self, method: &str, params: Value) -> Value {
        let body = request(method, params).to_string();
        let head = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            self.port,
            self.key,
            body.len()
        );
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        s.write_all(head.as_bytes()).unwrap();
        s.write_all(body.as_bytes()).unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).unwrap();
        let at = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let status = String::from_utf8_lossy(&raw[..12]).to_string();
        assert!(status.contains("200"), "{status}");
        serde_json::from_slice(&raw[at..]).unwrap()
    }

    /// A tool call; its whole `result`.
    pub fn call_raw(&self, tool: &str, args: Value) -> Value {
        let reply = self.rpc("tools/call", json!({"name": tool, "arguments": args}));
        assert!(reply.get("error").is_none(), "protocol error: {reply}");
        reply["result"].clone()
    }

    /// A tool call that must succeed; its first text block parsed as JSON.
    pub fn call(&self, tool: &str, args: Value) -> Value {
        let result = self.call_raw(tool, args);
        assert_ne!(result["isError"], json!(true), "tool error: {result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    /// A tool call that must fail; its message.
    pub fn call_err(&self, tool: &str, args: Value) -> String {
        let result = self.call_raw(tool, args);
        assert_eq!(
            result["isError"],
            json!(true),
            "expected an error: {result}"
        );
        result["content"][0]["text"].as_str().unwrap().to_string()
    }
}
