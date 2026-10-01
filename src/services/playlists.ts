import { commands, type Playlist, type Track } from "../bindings";
import { unwrap } from "./ipc";

export type { Playlist };

/** 歌单 CRUD 命令封装（技术设计文档 §5 playlist_*） */
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
};

export type { Track };
