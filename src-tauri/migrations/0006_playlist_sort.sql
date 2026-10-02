-- 歌单自定义排序（侧边栏拖拽重排）：position 升序，同位按 id 兜底稳定

ALTER TABLE playlists ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
