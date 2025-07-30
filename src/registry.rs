use crate::types;
use metrics::Key;
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Registry {
    pub counters: BTreeMap<Key, Vec<types::Sample>>,
    pub gauges: BTreeMap<Key, Vec<types::Sample>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            counters: BTreeMap::new(),
            gauges: BTreeMap::new(),
        }
    }

    /// Clears all of the registry contents.
    pub fn clear(&mut self) {
        // only keep the latest counter for each key
        for samples in self.counters.values_mut() {
            if let Some(sample) = samples.last() {
                let sample = sample.to_owned();
                samples.clear();
                samples.push(sample);
            }
        }

        for samples in self.gauges.values_mut() {
            if let Some(sample) = samples.last() {
                let sample = sample.to_owned();
                samples.clear();
                samples.push(sample);
            }
        }
    }

    /// Increment a counter, adding the given value to the last value.
    pub fn counter_increment(&mut self, timestamp: SystemTime, key: Key, value: u64) {
        let timestamp = self.timestamp_millis(timestamp);

        if self.counters.contains_key(&key) {
            let samples = self.counters.get_mut(&key).unwrap();
            let old = samples.last_mut().unwrap();

            if old.timestamp == timestamp {
                old.value += value as f64;
            } else {
                let old = old.value;
                samples.push(types::Sample {
                    timestamp,
                    value: old + value as f64,
                });
            }
        } else {
            self.counters.insert(
                key,
                vec![types::Sample {
                    timestamp,
                    value: value as f64,
                }],
            );
        }
    }

    /// Set the absolute value of a counter.
    pub fn counter_set(&mut self, timestamp: SystemTime, key: Key, value: u64) {
        let timestamp = self.timestamp_millis(timestamp);

        if self.counters.contains_key(&key) {
            let samples = self.counters.get_mut(&key).unwrap();
            let old = samples.last_mut().unwrap();

            if old.timestamp == timestamp {
                old.value = value as f64;
            } else {
                samples.push(types::Sample {
                    timestamp,
                    value: value as f64,
                });
            }
        } else {
            self.counters.insert(
                key,
                vec![types::Sample {
                    timestamp,
                    value: value as f64,
                }],
            );
        }
    }

    /// Increment a guage, adding the new value to the last value.
    pub fn gauge_increment(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        let timestamp = self.timestamp_millis(timestamp);

        if self.gauges.contains_key(&key) {
            let samples = self.gauges.get_mut(&key).unwrap();
            let old = samples.last_mut().unwrap();

            if old.timestamp == timestamp {
                old.value += value;
            } else {
                let old = old.value;
                samples.push(types::Sample {
                    timestamp,
                    value: old + value,
                });
            }
        } else {
            self.gauges
                .insert(key, vec![types::Sample { timestamp, value }]);
        }
    }

    /// Increment a guage, adding the new value to the last value.
    pub fn gauge_decrement(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        self.gauge_increment(timestamp, key, -value);
    }

    /// Set the absolute value of a gauge.
    pub fn gauge_set(&mut self, timestamp: SystemTime, key: Key, value: f64) {
        let timestamp = self.timestamp_millis(timestamp);

        if self.gauges.contains_key(&key) {
            let samples = self.gauges.get_mut(&key).unwrap();
            let old = samples.last_mut().unwrap();

            if old.timestamp == timestamp {
                old.value = value;
            } else {
                samples.push(types::Sample { timestamp, value });
            }
        } else {
            self.gauges
                .insert(key, vec![types::Sample { timestamp, value }]);
        }
    }

    fn timestamp_millis(&self, timestamp: SystemTime) -> i64 {
        // todo: dont use SystemTime as we can't then set custom timestamps.
        timestamp.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
    }
}
