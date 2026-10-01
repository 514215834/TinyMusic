/** LRC 歌词解析（技术设计文档 §7：纯函数，独立可测）；M5 扩展字级时间标签 */

export interface LrcWord {
  /** 字词起始时间（毫秒） */
  timeMs: number;
  text: string;
}

export interface LrcLine {
  /** 行时间戳（毫秒） */
  timeMs: number;
  /** 行文本（时间标签之外的剩余内容，可能为空=间奏行） */
  text: string;
  /** 字级时间标签段（增强型 LRC）；无字级标签的行不存在 → 回退逐行滚动 */
  words?: LrcWord[];
}

/** 兼容 [mm:ss] / [mm:ss.xx] / [mm:ss.xxx]（一位/两位小数按十进制补齐） */
const TIME_TAG = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/** 元数据标签（[ti:xx]/[ar:xx]/[offset:xx] 等）：与行时间标签同行出现时一并剥离（不影响 [Chorus] 类普通方括号文本） */
const META_TAG = /\[[a-zA-Z]+:[^\]]*\]/g;

/** 字级时间标签（增强型 LRC）：<mm:ss> / <mm:ss.xx> / <mm:ss.xxx> */
const WORD_TAG = /<(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?>/g;

/** 解析 [mm:ss.xx] 形式标签为毫秒（match = [全文, 分, 秒, 小数?]） */
function tagToMs(match: RegExpExecArray): number {
  const [, min, sec, frac] = match;
  let fracMs = 0;
  if (frac) {
    const v = Number(frac);
    fracMs = frac.length === 1 ? v * 100 : frac.length === 2 ? v * 10 : v;
  }
  return Number(min) * 60_000 + Number(sec) * 1000 + fracMs;
}

/**
 * 解析 LRC 文本：支持一行多时间标签、忽略元数据行（[ti:]/[ar:] 等），
 * 行内字级标签 <mm:ss.xx> 解析为 words（输出按时间升序排列）。
 */
export function parseLrc(content: string): LrcLine[] {
  const lines: LrcLine[] = [];
  for (const raw of content.split(/\r?\n/)) {
    TIME_TAG.lastIndex = 0;
    const stamps: number[] = [];
    let match: RegExpExecArray | null;
    while ((match = TIME_TAG.exec(raw))) {
      stamps.push(tagToMs(match));
    }
    if (!stamps.length) continue; // 元数据行或空行
    const stripped = raw.replace(TIME_TAG, "").replace(META_TAG, "");
    const text = stripped.replace(WORD_TAG, "").trim();
    // 字级时间只解析一次（相对首个行标签）；一行多时间标签重复演唱时按差值平移
    const baseWords = text ? parseWordTimings(stripped, stamps[0]) : null;
    for (const timeMs of stamps) {
      const line: LrcLine = { timeMs, text };
      if (baseWords) {
        const delta = timeMs - stamps[0];
        line.words = baseWords.map((w) => ({
          timeMs: w.timeMs + delta,
          text: w.text,
        }));
      }
      lines.push(line);
    }
  }
  return lines.sort((a, b) => a.timeMs - b.timeMs);
}

/**
 * 解析行内字级时间标签为字词段。
 * 首个标签前的文本视为从行时间起唱；空段跳过；无标签返回 null（回退逐行）。
 */
export function parseWordTimings(content: string, lineTimeMs: number): LrcWord[] | null {
  WORD_TAG.lastIndex = 0;
  const marks: { timeMs: number; index: number; length: number }[] = [];
  let match: RegExpExecArray | null;
  while ((match = WORD_TAG.exec(content))) {
    marks.push({ timeMs: tagToMs(match), index: match.index, length: match[0].length });
  }
  if (!marks.length) return null;

  const words: LrcWord[] = [];
  const lead = content.slice(0, marks[0].index).trim();
  if (lead) words.push({ timeMs: lineTimeMs, text: lead });
  for (let i = 0; i < marks.length; i++) {
    const start = marks[i].index + marks[i].length;
    const end = i + 1 < marks.length ? marks[i + 1].index : content.length;
    const seg = content.slice(start, end).trim();
    if (seg) words.push({ timeMs: marks[i].timeMs, text: seg });
  }
  return words.length ? words : null;
}

/**
 * 二分定位当前应高亮的行（最后一行 timeMs <= timeMs）。
 * 无命中（歌曲尚未唱到第一行）返回 -1。
 */
export function activeLrcIndex(lines: LrcLine[], timeMs: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].timeMs <= timeMs) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found;
}

/** 定位行内当前字词（最后一个 timeMs <= timeMs 的段）；未唱到返回 -1。字词量小，线性足够 */
export function activeWordIndex(words: LrcWord[], timeMs: number): number {
  let found = -1;
  for (let i = 0; i < words.length; i++) {
    if (words[i].timeMs <= timeMs) found = i;
    else break;
  }
  return found;
}
