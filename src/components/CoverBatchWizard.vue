<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Check, ImageDown, SkipForward, X } from "lucide-vue-next";
import { libraryApi, type AlbumInfo } from "../services/library";
import { scrapeApi, type AlbumCandidate } from "../services/scrape";
import { t } from "../i18n";

/**
 * 缺失封面批量刮削向导（M6）：逐专辑展示候选（多站点合并，J-Pop iTunes JP 优先），
 * 逐个人工确认——Enter 应用首个候选 / Esc 跳过；未命中可自动跳过。
 * 完成后给 成功/跳过/未命中/失败 统计。
 */
const emit = defineEmits<{ close: []; done: [] }>();

const missing = ref<AlbumInfo[]>([]);
const loadingList = ref(true);
const index = ref(0);
const candidates = ref<AlbumCandidate[]>([]);
const searching = ref(false);
const applying = ref(false);
const stepError = ref("");
const autoSkip = ref(true);
const highlightFirst = ref(false);

const stats = ref({ applied: 0, skipped: 0, missed: 0, failed: 0 });
const finished = ref(false);

const current = computed(() => missing.value[index.value] ?? null);
const progress = computed(() => `${Math.min(index.value + 1, missing.value.length)} / ${missing.value.length}`);

async function loadMissing() {
  const albums = await libraryApi.albumsQuery();
  missing.value = albums.filter((a) => !a.coverFile);
}

async function searchCurrent() {
  const album = current.value;
  if (!album) return;
  searching.value = true;
  stepError.value = "";
  candidates.value = [];
  highlightFirst.value = false;
  try {
    candidates.value = await scrapeApi.album(album.id, null, null);
  } catch (e) {
    stepError.value = String(e instanceof Error ? e.message : e);
  } finally {
    searching.value = false;
    if (!candidates.value.length && autoSkip.value && !stepError.value) {
      stats.value.missed += 1;
      void advance();
    }
  }
}

async function apply(candidate: AlbumCandidate) {
  const album = current.value;
  if (!album || applying.value) return;
  applying.value = true;
  stepError.value = "";
  try {
    await scrapeApi.applyAlbum(album.id, candidate);
    stats.value.applied += 1;
    void advance();
  } catch (e) {
    stepError.value = String(e instanceof Error ? e.message : e);
    stats.value.failed += 1;
  } finally {
    applying.value = false;
  }
}

function skip() {
  if (finished.value) return;
  stats.value.skipped += 1;
  void advance();
}

async function advance() {
  if (index.value >= missing.value.length - 1) {
    finished.value = true;
    emit("done");
    return;
  }
  index.value += 1;
  await searchCurrent();
}

