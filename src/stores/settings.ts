import { defineStore } from "pinia";
import { settingsApi } from "../services/settings";
import { setLocale, type Locale } from "../i18n";

export type ThemeChoice = "system" | "light" | "dark";

interface Appearance {
  theme: ThemeChoice;
  locale: Locale;
  /** 自定义主题色（#rrggbb）；空串 = 使用内置默认（不写内联变量） */
  accent: string;
  /** 正在播放页封面模糊氛围背景（低性能设备可关闭） */
  ambient: boolean;
}

/** 十六进制主题色格式（#rgb / #rrggbb） */
export function isValidAccent(color: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(color);
}

/** 归一化为 #rrggbb */
export function normalizeAccent(color: string): string {
  if (color.length === 4) {
    return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`.toLowerCase();
  }
  return color.toLowerCase();
}

/** WCAG 相对亮度（sRGB 线性化），用于决定强调色背景上的文字明暗 */
function relativeLuminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function applyTheme(theme: ThemeChoice) {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
}

/** 应用自定义主题色：写入内联 --accent 覆盖默认；空值恢复内置色 */
function applyAccent(color: string) {
  const root = document.documentElement;
  if (!color) {
    root.style.removeProperty("--accent");
    root.style.removeProperty("--on-accent");
    return;
  }
  const hex = normalizeAccent(color);
  root.style.setProperty("--accent", hex);
  // 亮度高的强调色（黄/青/浅绿）白字不可读 → 深色文字（同 YesPlayMusic 的对比度处理）
  const onAccent = relativeLuminance(hex) > 0.55 ? "#17181c" : "#ffffff";
  root.style.setProperty("--on-accent", onAccent);
}

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    theme: "system" as ThemeChoice,
    locale: "zh" as Locale,
    accent: "",
    ambient: true,
    loaded: false,
  }),
  actions: {
    async load() {
      if (this.loaded) return;
      try {
        const raw = await settingsApi.get("appearance");
        if (raw) {
          const saved = JSON.parse(raw) as Partial<Appearance>;
          if (saved.theme === "light" || saved.theme === "dark" || saved.theme === "system") {
            this.theme = saved.theme;
          }
          if (saved.locale === "en" || saved.locale === "zh") this.locale = saved.locale;
          if (typeof saved.accent === "string" && isValidAccent(saved.accent)) {
            this.accent = normalizeAccent(saved.accent);
          }
          if (typeof saved.ambient === "boolean") this.ambient = saved.ambient;
        }
      } catch (e) {
        console.error("读取外观设置失败", e);
      }
      this.loaded = true;
      applyTheme(this.theme);
      applyAccent(this.accent);
      setLocale(this.locale);
    },
    setTheme(theme: ThemeChoice) {
      this.theme = theme;
      applyTheme(theme);
      void this.save();
    },
    setLocale(locale: Locale) {
      this.locale = locale;
      setLocale(locale);
      void this.save();
    },
    /** 自定义主题色（预设色板或取色器），空串恢复默认；即时生效并持久化 */
    setAccent(color: string) {
      this.accent = color ? normalizeAccent(color) : "";
      applyAccent(this.accent);
      void this.save();
    },
    setAmbient(on: boolean) {
      this.ambient = on;
      void this.save();
    },
    async save() {
      const appearance: Appearance = {
        theme: this.theme,
        locale: this.locale,
        accent: this.accent,
        ambient: this.ambient,
      };
      await settingsApi.set("appearance", JSON.stringify(appearance)).catch(() => {});
    },
  },
});
