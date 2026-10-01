-- M4 曲库与数据进阶：智能歌单（技术设计文档 §4：规则 JSON 实时生成，不物化）

CREATE TABLE smart_playlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    rules       TEXT NOT NULL,
    track_limit INTEGER,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
