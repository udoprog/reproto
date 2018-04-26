extern crate languageserver_types as ty;
extern crate linked_hash_map;
#[macro_use]
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
mod workspace;

use self::workspace::{Completion, Jump, LoadedFile, Workspace};
use self::ContentType::*;
use core::errors::Result;
use core::{Context, ContextItem, Diagnostics, Encoding, Loc, RealFilesystem, Source};
use ropey::Rope;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::ops::DerefMut;
use std::path::Path;
use std::rc::Rc;
use std::result;
use std::sync::{Arc, Mutex};
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

/// Reads input stream for server.
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

/// A language server channel, taking care of locking and sending notifications.
struct Channel<W> {
    /// A writer and buffer pair.
    output: Arc<Mutex<(Vec<u8>, W)>>,
}

impl<W> ::std::clone::Clone for Channel<W> {
    fn clone(&self) -> Self {
        Channel {
            output: Arc::clone(&self.output),
        }
    }
}

impl<W> Channel<W> {
    pub fn new(writer: W) -> Self {
        Self {
            output: Arc::new(Mutex::new((Vec::new(), writer))),
        }
    }
}

impl<W> Channel<W>
where
    W: Write,
{
    /// Send a complete frame.
    fn send_frame<T>(&self, response: T) -> Result<()>
    where
        T: fmt::Debug + serde::Serialize,
    {
        let mut guard = self.output.lock().map_err(|_| "lock poisoned")?;
        let &mut (ref mut buffer, ref mut writer) = guard.deref_mut();

        buffer.clear();
        json::to_writer(&mut *buffer, &response)?;

        write!(writer, "Content-Length: {}\r\n\r\n", buffer.len())?;
        writer.write_all(buffer)?;
        writer.flush()?;
        Ok(())
    }

    /// Send a notification.
    fn send_notification<S: AsRef<str>, T>(&self, method: S, params: T) -> Result<()>
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
    fn send<T>(&self, request_id: Option<envelope::RequestId>, message: T) -> Result<()>
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
        &self,
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
}

pub struct Logger<L>
where
    L: Send,
{
    log: Mutex<L>,
}

impl<L> Logger<L>
where
    L: Send,
{
    pub fn new(log: L) -> Self {
        Self {
            log: Mutex::new(log),
        }
    }
}

impl<L> log::Log for Logger<L>
where
    L: Send + Write,
{
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            let mut lock = self.log.lock().expect("poisoned lock");
            writeln!(lock, "{}: {}", record.level(), record.args()).unwrap();
        }
    }
}

/// Logger that sends logs as notifications to client.
struct NotificationLogger<W>
where
    W: Send,
{
    channel: Channel<W>,
}

impl<W> NotificationLogger<W>
where
    W: Send,
{
    pub fn new(channel: Channel<W>) -> Self {
        Self { channel }
    }
}

impl<W> log::Log for NotificationLogger<W>
where
    W: Send + Write,
{
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        use log::LogLevel::*;

        if !self.enabled(record.metadata()) {
            return;
        }

        let typ = match record.level() {
            Error => ty::MessageType::Error,
            Warn => ty::MessageType::Warning,
            Info => ty::MessageType::Info,
            _ => ty::MessageType::Log,
        };

        let notification = ty::LogMessageParams {
            typ,
            message: record.args().to_string(),
        };

        self.channel
            .send_notification("window/logMessage", notification)
            .expect("failed to send notification");
    }
}

pub fn server<L: 'static, R, W: 'static>(
    log: Option<L>,
    reader: R,
    writer: W,
    level: log::LogLevelFilter,
) -> Result<()>
where
    L: Send + Write,
    R: Read,
    W: Send + Write,
{
    let channel = Channel::new(writer);

    if let Some(log) = log {
        let logger = Logger::new(log);

        log::set_logger(|max_level| {
            max_level.set(level);
            Box::new(logger)
        })?;
    } else {
        log::set_logger(|max_level| {
            max_level.set(level);
            Box::new(NotificationLogger::new(channel.clone()))
        })?;
    }

    match try_server(reader, channel) {
        Err(e) => {
            error!("error: {}", e.display());

            for cause in e.causes().skip(1) {
                error!("caused by: {}", cause.display());
            }

            return Err(e);
        }
        Ok(()) => {
            return Ok(());
        }
    }
}

