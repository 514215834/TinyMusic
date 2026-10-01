<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  Disc3,
  Heart,
  Library,
  ListMusic,
  Pencil,
  Plus,
  Settings,
  Trash2,
  User,
} from "lucide-vue-next";
import { playlistsApi, type Playlist } from "../services/playlists";
import { useKeyboardShortcuts } from "../composables/useKeyboardShortcuts";
import { usePlayerStore } from "../stores/player";
import { useSettingsStore } from "../stores/settings";
import { t } from "../i18n";
import PlayerBar from "./PlayerBar.vue";
import NowPlaying from "./NowPlaying.vue";
// 主窗口三区布局：左侧导航 + 中部内容 + 底部播放条（功能设计文档 §4.1）

useKeyboardShortcuts();
const player = usePlayerStore();
const settings = useSettingsStore();
void settings.load();

const playlists = ref<Playlist[]>([]);

onMounted(loadPlaylists);

async function loadPlaylists() {
  playlists.value = await playlistsApi.list();
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
  await loadPlaylists();
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
  await loadPlaylists();
}

async function remove(p: Playlist) {
  await playlistsApi.remove(p.id);
  await loadPlaylists();
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
        <RouterLink class="nav-item" :to="{ name: 'favorites' }">
          <Heart :size="16" />
          {{ t("nav.favorites") }}
        </RouterLink>
        <RouterLink class="nav-item" :to="{ name: 'settings' }">
          <Settings :size="16" />
          {{ t("nav.settings") }}
        </RouterLink>
      </nav>

      <div class="playlists-head">
        <span class="playlists-title dim">{{ t("sidebar.playlists") }}</span>
        <button class="pl-add" :title="t('sidebar.addPlaylist')" @click="startCreate">
          <Plus :size="13" />
        </button>
      </div>

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
