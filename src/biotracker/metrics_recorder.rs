use egui::epaint::ahash::HashMap;
use metrics::*;
use metrics_util::registry::{AtomicStorage, Registry};
use std::sync::{Arc, Mutex};

pub struct MetricsRecorder {
    registry: Registry<Key, AtomicStorage>,
    inner: Arc<Mutex<Inner>>,
}

pub struct MetricDescription {
    pub unit: Option<Unit>,
    pub text: SharedString,
}

struct Inner {
    summaries: HashMap<Key, metrics_util::Summary>,
    descriptions: HashMap<u64, MetricDescription>,
}

impl MetricsRecorder {
    pub fn new() -> Self {
        Self {
            registry: Registry::new(AtomicStorage),
            inner: Arc::new(Mutex::new(Inner {
                summaries: HashMap::default(),
                descriptions: HashMap::default(),
            })),
        }
    }

    pub fn describe(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        let mut inner = self.inner.lock().unwrap();
        let key = Key::from_name(key_name);
        let hash = key.get_hash();
        inner.descriptions.insert(
            hash,
            MetricDescription {
                unit,
                text: description,
            },
        );
    }
}

impl Recorder for MetricsRecorder {
    fn describe_counter(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.describe(key_name, unit, description);
    }

    fn describe_gauge(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.describe(key_name, unit, description);
    }

    fn describe_histogram(&self, key_name: KeyName, unit: Option<Unit>, description: SharedString) {
        self.describe(key_name, unit, description);
    }

    fn register_counter(&self, key: &Key) -> Counter {
        self.registry
            .get_or_create_counter(key, |c| c.clone().into())
    }

    fn register_gauge(&self, key: &Key) -> Gauge {
        self.registry.get_or_create_gauge(key, |c| c.clone().into())
    }

    fn register_histogram(&self, key: &Key) -> Histogram {
        self.registry
            .get_or_create_histogram(key, |c| c.clone().into())
    }
}

impl MetricsRecorder {
    /// Update summaries with recent histogram data
    pub fn update_summaries(&self) {
        let mut inner = self.inner.lock().unwrap();
        self.registry.visit_histograms(|key, h| {
            let summary = match inner.summaries.get_mut(key) {
                Some(s) => s,
                None => {
                    let s = metrics_util::Summary::with_defaults();
                    inner.summaries.insert(key.clone(), s);
                    inner.summaries.get_mut(key).unwrap()
                }
            };
            h.clear_with(|vs| {
                for v in vs {
                    summary.add(*v)
                }
            });
        });
    }

    /// Call the given closure with each key and summary
    pub fn visit_summaries(
        &self,
        mut f: impl FnMut(&Key, &metrics_util::Summary, Option<&MetricDescription>),
    ) {
        let inner = self.inner.lock().unwrap();
        for (ref key, ref summary) in inner.summaries.iter() {
            let description = inner.descriptions.get(&key.get_hash());
            f(key, summary, description);
        }
    }
}
