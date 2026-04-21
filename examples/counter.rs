use metrics::counter;
use metrics_exporter_prometheus_write::Batcher;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    tracing_subscriber::fmt::init();

    Batcher::builder()
        .batch_interval(Duration::from_millis(200))
        .install()
        .unwrap();

    println!("Installed batcher.");

    println!("Start sending samples.");

    for _ in 0..100 {
        counter!("example").increment(1);
        sleep(Duration::from_millis(100));
    }

    println!("Done sending samples.");
}
