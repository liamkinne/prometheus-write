use metrics::counter;
use metrics_exporter_prometheus_write::Builder;
use std::{thread::sleep, time::Duration};

fn main() {
    tracing_subscriber::fmt::init();

    Builder::new()
        .tick_interval(Duration::from_millis(200))
        .install()
        .unwrap();

    println!("Installed batcher.");

    println!("Start sending samples.");

    loop {
        counter!("example").increment(1);
        sleep(Duration::from_millis(100));
    }
}
