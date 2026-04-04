use serde::de::DeserializeOwned;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub url: String,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8123".into(),
            database: "default".into(),
            username: "default".into(),
            password: String::new(),
        }
    }
}

pub struct ClickHouseClient {
    http: reqwest::Client,
    config: ConnectionConfig,
}

impl ClickHouseClient {
    pub fn new(config: ConnectionConfig) -> Result<Self, ConnectionError> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(ConnectionError::Http)?;
        Ok(Self { http, config })
    }

    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Ping the server. Returns Ok(()) if reachable.
    pub async fn ping(&self) -> Result<(), ConnectionError> {
        let url = format!("{}/ping", self.config.url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(ConnectionError::Http)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(ConnectionError::QueryFailed(body))
        }
    }

    /// Execute a query and deserialize rows from JSONEachRow format.
    pub async fn query_json<T: DeserializeOwned>(
        &self,
        sql: &str,
    ) -> Result<Vec<T>, ConnectionError> {
        let text = self.query_text_with_format(sql, "JSONEachRow").await?;
        text.lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).map_err(ConnectionError::Json))
            .collect()
    }

    /// Execute a query and return raw text output.
    pub async fn query_text(&self, sql: &str) -> Result<String, ConnectionError> {
        self.query_text_with_format(sql, "TabSeparated").await
    }

    async fn query_text_with_format(
        &self,
        sql: &str,
        format: &str,
    ) -> Result<String, ConnectionError> {
        let url = format!(
            "{}/?database={}&default_format={}",
            self.config.url, self.config.database, format,
        );
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .body(sql.to_owned())
            .send()
            .await
            .map_err(ConnectionError::Http)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectionError::QueryFailed(body));
        }

        resp.text().await.map_err(ConnectionError::Http)
    }
}

#[derive(Debug)]
pub enum ConnectionError {
    Http(reqwest::Error),
    QueryFailed(String),
    Json(serde_json::Error),
    NotConnected,
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::QueryFailed(msg) => write!(f, "Query failed: {msg}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::NotConnected => write!(f, "Not connected to ClickHouse"),
        }
    }
}

impl std::error::Error for ConnectionError {}
