# Prometheus Remote Write Exporter

This crate implements a metrics exporter that uses the Prometheus remote write
API to push samples to a prometheus instance. It is intended only for use cases
where you want to send high-frequency data to Prometheus rather than relying on
the usual quantization dictated by the scrape interval.

## Why not Pushgateway?

[Pushgateway](https://prometheus.io/docs/practices/pushing/) only stores the
latest value for each metric which makes sense if you're only interested in
summarising metrics and not seeing how they change over time. This crate writes
the metrics directly to the prometheus instance and only applies overwriting
values if the latest value and the new value occur in the same millisecond.

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
