-- M0 基础曲库结构（技术设计文档 §4 数据模型）

CREATE TABLE folders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    path       TEXT NOT NULL UNIQUE,
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE artists (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE albums (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    artist     TEXT NOT NULL DEFAULT '',
    year       INTEGER,
    cover_file TEXT,
    UNIQUE (name, artist)
);

CREATE TABLE tracks (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    path         TEXT NOT NULL UNIQUE,
    folder_id    INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    artist_id    INTEGER REFERENCES artists(id),
    album_id     INTEGER REFERENCES albums(id),
    track_no     INTEGER,
    disc         INTEGER,
    duration_sec REAL,
    sample_rate  INTEGER,
    bitrate      INTEGER,
    year         INTEGER,
    genre        TEXT,
    cover_file   TEXT,
    mtime_ms     INTEGER NOT NULL DEFAULT 0,
    size         INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tracks_folder ON tracks (folder_id);
CREATE INDEX idx_tracks_album  ON tracks (album_id);
CREATE INDEX idx_tracks_artist ON tracks (artist_id);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
