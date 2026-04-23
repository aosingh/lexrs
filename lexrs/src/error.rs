use thiserror::Error;

#[derive(Debug, Error)]
pub enum LexError {
    #[error("Invalid wildcard expression: {0}")]
    InvalidWildcard(String),

    #[error("Words must be inserted in alphabetical order. Previous: '{prev}', Current: '{curr}'")]
    OrderViolation { prev: String, curr: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
