#![doc = include_str!("../README.md")]

mod types {
    include!(concat!(env!("OUT_DIR"), "/prometheus.rs"));
}
mod batcher;
mod registry;

pub use batcher::Batcher;
pub use batcher::Builder;
