extern crate languageserver_types as ty;
extern crate linked_hash_map;
extern crate log;
extern crate reproto_ast as ast;
extern crate reproto_core as core;
extern crate reproto_env as env;
extern crate reproto_lexer as lexer;
extern crate reproto_manifest as manifest;
extern crate reproto_parser as parser;
extern crate serde;
extern crate serde_json as json;
#[macro_use]
extern crate serde_derive;
extern crate ropey;
extern crate url;
extern crate url_serde;

mod envelope;
mod internal_log;
mod workspace;

use self::ContentType::*;
use self::workspace::{LoadedFile, Position, Workspace};
use core::errors::Result;
use serde::Deserialize;
use std::collections::Bound;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::result;
use url::Url;

#[derive(Debug)]
enum ContentType {
    JsonRPC,
}

#[derive(Debug)]
struct Headers {
    content_type: ContentType,
    content_length: u32,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            content_type: JsonRPC,
            content_length: 0u32,
        }
    }

    fn clear(&mut self) {
        self.content_type = JsonRPC;
        self.content_length = 0;
    }
}

struct InputReader<R> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R> InputReader<R>
where
    R: BufRead,
{
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
        }
    }

    fn next_line<'a>(&'a mut self) -> Result<Option<&'a [u8]>> {
        self.buffer.clear();
        self.reader.read_until('\n' as u8, &mut self.buffer)?;

        if self.buffer.is_empty() {
            return Ok(None);
        }

        Ok(Some(trim(&self.buffer)))
    }
}

impl<R> Read for InputReader<R>
where
    R: BufRead,
{
    fn read(&mut self, buf: &mut [u8]) -> result::Result<usize, io::Error> {
        self.reader.read(buf)
    }
}

pub fn server<L, R, W: 'static>(log: &mut L, reader: R, writer: W) -> Result<()>
where
    L: internal_log::InternalLog,
    R: Read,
    W: Send + Write,
{
    match try_server(log, reader, writer) {
        Err(e) => {
            writeln!(log, "error: {}", e.display())?;

            for cause in e.causes().skip(1) {
                writeln!(log, "caused by: {}", cause.display())?;
            }

            return Err(e);
        }
        Ok(()) => {
            return Ok(());
        }
    }
}

pub fn try_server<L, R, W: 'static>(log: &mut L, reader: R, writer: W) -> Result<()>
where
    L: internal_log::InternalLog,
    R: Read,
    W: Send + Write,
{
    let mut server = Server::new(log, reader, writer);
    server.run()?;
    Ok(())
}

/// Trim the string from whitespace.
fn trim(data: &[u8]) -> &[u8] {
    let s = data.iter()
        .position(|b| *b != b'\n' && *b != b'\r' && *b != b' ')
        .unwrap_or(data.len());

    let data = &data[s..];

    let e = data.iter()
        .rev()
        .position(|b| *b != b'\n' && *b != b'\r' && *b != b' ')
        .map(|p| data.len() - p)
        .unwrap_or(0usize);

    &data[..e]
}

/// Server abstraction
pub struct Server<'a, L: 'a, R, W> {
    workspace: Option<Workspace>,
    buffer: Vec<u8>,
    headers: Headers,
    log: &'a mut L,
    reader: InputReader<BufReader<R>>,
    writer: W,
}

