// This module is kept for the connection pool compatibility but all database
// operations now go through src/drivers/. The commands are registered in lib.rs.
pub use crate::connection_pool::get_cache;
