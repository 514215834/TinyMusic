import {
  commands,
  type AlbumCandidate,
  type LyricsCandidate,
  type ScrapeSource,
} from "../bindings";
import { unwrap } from "./ipc";

export type { AlbumCandidate, LyricsCandidate, ScrapeSource };

/** 在线刮削（M4+多站点增强）：iTunes（JP 店面优先）/ Deezer / MusicBrainz / LRCLIB，
 *  source 为 null 时全部站点合并检索；query 为自定义检索词（覆盖标签元数据） */
export const scrapeApi = {
  /** 按专辑搜索封面与专辑信息（多站点） */
  album: async (albumId: number, source: ScrapeSource | null, query: string | null) =>
    unwrap(await commands.scrapeAlbum(albumId, source, query)),
  /** 按单曲搜索封面（iTunes/Deezer） */
  track: async (trackId: number, source: ScrapeSource | null, query: string | null) =>
    unwrap(await commands.scrapeTrack(trackId, source, query)),
  /** 应用专辑候选：封面 + 年份/流派写回专辑及其曲目 */
  applyAlbum: async (albumId: number, candidate: AlbumCandidate) =>
    unwrap(await commands.scrapeApplyAlbum(albumId, candidate)),
  /** 应用单曲候选：仅该曲目封面 */
  applyTrack: async (trackId: number, candidate: AlbumCandidate) =>
    unwrap(await commands.scrapeApplyTrack(trackId, candidate)),
  /** 搜索歌词候选（LRCLIB；query 为自定义检索词，走 q 模糊匹配） */
  lyrics: async (trackId: number, query: string | null) =>
    unwrap(await commands.scrapeLyrics(trackId, query)),
  /** 应用歌词候选（同步歌词优先） */
  applyLyrics: async (trackId: number, candidate: LyricsCandidate) =>
    unwrap(await commands.scrapeApplyLyrics(trackId, candidate)),
};
