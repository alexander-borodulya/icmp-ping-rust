use futures::Future;
use tokio::time;

use crate::config::ICMP_ECHO_TIMEOUT_DURATION_SECONDS;

pub async fn emulate_payload_delay(seq: u16) {
    let delay_seconds = match seq {
        // 0 => 3,
        // 1 => 1,
        // 2 => 6,
        // 3 => 1,
        // 4 => 2,
        // 5 => 2,
        // 6 => 2,
        // 7 => 4,
        // 8 => 2,
        // 9 => 2,
        _ => 0,
    };
    tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await
}

pub fn timeout_future<F>(future: F) -> time::Timeout<F>
where
    F: Future,
{
    time::timeout(ICMP_ECHO_TIMEOUT_DURATION_SECONDS, future)
}
