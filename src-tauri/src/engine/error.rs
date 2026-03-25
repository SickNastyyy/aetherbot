#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Data fetch failed for {exchange}/{pair}: {msg}")]
    DataFetch {
        exchange: String,
        pair: String,
        msg: String,
    },

    #[error("Insufficient candle data: have {have}, need {need}")]
    InsufficientData { have: usize, need: usize },

    #[error("Risk check vetoed trade: {reason}")]
    RiskVeto { reason: String },

    #[error("Order execution failed: {reason}")]
    ExecutionFailed { reason: String },

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}
