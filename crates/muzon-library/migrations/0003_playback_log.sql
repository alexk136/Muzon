-- SPDX-License-Identifier: MIT OR Apache-2.0
-- 0003_playback_log.sql — playback log for the "Today" stat card.
--
-- Implements TZ §3.6.2 "Сегодня" stat: today's listening time
-- and track count. Rows are written by muzon-audio on every
-- EndOfStream event (per the 0013 expected-fix-direction step
-- 2; the call site lands in 0009's engine.rs in a v0.2.0
-- hardening pass).
--
-- The migration is idempotent: re-applying it is a no-op
-- (sqlx's migrate! macro tracks applied versions in
-- _sqlx_migrations).

CREATE TABLE IF NOT EXISTS playback_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_playback_log_played_at
    ON playback_log(played_at);
CREATE INDEX IF NOT EXISTS idx_playback_log_track_played_at
    ON playback_log(track_id, played_at);
