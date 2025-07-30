use crate::{
    registry::Registry,
    types::{self, metric_metadata::MetricType},
};
use crossbeam::channel::{Receiver, Sender, select};
use metrics::{Key, KeyName, Recorder, SetRecorderError, SharedString, Unit};
use prost::Message;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Debug)]
pub enum MetricOperation {
    IncrementCounter(u64),
    SetCounter(u64),
    IncrementGauge(f64),
    DecrementGauge(f64),
    SetGauge(f64),
}

#[derive(Debug)]
pub enum Command {
    Metadata(KeyName, MetricType, Option<Unit>, SharedString),
    Operation(SystemTime, Key, MetricOperation),
}

/// Builder for the [`Batcher`].
#[derive(Debug, Clone)]
pub struct Builder {
    tick_interval: Duration,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
        }
    }

    /// Change the interval between batch writes.
    ///
    /// Default is 0.1s.
    pub fn tick_interval(mut self, interval: Duration) -> Self {
        self.tick_interval = interval;
        self
    }

    /// Set the global recorder
    pub fn install(self) -> Result<(), SetRecorderError<Batcher>> {
        let (tx_cmds, rx_cmd) = crossbeam::channel::unbounded();

        std::thread::spawn(move || batch_worker(rx_cmd, self.tick_interval));

        metrics::set_global_recorder(Batcher {
            inner: Arc::new(BatcherInner { tx_cmds }),
        })
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch sample sender.
pub struct Batcher {
    inner: Arc<BatcherInner>,
}

impl Batcher {
    /// Send a command to the worker thread.
    pub fn send(&self, command: Command) {
        self.inner.send(command);
    }
}

impl Recorder for Batcher {
    fn describe_counter(&self, key: KeyName, unit: Option<Unit>, desc: SharedString) {
        self.send(Command::Metadata(key, MetricType::Counter, unit, desc));
    }

    fn describe_gauge(&self, key: KeyName, unit: Option<Unit>, desc: SharedString) {
        self.send(Command::Metadata(key, MetricType::Gauge, unit, desc));
    }

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _desc: SharedString) {
        unimplemented!("Histogram not yet supported.")
    }

    fn register_counter(&self, key: &Key, _meta: &metrics::Metadata<'_>) -> metrics::Counter {
        metrics::Counter::from_arc(Arc::new(Counter {
            key: key.clone(),
            inner: self.inner.clone(),
        }))
    }

    fn register_gauge(&self, key: &Key, _meta: &metrics::Metadata<'_>) -> metrics::Gauge {
        metrics::Gauge::from_arc(Arc::new(Gauge {
            key: key.clone(),
            inner: self.inner.clone(),
        }))
    }

    fn register_histogram(&self, _key: &Key, _meta: &metrics::Metadata<'_>) -> metrics::Histogram {
        unimplemented!("Histogram not yet supported.")
    }
}

pub struct Counter {
    key: Key,
    inner: Arc<BatcherInner>,
}

impl metrics::CounterFn for Counter {
    fn increment(&self, value: u64) {
        self.inner.send(Command::Operation(
            SystemTime::now(),
            self.key.clone(),
            MetricOperation::IncrementCounter(value),
        ));
    }

    fn absolute(&self, value: u64) {
        self.inner.send(Command::Operation(
            SystemTime::now(),
            self.key.clone(),
            MetricOperation::SetCounter(value),
        ));
    }
}

pub struct Gauge {
    key: Key,
    inner: Arc<BatcherInner>,
}

impl metrics::GaugeFn for Gauge {
    fn increment(&self, value: f64) {
        self.inner.send(Command::Operation(
            SystemTime::now(),
            self.key.clone(),
            MetricOperation::IncrementGauge(value),
        ));
    }

    fn decrement(&self, value: f64) {
        self.inner.send(Command::Operation(
            SystemTime::now(),
            self.key.clone(),
            MetricOperation::DecrementGauge(value),
        ));
    }

    fn set(&self, value: f64) {
        self.inner.send(Command::Operation(
            SystemTime::now(),
            self.key.clone(),
            MetricOperation::SetGauge(value),
        ));
    }
}

struct BatcherInner {
    tx_cmds: Sender<Command>,
}

impl BatcherInner {
    /// Send a command to the worker thread.
    pub fn send(&self, command: Command) {
        self.tx_cmds.send(command).ok();
    }
}

fn batch_worker(rx_cmd: Receiver<Command>, interval: Duration) {
    let rx_tick = crossbeam::channel::tick(interval);
    let mut registry = Registry::new();

    fn write(registry: &mut Registry) {
        let mut timeseries = vec![];

        for (key, samples) in &registry.counters {
            let mut labels = vec![types::Label {
                name: "__name__".to_owned(),
                value: key.name().to_owned(),
            }];

            for label in key.labels() {
                labels.push(types::Label {
                    name: label.key().to_string(),
                    value: label.value().to_string(),
                })
            }

            timeseries.push(types::TimeSeries {
                labels,
                samples: samples.clone(),
                exemplars: vec![],
            })
        }

        for (key, samples) in &registry.gauges {
            let mut labels = vec![types::Label {
                name: "__name__".to_owned(),
                value: key.name().to_owned(),
            }];

            for label in key.labels() {
                labels.push(types::Label {
                    name: label.key().to_string(),
                    value: label.value().to_string(),
                })
            }

            timeseries.push(types::TimeSeries {
                labels,
                samples: samples.clone(),
                exemplars: vec![],
            })
        }

        let write_request = types::WriteRequest {
            timeseries,
            // doesn't do anything in v.0.1.0 protocol
            metadata: vec![],
        };

        let compressed =
            match snap::raw::Encoder::new().compress_vec(&write_request.encode_to_vec()) {
                Ok(c) => c,
                Err(err) => {
                    log::error!("Compression failed: {:?}", err);
                    return;
                }
            };

        let mut response = match ureq::post("http://localhost:9090/api/v1/write")
            .config()
            .timeout_global(Some(Duration::from_millis(100)))
            .build()
            .content_type("application/x-protobuf")
            .header("Content-Encoding", "snappy")
            .header("User-Agent", "prom-push")
            .header("X-Prometheus-Remote-Write-Version", "1.0.0")
            .send(&compressed)
        {
            Ok(r) => r,
            Err(err) => {
                log::error!("Request failed: {:?}", err);
                return;
            }
        };

        if response.status().is_client_error() {
            log::error!(
                "Prometheus returned a client error: {:?}",
                response.body_mut().read_to_string()
            );
        }

        if response.status().is_server_error() {
            log::error!(
                "Prometheus returned a server error: {:?}",
                response.body_mut().read_to_string()
            );
        }

        registry.clear();
    }

    loop {
        select! {
            recv(rx_cmd) -> cmd => {
                if let Ok(Command::Operation(timestamp, key, op)) = cmd { match op {
                    MetricOperation::IncrementCounter(value) => {
                        registry.counter_increment(timestamp, key, value);
                    },
                    MetricOperation::SetCounter(value) => {
                        registry.counter_set(timestamp, key, value);
                    },
                    MetricOperation::IncrementGauge(value) => {
                        registry.gauge_increment(timestamp, key, value);
                    },
                    MetricOperation::DecrementGauge(value) => {
                        registry.gauge_decrement(timestamp, key, value);
                    },
                    MetricOperation::SetGauge(value) => {
                        registry.gauge_set(timestamp, key, value);
                    },
                } };
            },
            recv(rx_tick) -> _ => {
                write(&mut registry);
            },
        }
    }
}
