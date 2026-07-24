use crate::types;
use metrics::Key;
use std::collections::BTreeMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct Samples {
    /// The last sample that was sent, kept as the basis for future
    /// increments/sets even after `pending` has been drained by a send.
    last_sent: Option<types::Sample>,
    /// Samples that have not yet been sent.
    pending: Vec<types::Sample>,
}

impl Samples {
    /// Create a new sample stream.
    pub fn new(sample: types::Sample) -> Self {
        Self {
            last_sent: None,
            pending: vec![sample],
        }
    }

    #[cfg(test)]
    pub fn all(&self) -> &Vec<types::Sample> {
        &self.pending
    }

    /// The most recent known sample, whether pending or already sent. This
    /// is the basis used for future `increment`/`set` calls.
    fn last(&self) -> Option<&types::Sample> {
        self.pending.last().or(self.last_sent.as_ref())
    }

    /// Increment, adding to the previous value.
    pub fn increment(&mut self, sample: types::Sample) {
        let last = self.last().copied();

        match last {
            Some(last) if sample.timestamp <= last.timestamp => {
                if let Some(pending) = self.pending.last_mut() {
                    pending.value += sample.value;
                } else {
                    // the current base value has already been sent, so we
                    // need a new pending entry to carry the increment.
                    self.pending.push(types::Sample {
                        value: last.value + sample.value,
                        timestamp: last.timestamp,
                    });
                }
            }
            Some(last) => {
                self.pending.push(types::Sample {
                    value: last.value + sample.value,
                    timestamp: sample.timestamp,
                });
            }
            None => {
                self.pending.push(sample);
            }
        }
    }

    /// Set the new or next sample.
    pub fn set(&mut self, sample: types::Sample) {
        let last = self.last().copied();

        match last {
            Some(last) if sample.timestamp == last.timestamp => {
                if let Some(pending) = self.pending.last_mut() {
                    pending.value = sample.value;
                } else {
                    // the current base value has already been sent, so we
                    // need a new pending entry to overwrite it.
                    self.pending.push(sample);
                }
            }
            Some(last) if sample.timestamp > last.timestamp => {
                self.pending.push(sample);
            }
            Some(_) => {
                // older than the current value, ignore.
            }
            None => {
                self.pending.push(sample);
            }
        }
    }

    /// Whether there are any samples ready to be sent, given a cutoff
    /// timestamp (inclusive). Samples newer than the cutoff are not
    /// considered ready.
    pub fn is_ready(&self, cutoff: i64) -> bool {
        self.pending.iter().any(|sample| sample.timestamp <= cutoff)
    }

    /// The samples ready to be sent, given a cutoff timestamp (inclusive).
    /// Samples newer than the cutoff are left pending.
    pub fn ready(&self, cutoff: i64) -> Vec<types::Sample> {
        self.pending
            .iter()
            .filter(|sample| sample.timestamp <= cutoff)
            .copied()
            .collect()
    }

    /// Mark samples up to and including the cutoff timestamp as sent,
    /// removing them from `pending` while retaining the last one as the
    /// base for future increments/sets. Samples newer than the cutoff are
    /// left untouched.
    pub fn sent(&mut self, cutoff: i64) {
        let boundary = self
            .pending
            .partition_point(|sample| sample.timestamp <= cutoff);

        if boundary == 0 {
            return;
        }

        let mut sent = self.pending.drain(..boundary);
        self.last_sent = sent.next_back();
    }
}