impl<'a, L, R, W> Server<'a, L, R, W>
where
    L: internal_log::InternalLog,
    R: Read,
    W: Write,
{
    pub fn new(log: &'a mut L, reader: R, writer: W) -> Self {
        Self {
            workspace: None,
            buffer: Vec::new(),
            headers: Headers::new(),
            log: log,
            reader: InputReader::new(BufReader::new(reader)),
            writer: writer,
        }
    }

    /// Send a complete frame.
    fn send_frame<T>(&mut self, response: T) -> Result<()>
    where
        T: fmt::Debug + serde::Serialize,
    {
        self.buffer.clear();
        json::to_writer(&mut self.buffer, &response)?;

        write!(self.writer, "Content-Length: {}\r\n\r\n", self.buffer.len())?;
        self.writer.write_all(&self.buffer)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Send a notification.
    fn send_notification<S: AsRef<str>, T>(&mut self, method: S, params: T) -> Result<()>
    where
        T: fmt::Debug + serde::Serialize,
    {
        let envelope = envelope::NotificationMessage::<T> {
            jsonrpc: "2.0",
            method: method.as_ref().to_string(),
            params: Some(params),
        };

        self.send_frame(envelope)
    }

    /// Send a response message.
    fn send<T>(&mut self, request_id: Option<envelope::RequestId>, message: T) -> Result<()>
    where
        T: fmt::Debug + serde::Serialize,
    {
        let envelope = envelope::ResponseMessage::<T, ()> {
            jsonrpc: "2.0",
            id: request_id,
            result: Some(message),
            error: None,
        };

        self.send_frame(envelope)
    }

    /// Send an error.
    fn send_error<D>(
        &mut self,
        request_id: Option<envelope::RequestId>,
        error: envelope::ResponseError<D>,
    ) -> Result<()>
    where
        D: fmt::Debug + serde::Serialize,
    {
        let envelope = envelope::ResponseMessage::<(), D> {
            jsonrpc: "2.0",
            id: request_id,
            result: None,
            error: Some(error),
        };

        self.send_frame(envelope)
    }

    /// Read headers.
    fn read_headers(&mut self) -> Result<bool> {
        self.headers.clear();

        loop {
            let line = self.reader.next_line()?;

            let line = match line {
                Some(line) => line,
                None => return Ok(false),
            };

            if line == b"" {
                break;
            }

            let mut parts = line.splitn(2, |b| *b == b':');

            let (key, value) = match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => (trim(key), trim(value)),
                out => {
                    return Err(format!("bad header: {:?}", out).into());
                }
            };

            match key {
                b"Content-Type" => match value {
                    b"application/vscode-jsonrpc; charset=utf-8" => {
                        self.headers.content_type = JsonRPC;
                    }
                    value => {
                        return Err(format!("bad value: {:?}", value).into());
                    }
                },
                b"Content-Length" => {
                    let value = ::std::str::from_utf8(value)
                        .map_err(|e| format!("bad content-length: {:?}: {}", value, e))?;

                    let value = value
                        .parse::<u32>()
                        .map_err(|e| format!("bad content-length: {}: {}", value, e))?;

                    self.headers.content_length = value;
                }
                key => {
                    return Err(format!("bad header: {:?}", key).into());
                }
            }
        }

        Ok(true)
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            if !self.read_headers()? {
                break;
            }

            if self.headers.content_length == 0 {
                continue;
            }

            match self.headers.content_type {
                JsonRPC => {
                    let request: envelope::RequestMessage = {
                        let reader = (&mut self.reader).take(self.headers.content_length as u64);
                        json::from_reader(reader)?
                    };

                    match request.method.as_str() {
                        "initialize" => {
                            let params = ty::InitializeParams::deserialize(request.params)?;
                            self.initialize(request.id, params)?;
                        }
                        "initialized" => {
                            let params = ty::InitializedParams::deserialize(request.params)?;
                            self.initialized(request.id, params)?;
                        }
                        "shutdown" => {
                            self.shutdown()?;
                        }
                        "textDocument/didChange" => {
                            let params =
                                ty::DidChangeTextDocumentParams::deserialize(request.params)?;
                            self.text_document_did_change(params)?;
                        }
                        "textDocument/didOpen" => {
                            let params =
                                ty::DidOpenTextDocumentParams::deserialize(request.params)?;
                            self.text_document_did_open(params)?;
                        }
                        "textDocument/didSave" => {
                            let params =
                                ty::DidSaveTextDocumentParams::deserialize(request.params)?;
                            self.text_document_did_save(params)?;
                        }
                        "textDocument/completion" => {
                            let params = ty::CompletionParams::deserialize(request.params)?;
                            self.text_document_completion(request.id, params)?;
                        }
                        "textDocument/definition" => {
                            let params =
                                ty::TextDocumentPositionParams::deserialize(request.params)?;
                            self.text_document_definition(params)?;
                        }
                        "workspace/didChangeConfiguration" => {
                            let params =
                                ty::DidChangeConfigurationParams::deserialize(request.params)?;
                            self.workspace_did_change_configuration(request.id, params)?;
                        }
                        method => {
                            writeln!(self.log, "unsupported method: {}", method)?;

                            self.send_error(
                                request.id,
                                envelope::ResponseError {
                                    code: envelope::Code::MethodNotFound,
                                    message: "No such method".to_string(),
                                    data: Some(()),
                                },
                            )?;

                            continue;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// Handler for `initialize`.
    fn initialize(
        &mut self,
        request_id: Option<envelope::RequestId>,
        params: ty::InitializeParams,
    ) -> Result<()> {
        if let Some(path) = params.root_path.as_ref() {
            let mut workspace = Workspace::new(path);

            if let Err(e) = workspace.reload(self.log) {
                writeln!(
                    self.log,
                    "error when loading workspace: {}: {}",
                    path,
                    e.display()
                )?;
            }

            self.workspace = Some(workspace);
        }

        let result = ty::InitializeResult {
            capabilities: ty::ServerCapabilities {
                text_document_sync: Some(ty::TextDocumentSyncCapability::Kind(
                    ty::TextDocumentSyncKind::Incremental,
                )),
                completion_provider: Some(ty::CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec!["::".into()]),
                }),
                definition_provider: Some(true),
                ..ty::ServerCapabilities::default()
            },
        };

        self.send(request_id, result)?;
        Ok(())
    }

    /// Handler for `initialized`.
    fn initialized(
        &mut self,
        request_id: Option<envelope::RequestId>,
        _params: ty::InitializedParams,
    ) -> Result<()> {
        writeln!(self.log, "initialized: {:?}", request_id)?;
        Ok(())
    }

    /// Handler for `workspace/didChangeConfiguration`.
    fn workspace_did_change_configuration(
        &mut self,
        _: Option<envelope::RequestId>,
        _: ty::DidChangeConfigurationParams,
    ) -> Result<()> {
        Ok(())
    }

    fn send_diagnostics(&mut self, url: &Url, file: &LoadedFile) -> Result<()> {
        let mut diagnostics = Vec::new();

        for d in file.diag.items() {
            match *d {
                core::Diagnostic::Error(ref span, ref m) => {
                    let (line_start, line_end, col_start, col_end) =
                        core::utils::find_range(file.diag.source.read()?, (span.start, span.end))?;

                    let start = ty::Position {
                        line: line_start as u64,
                        character: col_start as u64,
                    };

                    let end = ty::Position {
                        line: line_end as u64,
                        character: col_end as u64,
                    };

                    let range = ty::Range { start, end };

                    let d = ty::Diagnostic {
                        range: range,
                        message: m.to_string(),
                        ..ty::Diagnostic::default()
                    };

                    diagnostics.push(d);
                }
                _ => {}
            }
        }

        self.send_notification(
            "textDocument/publishDiagnostics",
            ty::PublishDiagnosticsParams {
                uri: url.clone(),
                diagnostics: diagnostics,
            },
        )?;

        Ok(())
    }

    /// Handler for `textDocument/didSave`.
    fn text_document_did_save(&mut self, _: ty::DidSaveTextDocumentParams) -> Result<()> {
        if let Some(mut workspace) = self.workspace.take() {
            if let Err(e) = workspace.reload(self.log) {
                writeln!(
                    self.log,
                    "error when reloading workspace: {}: {}",
                    workspace.root_path.display(),
                    e.display()
                )?;
            }

            for (path, file) in &workspace.files {
                let file = file.try_borrow()?;
                self.send_diagnostics(path, &file)?;
            }

            self.workspace = Some(workspace);
        }

        Ok(())
    }

    /// Handler for `textDocument/didChange`.
    fn text_document_did_change(&mut self, _: ty::DidChangeTextDocumentParams) -> Result<()> {
        Ok(())
    }

    /// Handler for `textDocument/didOpen`.
    fn text_document_did_open(&mut self, _: ty::DidOpenTextDocumentParams) -> Result<()> {
        Ok(())
    }

    /// Handler for `textDocument/completion`.
    fn text_document_completion(
        &mut self,
        request_id: Option<envelope::RequestId>,
        params: ty::CompletionParams,
    ) -> Result<()> {
        writeln!(self.log, "params: {:#?}", params)?;
        let mut items: Vec<ty::CompletionItem> = Vec::new();

        let url = params.text_document.uri;
        let position = params.position;

        if let Some(workspace) = self.workspace.as_mut() {
            if let Some(file) = workspace.files.get(&url) {
                let file = file.try_borrow()?;

                let end = Position {
                    line: position.line,
                    col: position.character,
                };

                writeln!(self.log, "end: {:?}, {:?}", end, file.types)?;

                let mut range = file.types.range((Bound::Unbounded, Bound::Included(&end)));

                if let Some((_, (range, value))) = range.next_back() {
                    if range.contains(&end) {
                        writeln!(self.log, "match: {:?} => {:?} => {:?}", end, range, value)?;
                    }
                }
            }
        }

        self.send(request_id, items)?;
        Ok(())
    }

    /// Handler for `textDocument/definition`.
    fn text_document_definition(&mut self, _: ty::TextDocumentPositionParams) -> Result<()> {
        Ok(())
    }
}
