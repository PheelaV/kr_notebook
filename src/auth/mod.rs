//! Authentication module for multi-user support.

pub mod db;
pub mod handlers;
pub mod middleware;
pub mod password;

pub use handlers::*;
pub use middleware::{AuthContext, OptionalAuth, SESSION_COOKIE_NAME};
