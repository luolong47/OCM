//! Shared application state, cloned into every handler.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::cache::Catalog;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub catalog: Catalog,
}
