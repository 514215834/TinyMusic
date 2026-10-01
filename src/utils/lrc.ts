/** LRC 歌词解析（技术设计文档 §7：纯函数，独立可测） */

export interface LrcLine {
  /** 行时间戳（毫秒） */
  timeMs: number;
  /** 行文本（时间标签之外的剩余内容，可能为空=间奏行） */
  text: string;
}

/** 兼容 [mm:ss] / [mm:ss.xx] / [mm:ss.xxx]（一位/两位小数按十进制补齐） */
const TIME_TAG = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/**
 * 解析 LRC 文本：支持一行多时间标签、忽略元数据行（[ti:]/[ar:] 等），
 * 输出按时间升序排列。
 */
export function parseLrc(content: string): LrcLine[] {
  const lines: LrcLine[] = [];
  for (const raw of content.split(/\r?\n/)) {
    TIME_TAG.lastIndex = 0;
    const stamps: number[] = [];
    let match: RegExpExecArray | null;
    while ((match = TIME_TAG.exec(raw))) {
      const [, min, sec, frac] = match;
      let fracMs = 0;
      if (frac) {
        const v = Number(frac);
        fracMs = frac.length === 1 ? v * 100 : frac.length === 2 ? v * 10 : v;
      }
      stamps.push(Number(min) * 60_000 + Number(sec) * 1000 + fracMs);
    }
    if (!stamps.length) continue; // 元数据行或空行
    const text = raw.replace(TIME_TAG, "").trim();
    for (const timeMs of stamps) lines.push({ timeMs, text });
  }
  return lines.sort((a, b) => a.timeMs - b.timeMs);
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
