use crate::types;
use metrics::Key;
use std::collections::BTreeMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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

            if sample.timestamp <= last.timestamp {
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
            if sample.timestamp == last.timestamp {
                // assign new value
                last.value = sample.value
            } else if sample.timestamp > last.timestamp {
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

    /// Has this sample been sent already.
    pub fn is_sent(&self) -> bool {
        self.sent
    }

    /// Remove all elements except the last.
    pub fn sent(&mut self) {
        self.sent = true;

        let last = self.samples.last().copied();
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
}

fn timestamp_millis(timestamp: SystemTime) -> i64 {
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
    fn registry_counter_increment_duplicate_sample() {
        let mut registry = Registry::new();

        let time = SystemTime::now();
        let key = Key::from_name("test");

        assert!(registry.counters.is_empty());
        assert!(registry.gauges.is_empty());

        // first sample
        registry.counter_increment(time, key.clone(), 50);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().samples[0].value, 50.0);
        assert!(registry.gauges.is_empty());

        // duplicate sample
        registry.counter_increment(time, key.clone(), 100);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().samples[0].value, 150.0);
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
        assert_eq!(registry.counters.get(&key).unwrap().samples[0].value, 50.0);
        assert!(registry.gauges.is_empty());

        // duplicate sample
        registry.counter_set(time, key.clone(), 100);
        assert_eq!(registry.counters.len(), 1);
        assert_eq!(registry.counters.get(&key).unwrap().samples[0].value, 100.0);
        assert!(registry.gauges.is_empty());
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