fn try_server<R, W: 'static>(reader: R, channel: Channel<W>) -> Result<()>
where
    R: Read,
    W: Send + Write,
{
    let ctx = Context::new(Box::new(RealFilesystem::new()));
    let mut server = Server::new(reader, channel, Rc::new(ctx));
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
struct Server<R, W> {
    workspace: Option<RefCell<Workspace>>,
    headers: Headers,
    reader: InputReader<BufReader<R>>,
    channel: Channel<W>,
    ctx: Rc<Context>,
}

impl<R, W> Server<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(reader: R, channel: Channel<W>, ctx: Rc<Context>) -> Self {
        Self {
            workspace: None,
            headers: Headers::new(),
            reader: InputReader::new(BufReader::new(reader)),
            channel,
            ctx,
        }
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

                    debug!("received: {:#?}", request);

                    match request.method.as_str() {
                        "initialize" => {
                            let params = ty::InitializeParams::deserialize(request.params)?;
                            self.initialize(request.id, params)?;
                        }
                        "initialized" => {
                            let params = ty::InitializedParams::deserialize(request.params)?;
                            self.initialized(params)?;
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
                        "textDocument/didClose" => {
                            let params =
                                ty::DidCloseTextDocumentParams::deserialize(request.params)?;
                            self.text_document_did_close(params)?;
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
                            self.text_document_definition(request.id, params)?;
                        }
                        "workspace/didChangeConfiguration" => {
                            let params =
                                ty::DidChangeConfigurationParams::deserialize(request.params)?;
                            self.workspace_did_change_configuration(request.id, params)?;
                        }
                        "completionItem/resolve" => {
                            let params = ty::CompletionItem::deserialize(request.params)?;
                            self.completion_item_resolve(params)?;
                        }
                        "$/cancelRequest" => {
                            // ignore
                        }
                        method => {
                            error!("unsupported method: {}", method);

                            self.channel.send_error(
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
            let path = Path::new(path.as_str());

            debug!("loading project: {}", path.display());

            let path = path.canonicalize()
                .map_err(|_| format!("could not canonicalize root path: {}", path.display()))?;

            let mut workspace = Workspace::new(path, self.ctx.clone());
            workspace.reload()?;
            self.workspace = Some(RefCell::new(workspace));
        }

        self.send_workspace_diagnostics()?;

        let result = ty::InitializeResult {
            capabilities: ty::ServerCapabilities {
                text_document_sync: Some(ty::TextDocumentSyncCapability::Kind(
                    ty::TextDocumentSyncKind::Incremental,
                )),
                completion_provider: Some(ty::CompletionOptions {
                    trigger_characters: Some(vec![":".into(), ".".into()]),
                    ..ty::CompletionOptions::default()
                }),
                definition_provider: Some(true),
                ..ty::ServerCapabilities::default()
            },
        };

        self.channel.send(request_id, result)?;
        Ok(())
    }

    /// Handler for `initialized`.
    fn initialized(&mut self, _params: ty::InitializedParams) -> Result<()> {
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

    /// Send all diagnostics for a workspace.
    fn send_workspace_diagnostics(&self) -> Result<()> {
        let mut diagnostics_by_url = HashMap::new();

        for item in self.ctx.items()?.iter() {
            match *item {
                ContextItem::Diagnostics { ref diagnostics } => {
                    if let Some(url) = diagnostics.source.url() {
                        diagnostics_by_url.insert(url, diagnostics.clone());
                    }
                }
            }
        }

        if let Some(workspace) = self.workspace.as_ref() {
            let workspace = workspace
                .try_borrow()
                .map_err(|_| "failed to access workspace immutably")?;

            for (url, file) in workspace.files() {
                self.send_diagnostics(url, &file, &diagnostics_by_url)?;
            }
        }

        Ok(())
    }

    /// Send diagnostics for a single file.
    fn send_diagnostics(
        &self,
        url: &Url,
        file: &LoadedFile,
        diagnostics_by_url: &HashMap<Url, Diagnostics>,
    ) -> Result<()> {
        let mut diagnostics = Vec::new();

        let by_url = diagnostics_by_url.get(url);
        let by_url_chain = by_url.iter().flat_map(|d| d.items());

        for d in file.diag.items().chain(by_url_chain) {
            match *d {
                core::Diagnostic::Error(ref span, ref m) => {
                    let (start, end) = file.diag.source.span_to_range(*span, Encoding::Utf16)?;
                    let range = convert_range(start, end);

                    let d = ty::Diagnostic {
                        range: range,
                        message: m.to_string(),
                        severity: Some(ty::DiagnosticSeverity::Error),
                        ..ty::Diagnostic::default()
                    };

                    diagnostics.push(d);
                }
                core::Diagnostic::Info(ref span, ref m) => {
                    info!("info: {:?}: {}", span, m);

                    let (start, end) = file.diag.source.span_to_range(*span, Encoding::Utf16)?;
                    let range = convert_range(start, end);

                    let d = ty::Diagnostic {
                        range: range,
                        message: m.to_string(),
                        severity: Some(ty::DiagnosticSeverity::Information),
                        ..ty::Diagnostic::default()
                    };

                    diagnostics.push(d);
                }
                _ => {}
            }
        }

        self.channel.send_notification(
            "textDocument/publishDiagnostics",
            ty::PublishDiagnosticsParams {
                uri: url.clone(),
                diagnostics: diagnostics,
            },
        )?;

        Ok(())
    }

    /// Handler for `textDocument/didSave`.
    fn text_document_did_save(&self, _: ty::DidSaveTextDocumentParams) -> Result<()> {
        if let Some(workspace) = self.workspace.as_ref() {
            let mut workspace = workspace
                .try_borrow_mut()
                .map_err(|_| "failed to access mutable workspace")?;

            workspace.reload()?;
        }

        self.send_workspace_diagnostics()?;
        Ok(())
    }

    /// Handler for `textDocument/didChange`.
    fn text_document_did_change(&self, params: ty::DidChangeTextDocumentParams) -> Result<()> {
        let text_document = params.text_document;
        let url = text_document.uri;

        {
            let workspace = match self.workspace.as_ref() {
                Some(workspace) => workspace,
                None => return Ok(()),
            };

            let mut workspace = workspace
                .try_borrow_mut()
                .map_err(|_| "failed to access mutable workspace")?;

            if params.content_changes.is_empty() {
                return Ok(());
            }

            match workspace.edited_files.get_mut(&url) {
                Some(file) => {
                    let rope = match file.diag.source.as_mut_rope() {
                        Some(rope) => rope,
                        None => return Ok(()),
                    };

                    for content_change in &params.content_changes {
                        let start = match content_change.range {
                            // replace range
                            Some(ref range) => {
                                let start = &range.start;
                                let end = &range.end;

                                let start = rope.line_to_char(start.line as usize)
                                    + start.character as usize;
                                let end =
                                    rope.line_to_char(end.line as usize) + end.character as usize;

                                rope.remove(start..end);
                                start
                            }
                            // replace all
                            None => {
                                rope.remove(..);
                                0
                            }
                        };

                        if content_change.text != "" {
                            rope.insert(start, &content_change.text);
                        }
                    }
                }
                None => return Ok(()),
            }

            workspace.reload()?;
        }

        self.send_workspace_diagnostics()?;
        Ok(())
    }

    /// Handler for `textDocument/didOpen`.
    fn text_document_did_open(&self, params: ty::DidOpenTextDocumentParams) -> Result<()> {
        let text_document = params.text_document;

        if let Some(workspace) = self.workspace.as_ref() {
            let url = text_document.uri;
            let text = text_document.text;

            let rope = Rope::from_str(&text);
            let source = Source::rope(url.clone(), rope);

            let mut loaded = LoadedFile {
                url: url.clone(),
                jumps: BTreeMap::new(),
                completions: BTreeMap::new(),
                prefixes: HashMap::new(),
                symbols: HashMap::new(),
                symbol: HashMap::new(),
                diag: Diagnostics::new(source.clone()),
            };

            let mut workspace = workspace
                .try_borrow_mut()
                .map_err(|_| "failed to access mutable workspace")?;

            workspace.edited_files.insert(url, loaded);
            workspace.reload()?;
        }

        self.send_workspace_diagnostics()?;
        Ok(())
    }

    /// Handler for `textDocument/didClose`.
    fn text_document_did_close(&self, params: ty::DidCloseTextDocumentParams) -> Result<()> {
        let text_document = params.text_document;

        if let Some(workspace) = self.workspace.as_ref() {
            let url = text_document.uri;

            let mut workspace = workspace
                .try_borrow_mut()
                .map_err(|_| "failed to access mutable workspace")?;

            workspace.edited_files.remove(&url);
            workspace.reload()?;
        }

        self.send_workspace_diagnostics()?;
        Ok(())
    }

    /// Handler for `textDocument/completion`.
    fn text_document_completion(
        &self,
        request_id: Option<envelope::RequestId>,
        params: ty::CompletionParams,
    ) -> Result<()> {
        let mut response = ty::CompletionList {
            ..ty::CompletionList::default()
        };

        self.completion_items(params, &mut response)?;
        self.channel.send(request_id, response)?;
        Ok(())
    }

    /// Populate completion items for the given request.
    fn completion_items(
        &self,
        params: ty::CompletionParams,
        list: &mut ty::CompletionList,
    ) -> Result<()> {
        let url = params.text_document.uri;

        let workspace = match self.workspace.as_ref() {
            Some(workspace) => workspace,
            None => return Ok(()),
        };

        let workspace = workspace
            .try_borrow()
            .map_err(|_| "failed to access immutable workspace")?;

        let (file, value) = match workspace.find_completion(&url, params.position)? {
            Some(v) => v,
            None => return Ok(()),
        };

        debug!("type completion: {:?}", value);

        match *value {
            Completion::Package { ref results, .. } => {
                for r in results {
                    list.items.push(ty::CompletionItem {
                        label: r.to_string(),
                        kind: Some(ty::CompletionItemKind::Module),
                        ..ty::CompletionItem::default()
                    });
                }
            }
            Completion::Any => {
                let candidates = vec![
                    "string", "bytes", "u32", "u64", "i32", "i64", "float", "double", "datetime",
                    "any",
                ];

                for (prefix, value) in &file.prefixes {
                    list.items.push(ty::CompletionItem {
                        label: format!("{}::", prefix),
                        kind: Some(ty::CompletionItemKind::Module),
                        detail: Some(value.package.to_string()),
                        ..ty::CompletionItem::default()
                    });
                }

                for c in candidates {
                    list.items.push(ty::CompletionItem {
                        label: c.to_string(),
                        kind: Some(ty::CompletionItemKind::Keyword),
                        ..ty::CompletionItem::default()
                    });
                }

                for symbol in file.symbols.keys() {
                    if symbol.len() != 1 {
                        continue;
                    }

                    let symbol = symbol.join("::");

                    list.items.push(ty::CompletionItem {
                        label: symbol,
                        kind: Some(ty::CompletionItemKind::Class),
                        ..ty::CompletionItem::default()
                    });
                }
            }
            Completion::Absolute {
                ref prefix,
                ref path,
                ref suffix,
            } => {
                let file = if let Some(ref prefix) = *prefix {
                    match file.prefixes
                        .get(prefix)
                        .and_then(|p| workspace.packages.get(&p.package))
                        .and_then(|url| workspace.file(url))
                    {
                        Some(file) => file,
                        None => return Ok(()),
                    }
                } else {
                    file
                };

                if let Some(symbols) = file.symbols.get(path) {
                    if let Some(ref suffix) = *suffix {
                        for s in symbols.iter().filter(|s| {
                            s.name.to_lowercase().starts_with(&suffix.to_lowercase())
                                && Loc::borrow(&s.name) != suffix
                        }) {
                            list.items.push(ty::CompletionItem {
                                label: s.name.to_string(),
                                kind: Some(ty::CompletionItemKind::Class),
                                documentation: s.to_documentation(),
                                ..ty::CompletionItem::default()
                            });
                        }
                    } else {
                        for s in symbols {
                            list.items.push(ty::CompletionItem {
                                label: s.name.to_string(),
                                kind: Some(ty::CompletionItemKind::Class),
                                documentation: s.to_documentation(),
                                ..ty::CompletionItem::default()
                            });
                        }
                    }
                };
            }
        }

        Ok(())
    }

    fn completion_item_resolve(&self, _: ty::CompletionItem) -> Result<()> {
        Ok(())
    }

    /// Handler for `textDocument/definition`.
    fn text_document_definition(
        &self,
        request_id: Option<envelope::RequestId>,
        params: ty::TextDocumentPositionParams,
    ) -> Result<()> {
        let mut response: Option<ty::request::GotoDefinitionResponse> = None;
        self.definition(params, &mut response)?;
        self.channel.send(request_id, response)?;
        Ok(())
    }

    /// Populate the goto definition response.
    fn definition(
        &self,
        params: ty::TextDocumentPositionParams,
        response: &mut Option<ty::request::GotoDefinitionResponse>,
    ) -> Result<()> {
        let url = params.text_document.uri;

        let workspace = match self.workspace.as_ref() {
            Some(workspace) => workspace,
            None => return Ok(()),
        };

        let workspace = workspace
            .try_borrow()
            .map_err(|_| "failed to access immutable workspace")?;

        let (file, value) = match workspace.find_jump(&url, params.position)? {
            Some(v) => v,
            None => return Ok(()),
        };

        debug!("definition: {}: {:?}", file.url, value);

        match *value {
            Jump::Absolute {
                ref prefix,
                ref path,
            } => {
                let (uri, file) = if let Some(ref prefix) = *prefix {
                    let prefix = match file.prefixes.get(prefix) {
                        Some(prefix) => prefix,
                        None => return Ok(()),
                    };

                    let url = match workspace.packages.get(&prefix.package) {
                        Some(url) => url,
                        None => return Ok(()),
                    };

                    match workspace.file(url) {
                        Some(file) => (url.clone(), file),
                        None => return Ok(()),
                    }
                } else {
                    (url, file)
                };

                let span = match file.symbol.get(path) {
                    Some(span) => *span,
                    None => return Ok(()),
                };

                let (start, end) = file.diag.source.span_to_range(span, Encoding::Utf16)?;
                let range = convert_range(start, end);
                let location = ty::Location { uri, range };

                *response = Some(ty::request::GotoDefinitionResponse::Scalar(location));
            }
            Jump::Package { ref prefix } => {
                let prefix = match file.prefixes.get(prefix) {
                    Some(prefix) => prefix,
                    None => return Ok(()),
                };

                let uri = match workspace.packages.get(&prefix.package) {
                    Some(url) => url.clone(),
                    None => return Ok(()),
                };

                let range = ty::Range::default();
                let location = ty::Location { uri, range };
                *response = Some(ty::request::GotoDefinitionResponse::Scalar(location));
            }
            Jump::Prefix { ref prefix } => {
                let prefix = match file.prefixes.get(prefix) {
                    Some(prefix) => prefix,
                    None => return Ok(()),
                };

                let (start, end) = file.diag
                    .source
                    .span_to_range(prefix.span, Encoding::Utf16)?;
                let range = convert_range(start, end);

                let location = ty::Location {
                    uri: url.clone(),
                    range,
                };

                *response = Some(ty::request::GotoDefinitionResponse::Scalar(location));
            }
        }

        Ok(())
    }
}

/// Convert an internal range into a language-server range.
fn convert_range(start: core::Position, end: core::Position) -> ty::Range {
    let start = ty::Position {
        line: start.line as u64,
        character: start.col as u64,
    };

    let end = ty::Position {
        line: end.line as u64,
        character: end.col as u64,
    };

    ty::Range { start, end }
}
