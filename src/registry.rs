use crate::types;
use metrics::Key;
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct Samples {
    sent: bool,
    samples: Vec<types::Sample>,
}

impl Samples {
    /// Create a new sample stream.
    pub fn new(sample: types::Sample) -> Self {
        Self {
            sent: false,
            samples: vec![sample],
        }
    }

    pub fn all(&self) -> &Vec<types::Sample> {
        &self.samples
    }

    /// Increment, adding to the previous value.
    pub fn increment(&mut self, sample: types::Sample) {
        if let Some(last) = self.samples.last_mut() {
            let current = last.value;

            if last.timestamp == sample.timestamp {
                // increment old value
                last.value += sample.value;
            } else {
                // the existing sample has already been sent
                if self.sent {
                    self.samples.clear();
                }

                self.samples.push(types::Sample {
                    value: sample.value + current,
                    timestamp: sample.timestamp,
                });
                self.sent = false;
            }
        } else {
            self.sent = false;
            self.samples.push(sample);
        }
    }

    /// Set the new or next sample.
    pub fn set(&mut self, sample: types::Sample) {
        if let Some(last) = self.samples.last_mut() {
            if last.timestamp == sample.timestamp {
                // assign new value
                last.value = sample.value
            } else {
                // the existing sample has already been sent
                if self.sent {
                    self.samples.clear();
                }

                self.samples.push(types::Sample {
                    value: sample.value,
                    timestamp: sample.timestamp,
                });
                self.sent = false;
            }
        } else {
            self.sent = false;
            self.samples.push(sample);
        }
    }

    /// Remove all elements except the last.
    pub fn sent(&mut self) {
        self.sent = true;

        let last = self.samples.last().map(|s| s.clone());
        self.samples.clear();
        if let Some(last) = last {
            self.samples.push(last);
        }
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

    /// Mark samples as sent.
    pub fn sent(&mut self) {
        for samples in self.counters.values_mut() {
            samples.sent();
        }

        for samples in self.gauges.values_mut() {
            samples.sent();
        }
    }

    /// Increment a counter, adding the given value to the last value.
    pub fn counter_increment(&mut self, timestamp: SystemTime, key: Key, value: u64) {
        let sample = types::Sample {
            timestamp: self.timestamp_millis(timestamp),
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
            timestamp: self.timestamp_millis(timestamp),
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
            timestamp: self.timestamp_millis(timestamp),
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
            timestamp: self.timestamp_millis(timestamp),
            value,
        };

        if let Some(samples) = self.gauges.get_mut(&key) {
            samples.set(sample);
        } else {
            self.gauges.insert(key, Samples::new(sample));
        }
    }

    fn timestamp_millis(&self, timestamp: SystemTime) -> i64 {
        // todo: dont use SystemTime as we can't then set custom timestamps.
        timestamp.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
    }
}
