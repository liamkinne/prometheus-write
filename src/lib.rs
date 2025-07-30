mod types {
    include!(concat!(env!("OUT_DIR"), "/prometheus.rs"));
}
mod batcher;
mod registry;

pub use batcher::{Batcher, BatcherBuilder, Command, MetricOperation};
