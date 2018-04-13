//! Types to deserialize.

use json;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMessage {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub method: String,
    pub params: json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage<T> {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage<T, D> {
    pub jsonrpc: &'static str,
    pub id: Option<RequestId>,
    pub result: Option<T>,
    pub error: Option<ResponseError<D>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Code {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ServerErrorStart = -32099,
    ServerErrorEnd = -32000,
    ServerNotInitialized = -32002,
    UnknownErrorCode = -32001,
    RequestCancelled = -32800,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError<D> {
    pub code: Code,
    pub message: String,
    pub data: Option<D>,
}
