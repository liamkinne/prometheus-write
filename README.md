# Prometheus Remote Write Exporter

This crate implements a metrics exporter that uses the Prometheus remote write
API to push samples to a prometheus instance. It is intended only for use cases
where you want to send high-frequency data to Prometheus rather than relying on
the usual quantization dictated by the scrape interval.

## Getting Started

```rust
use metrics_exporter_prometheus_write::Batcher;
use metrics::counter;
use metrics::gauge;

Batcher::builder()
    .install()
    .unwrap();

counter!("my_counter").increment(1);
gauge!("my_gauge").set(45.0);
```
