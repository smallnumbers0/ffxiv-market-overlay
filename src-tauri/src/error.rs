//! One error type for every fallible path in the app.
//!
//! Commands return `Result<T, AppError>`; `AppError` serialises to a plain
//! `{ kind, message }` object so the frontend can branch on `kind` (to show a
//! "couldn't reach Universalis" state, say) without string-matching prose.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The item catalog is missing, empty, or unreadable.
    #[error("item catalog unavailable: {0}")]
    Catalog(String),

    /// Universalis could not be reached, timed out, or replied with an error.
    #[error("couldn't reach Universalis: {0}")]
    Universalis(String),

    /// XIVAPI could not be reached during a catalog sync.
    #[error("couldn't reach XIVAPI: {0}")]
    XivApi(String),

    /// Reading or writing the on-disk config failed.
    #[error("settings could not be saved: {0}")]
    Config(String),

    /// The request itself was wrong (unknown item, empty world, bad hotkey).
    #[error("{0}")]
    Invalid(String),

    /// Anything the user can neither cause nor fix.
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    /// Stable machine-readable tag. Keep in sync with `ErrorKind` in
    /// `src/lib/tauriApi.ts`.
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Catalog(_) => "catalog",
            AppError::Universalis(_) => "universalis",
            AppError::XivApi(_) => "xivapi",
            AppError::Config(_) => "config",
            AppError::Invalid(_) => "invalid",
            AppError::Internal(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Catalog(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
