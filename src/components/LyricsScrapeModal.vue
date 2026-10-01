<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Check, Globe, Search, X } from "lucide-vue-next";
import { scrapeApi, type LyricsCandidate } from "../services/scrape";
import { formatTime } from "../utils";
import { t } from "../i18n";

const props = defineProps<{ trackId: number; name: string; artist?: string | null }>();
const emit = defineEmits<{ close: []; applied: [candidate: LyricsCandidate] }>();

const loading = ref(true);
const error = ref("");
const candidates = ref<LyricsCandidate[]>([]);
const appliedId = ref<number | null>(null);
const applyingId = ref<number | null>(null);
/** 自定义检索词（空 = 按标签元数据检索，走 LRCLIB q 模糊匹配） */
const term = ref("");

async function load() {
  loading.value = true;
  error.value = "";
  try {
    candidates.value = await scrapeApi.lyrics(props.trackId, term.value.trim() || null);
    if (!candidates.value.length) error.value = t("scrape.noResult");
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  term.value = [props.name, props.artist].filter(Boolean).join(" ").trim();
  await load();
});

async function apply(candidate: LyricsCandidate) {
  applyingId.value = candidate.id;
  error.value = "";
  try {
    await scrapeApi.applyLyrics(props.trackId, candidate);
    appliedId.value = candidate.id;
    emit("applied", candidate);
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    applyingId.value = null;
  }
}
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal">
      <header class="m-head">
        <Globe :size="14" />
        <div class="m-title">
          <h3>{{ t("scrape.lyricsTitle") }}</h3>
          <span class="dim">{{ name }}</span>
        </div>
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>

      <!-- 自定义检索词 -->
      <div class="m-tools">
        <div class="search-row">
          <input
            v-model="term"
            :placeholder="t('scrape.termPlaceholder')"
            spellcheck="false"
            @keydown.enter="void load()"
          />
          <button class="go" :title="t('scrape.search')" @click="void load()">
            <Search :size="13" />
            {{ t("scrape.search") }}
          </button>
        </div>
      </div>

      <div class="m-body">
        <div v-if="loading" class="m-empty dim">{{ t("scrape.searching") }}</div>
        <div v-else-if="error && !candidates.length" class="m-empty">
          <span class="error">{{ error }}</span>
          <span class="dim hint">{{ t("scrape.offlineHint") }}</span>
        </div>
        <template v-else>
          <p v-if="error" class="error">{{ error }}</p>
          <p v-if="appliedId != null" class="ok">{{ t("scrape.applied", { provider: "LRCLIB" }) }}</p>
          <div v-for="c in candidates" :key="c.id" class="cand">
            <span class="info">
              <span class="c-name">
                {{ c.trackName }} — {{ c.artistName }}
                <span v-if="c.syncedLyrics" class="badge">{{ t("scrape.synced") }}</span>
              </span>
              <span class="dim c-meta">
                <template v-if="c.durationSec">· {{ formatTime(c.durationSec) }} </template>
                <template v-if="c.instrumental">· {{ t("scrape.instrumental") }}</template>
                <template v-else-if="!c.syncedLyrics">· {{ t("scrape.plainOnly") }}</template>
              </span>
            </span>
            <button class="apply" :disabled="applyingId != null" @click="apply(c)">
              <Check :size="13" />
              {{ t("scrape.apply") }}
            </button>
          </div>
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
  z-index: 220;
  animation: fade-in 0.15s ease both;
}

.modal {
  animation: pop-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  width: 520px;
  max-width: calc(100vw - 48px);
  height: 440px;
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

.m-tools {
  padding: 10px 12px 0;
}

.search-row {
  display: flex;
  gap: 8px;
}

.search-row input {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg);
  color: var(--text);
  font-size: 13px;
  outline: none;
  min-width: 0;
}

.search-row input:focus {
  border-color: var(--accent);
}

.search-row .go {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 12px;
  flex-shrink: 0;
}

.search-row .go svg {
  display: block;
}

.search-row .go:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.m-body {
  flex: 1;
  overflow: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.m-empty {
  padding: 40px 20px;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.error {
  color: #ef4444;
  font-size: 12px;
  margin: 4px 6px 0;
}

.ok {
  color: var(--accent);
  font-size: 12px;
  margin: 4px 6px;
}

.hint {
  font-size: 12px;
}

.cand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
}

.cand:hover {
  background: var(--bg-hover);
}

.info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.c-name {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--accent);
  font-size: 10px;
}

.c-meta {
  font-size: 12px;
}

.apply {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 12px;
}

.apply svg {
  display: block;
}

.apply:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.apply:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
