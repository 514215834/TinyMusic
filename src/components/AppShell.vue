<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Disc3,
  Folder,
  Heart,
  History,
  Library,
  ListMusic,
  Pencil,
  Plus,
  Settings,
  Sparkles,
  Trash2,
  Upload,
  User,
} from "lucide-vue-next";
import { playlistsApi, type Playlist } from "../services/playlists";
import { smartApi, type SmartPlaylist } from "../services/smartPlaylists";
import { useKeyboardShortcuts } from "../composables/useKeyboardShortcuts";
import { usePlayerStore } from "../stores/player";
import { useSettingsStore } from "../stores/settings";
import { t } from "../i18n";
import PlayerBar from "./PlayerBar.vue";
import NowPlaying from "./NowPlaying.vue";
// 主窗口三区布局：左侧导航 + 中部内容 + 底部播放条（功能设计文档 §4.1）

useKeyboardShortcuts();
const router = useRouter();
const player = usePlayerStore();
const settings = useSettingsStore();
void settings.load();

const playlists = ref<Playlist[]>([]);
const smartPlaylists = ref<SmartPlaylist[]>([]);

onMounted(loadAll);

async function loadAll() {
  const [pls, smarts] = await Promise.all([playlistsApi.list(), smartApi.list()]);
  playlists.value = pls;
  smartPlaylists.value = smarts;
}

/** 智能歌单：+ 直接创建（默认规则=播放≥1次），跳转后规则可改 */
async function createSmart() {
  const created = await smartApi.create(t("smart.defaultName"), [{ field: "playCount", op: "gte", num: 1, text: null }], null);
  await loadAll();
  void router.push({ name: "smart-playlist", params: { id: created.id } });
}

/* 新建歌单：行内输入 */
const creating = ref(false);
const newName = ref("");
const nameInput = ref<HTMLInputElement | null>(null);

function startCreate() {
  creating.value = true;
  newName.value = "";
  requestAnimationFrame(() => nameInput.value?.focus());
}

async function confirmCreate() {
  const name = newName.value.trim();
  if (name) await playlistsApi.create(name);
  creating.value = false;
  await loadAll();
}

/* 重命名歌单：行内输入 */
const renamingId = ref<number | null>(null);
const renameValue = ref("");
const renameInput = ref<HTMLInputElement | null>(null);

function startRename(p: Playlist) {
  renamingId.value = p.id;
  renameValue.value = p.name;
  requestAnimationFrame(() => renameInput.value?.focus());
}

async function confirmRename() {
  if (renamingId.value != null) {
    const name = renameValue.value.trim();
    if (name) await playlistsApi.rename(renamingId.value, name);
  }
  renamingId.value = null;
  await loadAll();
}

async function remove(p: Playlist) {
  await playlistsApi.remove(p.id);
  await loadAll();
}

async function removeSmart(s: SmartPlaylist) {
  await smartApi.remove(s.id);
  await loadAll();
  if (
    router.currentRoute.value.name === "smart-playlist" &&
    Number(router.currentRoute.value.params.id) === s.id
  ) {
    void router.push({ name: "library" });
  }
}

/* M3：从 M3U8 导入为新歌单（曲库中未命中的路径自动跳过，结果由后端返回） */
const importError = ref("");

function playlistNameFromPath(path: string) {
  const file = path.split(/[\\/]/).pop() ?? path;
  return file.replace(/\.(m3u8?|M3U8?)$/, "");
}

