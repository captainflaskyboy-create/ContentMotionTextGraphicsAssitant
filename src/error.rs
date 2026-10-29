use serde_json::{json, Value};

#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn json(&self) -> Value {
        json!({"ok":false,"error":{"code":self.code,"message":self.message}})
    }
    pub fn exit_code(&self) -> i32 {
        match self.code {
            "cancelled" => 130,
            "output_exists" => 3,
            "native_unavailable" => 4,
            "invalid_scene" | "invalid_arguments" => 2,
            _ => 5,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new("io_failed", e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new("invalid_scene", e.to_string())
    }
}
impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Self::new("native_failed", e.to_string())
    }
}
