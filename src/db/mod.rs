pub mod collections;
pub mod models;

mod connection;
pub use connection::{connect, ensure_indexes};
