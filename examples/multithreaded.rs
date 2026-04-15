use metrics::counter;
use metrics_exporter_prometheus_write::Batcher;
use std::{
    thread::{self, sleep},
    time::Duration,
};

fn main() {
    tracing_subscriber::fmt::init();

    Batcher::builder()
        .batch_interval(Duration::from_millis(100))
        .install()
        .unwrap();

    thread::spawn(|| {
        loop {
            counter!("example").absolute(1);
            sleep(Duration::from_millis(100));
        }
    });

    thread::spawn(|| {
        loop {
            counter!("example").increment(1);
            sleep(Duration::from_millis(100));
        }
    });

    sleep(Duration::from_secs(5));
}
