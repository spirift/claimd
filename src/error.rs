use std::fmt;
use uuid::Uuid;

use crate::model::Status;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotFound { id_prefix: String },
    AmbiguousPrefix { id_prefix: String, matches: Vec<Uuid> },
    AlreadyClaimed { id: Uuid, by: Option<String> },
    AlreadyLocked,
    InvalidTransition { id: Uuid, from: Status, to: Status },
    HasPendingDeps { id: Uuid, pending: Vec<Uuid> },
    StoreNotInitialized,
    ProjectInactive,
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::AlreadyClaimed { .. } | Error::AlreadyLocked | Error::HasPendingDeps { .. } | Error::ProjectInactive => 2,
            _ => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Error::Io(_) => "io_error",
            Error::Json(_) => "json_error",
            Error::NotFound { .. } => "not_found",
            Error::AmbiguousPrefix { .. } => "ambiguous_prefix",
            Error::AlreadyClaimed { .. } => "already_claimed",
            Error::AlreadyLocked => "already_locked",
            Error::InvalidTransition { .. } => "invalid_transition",
            Error::HasPendingDeps { .. } => "has_pending_deps",
            Error::StoreNotInitialized => "not_initialized",
            Error::ProjectInactive => "project_inactive",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::NotFound { id_prefix } => write!(f, "No todo found matching '{id_prefix}'"),
            Error::AmbiguousPrefix { id_prefix, matches } => {
                write!(f, "Prefix '{id_prefix}' is ambiguous, matches: ")?;
                for (i, id) in matches.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", &id.to_string()[..8])?;
                }
                Ok(())
            }
            Error::AlreadyClaimed { id, by } => {
                let short = &id.to_string()[..8];
                match by {
                    Some(agent) => write!(f, "Todo {short} is already being worked on by '{agent}'"),
                    None => write!(f, "Todo {short} is already being worked on"),
                }
            }
            Error::AlreadyLocked => write!(f, "Store is locked by another process — try again"),
            Error::InvalidTransition { id, from, to } => {
                let short = &id.to_string()[..8];
                write!(f, "Cannot transition {short} from {from} to {to}")
            }
            Error::HasPendingDeps { id, pending } => {
                let short = &id.to_string()[..8];
                let deps: Vec<String> = pending.iter().map(|u| u.to_string()[..8].to_string()).collect();
                write!(f, "Todo {short} has unresolved dependencies: {}", deps.join(", "))
            }
            Error::StoreNotInitialized => write!(f, "Store not initialized. Run 'claimd init' first."),
            Error::ProjectInactive => write!(f, "Project is inactive — claiming is disabled"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self { Error::Json(e) }
}

pub type Result<T> = std::result::Result<T, Error>;
