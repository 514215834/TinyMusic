import {
  commands,
  type AlbumCandidate,
  type LyricsCandidate,
} from "../bindings";
import { unwrap } from "./ipc";

export type { AlbumCandidate, LyricsCandidate };

/** 在线刮削（M4）：iTunes（JP 店面优先）/LRCLIB，手动触发，候选确认后应用 */
export const scrapeApi = {
  /** 按专辑搜索封面与专辑信息 */
  album: async (albumId: number) => unwrap(await commands.scrapeAlbum(albumId)),
  /** 按单曲搜索封面 */
  track: async (trackId: number) => unwrap(await commands.scrapeTrack(trackId)),
  /** 应用专辑候选：封面 + 年份/流派写回专辑及其曲目 */
  applyAlbum: async (albumId: number, candidate: AlbumCandidate) =>
    unwrap(await commands.scrapeApplyAlbum(albumId, candidate)),
  /** 应用单曲候选：仅该曲目封面 */
  applyTrack: async (trackId: number, candidate: AlbumCandidate) =>
    unwrap(await commands.scrapeApplyTrack(trackId, candidate)),
  /** 按曲目搜索歌词候选（LRCLIB） */
  lyrics: async (trackId: number) => unwrap(await commands.scrapeLyrics(trackId)),
  /** 应用歌词候选（同步歌词优先） */
  applyLyrics: async (trackId: number, candidate: LyricsCandidate) =>
    unwrap(await commands.scrapeApplyLyrics(trackId, candidate)),
};
