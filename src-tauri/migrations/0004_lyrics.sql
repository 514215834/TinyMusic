-- M2 歌词：内嵌歌词缓存列（.lrc 同名文件运行时直读，不落库）
ALTER TABLE tracks ADD COLUMN lyrics TEXT;
