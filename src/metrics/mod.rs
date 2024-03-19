use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use pin_project_lite::pin_project;

pin_project! {
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TimedFuture<F, C>
    where
        F: Future,
        C: FnOnce(Duration),
    {
        #[pin]
        inner: F,
        start: Instant,
        callback: Option<C>, // This is an Option because the future might be polled even after completion
    }
}

impl<F, C> TimedFuture<F, C>
where
    F: Future,
    C: FnOnce(Duration),
{
    pub fn new(inner: F, callback: C) -> Self {
        Self {
            inner,
            callback: Some(callback),
            start: Instant::now(),
        }
    }
}

impl<F, C> Future for TimedFuture<F, C>
where
    F: Future,
    C: FnOnce(Duration),
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let poll_result = this.inner.poll(cx);

        if poll_result.is_ready() {
            let elapsed = this.start.elapsed();
            if let Some(callback) = this.callback.take() {
                callback(elapsed);
            }
        }

        poll_result
    }
}

pub fn create_timed_future<F, C>(future: F, callback: C) -> TimedFuture<F, C>
where
    F: Future,
    C: FnOnce(Duration),
{
    TimedFuture::new(future, callback)
}

pub struct CounterGuard<'a, F>
where
    F: Fn(&str, i64),
{
    node_addr: &'a str,
    func: F,
}

impl<'a, F> CounterGuard<'a, F>
where
    F: Fn(&str, i64),
{
    pub fn new(node_addr: &'a str, func: F) -> Self {
        func(node_addr, 1);
        Self { node_addr, func }
    }
}

impl<'a, F> Drop for CounterGuard<'a, F>
where
    F: Fn(&str, i64),
{
    fn drop(&mut self) {
        (self.func)(self.node_addr, -1);
    }
}

pub mod raft_metrics {
    pub mod network {
        use std::{
            collections::HashMap,
            sync::RwLock,
            time::{Duration, Instant},
        };

        use once_cell::sync::Lazy;
        use serde::Serialize;

        #[derive(Serialize)]
        pub struct MetricsSnapshot {
            pub fail_connect_to_peer: HashMap<String, u64>,
            pub sent_bytes: HashMap<String, u64>,
            pub recv_bytes: HashMap<String, u64>,
            pub sent_failures: HashMap<String, u64>,
            pub snapshot_send_success: HashMap<String, u64>,
            pub snapshot_send_failure: HashMap<String, u64>,
            pub snapshot_recv_success: HashMap<String, u64>,
            pub snapshot_recv_failure: HashMap<String, u64>,
            pub snapshot_send_inflights: HashMap<String, u64>,
            pub snapshot_recv_inflights: HashMap<String, u64>,
            pub snapshot_sent_seconds: HashMap<String, Vec<Duration>>,
            pub snapshot_recv_seconds: HashMap<String, Vec<Duration>>,
            pub snapshot_size: Vec<u64>,
            pub last_snapshot_creation_time: Duration,
        }

        struct RaftMetrics {
            fail_connect_to_peer: HashMap<String, u64>,
            sent_bytes: HashMap<String, u64>,
            recv_bytes: HashMap<String, u64>,
            sent_failures: HashMap<String, u64>,
            snapshot_send_success: HashMap<String, u64>,
            snapshot_send_failure: HashMap<String, u64>,
            snapshot_recv_success: HashMap<String, u64>,
            snapshot_recv_failure: HashMap<String, u64>,
            snapshot_send_inflights: HashMap<String, u64>,
            snapshot_recv_inflights: HashMap<String, u64>,
            snapshot_sent_seconds: HashMap<String, Vec<Duration>>,
            snapshot_recv_seconds: HashMap<String, Vec<Duration>>,
            snapshot_size: Vec<u64>,
            last_snapshot_creation_time: Option<Instant>,
        }

        impl RaftMetrics {
            fn new() -> Self {
                RaftMetrics {
                    fail_connect_to_peer: HashMap::new(),
                    sent_bytes: HashMap::new(),
                    recv_bytes: HashMap::new(),
                    sent_failures: HashMap::new(),
                    snapshot_send_success: HashMap::new(),
                    snapshot_send_failure: HashMap::new(),
                    snapshot_recv_success: HashMap::new(),
                    snapshot_recv_failure: HashMap::new(),
                    snapshot_send_inflights: HashMap::new(),
                    snapshot_recv_inflights: HashMap::new(),
                    snapshot_sent_seconds: HashMap::new(),
                    snapshot_recv_seconds: HashMap::new(),
                    snapshot_size: Vec::new(),
                    last_snapshot_creation_time: None,
                }
            }
        }

