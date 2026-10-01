-- 回填搜索索引：0002 建 FTS 表时已有曲目的行不会触发同步触发器，
-- 用 OR REPLACE 保证重复执行/自愈安全
INSERT OR REPLACE INTO tracks_fts(rowid, title, artist, album)
SELECT t.id,
       t.title,
       COALESCE((SELECT name FROM artists WHERE id = t.artist_id), ''),
       COALESCE((SELECT name FROM albums  WHERE id = t.album_id ), '')
FROM tracks t;
