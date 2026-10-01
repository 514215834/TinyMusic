import { defineStore } from "pinia";
import { settingsApi } from "../services/settings";
import { setLocale, type Locale } from "../i18n";

export type ThemeChoice = "system" | "light" | "dark";

interface Appearance {
  theme: ThemeChoice;
  locale: Locale;
}

/** 三态主题应用：data-theme 覆盖；system 时移除属性交由 prefers-color-scheme（main.css） */
function applyTheme(theme: ThemeChoice) {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
}

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    theme: "system" as ThemeChoice,
    locale: "zh" as Locale,
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
        }
      } catch (e) {
        console.error("读取外观设置失败", e);
      }
      this.loaded = true;
      applyTheme(this.theme);
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
    async save() {
      const appearance: Appearance = { theme: this.theme, locale: this.locale };
      await settingsApi.set("appearance", JSON.stringify(appearance)).catch(() => {});
    },
  },
});
