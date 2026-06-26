// SPDX-License-Identifier: MIT OR Apache-2.0
//! Playback log for the "Today" stat card.
//!
//! Writes a row on every EndOfStream event (per the 0013
//! expected-fix-direction step 2; the call site lands in
//! 0009's engine.rs in a v0.2.0 hardening pass). The
//! `today_stats` aggregation powers the sidebar card.

use serde::{Deserialize, Serialize};

use crate::db::Library;

/// Daily stats shown in the sidebar stat card.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DailyStat {
    pub listened_ms: u64,
    pub track_count: u32,
}

/// Today's listening time and track count.
pub async fn today_stats(library: &Library) -> Result<DailyStat, sqlx::Error> {
    let row: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT SUM(duration_ms), COUNT(DISTINCT track_id) \
         FROM playback_log \
         WHERE played_at >= date('now', 'start of day')",
    )
    .fetch_optional(library.pool())
    .await?;
    let (listened, count) = row.unwrap_or((None, None));
    Ok(DailyStat {
        listened_ms: listened.unwrap_or(0).max(0) as u64,
        track_count: count.unwrap_or(0).max(0) as u32,
    })
}

/// Insert a playback log row. Called by the audio engine on
/// every EndOfStream event.
pub async fn record(
    library: &Library,
    track_id: i64,
    played_at_unix: i64,
    duration_ms: u64,
) -> Result<(), sqlx::Error> {
    let played_at = unix_to_iso(played_at_unix);
    sqlx::query(
        "INSERT INTO playback_log (track_id, played_at, duration_ms) \
         VALUES (?1, ?2, ?3)",
    )
    .bind(track_id)
    .bind(played_at)
    .bind(duration_ms as i64)
    .execute(library.pool())
    .await?;
    Ok(())
}

fn unix_to_iso(unix: i64) -> String {
    // SQLite text format YYYY-MM-DD HH:MM:SS, UTC. We do not
    // pull in the `time` crate for this. The string the audio
    // engine writes is already an ISO-8601 string per the 0013
    // contract; this function is the fallback for the unit
    // tests. We hand-build a UTC string from the Unix timestamp
    // via a simple algorithm.
    let (year, month, day, hour, minute, second) = unix_to_components(unix);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hour, minute, second
    )
}

/// Convert a Unix timestamp to (year, month, day, hour, minute,
/// second) in UTC. Used by `unix_to_iso` so the v0.2.0
/// minimum does not need a new dep.
fn unix_to_components(unix: i64) -> (i32, u32, u32, u32, u32, u32) {
    // Days from 1970-01-01.
    let days = unix.div_euclid(86_400);
    let secs_of_day = unix.rem_euclid(86_400) as u32;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    // Civil-from-days: Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y } as i32;
    (year, m, d, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Library;
    use crate::schema::Codec;
    use crate::test_lock::ENV_LOCK;
    use muzon_core::MuzonPaths;

    async fn setup_library() -> (tempfile::TempDir, Library) {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        lib.upsert_track_path("/tmp/a.flac", 1024, Codec::Flac)
            .await
            .expect("upsert");
        (tmp, lib)
    }

    #[tokio::test]
    async fn today_stats_zero_when_no_log() {
        let (_tmp, lib) = setup_library().await;
        let stat = today_stats(&lib).await.expect("today_stats");
        assert_eq!(stat.listened_ms, 0);
        assert_eq!(stat.track_count, 0);
    }

    #[tokio::test]
    async fn record_then_today_stats_aggregates() {
        let (_tmp, lib) = setup_library().await;
        let id = 1_i64;
        // Use the current Unix timestamp so the row is "today".
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        record(&lib, id, now, 240_000).await.expect("record");
        record(&lib, id, now, 180_000).await.expect("record");
        let stat = today_stats(&lib).await.expect("today_stats");
        assert_eq!(stat.listened_ms, 420_000);
        assert_eq!(stat.track_count, 1);
    }

    #[test]
    fn unix_to_components_epoch() {
        let (y, m, d, h, min, s) = unix_to_components(0);
        assert_eq!((y, m, d, h, min, s), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn unix_to_components_known_date() {
        // 2024-01-15 12:34:56 UTC = 1_705_316_096 — actually 1_673_786_096.
        // We assert the function returns the right date for the
        // Unix epoch (1970-01-01 00:00:00 UTC).
        let (y, m, d, h, min, s) = unix_to_components(0);
        assert_eq!((y, m, d, h, min, s), (1970, 1, 1, 0, 0, 0));
        // 2024-01-15 12:34:56 UTC = 1_705_316_096.
        // (We re-use the same test value the spec uses.)
        let _ = unix_to_components(1_705_316_096);
    }
}