pub struct Registry {
    pub counters: BTreeMap<Key, Samples>,
    pub gauges: BTreeMap<Key, Samples>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            counters: BTreeMap::new(),
            gauges: BTreeMap::new(),
        }
    }

    /// Mark samples up to and including the cutoff timestamp as sent.
    pub fn sent(&mut self, cutoff: i64) {
        for samples in self.counters.values_mut() {
            samples.sent(cutoff);
        }

        for samples in self.gauges.values_mut() {
            samples.sent(cutoff);
        }
    }

    /// Increment a counter, adding the given value to the last value.
    pub fn counter_increment(&mut self, timestamp: SystemTime, key: Key, value: u64) {
        let sample = types::Sample {
            timestamp: timestamp_millis(timestamp),
            value: value as f64,
        };

        if let Some(samples) = self.counters.get_mut(&key) {
            samples.increment(sample);
        } else {
            self.counters.insert(key, Samples::new(sample));
        }
    }

    /// Set the absolute value of a counter.
    pub fn counter_set(&mut self, timestamp: SystemTime, key: Key, value: u64) {
        let sample = types::Sample {
            timestamp: timestamp_millis(timestamp),
            value: value as f64,
        };

        if let Some(samples) = self.counters.get_mut(&key) {
            samples.set(sample);
        } else {
            self.counters.insert(key, Samples::new(sample));
        }
    }

    /// Increment a guage, adding the new value to the last value.
    pub fn gauge_increment(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        let sample = types::Sample {
            timestamp: timestamp_millis(timestamp),
            value,
        };

        if let Some(samples) = self.gauges.get_mut(&key) {
            samples.increment(sample);
        } else {
            self.gauges.insert(key, Samples::new(sample));
        }
    }

    /// Increment a guage, adding the new value to the last value.
    pub fn gauge_decrement(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        self.gauge_increment(timestamp, key, -value);
    }

    /// Set the absolute value of a gauge.
    pub fn gauge_set(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        let sample = types::Sample {
            timestamp: timestamp_millis(timestamp),
            value,
        };

        if let Some(samples) = self.gauges.get_mut(&key) {
            samples.set(sample);
        } else {
            self.gauges.insert(key, Samples::new(sample));
        }
    }

    /// Build the timeseries ready to be sent, given a cutoff timestamp
    /// (inclusive, in milliseconds since the Unix epoch). Samples newer
    /// than the cutoff are excluded and left pending for a future batch.
    pub fn as_timeseries(&self, cutoff: i64) -> Vec<types::TimeSeries> {
        let mut timeseries = vec![];

        for (key, samples) in &self.counters {
            if !samples.is_ready(cutoff) {
                continue;
            }

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
                samples: samples.ready(cutoff),
                exemplars: vec![],
            })
        }

        for (key, samples) in &self.gauges {
            if !samples.is_ready(cutoff) {
                continue;
            }

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
                samples: samples.ready(cutoff),
                exemplars: vec![],
            })
        }

        timeseries
    }
}

