-- M1 曲库与歌单体验（技术设计文档 §4 数据模型）

CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (playlist_id, track_id)
);

CREATE INDEX idx_playlist_tracks_pos ON playlist_tracks (playlist_id, position);

CREATE TABLE favorites (
    track_id   INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE play_history (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id  INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_history_track ON play_history (track_id);

-- 搜索虚表（技术设计文档 §5 search）：trigram 分词器支持中文子串，
-- LIKE 查询在 ≥3 字符时自动走 trigram 索引，短查询回退全表扫描
CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title,
    artist,
    album,
    tokenize = 'trigram'
);

CREATE TRIGGER tracks_fts_insert
AFTER INSERT ON tracks
BEGIN
    INSERT INTO tracks_fts(rowid, title, artist, album)
    VALUES (NEW.id, NEW.title,
            COALESCE((SELECT name FROM artists WHERE id = NEW.artist_id), ''),
            COALESCE((SELECT name FROM albums  WHERE id = NEW.album_id ), ''));
END;

CREATE TRIGGER tracks_fts_delete
AFTER DELETE ON tracks
BEGIN
    DELETE FROM tracks_fts WHERE rowid = OLD.id;
END;

CREATE TRIGGER tracks_fts_update
AFTER UPDATE OF title, artist_id, album_id ON tracks
BEGIN
    DELETE FROM tracks_fts WHERE rowid = OLD.id;
    INSERT INTO tracks_fts(rowid, title, artist, album)
    VALUES (OLD.id, NEW.title,
            COALESCE((SELECT name FROM artists WHERE id = NEW.artist_id), ''),
            COALESCE((SELECT name FROM albums  WHERE id = NEW.album_id ), ''));
END;
