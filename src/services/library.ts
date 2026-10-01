import {
  commands,
  type AlbumInfo,
  type ArtistInfo,
  type Folder,
  type TagPatch,
  type Track,
  type TrackPage,
} from "../bindings";
import { convertFileSrc } from "@tauri-apps/api/core";
import { unwrap } from "./ipc";

export type { AlbumInfo, ArtistInfo, Folder, Track, TrackPage };

/** 曲库相关命令封装（技术设计文档 §8：services 之外禁止直接 invoke） */
export const libraryApi = {
  folderAdd: async (path: string) => unwrap(await commands.folderAdd(path)),
  folderList: async () => unwrap(await commands.folderList()),
  folderRemove: async (id: number) => unwrap(await commands.folderRemove(id)),
  rescan: async () => unwrap(await commands.libraryRescan()),
  tracksQuery: async (page: number, pageSize: number) =>
    unwrap(await commands.tracksQuery(page, pageSize, null)),
  trackGet: async (id: number) => unwrap(await commands.trackGet(id)),
  searchTracks: async (q: string, page = 1, pageSize = 200) =>
    unwrap(await commands.searchTracks(q, page, pageSize)),
  albumsQuery: async () => unwrap(await commands.albumsQuery()),
  artistsQuery: async () => unwrap(await commands.artistsQuery()),
  albumTracks: async (albumId: number) => unwrap(await commands.albumTracks(albumId)),
  artistTracks: async (artistId: number) => unwrap(await commands.artistTracks(artistId)),
  folderTracks: async (folderId: number) => unwrap(await commands.folderTracks(folderId)),
  favoritesList: async () => unwrap(await commands.favoritesList()),
  favoritesIds: async () => unwrap(await commands.favoritesIds()),
  favoriteToggle: async (trackId: number) => unwrap(await commands.favoriteToggle(trackId)),
  historyAdd: async (trackId: number) => unwrap(await commands.historyAdd(trackId)),
  lyricsGet: async (trackId: number) => unwrap(await commands.lyricsGet(trackId)),
  coverPath: async (file: string) => unwrap(await commands.coverPath(file)),
  /** 标签编辑（M4）：写回源文件并同步曲库，返回更新后的曲目 */
  tagUpdate: async (trackId: number, patch: TagPatch) =>
    unwrap(await commands.tagUpdate(trackId, patch)),
};

/** cover_file（缓存文件名）→ asset 协议 URL；无封面返回 null */
export async function coverUrl(file: string | null | undefined): Promise<string | null> {
  if (!file) return null;
  const path = await libraryApi.coverPath(file);
  return path ? convertFileSrc(path) : null;
}

/** 播放统计（M3）：最近播放 / 最常播放，数据源 play_history */
export const statsApi = {
  recent: async (limit = 100) => unwrap(await commands.historyRecent(limit)),
  top: async (limit = 100) => unwrap(await commands.historyTop(limit)),
};