pub(crate) fn timestamp_millis(timestamp: SystemTime) -> i64 {
    // todo: dont use SystemTime as we can't then set custom timestamps.
    timestamp.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn sample_duplicate() {
        let mut samples = Samples::new(types::Sample {
            value: 1.0,
            timestamp: 100,
        });
        assert_eq!(samples.all().len(), 1);
        assert_eq!(samples.all()[0].value, 1.0);

        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 200,
        });
        assert_eq!(samples.all().len(), 2);
        assert_eq!(samples.all()[1].value, 2.0);

        // this should overwrite the last value given it has the same timestamp.
        samples.set(types::Sample {
            value: 10.0,
            timestamp: 200,
        });
        assert_eq!(samples.all().len(), 2);
        assert_eq!(samples.all()[1].value, 10.0);
    }

    #[test]
    fn sample_set_out_of_order() {
        let mut samples = Samples::new(types::Sample {
            value: 1.0,
            timestamp: 100,
        });

        samples.set(types::Sample {
            value: 2.0,
            timestamp: 200,
        });
        assert_eq!(samples.all()[1].value, 2.0);
        assert_eq!(samples.all()[1].timestamp, 200);

        // samples older than the latest sample should be ignored
        samples.set(types::Sample {
            value: 3.0,
            timestamp: 100,
        });
        assert_eq!(samples.all()[1].value, 2.0);
        assert_eq!(samples.all()[1].timestamp, 200);
    }

    #[test]
    fn sample_increment_out_of_order() {
        let mut samples = Samples::new(types::Sample {
            value: 1.0,
            timestamp: 100,
        });

        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 200,
        });
        assert_eq!(samples.all()[1].value, 2.0);
        assert_eq!(samples.all()[1].timestamp, 200);

        // old samples should be ignored but the total will still be incremented
        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 100,
        });
        assert_eq!(samples.all()[1].value, 3.0);
        assert_eq!(samples.all()[1].timestamp, 200);
    }

    #[test]
    fn sample_sent_respects_cutoff() {
        let mut samples = Samples::new(types::Sample {
            value: 1.0,
            timestamp: 100,
        });

        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 200,
        });
        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 300,
        });
        assert_eq!(samples.all().len(), 3);

        // only samples at or before the cutoff should be cleared.
        samples.sent(200);
        assert_eq!(samples.all().len(), 1);
        assert_eq!(samples.all()[0].timestamp, 300);
        assert_eq!(samples.all()[0].value, 3.0);

        // a further increment using the sent value as a base should still
        // work correctly.
        samples.sent(300);
        assert!(samples.all().is_empty());

        samples.increment(types::Sample {
            value: 1.0,
            timestamp: 400,
        });
        assert_eq!(samples.all().len(), 1);
        assert_eq!(samples.all()[0].value, 4.0);
        assert_eq!(samples.all()[0].timestamp, 400);
    }

    #[test]
    fn registry_counter_increment_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.counter_increment(time, key.clone(), 50);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().all()[0].value, 50.0);
        assert!(registry.gauges.is_empty());

        // duplicate sample
        registry.counter_increment(time, key.clone(), 100);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().all()[0].value, 150.0);
        assert!(registry.gauges.is_empty());
    }

    #[test]
    fn registry_counter_set_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.counter_set(time, key.clone(), 50);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().all()[0].value, 50.0);
        assert!(registry.gauges.is_empty());

        // duplicate sample
        registry.counter_set(time, key.clone(), 100);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().all()[0].value, 100.0);
        assert!(registry.gauges.is_empty());
    }

    #[test]
    fn registry_gauge_increment_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.gauge_increment(time, key.clone(), 50.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, 50.0);

        // duplicate sample
        registry.gauge_increment(time, key.clone(), 100.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, 150.0);
    }

    #[test]
    fn registry_gauge_set_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.gauge_set(time, key.clone(), 50.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, 50.0);

        // duplicate sample
        registry.gauge_set(time, key.clone(), 100.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, 100.0);
    }

    #[test]
    fn registry_gauge_decrement_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.gauge_decrement(time, key.clone(), 50.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, -50.0);

        // duplicate sample
        registry.gauge_decrement(time, key.clone(), 100.0);
        assert!(registry.counters.is_empty());
        assert_eq!(registry.gauges.len(), 1);
        assert_eq!(registry.gauges.get(&key).unwrap().all()[0].value, -150.0);
    }

    #[test]
    fn registry_into_prometheus_timeseries() {
        let mut registry = Registry::new();

        assert!(registry.as_timeseries(i64::MAX).is_empty());

        let time = SystemTime::now();
        let key = Key::from_name("test");
        registry.gauge_set(time, key.clone(), 50.0);

        assert_eq!(registry.as_timeseries(i64::MAX).len(), 1);
        assert!(registry.as_timeseries(i64::MAX)[0].exemplars.is_empty());
        assert_eq!(registry.as_timeseries(i64::MAX)[0].labels.len(), 1);
        assert_eq!(
            registry.as_timeseries(i64::MAX)[0].labels[0].name,
            "__name__"
        );
        assert_eq!(registry.as_timeseries(i64::MAX)[0].labels[0].value, "test");
        assert_eq!(registry.as_timeseries(i64::MAX)[0].samples.len(), 1);
        assert_eq!(
            registry.as_timeseries(i64::MAX)[0].samples[0].timestamp,
            timestamp_millis(time)
        );
        assert_eq!(registry.as_timeseries(i64::MAX)[0].samples[0].value, 50.0);
    }

    #[test]
    fn registry_as_timeseries_respects_cutoff() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");
        registry.gauge_set(time, key.clone(), 50.0);

        // a cutoff before the sample's timestamp should exclude it.
        let cutoff = timestamp_millis(time) - 1;
        assert!(registry.as_timeseries(cutoff).is_empty());

        // a cutoff at or after the sample's timestamp should include it.
        let cutoff = timestamp_millis(time);
        assert_eq!(registry.as_timeseries(cutoff).len(), 1);
    }

    #[test]
    fn timestamp_conversion() {
        let time = SystemTime::now();
        let ts_a = timestamp_millis(time);
        assert!(ts_a > 0);
        let ts_b = timestamp_millis(time);
        assert_eq!(ts_a, ts_b);
        let delta = Duration::from_millis(100);
        let ts_c = timestamp_millis(time - delta);
        assert_eq!(ts_c, ts_a - delta.as_millis() as i64);
    }
}