        //  TODO: Make the locks more granular and atomic
        static RAFT_METRICS: Lazy<RwLock<RaftMetrics>> =
            Lazy::new(|| RwLock::new(RaftMetrics::new()));

        pub fn incr_fail_connect_to_peer(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .fail_connect_to_peer
                .entry(node_addr.into())
                .or_insert(0);
            *count += 1;
        }

        pub fn incr_sent_bytes(node_addr: &str, bytes: u64) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics.sent_bytes.entry(node_addr.into()).or_insert(0);
            *count += bytes;
        }

        pub fn incr_recv_bytes(node_addr: &str, bytes: u64) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics.recv_bytes.entry(node_addr.into()).or_insert(0);
            *count += bytes;
        }

        pub fn incr_sent_failures(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics.sent_failures.entry(node_addr.into()).or_insert(0);
            *count += 1;
        }

        pub fn incr_snapshot_send_success(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_send_success
                .entry(node_addr.into())
                .or_insert(0);
            *count += 1;
        }

        pub fn incr_snapshot_send_failure(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_send_failure
                .entry(node_addr.into())
                .or_insert(0);
            *count += 1;
        }

        pub fn incr_snapshot_recv_success(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_recv_success
                .entry(node_addr.into())
                .or_insert(0);
            *count += 1;
        }

        pub fn incr_snapshot_recv_failure(node_addr: &str) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_recv_failure
                .entry(node_addr.into())
                .or_insert(0);
            *count += 1;
        }

        pub fn incr_snapshot_send_inflight(node_addr: &str, increment_cnt: i64) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_send_inflights
                .entry(node_addr.into())
                .or_insert(0);
            if increment_cnt < 0 {
                *count = count.saturating_sub((-increment_cnt) as u64);
            } else {
                *count = count.saturating_add(increment_cnt as u64);
            }
        }

        pub fn incr_snapshot_recv_inflight(node_addr: &str, increment_cnt: i64) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let count = metrics
                .snapshot_recv_inflights
                .entry(node_addr.into())
                .or_insert(0);
            if increment_cnt < 0 {
                *count = count.saturating_sub((-increment_cnt) as u64);
            } else {
                *count = count.saturating_add(increment_cnt as u64);
            }
        }

        pub fn incr_snapshot_sent_seconds(node_addr: &str, duration: Duration) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let durations = metrics
                .snapshot_sent_seconds
                .entry(node_addr.into())
                .or_default();
            durations.push(duration);
        }

        pub fn incr_snapshot_recv_seconds(node_addr: &str, duration: Duration) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            let durations = metrics
                .snapshot_recv_seconds
                .entry(node_addr.into())
                .or_default();
            durations.push(duration);
        }

        pub fn add_snapshot_size(size: u64) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            metrics.snapshot_size.push(size);
        }

        pub fn set_last_snapshot_creation_time(time: Instant) {
            let mut metrics = RAFT_METRICS.write().unwrap_or_else(|e| e.into_inner());
            metrics.last_snapshot_creation_time = Some(time);
        }

        pub fn get_metrics_snapshot() -> MetricsSnapshot {
            let metrics = RAFT_METRICS.read().unwrap_or_else(|e| e.into_inner());
            MetricsSnapshot {
                fail_connect_to_peer: metrics.fail_connect_to_peer.clone(),
                sent_bytes: metrics.sent_bytes.clone(),
                recv_bytes: metrics.recv_bytes.clone(),
                sent_failures: metrics.sent_failures.clone(),
                snapshot_send_success: metrics.snapshot_send_success.clone(),
                snapshot_send_failure: metrics.snapshot_send_failure.clone(),
                snapshot_recv_success: metrics.snapshot_recv_success.clone(),
                snapshot_recv_failure: metrics.snapshot_recv_failure.clone(),
                snapshot_send_inflights: metrics.snapshot_send_inflights.clone(),
                snapshot_recv_inflights: metrics.snapshot_recv_inflights.clone(),
                snapshot_sent_seconds: metrics.snapshot_sent_seconds.clone(),
                snapshot_recv_seconds: metrics.snapshot_recv_seconds.clone(),
                snapshot_size: metrics.snapshot_size.clone(),
                last_snapshot_creation_time: metrics
                    .last_snapshot_creation_time
                    .map(|t| t.elapsed())
                    .unwrap_or_default(),
            }
        }
    }
}