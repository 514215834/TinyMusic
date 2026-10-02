import { commands, type Playlist, type PlaylistImport, type Track } from "../bindings";
import { unwrap } from "./ipc";

export type { Playlist, PlaylistImport };

/** 歌单 CRUD 命令封装（技术设计文档 §5 playlist_*；M3 增加 M3U8 导入导出） */
export const playlistsApi = {
  list: async () => unwrap(await commands.playlistList()),
  create: async (name: string) => unwrap(await commands.playlistCreate(name)),
  rename: async (id: number, name: string) => unwrap(await commands.playlistRename(id, name)),
  remove: async (id: number) => unwrap(await commands.playlistDelete(id)),
  tracks: async (id: number) => unwrap(await commands.playlistTracks(id)),
  addTracks: async (id: number, trackIds: number[]) =>
    unwrap(await commands.playlistAddTracks(id, trackIds)),
  removeTrack: async (id: number, trackId: number) =>
    unwrap(await commands.playlistRemoveTrack(id, trackId)),
  reorder: async (id: number, trackIds: number[]) =>
    unwrap(await commands.playlistReorder(id, trackIds)),
  /** 侧边栏歌单拖拽重排（全量提交顺序） */
  reorderPlaylists: async (ids: number[]) => unwrap(await commands.playlistReorderPlaylists(ids)),
  /** 导出为 UTF-8 M3U8 文件（绝对路径），返回写入曲目数 */
  exportM3u8: async (id: number, path: string) => unwrap(await commands.playlistExport(id, path)),
  /** 从 M3U8 导入为新歌单，返回 新歌单id/导入数/跳过数 */
  importM3u8: async (path: string, name: string) =>
    unwrap(await commands.playlistImport(path, name)),
};

export type { Track };
