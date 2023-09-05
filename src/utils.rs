use crate::{config::ICMP_ECHO_TIMEOUT_DURATION_SECONDS, ping_app::EchoHandleMap};
use futures::Future;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    time::{self, Instant},
};

#[cfg(feature = "ENABLE_EMULATE_DELAY")]
pub async fn emulate_payload_delay(seq: u16) {
    let delay_seconds = match seq {
        0 => 3,
        1 => 1,
        2 => 6,
        3 => 1,
        4 => 2,
        5 => 2,
        6 => 2,
        7 => 4,
        8 => 2,
        9 => 2,
        _ => return,
    };
    tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await
}

pub fn timeout_future<F>(future: F) -> time::Timeout<F>
where
    F: Future,
{
    time::timeout(ICMP_ECHO_TIMEOUT_DURATION_SECONDS, future)
}

pub async fn interval_tick(
    seq_num: u16,
    ping_count: u16,
    interval: &mut time::Interval,
) -> Option<Instant> {
    if seq_num < ping_count - 1 {
        Some(interval.tick().await)
    } else {
        None
    }
}

pub async fn dump_conn_map(conn_map: Arc<Mutex<EchoHandleMap>>) {
    log::debug!("dump_conn_map - started");
    for connection_handle in conn_map.lock().await.values() {
        log::debug!("{}", connection_handle);
    }
    log::debug!("dump_conn_map - finished");
}
