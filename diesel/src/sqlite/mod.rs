//! Provides types and functions related to working with SQLite
//!
//! Much of this module is re-exported from database agnostic locations.
//! However, if you are writing code specifically to extend Diesel on
//! SQLite, you may need to work with this module directly.

mod backend;
mod connection;
mod types;

pub mod query_builder;

pub use self::backend::{Sqlite, SqliteType};
pub use self::connection::SqliteConnection;
pub use self::connection::SqliteValue;
pub use self::query_builder::SqliteQueryBuilder;

/// Trait for the implementation of a SQLite aggregate function
///
/// This trait is to be used in conjunction with the `sql_function!`
/// macro for defining a custom SQLite aggregate function. See
/// the documentation [there](super::prelude::sql_function!) for details.
pub trait SqliteAggregateFunction<Args>: Default {
    /// The result type of the SQLite aggregate function
    type Output;

    /// The `step()` method is called once for every record of the query
    fn step(&mut self, args: Args);

    /// After the last row has been processed, the `finalize()` method is
    /// called to compute the result of the aggregate function. If no rows
    /// were processed `aggregator` will be `None` and `finalize()` can be
    /// used to specify a default result
    fn finalize(aggregator: Option<Self>) -> Self::Output;
}
