<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { CopyX, RefreshCw } from "lucide-vue-next";
import type { Track } from "../services/library";
import { duplicatesApi, type DuplicateGroup } from "../services/duplicates";
import { formatTime } from "../utils";
import { useCovers } from "../composables/useCovers";
import { t } from "../i18n";

/** 重复曲目检测与清理（M6）：源文件移入回收站（可恢复），曲库记录删除 */
const groups = ref<DuplicateGroup[]>([]);
const loading = ref(false);
const notice = ref("");
const error = ref("");
const busyId = ref<number | null>(null);
const busyGroup = ref<number | null>(null);

/** 各组首个为保留候选（添加时间最早） */
const totalDuplicates = computed(() =>
  groups.value.reduce((n, g) => n + g.items.length - 1, 0),
);

function groupKey(g: DuplicateGroup, i: number) {
  return `${g.title}-${g.artist}-${i}`;
}

async function scan() {
  loading.value = true;
  error.value = "";
  notice.value = "";
  try {
    groups.value = await duplicatesApi.scan();
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    loading.value = false;
  }
}

async function resolveGroup(g: DuplicateGroup, index: number) {
  const keep = g.items[0];
  if (!keep) return;
  busyGroup.value = index;
  error.value = "";
  notice.value = "";
  try {
    const result = await duplicatesApi.resolve(
      keep.track.id,
      g.items.slice(1).map((it) => it.track.id),
    );
    notice.value = t("dup.groupResolved", { n: result.removed, f: result.failed });
    if (result.failed === 0) {
      groups.value.splice(index, 1);
    } else {
      await scan();
    }
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    busyGroup.value = null;
  }
}

async function trashOne(g: DuplicateGroup, gi: number, itemIndex: number) {
  const keep = g.items[0];
  const item = g.items[itemIndex];
  if (!keep || !item) return;
  busyId.value = item.track.id;
  error.value = "";
  try {
    const result = await duplicatesApi.resolve(keep.track.id, [item.track.id]);
    if (result.removed > 0) {
      g.items.splice(itemIndex, 1);
      if (g.items.length < 2) groups.value.splice(gi, 1);
      notice.value = t("dup.oneResolved", { n: result.removed });
    } else {
      error.value = result.firstError ?? t("dup.failed", { n: result.failed });
    }
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    busyId.value = null;
  }
}

/* 封面懒解析（第一屏行数有限，直接全量） */
const visibleTracks = computed<Track[]>(() =>
  groups.value.flatMap((g) => g.items.map((it) => it.track)),
);
const covers = useCovers(() => visibleTracks.value);

function sizeLabel(bytes: number) {
  const mb = bytes / 1048576;
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

onMounted(scan);
</script>

<template>
  <section class="dup-view">
    <header class="toolbar">
      <CopyX :size="18" />
      <h2>{{ t("dup.title") }}</h2>
      <span v-if="groups.length" class="dim">
        {{ t("dup.groupCount", { g: groups.length, n: totalDuplicates }) }}
      </span>
      <div class="spacer"></div>
      <span v-if="notice" class="notice">{{ notice }}</span>
      <span v-if="error" class="error">{{ error }}</span>
      <button :disabled="loading" @click="scan">
        <RefreshCw :size="14" />
        {{ t("dup.rescan") }}
      </button>
    </header>

    <div v-if="loading" class="dim empty-state">{{ t("library.loading") }}</div>
    <div v-else-if="!groups.length" class="empty-state">
      <p class="dim">{{ t("dup.empty") }}</p>
      <p class="dim hint">{{ t("dup.emptyHint") }}</p>
    </div>

    <div v-else class="group-list">
      <div v-for="(g, gi) in groups" :key="groupKey(g, gi)" class="group">
        <header class="g-head">
          <div class="g-title">
            <span class="name">{{ g.title }}</span>
            <span class="dim">{{ g.artist || t("albums.unknownArtist") }} ·
              {{ t("albums.trackCount", { n: g.items.length }) }}</span>
          </div>
          <button
            class="resolve"
            :disabled="busyGroup != null"
            @click="resolveGroup(g, gi)"
          >
            {{ t("dup.keepFirst") }}
          </button>
        </header>
        <div class="items">
          <div
            v-for="(item, ii) in g.items"
            :key="item.track.id"
            class="item"
            :class="{ keep: ii === 0 }"
          >
            <span class="badge">{{ ii === 0 ? t("dup.keep") : t("dup.duplicate") }}</span>
            <span class="cover">
              <img v-if="covers[item.track.id]" :src="covers[item.track.id] ?? undefined" alt="" />
              <CopyX v-else :size="13" />
            </span>
            <span class="info">
              <span class="line1">{{ item.track.album ?? "—" }} ·
                {{ item.track.durationSec ? formatTime(item.track.durationSec) : "—" }} ·
                {{ item.size != null ? sizeLabel(item.size) : "—" }}</span>
              <span class="path dim" :title="item.track.path">{{ item.track.path }}</span>
            </span>
            <button
              v-if="ii > 0"
              class="trash"
              :disabled="busyId != null || busyGroup != null"
              :title="t('dup.trash')"
              @click="trashOne(g, gi, ii)"
            >
              {{ t("dup.trash") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.dup-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar h2 {
  margin: 0;
  font-size: 18px;
}

.spacer {
  flex: 1;
}

.toolbar button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
}

.toolbar button svg {
  display: block;
}

.toolbar button:hover:not(:disabled) {
  background: var(--bg-hover);
}

.toolbar button:disabled {
  opacity: 0.5;
  cursor: default;
}

.notice {
  color: var(--accent);
  font-size: 12px;
}

.error {
  color: #ef4444;
  font-size: 12px;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  text-align: center;
}

.empty-state p {
  margin: 0;
}

.hint {
  font-size: 12px;
}

.group-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-gutter: stable;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-bottom: 88px;
}

.group {
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-elev);
  overflow: hidden;
}

.g-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}

.g-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.g-title .name {
  font-weight: 600;
}

.resolve {
  flex-shrink: 0;
  padding: 6px 12px;
  border: 1px solid var(--accent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  color: var(--accent);
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
}

.resolve:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
}

.resolve:disabled {
  opacity: 0.5;
  cursor: default;
}

.item {
  display: grid;
  grid-template-columns: 52px auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
}

.item + .item {
  border-top: 1px dashed var(--border);
}

.item.keep {
  background: color-mix(in srgb, var(--accent) 6%, transparent);
}

.badge {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 999px;
  text-align: center;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}

.item:not(.keep) .badge {
  background: var(--bg-hover);
  color: var(--text-dim);
}

.cover {
  width: 32px;
  height: 32px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.info {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.line1 {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trash {
  flex-shrink: 0;
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  font-size: 12px;
}

.trash:hover:not(:disabled) {
  border-color: #ef4444;
  color: #ef4444;
}

.trash:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
