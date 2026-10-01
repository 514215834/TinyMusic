<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ChevronDown, Music, TextSearch } from "lucide-vue-next";
import { coverUrl, libraryApi } from "../services/library";
import { usePlayerStore } from "../stores/player";
import { activeLrcIndex, parseLrc, type LrcLine } from "../utils/lrc";
import { formatTime } from "../utils";
import { t } from "../i18n";
import LyricsScrapeModal from "./LyricsScrapeModal.vue";

const player = usePlayerStore();
const cover = ref<string | null>(null);
const lyrics = ref<LrcLine[]>([]);
const activeIdx = ref(-1);
const lyricBox = ref<HTMLElement | null>(null);
const lyricsScraping = ref(false);

let rafId = 0;
let coverToken = 0;

const title = computed(() => player.currentTrack?.title ?? t("player.nowPlayingEmpty"));
const artist = computed(() => player.currentTrack?.artist ?? "—");

async function loadCover() {
  const token = ++coverToken;
  const file = player.currentTrack?.coverFile;
  const url = await coverUrl(file);
  if (token === coverToken) cover.value = url;
}

async function loadLyrics() {
  lyrics.value = [];
  activeIdx.value = -1;
  const id = player.currentTrack?.id;
  if (id == null) return;
  try {
    const raw = await libraryApi.lyricsGet(id);
    if (raw && player.currentTrack?.id === id) lyrics.value = parseLrc(raw);
  } catch {
    lyrics.value = [];
  }
}

function tick() {
  if (player.nowPlayingOpen) {
    const timeMs = player.getTime() * 1000;
    const idx = activeLrcIndex(lyrics.value, timeMs);
    if (idx !== activeIdx.value) {
      activeIdx.value = idx;
      // 当前行滚动到歌词区中部
      if (idx >= 0 && lyricBox.value) {
        const el = lyricBox.value.querySelector<HTMLElement>(`[data-line="${idx}"]`);
        if (el) {
          lyricBox.value.scrollTo({
            top: el.offsetTop - lyricBox.value.clientHeight / 2 + el.clientHeight / 2,
            behavior: "smooth",
          });
        }
      }
    }
  }
  rafId = requestAnimationFrame(tick);
}

onMounted(() => {
  void loadCover();
  void loadLyrics();
  rafId = requestAnimationFrame(tick);
});

onBeforeUnmount(() => cancelAnimationFrame(rafId));

watch(
  () => player.currentTrack?.id,
  () => {
    void loadCover();
    void loadLyrics();
  },
);

function seekTo(line: LrcLine) {
  if (line.text && line.timeMs >= 0) player.seek(line.timeMs / 1000);
}
</script>

<template>
  <div class="now-playing">
    <button class="collapse" :title="t('common.close')" @click="player.toggleNowPlaying()">
      <ChevronDown :size="18" />
    </button>
    <button
      v-if="player.currentTrack"
      class="collapse lyrics-scrape"
      :title="t('scrape.lyricsAction')"
      @click="lyricsScraping = true"
    >
      <TextSearch :size="16" />
    </button>

    <div class="stage">
      <div class="cover-wrap">
        <img v-if="cover" :src="cover" alt="" />
        <Music v-else :size="72" :stroke-width="1.2" />
      </div>
      <div class="meta">
        <div class="title">{{ title }}</div>
        <div class="artist">{{ artist }}</div>
      </div>
    </div>

    <div ref="lyricBox" class="lyrics">
      <template v-if="lyrics.length">
        <div
          v-for="(line, i) in lyrics"
          :key="`${line.timeMs}-${i}`"
          class="line"
          :class="{ active: i === activeIdx, empty: !line.text }"
          :data-line="i"
          @click="seekTo(line)"
        >
          {{ line.text || "···" }}
        </div>
      </template>
      <div v-else class="no-lyrics dim">{{ t("player.noLyrics") }}</div>
    </div>

    <Teleport to="body">
      <LyricsScrapeModal
        v-if="lyricsScraping && player.currentTrack"
        :track-id="player.currentTrack.id"
        :name="player.currentTrack.title"
        @close="lyricsScraping = false"
        @applied="() => void loadLyrics()"
      />
    </Teleport>

    <div class="controls">
      <span class="time dim">{{ formatTime(player.positionSec) }}</span>
      <input
        class="slider"
        type="range"
        min="0"
        :max="player.durationSec || 0"
        step="0.1"
        :value="player.positionSec"
        @input="player.seek(Number(($event.target as HTMLInputElement).value))"
      />
      <span class="time dim">{{ formatTime(player.durationSec) }}</span>
    </div>
  </div>
</template>

<style scoped>
.now-playing {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: grid;
  grid-template-rows: 1fr auto auto;
  gap: 20px;
  padding: 32px 48px 24px;
  background: var(--bg);
}

.collapse {
  position: absolute;
  top: 16px;
  right: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--bg-elev);
  color: var(--text-dim);
  cursor: pointer;
}

.collapse svg {
  display: block;
}

.collapse:hover {
  color: var(--text);
}

.lyrics-scrape {
  top: 16px;
  right: 60px;
}

.stage {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 40px;
  min-height: 0;
}

.cover-wrap {
  width: min(320px, 34vh);
  height: min(320px, 34vh);
  border-radius: 16px;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  overflow: hidden;
  flex-shrink: 0;
}

.cover-wrap img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.meta {
  min-width: 0;
}

.title {
  font-size: 26px;
  font-weight: 700;
  margin-bottom: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.artist {
  font-size: 16px;
}

.lyrics {
  overflow: auto;
  text-align: center;
  padding: 40px 0;
  scroll-behavior: smooth;
}

.line {
  font-size: 16px;
  line-height: 2.1;
  color: var(--text-dim);
  cursor: pointer;
  transition:
    color 0.15s,
    transform 0.15s;
}

.line:hover {
  color: var(--text);
}

.line.active {
  color: var(--accent);
  font-weight: 600;
  transform: scale(1.05);
}

.line.empty {
  cursor: default;
  opacity: 0.4;
}

.no-lyrics {
  text-align: center;
  padding: 60px 0;
}

.controls {
  display: flex;
  align-items: center;
  gap: 14px;
  max-width: 640px;
  width: 100%;
  margin: 0 auto;
}

.time {
  font-size: 12px;
  min-width: 40px;
  text-align: center;
}

.slider {
  flex: 1;
  accent-color: var(--accent);
}
</style>
