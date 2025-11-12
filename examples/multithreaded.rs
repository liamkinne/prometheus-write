use metrics::counter;
use metrics_exporter_prometheus_write::Builder;
use std::{
    thread::{self, sleep},
    time::Duration,
};

fn main() {
    tracing_subscriber::fmt::init();

    Builder::new()
        .tick_interval(Duration::from_millis(100))
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