async function importPlaylist() {
  importError.value = "";
  const selected = await open({
    multiple: false,
    filters: [{ name: "M3U8", extensions: ["m3u8", "m3u"] }],
  });
  if (typeof selected !== "string") return;
  try {
    const result = await playlistsApi.importM3u8(selected, playlistNameFromPath(selected));
    await loadAll();
    void router.push({ name: "playlist", params: { id: result.playlistId } });
  } catch (e) {
    importError.value = String(e instanceof Error ? e.message : e);
  }
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">TinyMusic</div>
      <nav class="nav">
        <RouterLink class="nav-item" :to="{ name: 'library' }">
          <Library :size="16" />
          {{ t("nav.library") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'albums' }">
          <Disc3 :size="16" />
          {{ t("nav.albums") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'artists' }">
          <User :size="16" />
          {{ t("nav.artists") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'folders' }">
          <Folder :size="16" />
          {{ t("nav.folders") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'favorites' }">
          <Heart :size="16" />
          {{ t("nav.favorites") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'history' }">
          <History :size="16" />
          {{ t("nav.history") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'settings' }">
          <Settings :size="16" />
          {{ t("nav.settings") }}
        </RouterLink>
      </nav>

      <div class="playlists-head">
        <span class="playlists-title dim">{{ t("sidebar.playlists") }}</span>
        <button class="pl-add" :title="t('playlist.import')" @click="importPlaylist">
          <Upload :size="13" />
        </button>
        <button class="pl-add" :title="t('sidebar.addPlaylist')" @click="startCreate">
          <Plus :size="13" />
        </button>
      </div>
      <div v-if="importError" class="pl-empty import-error">{{ importError }}</div>

      <div v-if="creating" class="pl-editor">
        <input
          ref="nameInput"
          v-model="newName"
          :placeholder="t('sidebar.playlistName')"
          spellcheck="false"
          @keydown.enter="confirmCreate"
          @keydown.esc="creating = false"
          @blur="confirmCreate"
        />
      </div>

      <nav class="pl-list">
        <RouterLink
          v-for="p in playlists"
          :key="p.id"
          class="pl-item"
          :to="{ name: 'playlist', params: { id: p.id } }"
          :title="p.name"
        >
          <ListMusic :size="14" />
          <template v-if="renamingId === p.id">
            <input
              ref="renameInput"
              v-model="renameValue"
              spellcheck="false"
              @keydown.enter="confirmRename"
              @keydown.esc="renamingId = null"
              @blur="confirmRename"
            />
          </template>
          <template v-else>
            <span class="pl-name">{{ p.name }}</span>
            <span class="pl-actions">
              <button
                class="pl-btn"
                :title="t('sidebar.renamePlaylist')"
                @click.prevent="startRename(p)"
              >
                <Pencil :size="12" />
              </button>
              <button
                class="pl-btn"
                :title="t('sidebar.deletePlaylist')"
                @click.prevent="remove(p)"
              >
                <Trash2 :size="12" />
              </button>
            </span>
          </template>
        </RouterLink>
        <div v-if="!playlists.length && !creating" class="pl-empty dim">
          {{ t("sidebar.emptyPlaylists") }}
        </div>
      </nav>

      <div class="playlists-head">
        <span class="playlists-title dim">{{ t("sidebar.smartPlaylists") }}</span>
        <button class="pl-add" :title="t('smart.addTip')" @click="createSmart">
          <Plus :size="13" />
        </button>
      </div>

      <nav class="pl-list">
        <RouterLink
          v-for="s in smartPlaylists"
          :key="s.id"
          class="pl-item"
          :to="{ name: 'smart-playlist', params: { id: s.id } }"
          :title="s.name"
        >
          <Sparkles :size="14" />
          <span class="pl-name">{{ s.name }}</span>
          <span class="pl-count dim">{{ s.trackCount }}</span>
          <span class="pl-actions">
            <button
              class="pl-btn"
              :title="t('sidebar.deletePlaylist')"
              @click.prevent="removeSmart(s)"
            >
              <Trash2 :size="12" />
            </button>
          </span>
        </RouterLink>
        <div v-if="!smartPlaylists.length" class="pl-empty dim">
          {{ t("sidebar.emptySmart") }}
        </div>
      </nav>
    </aside>

    <main class="content">
      <RouterView />
    </main>

    <footer class="player-bar">
      <PlayerBar />
    </footer>

    <NowPlaying v-if="player.nowPlayingOpen" />
  </div>
</template>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: 220px 1fr;
  grid-template-rows: 1fr 72px;
  grid-template-areas:
    "sidebar content"
    "player player";
  height: 100vh;
}

.sidebar {
  grid-area: sidebar;
  border-right: 1px solid var(--border);
  padding: 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: auto;
}

.brand {
  font-weight: 700;
  font-size: 18px;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  padding: 8px 10px;
  border-radius: 8px;
  color: var(--text);
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.nav-item svg {
  display: block;
  color: var(--text-dim);
}

.nav-item.router-link-active {
  background: var(--bg-hover);
  font-weight: 600;
}

.nav-item.router-link-active svg {
  color: var(--accent);
}

.playlists-head {
  display: flex;
  align-items: center;
  margin-top: 12px;
}

.playlists-title {
  flex: 1;
  font-size: 12px;
}

.pl-add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.pl-add:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.pl-add svg {
  display: block;
}

.pl-editor input {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--accent);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  font-size: 13px;
  outline: none;
}

.pl-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.pl-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 8px;
  color: var(--text);
  text-decoration: none;
  font-size: 13px;
}

.pl-item svg {
  display: block;
  color: var(--text-dim);
  flex-shrink: 0;
}

.pl-item.router-link-active {
  background: var(--bg-hover);
  font-weight: 600;
}

.pl-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pl-actions {
  display: none;
  gap: 2px;
}

.pl-item:hover .pl-actions {
  display: inline-flex;
}

.pl-count {
  font-size: 11px;
  flex-shrink: 0;
}

.pl-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.pl-btn:hover {
  background: var(--border);
  color: var(--text);
}

.pl-btn svg {
  display: block;
}

.pl-item input {
  flex: 1;
  min-width: 0;
  padding: 3px 6px;
  border: 1px solid var(--accent);
  border-radius: 6px;
  background: var(--bg-elev);
  color: var(--text);
  font-size: 13px;
  outline: none;
}

.pl-empty {
  padding: 6px 10px;
  font-size: 12px;
}

.import-error {
  color: #ef4444;
  word-break: break-all;
}

.content {
  grid-area: content;
  overflow: auto;
  padding: 20px 24px;
}

.player-bar {
  grid-area: player;
  border-top: 1px solid var(--border);
  display: flex;
  align-items: center;
  padding: 0 16px;
  position: relative;
}
</style>
