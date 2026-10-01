import { commands, type SmartPlaylist, type SmartRule } from "../bindings";
import { unwrap } from "./ipc";
import type { Track } from "./library";

export type { SmartPlaylist, SmartRule };

/** 智能歌单（M4）：规则式、列表实时生成，只读不可拖拽排序 */
export const smartApi = {
  list: async () => unwrap(await commands.smartPlaylistList()),
  tracks: async (id: number) => unwrap(await commands.smartPlaylistTracks(id)),
  create: async (name: string, rules: SmartRule[], trackLimit: number | null) =>
    unwrap(await commands.smartPlaylistCreate(name, rules, trackLimit)),
  update: async (id: number, name: string, rules: SmartRule[], trackLimit: number | null) =>
    unwrap(await commands.smartPlaylistUpdate(id, name, rules, trackLimit)),
  remove: async (id: number) => unwrap(await commands.smartPlaylistDelete(id)),
};
export type { Track };
