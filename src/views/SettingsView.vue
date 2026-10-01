<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useSettingsStore, type ThemeChoice } from "../stores/settings";
import { usePlayerStore, EQ_FREQS, EQ_MIN, EQ_MAX } from "../stores/player";
import { overlayApi } from "../services/overlay";
import type { Locale } from "../i18n";
import { t } from "../i18n";

const settings = useSettingsStore();
const player = usePlayerStore();

const themes: { value: ThemeChoice; label: string }[] = [
  { value: "system", label: "settings.theme.system" },
  { value: "light", label: "settings.theme.light" },
  { value: "dark", label: "settings.theme.dark" },
];

const locales: { value: Locale; label: string }[] = [
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
];

/** 预设主题色板：首项为内置默认（accent=""，恢复默认），其余为常用强调色 */
const ACCENT_PRESETS: { color: string; labelKey: string }[] = [
  { color: "", labelKey: "settings.accentDefault" },
  { color: "#3b82f6", labelKey: "" },
  { color: "#6366f1", labelKey: "" },
  { color: "#8b5cf6", labelKey: "" },
  { color: "#ec4899", labelKey: "" },
  { color: "#ef4444", labelKey: "" },
  { color: "#f97316", labelKey: "" },
  { color: "#10b981", labelKey: "" },
  { color: "#14b8a6", labelKey: "" },
];

/** 取色器镜像值（原生控件要求 #rrggbb；未自定义时以默认色为起点） */
const customColor = computed({
  get: () => (settings.accent ? settings.accent : "#3b82f6"),
  set: (v: string) => settings.setAccent(v),
});

/* 透明度滑杆与快捷键输入的本地镜像，change（松手/回车）时下发并持久化 */
const opacity = ref(0.6);
const shortcut = ref("Ctrl+Alt+M");
const shortcutErr = ref("");

onMounted(async () => {
  void settings.load();
  void loadDevices();
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

/* ---- M3 音频：输出设备 + EQ ---- */
const devices = ref<MediaDeviceInfo[]>([]);
const outputNotice = ref("");

async function loadDevices() {
  try {
    const list = await navigator.mediaDevices.enumerateDevices();
    devices.value = list.filter((d) => d.kind === "audiooutput");
  } catch {
    outputNotice.value = t("settings.outputUnavailable");
  }
}

async function onDeviceChange(e: Event) {
  const deviceId = (e.target as HTMLSelectElement).value;
  const ok = await player.setOutputDevice(deviceId);
  if (!ok) {
    outputNotice.value = t("settings.outputFail");
    void loadDevices();
    return;
  }
  outputNotice.value = "";
}

/** 频段刻度：≥1kHz 显示 kHz */
function freqLabel(freq: number) {
  return freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
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
      <div class="row">
        <span>{{ t("settings.accentColor") }}</span>
        <div class="accent-row">
          <button
            v-for="p in ACCENT_PRESETS"
            :key="p.color || 'default'"
            class="swatch"
            :class="{ active: settings.accent === p.color }"
            :style="{ background: p.color || 'linear-gradient(135deg, #60a5fa, #ec4899)' }"
            :title="p.labelKey ? t(p.labelKey) : p.color"
            @click="settings.setAccent(p.color)"
          ></button>
          <label class="custom-swatch" :title="t('settings.accentCustom')">
            <input v-model="customColor" type="color" />
            <span class="dim">{{ t("settings.accentCustom") }}</span>
          </label>
        </div>
      </div>
      <div class="row">
        <span :title="t('settings.ambientHint')">{{ t("settings.ambient") }}</span>
        <div class="segment">
          <button :class="{ active: settings.ambient }" @click="settings.setAmbient(true)">
            {{ t("common.on") }}
          </button>
          <button :class="{ active: !settings.ambient }" @click="settings.setAmbient(false)">
            {{ t("common.off") }}
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
      <div class="group-title dim">{{ t("settings.audio") }}</div>
      <div class="row">
        <span>{{ t("settings.outputDevice") }}</span>
        <select class="device-select" :value="player.outputDeviceId" @change="onDeviceChange">
          <option value="">{{ t("settings.outputDefault") }}</option>
          <option v-for="(d, i) in devices" :key="d.deviceId" :value="d.deviceId">
            {{ d.label || t("settings.outputDeviceN", { n: i + 1 }) }}
          </option>
        </select>
      </div>
      <div v-if="outputNotice" class="row">
        <span class="notice">{{ outputNotice }}</span>
      </div>
      <div class="row eq-head">
        <span>{{ t("settings.eq") }}</span>
        <div class="eq-controls">
          <button class="eq-reset" :title="t('settings.eqReset')" @click="player.resetEq()">
            {{ t("settings.eqReset") }}
          </button>
          <div class="segment">
            <button :class="{ active: player.eqEnabled }" @click="player.setEqEnabled(true)">
              {{ t("common.on") }}
            </button>
            <button :class="{ active: !player.eqEnabled }" @click="player.setEqEnabled(false)">
              {{ t("common.off") }}
            </button>
          </div>
        </div>
      </div>
      <div v-show="player.eqEnabled" class="eq-sliders">
        <label v-for="(freq, i) in EQ_FREQS" :key="freq">
          <input
            type="range"
            :min="EQ_MIN"
            :max="EQ_MAX"
            step="1"
            :value="player.eqGains[i] ?? 0"
            @input="player.setEqGain(i, Number(($event.target as HTMLInputElement).value))"
          />
          <span>{{ freqLabel(freq) }}</span>
        </label>
      </div>
      <div class="row">
        <span class="notice">{{ t("settings.eqHint") }}</span>
      </div>
    </div>

    <div class="group">
      <div class="group-title dim">{{ t("settings.about") }}</div>
      <div class="row">
        <span>{{ t("settings.aboutDesc") }}</span>
        <span class="dim">v0.7.0</span>
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
  /* 页面在 .content 内直接滚动：底部让位固定播放栏，末组不被遮挡 */
  padding-bottom: 96px;
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
  color: var(--on-accent);
}

.accent-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.swatch {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 2px solid transparent;
  border-radius: 50%;
  cursor: pointer;
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease;
}

.swatch:hover {
  transform: scale(1.12);
}

.swatch.active {
  box-shadow:
    0 0 0 2px var(--bg),
    0 0 0 4px var(--accent);
}

.custom-swatch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  font-size: 12px;
}

.custom-swatch input[type="color"] {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: none;
  cursor: pointer;
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

.device-select {
  min-width: 220px;
  max-width: 320px;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  font-size: 13px;
}

.notice {
  font-size: 12px;
  color: var(--text-dim);
}

.eq-head {
  margin-top: 2px;
}

.eq-controls {
  display: inline-flex;
  align-items: center;
  gap: 10px;
}

.eq-reset {
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  font-size: 12px;
  padding: 5px 10px;
  cursor: pointer;
}

.eq-reset:hover {
  background: var(--bg-hover);
}

/* 十段竖向滑杆：writing-mode 方案（WebView2/Chromium 支持），底=-12dB 顶=+12dB */
.eq-sliders {
  display: flex;
  justify-content: space-between;
  gap: 6px;
  padding: 2px 4px 0;
}

.eq-sliders label {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.eq-sliders input[type="range"] {
  writing-mode: vertical-lr;
  direction: rtl;
  width: 22px;
  height: 110px;
  accent-color: var(--accent);
}

.eq-sliders span {
  font-size: 10px;
  color: var(--text-dim);
}
</style>
