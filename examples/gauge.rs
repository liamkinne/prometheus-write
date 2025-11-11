use metrics::gauge;
use metrics_exporter_prometheus_write::Builder;
use std::{
    f64::consts::PI,
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn main() {
    tracing_subscriber::fmt::init();

    Builder::new()
        .tick_interval(Duration::from_millis(200))
        .install()
        .unwrap();

    println!("Installed batcher.");

    println!("Start sending samples.");

    loop {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        gauge!("example").set((seconds % (2.0 * PI)).sin());
        sleep(Duration::from_millis(100));
    }
}
