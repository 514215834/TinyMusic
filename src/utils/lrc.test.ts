import { describe, expect, it } from "vitest";
import { activeLrcIndex, activeWordIndex, parseLrc } from "./lrc";

describe("parseLrc", () => {
  it("解析标准 mm:ss.xx 时间标签", () => {
    const lines = parseLrc("[00:12.34]晴天\n[01:02]副歌");
    expect(lines).toEqual([
      { timeMs: 12_340, text: "晴天" },
      { timeMs: 62_000, text: "副歌" },
    ]);
  });

  it("一行多时间标签展开为多行", () => {
    const lines = parseLrc("[00:01.00][01:01.00]重复段");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toEqual({ timeMs: 1_000, text: "重复段" });
    expect(lines[1]).toEqual({ timeMs: 61_000, text: "重复段" });
  });

  it("忽略元数据行并处理两位小数", () => {
    const lines = parseLrc("[ti:晴天]\n[ar:周杰伦]\n[00:05.56]故事的小黄花");
    expect(lines).toEqual([{ timeMs: 5_560, text: "故事的小黄花" }]);
  });

  it("两位小数按百分秒、三位按毫秒解释", () => {
    expect(parseLrc("[00:01.5]a")[0].timeMs).toBe(1_500);
    expect(parseLrc("[00:01.50]a")[0].timeMs).toBe(1_500);
    expect(parseLrc("[00:01.500]a")[0].timeMs).toBe(1_500);
    expect(parseLrc("[00:01.512]a")[0].timeMs).toBe(1_512);
  });

  it("输出按时间升序（源文件乱序时）", () => {
    const lines = parseLrc("[01:00.00]后\n[00:30.00]中\n[00:01.00]前");
    expect(lines.map((l) => l.text)).toEqual(["前", "中", "后"]);
  });

  it("支持 CRLF 与空文本行（间奏）", () => {
    const lines = parseLrc("[00:01.00]第一句\r\n[00:03.00]\r\n[00:05.00]第二句");
    expect(lines).toHaveLength(3);
    expect(lines[1].text).toBe("");
  });
});

describe("activeLrcIndex", () => {
  const lines = parseLrc("[00:01.00]一\n[00:10.00]二\n[00:20.00]三");

  it("唱到第一行之前返回 -1", () => {
    expect(activeLrcIndex(lines, 0)).toBe(-1);
    expect(activeLrcIndex(lines, 999)).toBe(-1);
  });

  it("命中时间戳所在行（含边界时刻）", () => {
    expect(activeLrcIndex(lines, 1_000)).toBe(0);
    expect(activeLrcIndex(lines, 9_999)).toBe(0);
    expect(activeLrcIndex(lines, 10_000)).toBe(1);
    expect(activeLrcIndex(lines, 20_000)).toBe(2);
  });

  it("超出最后一行后停留在末行", () => {
    expect(activeLrcIndex(lines, 999_999)).toBe(2);
  });
});

describe("M5 逐字歌词", () => {
  it("字级标签解析为 words 且行文本剥离标签", () => {
    const lines = parseLrc("[00:12.00]歌<00:12.50>词逐<00:13.10>字");
    expect(lines[0].text).toBe("歌词逐字");
    expect(lines[0].words).toEqual([
      { timeMs: 12_000, text: "歌" },
      { timeMs: 12_500, text: "词逐" },
      { timeMs: 13_100, text: "字" },
    ]);
  });

  it("首个标签前的文本从行时间起唱", () => {
    const lines = parseLrc("[00:10.00]开场<00:10.80>白");
    expect(lines[0].words).toEqual([
      { timeMs: 10_000, text: "开场" },
      { timeMs: 10_800, text: "白" },
    ]);
  });

  it("无字级标签的行无 words 字段（回退逐行滚动）", () => {
    const lines = parseLrc("[00:01.00]普通行");
    expect(lines[0].words).toBeUndefined();
    expect(lines[0].text).toBe("普通行");
  });

  it("混合歌词只有带标签的行携带 words，且整体不报错", () => {
    const lines = parseLrc("[00:01.00]一<00:01.50>二\n[00:05.00]纯文本\n[00:09.00]三<00:09.20>四");
    expect(lines).toHaveLength(3);
    expect(lines[0].words).toBeDefined();
    expect(lines[1].words).toBeUndefined();
    expect(lines[2].words).toBeDefined();
  });

  it("一行多时间标签重复演唱时字级时间平移", () => {
    const lines = parseLrc("[00:10.00][01:10.00]副<00:10.50>歌");
    expect(lines[0].words?.map((w) => w.timeMs)).toEqual([10_000, 10_500]);
    expect(lines[1].words?.map((w) => w.timeMs)).toEqual([70_000, 70_500]);
    expect(lines[1].words?.map((w) => w.text)).toEqual(["副", "歌"]);
  });

  it("字词标签兼容两位百分秒与三位毫秒", () => {
    const a = parseLrc("[00:01.00]x<00:01.25>y")[0].words;
    const b = parseLrc("[00:01.00]x<00:01.250>y")[0].words;
    expect(a?.[1].timeMs).toBe(1_250);
    expect(b?.[1].timeMs).toBe(1_250);
  });

  it("字词标签与行时间标签互不干扰（方括号不误匹配尖括号）", () => {
    const lines = parseLrc("[00:01.00][ti:x]<00:02.00>词");
    expect(lines).toHaveLength(1);
    expect(lines[0].words?.map((w) => w.text)).toEqual(["词"]);
  });

  it("activeWordIndex 定位当前字词（含未唱与边界）", () => {
    const words = parseLrc("[00:10.00]a<00:10.50>b<00:11.00>c")[0].words!;
    expect(activeWordIndex(words, 9_999)).toBe(-1);
    expect(activeWordIndex(words, 10_000)).toBe(0);
    expect(activeWordIndex(words, 10_499)).toBe(0);
    expect(activeWordIndex(words, 10_500)).toBe(1);
    expect(activeWordIndex(words, 999_999)).toBe(2);
  });
});
