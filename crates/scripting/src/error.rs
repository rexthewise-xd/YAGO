use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScriptError {
    #[error("Script Compilation Error: {0}")]
    Compile(String),

    #[error("Script Runtime Error: {0}")]
    Runtime(String),

    #[error("Security Violation: {0}")]
    SecurityViolation(String),

    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
}
