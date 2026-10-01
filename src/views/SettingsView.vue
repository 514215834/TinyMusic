<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useSettingsStore, type ThemeChoice } from "../stores/settings";
import { overlayApi } from "../services/overlay";
import type { Locale } from "../i18n";
import { t } from "../i18n";

const settings = useSettingsStore();

const themes: { value: ThemeChoice; label: string }[] = [
  { value: "system", label: "settings.theme.system" },
  { value: "light", label: "settings.theme.light" },
  { value: "dark", label: "settings.theme.dark" },
];

const locales: { value: Locale; label: string }[] = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
];

/* 透明度滑杆与快捷键输入的本地镜像，change（松手/回车）时下发并持久化 */
const opacity = ref(0.6);
const shortcut = ref("Ctrl+Alt+M");
const shortcutErr = ref("");

onMounted(async () => {
  void settings.load();
  try {
    const raw = await import("../services/settings").then((m) => m.settingsApi.get("overlay"));
    if (raw) {
      const parsed = JSON.parse(raw) as { opacity?: number; shortcut?: string };
      opacity.value = parsed.opacity ?? 0.6;
      shortcut.value = parsed.shortcut ?? "Ctrl+Alt+M";
    }
  } catch {
    opacity.value = 0.6;
  }
});

async function onOpacityChange() {
  await overlayApi.setOpacity(Number(opacity.value.toFixed(2))).catch(() => {});
}

async function onShortcutChange() {
  const value = shortcut.value.trim();
  if (!value) {
    shortcutErr.value = t("settings.shortcutEmpty");
    return;
  }
  try {
    await overlayApi.setShortcut(value);
    shortcut.value = value;
    shortcutErr.value = "";
  } catch {
    shortcutErr.value = t("settings.shortcutError");
  }
}
</script>

<template>
  <section class="settings">
    <header class="toolbar">
      <h2>{{ t("settings.title") }}</h2>
    </header>

    <div class="group">
      <div class="group-title dim">{{ t("settings.appearance") }}</div>
      <div class="row">
        <span>{{ t("settings.theme") }}</span>
        <div class="segment">
          <button
            v-for="th in themes"
            :key="th.value"
            :class="{ active: settings.theme === th.value }"
            @click="settings.setTheme(th.value)"
          >
            {{ t(th.label) }}
          </button>
        </div>
      </div>
      <div class="row">
        <span>{{ t("settings.language") }}</span>
        <div class="segment">
          <button
            v-for="l in locales"
            :key="l.value"
            :class="{ active: settings.locale === l.value }"
            @click="settings.setLocale(l.value)"
          >
            {{ l.label }}
          </button>
        </div>
      </div>
    </div>

    <div class="group">
      <div class="group-title dim">{{ t("settings.miniPlayer") }}</div>
      <div class="row">
        <span>{{ t("settings.opacity") }}</span>
        <div class="opacity-row">
          <input
            v-model.number="opacity"
            type="range"
            min="0.3"
            max="0.9"
            step="0.05"
            @change="onOpacityChange"
          />
          <span class="dim">{{ Math.round(opacity * 100) }}%</span>
        </div>
      </div>
      <div class="row">
        <span>{{ t("settings.shortcut") }}</span>
        <input
          v-model="shortcut"
          class="shortcut-input"
          :placeholder="t('settings.shortcutHint')"
          spellcheck="false"
          @change="onShortcutChange"
          @keyup.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
      <div v-if="shortcutErr" class="row">
        <span class="shortcut-err">{{ shortcutErr }}</span>
      </div>
    </div>

    <div class="group">
      <div class="group-title dim">{{ t("settings.about") }}</div>
      <div class="row">
        <span>{{ t("settings.aboutDesc") }}</span>
        <span class="dim">v0.3.0-dev</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings {
  max-width: 640px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.toolbar h2 {
  margin: 0;
  font-size: 18px;
}

.group {
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.group-title {
  font-size: 12px;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.segment {
  display: inline-flex;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}

.segment button {
  padding: 6px 14px;
  border: none;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
}

.segment button + button {
  border-left: 1px solid var(--border);
}

.segment button.active {
  background: var(--accent);
  color: #fff;
}

.opacity-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 220px;
}

.opacity-row input {
  flex: 1;
  accent-color: var(--accent);
}

.opacity-row span {
  min-width: 40px;
  text-align: right;
  font-size: 12px;
}

.shortcut-input {
  width: 200px;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  font-size: 13px;
}

.shortcut-input:focus {
  outline: none;
  border-color: var(--accent);
}

.shortcut-err {
  color: #dc2626;
  font-size: 12px;
}
</style>
