//! Database access. The UI talks to one interface; driver differences live behind
//! `DbPool` and are exposed as capabilities (e.g. `filter::operators`).

pub mod cell;
pub mod changes;
pub mod choices;
pub mod data;
pub mod ddl;
pub mod engine;
pub mod exec;
pub mod filter;
pub mod meta;
pub mod objects;
mod pool;
pub mod split;
pub mod sql;
pub mod value;

pub use pool::{open_pool, DbPool, Target};

/// Binds text values (`String` or `Option<String>` for NULL) to a query, in order.
/// Postgres casts them to the column type with `$n::type` in the SQL.
macro_rules! bind_all {
    ($q:expr, $binds:expr) => {{
        let mut q = $q;
        for b in $binds {
            q = q.bind(b.clone());
        }
        q
    }};
}
pub(crate) use bind_all;
