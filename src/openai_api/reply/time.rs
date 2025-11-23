use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;

pub(super) fn now_tokyo_rfc3339() -> String {
    let now_utc: DateTime<Utc> = Utc::now();
    let jst = now_utc.with_timezone(&Tokyo);
    jst.to_rfc3339()
}
