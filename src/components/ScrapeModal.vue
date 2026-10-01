<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Check, Globe, ImageDown, X } from "lucide-vue-next";
import { scrapeApi, type AlbumCandidate } from "../services/scrape";
import { t } from "../i18n";

/**
 * 在线刮削候选选择（M4）：iTunes（JP 店面优先，J-Pop 主要来源）。
 * album 模式应用封面 + 年份/流派到专辑及曲目；track 模式仅应用该曲目封面。
 */
const props = defineProps<{
  mode: "album" | "track";
  targetId: number;
  /** 展示用检索对象名（专辑名或曲名） */
  name: string;
}>();
const emit = defineEmits<{ close: []; applied: [candidate: AlbumCandidate] }>();

const loading = ref(true);
const error = ref("");
const candidates = ref<AlbumCandidate[]>([]);
const appliedProvider = ref<string | null>(null);
const applying = ref<string | null>(null);

onMounted(async () => {
  try {
    candidates.value = props.mode === "album"
      ? await scrapeApi.album(props.targetId)
      : await scrapeApi.track(props.targetId);
    if (!candidates.value.length) error.value = t("scrape.noResult");
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    loading.value = false;
  }
});

async function apply(candidate: AlbumCandidate) {
  applying.value = candidate.provider + candidate.name;
  error.value = "";
  try {
    if (props.mode === "album") {
      await scrapeApi.applyAlbum(props.targetId, candidate);
    } else {
      await scrapeApi.applyTrack(props.targetId, candidate);
    }
    appliedProvider.value = candidate.provider;
    emit("applied", candidate);
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    applying.value = null;
  }
}

function providerLabel(provider: string) {
  const store = provider.split("-")[1]?.toUpperCase();
  return store ? `iTunes (${store})` : provider;
}
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal">
      <header class="m-head">
        <Globe :size="14" />
        <div class="m-title">
          <h3>{{ mode === "album" ? t("scrape.albumTitle") : t("scrape.trackTitle") }}</h3>
          <span class="dim">{{ name }}</span>
        </div>
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>

      <div class="m-body">
        <div v-if="loading" class="m-empty dim">{{ t("scrape.searching") }}</div>
        <div v-else-if="error && !candidates.length" class="m-empty">
          <span class="error">{{ error }}</span>
          <span class="dim hint">{{ t("scrape.offlineHint") }}</span>
        </div>
        <template v-else>
          <p v-if="error" class="error">{{ error }}</p>
          <p v-if="appliedProvider" class="ok">{{ t("scrape.applied", { provider: providerLabel(appliedProvider) }) }}</p>
          <div v-for="(c, i) in candidates" :key="`${c.provider}-${c.name}-${i}`" class="cand">
            <span class="thumb">
              <img v-if="c.artworkUrl" :src="c.artworkUrl" alt="" loading="lazy" />
              <ImageDown v-else :size="18" />
            </span>
            <span class="info">
              <span class="c-name">{{ c.name }}</span>
              <span class="dim c-meta">
                {{ c.artist }}
                <template v-if="c.year"> · {{ c.year }}</template>
                <template v-if="c.genre"> · {{ c.genre }}</template>
                <template v-if="c.trackCount"> · {{ t("albums.trackCount", { n: c.trackCount }) }}</template>
              </span>
              <span class="provider dim">{{ providerLabel(c.provider) }}</span>
            </span>
            <button class="apply" :disabled="applying != null" @click="apply(c)">
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
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 120;
}

.modal {
  width: 560px;
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

.thumb {
  flex: none;
  width: 44px;
  height: 44px;
  border-radius: 6px;
  background: var(--bg-hover);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
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

.c-meta {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.provider {
  font-size: 11px;
  opacity: 0.7;
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
