use once_cell::sync::Lazy;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

// グローバルな「最後にOpenAIへ投げた時刻」
static LAST_REPLY_AT: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

pub async fn wait_for_rate_limit(min_interval_ms: u64) {
    let mut guard = LAST_REPLY_AT.lock().await;
    let min_interval = Duration::from_millis(min_interval_ms);

    if let Some(last) = *guard {
        let elapsed = last.elapsed();
        if elapsed < min_interval {
            let wait = min_interval - elapsed;
            sleep(wait).await;
        }
    }

    *guard = Some(Instant::now());
}