function onKeydown(e: KeyboardEvent) {
  if (finished.value) {
    if (e.key === "Enter" || e.key === "Escape") emit("close");
    return;
  }
  if (e.key === "Enter" && candidates.value.length) {
    void apply(candidates.value[0]!);
    highlightFirst.value = true;
  } else if (e.key === "Escape") {
    skip();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  try {
    await loadMissing();
    if (missing.value.length) await searchCurrent();
    else finished.value = true;
  } finally {
    loadingList.value = false;
  }
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal">
      <header class="m-head">
        <ImageDown :size="14" />
        <div class="m-title">
          <h3>{{ t("coverWiz.title") }}</h3>
          <span v-if="!finished && missing.length" class="dim">{{ progress }} · {{ current?.name }}</span>
        </div>
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>

      <div class="m-body">
        <div v-if="loadingList" class="center dim">{{ t("library.loading") }}</div>

        <!-- 全部专辑都有封面 -->
        <div v-else-if="!missing.length" class="center">
          <Check :size="28" class="ok-icon" />
          <p class="dim">{{ t("coverWiz.none") }}</p>
        </div>

        <!-- 完成统计 -->
        <div v-else-if="finished" class="center summary">
          <Check :size="28" class="ok-icon" />
          <h4>{{ t("coverWiz.done") }}</h4>
          <div class="stats">
            <span class="stat ok">{{ t("coverWiz.applied", { n: stats.applied }) }}</span>
            <span class="stat dim">{{ t("coverWiz.skipped", { n: stats.skipped }) }}</span>
            <span class="stat dim">{{ t("coverWiz.missed", { n: stats.missed }) }}</span>
            <span v-if="stats.failed" class="stat error">{{ t("coverWiz.failed", { n: stats.failed }) }}</span>
          </div>
          <button class="primary" @click="emit('close')">{{ t("common.close") }}</button>
        </div>

        <!-- 向导步骤 -->
        <template v-else>
          <div class="progress-bar">
            <div class="fill" :style="{ width: `${((index) / missing.length) * 100}%` }"></div>
          </div>

          <div class="step-head">
            <div class="album-meta">
              <span class="album-name">{{ current?.name }}</span>
              <span class="dim">{{ current?.artist || t("albums.unknownArtist") }}</span>
            </div>
            <label class="auto-skip dim">
              <input v-model="autoSkip" type="checkbox" />
              {{ t("coverWiz.autoSkip") }}
            </label>
          </div>

          <div v-if="searching" class="center dim">{{ t("scrape.searching") }}</div>
          <template v-else>
            <p v-if="stepError" class="error">{{ stepError }}</p>
            <div v-if="candidates.length" class="cand-grid">
              <button
                v-for="(c, i) in candidates"
                :key="`${c.provider}-${i}`"
                class="cand"
                :class="{ first: i === 0 && highlightFirst }"
                :disabled="applying"
                :title="`${c.name} — ${c.artist}${c.year ? ` (${c.year})` : ''} · ${c.provider}`"
                @click="apply(c)"
              >
                <img v-if="c.artworkUrl" :src="c.artworkUrl" alt="" loading="lazy" />
                <ImageDown v-else :size="22" />
                <span class="cand-provider dim">{{ c.provider === "deezer" ? "Deezer" : c.provider === "musicbrainz" ? "MusicBrainz" : c.provider.split("-")[1]?.toUpperCase() ? `iTunes ${c.provider.split("-")[1]?.toUpperCase()}` : c.provider }}</span>
              </button>
            </div>
            <div v-else class="center dim no-result">
              <p>{{ t("scrape.noResult") }}</p>
            </div>

            <footer class="step-foot">
              <span class="dim key-hint">{{ t("coverWiz.keyHint") }}</span>
              <span class="spacer"></span>
              <button class="ghost" :disabled="applying" @click="skip">
                <SkipForward :size="13" />
                {{ t("coverWiz.skip") }}
              </button>
            </footer>
          </template>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 140;
  animation: fade-in 0.15s ease both;
}

.modal {
  animation: pop-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  width: 620px;
  max-width: calc(100vw - 48px);
  height: 480px;
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  background: var(--bg-elev);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.m-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
}

.m-title {
  flex: 1;
  min-width: 0;
}

.m-title h3 {
  margin: 0;
  font-size: 14px;
  color: var(--text);
}

.m-title .dim {
  font-size: 12px;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.m-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.m-close:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.m-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 14px;
  gap: 12px;
}

.center {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  text-align: center;
}

.ok-icon {
  color: var(--accent);
}

.no-result {
  min-height: 120px;
}

.summary h4 {
  margin: 0;
}

.stats {
  display: flex;
  gap: 14px;
  font-size: 13px;
}

.stat.ok {
  color: var(--accent);
  font-weight: 600;
}

.stat.error {
  color: #ef4444;
}

.progress-bar {
  height: 4px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--accent);
  border-radius: 999px;
  transition: width 0.25s ease;
}

.step-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.album-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.album-name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.auto-skip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  flex-shrink: 0;
  cursor: pointer;
}

.auto-skip input {
  accent-color: var(--accent);
}

.cand-grid {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  /* 固定行高：aspect-ratio 用在 grid 子项上时行高与拉伸高度互相打架（Chromium
     行高贡献按内容回落，格子比行高 → 行间重叠溢出），固定行高根治 */
  grid-auto-rows: 132px;
  gap: 10px;
  align-content: start;
}

.cand {
  position: relative;
  width: 100%;
  height: 100%;
  border: 2px solid transparent;
  border-radius: 10px;
  background: var(--bg-hover);
  color: var(--text-dim);
  cursor: pointer;
  overflow: hidden;
  padding: 0;
  transition:
    border-color 0.15s ease,
    transform 0.15s ease;
}

.cand:hover:not(:disabled) {
  border-color: var(--accent);
  transform: translateY(-2px);
}

.cand.first {
  border-color: var(--accent);
}

.cand:disabled {
  opacity: 0.6;
  cursor: default;
}

.cand img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.cand-provider {
  position: absolute;
  left: 4px;
  bottom: 4px;
  padding: 1px 7px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 10px;
}

.step-foot {
  display: flex;
  align-items: center;
  gap: 10px;
}

.key-hint {
  font-size: 11px;
}

.spacer {
  flex: 1;
}

.step-foot button,
.summary button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 7px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
}

.step-foot .ghost {
  border: 1px solid var(--border);
  background: var(--bg-elev);
  color: var(--text);
}

.step-foot .ghost svg {
  display: block;
}

.step-foot .ghost:hover:not(:disabled) {
  background: var(--bg-hover);
}

.summary button.primary {
  border: 1px solid var(--accent);
  background: var(--accent);
  color: var(--on-accent);
}

button:disabled {
  opacity: 0.5;
  cursor: default;
}

.error {
  margin: 0;
  color: #ef4444;
  font-size: 12px;
}
</style>
