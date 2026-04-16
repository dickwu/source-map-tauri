use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};
use lsp_types::{
    DidOpenTextDocumentParams, DocumentSymbol, InitializedParams, Range, TextDocumentIdentifier,
    TextDocumentItem, Uri,
};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct SymbolLocation {
    pub name: String,
    pub kind: lsp_types::SymbolKind,
    pub range: Range,
}

pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl LspClient {
    pub fn new(command: &str, root: &Path) -> Result<Self> {
        let mut child = Command::new(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn LSP server {}", command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("missing LSP stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing LSP stdout"))?;
        let mut client = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        client.initialize(root)?;
        Ok(client)
    }

    pub fn document_symbols(
        &mut self,
        path: &Path,
        text: &str,
        language_id: &str,
    ) -> Result<Vec<SymbolLocation>> {
        let uri = url::Url::from_file_path(path)
            .map_err(|_| anyhow!("failed to build file URI for {}", path.display()))?
            .to_string();
        let uri = Uri::from_str(&uri)
            .map_err(|_| anyhow!("failed to parse file URI for {}", path.display()))?;
        let open = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: language_id.to_owned(),
                version: 1,
                text: text.to_owned(),
            },
        };
        self.notify("textDocument/didOpen", serde_json::to_value(open)?)?;
        let response = self.request(
            "textDocument/documentSymbol",
            json!({
                "textDocument": TextDocumentIdentifier { uri }
            }),
        )?;
        let symbols = parse_document_symbol_response(&response)?;
        Ok(symbols)
    }

    fn initialize(&mut self, root: &Path) -> Result<()> {
        let root_uri = url::Url::from_directory_path(root)
            .map_err(|_| anyhow!("failed to build root URI for {}", root.display()))?
            .to_string();
        let root_uri = Uri::from_str(&root_uri)
            .map_err(|_| anyhow!("failed to parse root URI for {}", root.display()))?;
        let params = json!({
            "processId": null,
            "capabilities": {},
            "workspaceFolders": [
                {
                    "uri": root_uri,
                    "name": root
                        .file_name()
                        .and_then(|item| item.to_str())
                        .unwrap_or("workspace")
                }
            ]
        });
        let _ = self.request("initialize", params)?;
        self.notify("initialized", serde_json::to_value(InitializedParams {})?)?;
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&payload)?;
        loop {
            let message = self.read_message()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(anyhow!("LSP request {} failed: {}", method, error));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&payload)
    }

    fn write_message(&mut self, payload: &Value) -> Result<()> {
        let body = serde_json::to_vec(payload)?;
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len())?;
        self.stdin.write_all(&body)?;
        self.stdin.flush()?;
        Ok(())
    }

    fn read_message(&mut self) -> Result<Value> {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line)?;
            if line.is_empty() {
                return Err(anyhow!("LSP server closed stdout"));
            }
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(value.trim().parse::<usize>()?);
            }
        }
        let length = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
        let mut body = vec![0_u8; length];
        self.stdout.read_exact(&mut body)?;
        Ok(serde_json::from_slice(&body)?)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.request("shutdown", Value::Null);
        let _ = self.notify("exit", Value::Null);
        let _ = self.child.kill();
    }
}

fn parse_document_symbol_response(value: &Value) -> Result<Vec<SymbolLocation>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let symbols: Vec<DocumentSymbol> = serde_json::from_value(value.clone())?;
    let mut flattened = Vec::new();
    for symbol in symbols {
        flatten_document_symbol(&symbol, &mut flattened);
    }
    Ok(flattened)
}

fn flatten_document_symbol(symbol: &DocumentSymbol, out: &mut Vec<SymbolLocation>) {
    out.push(SymbolLocation {
        name: symbol.name.clone(),
        kind: symbol.kind,
        range: symbol.range,
    });
    if let Some(children) = &symbol.children {
        for child in children {
            flatten_document_symbol(child, out);
        }
    }
}

pub fn line_contains(range: &Range, one_based_line: u32) -> bool {
    let zero = one_based_line.saturating_sub(1);
    range.start.line <= zero && zero <= range.end.line
}

pub fn range_start_line(range: &Range) -> u32 {
    range.start.line + 1
}

pub fn range_end_line(range: &Range) -> u32 {
    range.end.line + 1
}
