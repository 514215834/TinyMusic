import { describe, expect, it } from "vitest";
import { activeLrcIndex, parseLrc } from "./lrc";

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
