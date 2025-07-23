use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::string::String;

#[derive(Debug, Clone)]
pub struct RpcConfig {
    #[allow(dead_code)]
    pub bitcoin_rpc_url: String,
    #[allow(dead_code)]
    pub metashrew_rpc_url: String,
    #[allow(dead_code)]
    pub timeout_seconds: u64,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            bitcoin_rpc_url: "http://bitcoinrpc:bitcoinrpc@localhost:8332".to_string(),
            metashrew_rpc_url: "http://localhost:8080".to_string(),
            timeout_seconds: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: JsonValue,
    pub id: u64,
}

impl RpcRequest {
    pub fn new(method: &str, params: JsonValue, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub result: Option<JsonValue>,
    pub error: Option<RpcError>,
    #[allow(dead_code)]
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcError {
    #[allow(dead_code)]
    pub code: i32,
    pub message: String,
    #[allow(dead_code)]
    pub data: Option<JsonValue>,
}

pub struct RpcClient {
    config: RpcConfig,
    request_id: std::sync::atomic::AtomicU64,
}

impl RpcClient {
    #[allow(dead_code)]
    pub fn new(config: RpcConfig) -> Self {
        Self {
            config,
            request_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn call(&self, url: &str, method: &str, params: JsonValue) -> Result<JsonValue, reqwest::Error> {
        let request = RpcRequest::new(method, params, self.next_id());
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&request)
            .send()
            .await?;
        let rpc_response: RpcResponse = response.json().await?;
        if let Some(error) = rpc_response.error {
            // In a real application, you'd want to return a proper error type
            panic!("RPC Error: {}", error.message);
        }
        Ok(rpc_response.result.unwrap_or_default())
    }

    #[allow(dead_code)]
    pub async fn bitcoin_call(&self, method: &str, params: JsonValue) -> Result<JsonValue, reqwest::Error> {
        self.call(&self.config.bitcoin_rpc_url, method, params).await
    }

    #[allow(dead_code)]
    pub async fn metashrew_call(&self, method: &str, params: JsonValue) -> Result<JsonValue, reqwest::Error> {
        self.call(&self.config.metashrew_rpc_url, method, params).await
    }
}