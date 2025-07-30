# Prometheus Remote Write Exporter

This crate implements a metrics exporter that uses the Prometheus remote write
API to push samples to a prometheus instance. It is intended only for use cases
where you want to send high-frequency data to Prometheus rather than relying on
the usual quantization dictated by the scrape interval.
