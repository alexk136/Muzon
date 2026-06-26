-- SPDX-License-Identifier: MIT OR Apache-2.0
-- 0001_initial.sql — v1 of the Muzon library schema.
--
-- Implements TZ.md §2.2.2 + §2.2.3 + §2.2.4 + §3.3:
-- eight entity tables, FTS5 virtual table over track metadata, the
-- triggers that keep the FTS index in sync, and the placeholders
-- for v0.5.0 (vec_tracks) and v0.4.0 (MusicBrainz / AcoustID columns).
--
-- The migration is idempotent: re-applying it is a no-op (sqlx's
-- migrate! macro tracks applied versions in _sqlx_migrations).

-- Tracks are the spine. The path is the natural key; everything
-- else is metadata. The ON DELETE CASCADE on the join tables
-- removes tags / genres / playlist entries when the parent is
-- gone; ON DELETE RESTRICT on Album/Artist keeps referential
-- integrity.
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL,
    duration_ms INTEGER,
    bitrate INTEGER,
    sample_rate INTEGER,
    channels INTEGER,
    codec TEXT,
    title TEXT,
    year INTEGER,
    track_no INTEGER,
    disc_no INTEGER,
    replaygain_track REAL,
    replaygain_album REAL,
    fingerprint TEXT,
    last_played_at INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0,
    skip_count INTEGER NOT NULL DEFAULT 0,
    -- v0.4.0 placeholders: MusicBrainz recording id, AcoustID
    mbid_recording TEXT,
    acoustid TEXT,
    -- v0.5.0 placeholder: the embedding column. The actual
    -- vec_tracks virtual table is added in a v0.5.0 migration.
    -- Keeping the BLOB column here means v0.1.0-0001 need not be
    -- rewritten when the embedding work lands.
    embedding BLOB,
    -- v0.4.0: tag-write back. v0.1.0 ships read-only metadata.
    needs_tag_write INTEGER NOT NULL DEFAULT 0,
    -- v0.3.0: cover art path (folder.jpg, cover.jpg, embedded)
    cover_path TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
CREATE INDEX IF NOT EXISTS idx_tracks_codec ON tracks(codec);
CREATE INDEX IF NOT EXISTS idx_tracks_year ON tracks(year);

CREATE TABLE IF NOT EXISTS artists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    sort_name TEXT,
    mbid TEXT,
    UNIQUE(name)
);

CREATE TABLE IF NOT EXISTS albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    album_artist_id INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    year INTEGER,
    cover_path TEXT,
    mbid_release TEXT,
    UNIQUE(title, album_artist_id)
);

CREATE TABLE IF NOT EXISTS genres (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    is_smart INTEGER NOT NULL DEFAULT 0,
    smart_predicate TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    last_scan_at INTEGER
);

-- Track → Album / Artist associations. AlbumArtist is on the album
-- (one per album); Track→Artist is many-to-many (collaborations).
CREATE TABLE IF NOT EXISTS track_albums (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE RESTRICT,
    PRIMARY KEY (track_id, album_id)
);

CREATE TABLE IF NOT EXISTS track_artists (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE RESTRICT,
    role TEXT NOT NULL DEFAULT 'primary',
    PRIMARY KEY (track_id, artist_id, role)
);

CREATE TABLE IF NOT EXISTS track_genres (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, genre_id)
);

CREATE TABLE IF NOT EXISTS track_tags (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, tag_id)
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);

CREATE TABLE IF NOT EXISTS folder_tracks (
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    PRIMARY KEY (folder_id, track_id)
);

-- FTS5 virtual table over the searchable columns. External-content
-- because the source of truth is the regular tables; the triggers
-- below keep the FTS index in sync.
--
-- v0.1.0 search is a single FTS5 query: `MATCH` on title, artist
-- names (joined via the application layer), album title, tag list,
-- and the file path. The query path is the §3.7 perf target:
-- < 50 ms on 50k tracks.
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
    title,
    artist_names,
    album_title,
    tag_names,
    path,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Sync triggers: every change to tracks / artists / albums / tags
-- propagates to tracks_fts. The application layer is responsible
-- for joining track_artists / track_albums / track_tags to compute
-- the denormalised artist_names / album_title / tag_names columns
-- and updating the FTS row. See `db::Library::upsert_track` and
-- `db::Library::search`.
CREATE TRIGGER IF NOT EXISTS tracks_fts_insert AFTER INSERT ON tracks
BEGIN
    INSERT INTO tracks_fts(rowid, title, artist_names, album_title, tag_names, path)
    VALUES (NEW.id,
            COALESCE(NEW.title, ''),
            '',
            '',
            '',
            COALESCE(NEW.path, ''));
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_update AFTER UPDATE OF title, path ON tracks
BEGIN
    UPDATE tracks_fts
       SET title = COALESCE(NEW.title, ''),
           path  = COALESCE(NEW.path, '')
     WHERE rowid = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_delete AFTER DELETE ON tracks
BEGIN
    DELETE FROM tracks_fts WHERE rowid = OLD.id;
END;
