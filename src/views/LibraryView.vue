<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, FolderPlus, RefreshCw, Search, X } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { useLibraryStore } from "../stores/library";
import { libraryApi, type Track } from "../services/library";
import { t } from "../i18n";

const library = useLibraryStore();

onMounted(() => {
  void library.init();
});

const searchInput = ref<HTMLInputElement | null>(null);
const q = ref("");
const results = ref<Track[] | null>(null);
let searchTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => library.searchFocusTick,
  () => searchInput.value?.focus(),
);

watch(q, (v) => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(async () => {
    const query = v.trim();
    if (!query) {
      results.value = null;
      return;
    }
    const page = await libraryApi.searchTracks(query, 1, 500);
    results.value = page.items;
  }, 150);
});

const shown = computed<Track[]>(() => results.value ?? library.tracks);

async function importFolder() {
  const selected = await open({ directory: true, multiple: false, title: "选择音乐文件夹" });
  if (typeof selected === "string") {
    await library.addFolder(selected);
  }
}

function folderName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
</script>

<template>
  <section class="library">
    <header class="toolbar">
      <h2>{{ t("library.title") }}</h2>
      <span v-if="library.scan.running" class="dim">
        {{ t("library.scanning", { done: library.scan.done, total: library.scan.total || "…" }) }}
      </span>
      <span v-else class="dim">
        {{
          results
            ? t("library.searchResult", { n: shown.length })
            : t("library.total", { n: library.total })
        }}
      </span>
      <span v-if="library.lastError" class="error">扫描出错：{{ library.lastError }}</span>
      <div class="spacer"></div>
      <div class="search-box">
        <Search :size="14" class="dim" />
        <input
          ref="searchInput"
          v-model="q"
          type="text"
          :placeholder="t('library.searchPlaceholder')"
          spellcheck="false"
        />
        <button v-if="q" class="clear-q" :title="t('common.close')" @click="q = ''">
          <X :size="12" />
        </button>
      </div>
      <PlayAllButton :load="() => libraryApi.tracksAll()" />
      <button @click="library.rescan()">
        <RefreshCw :size="14" />
        {{ t("library.rescan") }}
      </button>
      <button class="primary" @click="importFolder">
        <FolderPlus :size="14" />
        {{ t("library.import") }}
      </button>
    </header>

    <div v-if="library.folders.length" class="folders">
      <span v-for="f in library.folders" :key="f.id" class="folder-chip" :title="f.path">
        <Folder :size="12" class="chip-folder" />
        <span class="chip-name">{{ folderName(f.path) }}</span>
        <button
          class="chip-x"
          :title="t('library.removeFolder')"
          @click="library.removeFolder(f.id)"
        >
          <X :size="12" />
        </button>
      </span>
    </div>

    <TrackTable
      :tracks="shown"
      :empty-text="
        results
          ? t('library.noMatch')
          : library.scan.running
            ? t('library.scanningEmpty')
            : t('library.empty')
      "
      :load-more="results ? undefined : () => library.loadMore()"
    />
    <div v-if="library.loading" class="dim loading">{{ t("library.loading") }}</div>
  </section>
</template>

<style scoped>
.library {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.toolbar h2 {
  margin: 0;
  font-size: 18px;
}

.spacer {
  flex: 1;
}

.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
}

.search-box:focus-within {
  border-color: var(--accent);
}

.search-box input {
  width: 160px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font-size: 13px;
}

.search-box svg {
  display: block;
  flex-shrink: 0;
}

.clear-q {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.clear-q:hover {
  background: var(--border);
  color: var(--text);
}

button {
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

/* 图标按 block 渲染，保证与文字垂直居中对齐 */
button svg {
  display: block;
}

button.primary {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.error {
  color: #ef4444;
  font-size: 12px;
}

.folders {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.folder-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--bg-hover);
  font-size: 12px;
  max-width: 260px;
}

/* 名字单独截断出省略号，避免把关闭按钮挤出可视区 */
.chip-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chip-folder {
  color: var(--text-dim);
  flex-shrink: 0;
}

.chip-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  margin-right: -3px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  flex-shrink: 0;
}

.chip-x:hover {
  background: var(--border);
  color: var(--text);
}

.loading {
  padding: 8px;
  text-align: center;
  font-size: 12px;
}
</style>
