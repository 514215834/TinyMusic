<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Heart, ListPlus, ListStart, Music, Pencil, Play, X } from "lucide-vue-next";
import { libraryApi, type Track } from "../services/library";
import { useFavoritesStore } from "../stores/favorites";
import { usePlayerStore } from "../stores/player";
import { useCovers } from "../composables/useCovers";
import { formatTime } from "../utils";
import { t } from "../i18n";
import TagEditorModal from "./TagEditorModal.vue";
import ScrapeModal from "./ScrapeModal.vue";

const props = withDefaults(
  defineProps<{
    tracks: Track[];
    /** 曲目数超长时（曲库视图）向上滚动加载数据 */
    loadMore?: () => void;
    /** 歌单模式：显示移除按钮而非收藏/队列操作 */
    playlistMode?: boolean;
    /** 开启行拖拽（歌单排序），落点经 reorder 事件上报 */
    draggable?: boolean;
    emptyText?: string;
  }>(),
  { loadMore: undefined, playlistMode: false, draggable: false, emptyText: "暂无曲目" },
);
const emit = defineEmits<{ remove: [track: Track]; reorder: [from: number, to: number] }>();

const player = usePlayerStore();
const favorites = useFavoritesStore();
void favorites.load();

/* ---- 行拖拽重排（歌单排序）---- */
const dragIndex = ref<number | null>(null);
const overIndex = ref<number | null>(null);
function onDragStart(index: number) {
  dragIndex.value = index;
}
function onDragOver(index: number) {
  overIndex.value = index;
}
function onDrop() {
  if (dragIndex.value != null && overIndex.value != null && dragIndex.value !== overIndex.value) {
    emit("reorder", dragIndex.value, overIndex.value);
  }
  dragIndex.value = null;
  overIndex.value = null;
}

/* ---- 虚拟列表（固定行高）：只渲染可视窗口 ± overscan 的行 ---- */
const ROW_H = 44;
const OVERSCAN = 10;

const scroller = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportH = ref(0);
let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  if (scroller.value) {
    viewportH.value = scroller.value.clientHeight;
    resizeObserver = new ResizeObserver(() => {
      if (scroller.value) viewportH.value = scroller.value.clientHeight;
    });
    resizeObserver.observe(scroller.value);
  }
});
onBeforeUnmount(() => resizeObserver?.disconnect());

const start = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_H) - OVERSCAN));
const end = computed(() =>
  Math.min(props.tracks.length, Math.ceil((scrollTop.value + viewportH.value) / ROW_H) + OVERSCAN),
);
const rows = computed(() =>
  props.tracks.slice(start.value, end.value).map((track, i) => ({
    track,
    index: start.value + i,
  })),
);
const totalH = computed(() => props.tracks.length * ROW_H);

/* ---- 封面缩略图：只解析可视窗口内的行 ---- */
const covers = useCovers(() => rows.value.map((r) => r.track));

function onScroll(e: Event) {
  const el = e.target as HTMLElement;
  scrollTop.value = el.scrollTop;
  // 接近已加载数据尾部时向上层要更多数据（分页加载）
  if (props.loadMore && end.value >= props.tracks.length - 20) props.loadMore();
}

function playRow(index: number) {
  player.play(props.tracks, index);
}

async function toggleFav(track: Track) {
  await favorites.toggle(track.id);
}

function isCurrent(track: Track) {
  return player.currentTrack?.id === track.id;
}

/* ---- 标签编辑与单曲刮削（M4）：就地更新行对象，所有视图同步刷新 ---- */
const editingTrack = ref<Track | null>(null);
const scrapingTrack = ref<Track | null>(null);

async function onScrapeApplied(candidate: { artworkUrl?: string | null }) {
  const track = scrapingTrack.value;
  if (!track || !candidate.artworkUrl) return;
  // apply_track 返回落库的封面缓存文件名，回填行对象（useCovers 按 coverFile 变化重解析）
  const updated = await libraryApi.trackGet(track.id);
  if (updated) track.coverFile = updated.coverFile;
}

/** 标签保存：就地更新行对象（列表与弹窗共享同一响应式引用，所有视图同步刷新） */
function onTagSaved(updated: Track) {
  Object.assign(editingTrack.value ?? updated, updated);
}
</script>

<template>
  <div ref="scroller" class="track-table" @scroll="onScroll">
    <div class="row head">
      <span class="col idx">#</span>
      <span class="col title">{{ t("table.title") }}</span>
      <span class="col artist">{{ t("table.artist") }}</span>
      <span class="col album">{{ t("table.album") }}</span>
      <span class="col dur">{{ t("table.duration") }}</span>
    </div>
    <div class="body" :style="{ height: `${totalH}px` }">
      <div
        v-for="{ track, index } in rows"
        :key="track.id"
        class="row"
        :class="{
          playing: isCurrent(track),
          dragging: dragIndex === index,
          over: overIndex === index && dragIndex !== index,
        }"
        :style="{ top: `${index * ROW_H}px`, height: `${ROW_H}px` }"
        :draggable="draggable || undefined"
        @dblclick="playRow(index)"
        @dragstart="onDragStart(index)"
        @dragover.prevent="onDragOver(index)"
        @dragleave="overIndex = null"
        @dragend="onDrop"
        @drop.prevent="onDrop"
      >
        <span class="col idx">
          <span v-if="isCurrent(track)" class="eq" :class="{ paused: !player.isPlaying }">
            <i></i><i></i><i></i>
          </span>
          <span v-else class="idx-num">{{ index + 1 }}</span>
          <button class="idx-play" :title="t('player.play')" @click="playRow(index)">
            <Play :size="12" />
          </button>
        </span>
        <span class="col title" :class="{ active: isCurrent(track) }">
          <span class="cover">
            <img v-if="covers[track.id]" :src="covers[track.id] ?? undefined" alt="" />
            <Music v-else :size="13" />
          </span>
          <span class="t-name">{{ track.title }}</span>
        </span>
        <span class="col artist dim">{{ track.artist ?? "—" }}</span>
        <span class="col album dim">{{ track.album ?? "—" }}</span>
        <span class="col dur dim">{{
          track.durationSec ? formatTime(track.durationSec) : "—"
        }}</span>
        <span class="row-actions">
          <template v-if="!playlistMode">
            <button
              class="act"
              :class="{ on: favorites.has(track.id) }"
              :title="favorites.has(track.id) ? t('track.unfavorite') : t('track.favorite')"
              @click="toggleFav(track)"
            >
              <Heart :size="14" :fill="favorites.has(track.id) ? 'currentColor' : 'none'" />
            </button>
            <button class="act" :title="t('track.playNext')" @click="player.playNext(track)">
              <ListStart :size="14" />
            </button>
            <button class="act" :title="t('track.addToQueue')" @click="player.addToQueue(track)">
              <ListPlus :size="14" />
            </button>
          </template>
          <button v-else class="act" :title="t('playlist.remove')" @click="emit('remove', track)">
            <X :size="14" />
          </button>
          <button class="act" :title="t('tags.rowEdit')" @click="editingTrack = track">
            <Pencil :size="14" />
          </button>
        </span>
      </div>
    </div>
    <div v-if="!tracks.length" class="empty">
      <Music :size="24" />
      <span class="dim">{{ emptyText }}</span>
    </div>

    <Teleport to="body">
      <TagEditorModal
        v-if="editingTrack"
        :track="editingTrack"
        @close="editingTrack = null"
        @saved="onTagSaved"
        @scrape="scrapingTrack = editingTrack"
      />
      <ScrapeModal
        v-if="scrapingTrack"
        mode="track"
        :target-id="scrapingTrack.id"
        :name="scrapingTrack.title"
        @close="scrapingTrack = null"
        @applied="onScrapeApplied"
      />
    </Teleport>
  </div>
</template>

<style scoped>
.track-table {
  flex: 1;
  min-height: 0;
  overflow: auto;
  /* 滚动条占位恒定：数据加载/视图切换不因滚动条出现而横向抖动 */
  scrollbar-gutter: stable;
  border: 1px solid var(--border);
  border-radius: 12px;
  position: relative;
  /* 底部让位固定播放栏：最后几行可完整滚动到可见区 */
  padding-bottom: 76px;
}

.body {
  position: relative;
}

.row {
  display: grid;
  grid-template-columns:
    40px minmax(0, 1fr) minmax(90px, 160px) minmax(120px, 200px)
    48px 108px;
  gap: 8px;
  padding: 0 12px;
  align-items: center;
}

/* 表头：全宽 + 大写字距标签；与行（内缩 4px + 12px 内边距）左缘对齐 */
.row.head {
  position: sticky;
  top: 0;
  height: 40px;
  padding: 0 16px;
  background: var(--bg-elev);
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  border-bottom: 1px solid var(--border);
  z-index: 2;
}

.body .row {
  position: absolute;
  left: 4px;
  right: 4px;
  border-radius: 8px;
  cursor: default;
  transition: background-color 0.12s ease;
}

.body .row:hover {
  background: var(--bg-hover);
}

.row.playing {
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.row.playing:hover {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}

.row.dragging {
  opacity: 0.4;
}

.row.over {
  box-shadow: inset 0 2px 0 var(--accent);
}

.col {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col.idx {
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.idx-num {
  font-variant-numeric: tabular-nums;
}

/* 悬停时序号让位给播放键 */
.idx-play {
  display: none;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text);
  cursor: pointer;
}

.idx-play svg {
  display: block;
}

.row:hover .idx-num,
.row:hover .eq {
  display: none;
}

.row:hover .idx-play {
  display: inline-flex;
}

/* 正在播放：三段均衡条动画（暂停时静止） */
.eq {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  width: 14px;
  height: 12px;
}

.eq i {
  width: 3px;
  height: 12px;
  border-radius: 1px;
  background: var(--accent);
  transform-origin: bottom;
  animation: eq-bounce 1.1s ease-in-out infinite;
}

.eq i:nth-child(1) {
  animation-delay: -0.55s;
}

.eq i:nth-child(2) {
  animation-delay: -0.28s;
}

.eq.paused i {
  animation-play-state: paused;
}

@keyframes eq-bounce {
  0%,
  100% {
    transform: scaleY(0.3);
  }
  50% {
    transform: scaleY(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .eq i {
    animation: none;
  }
}

.col.dur {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

/* 标题列：封面 + 曲名 */
.col.title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.t-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cover {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  overflow: hidden;
}

.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.active {
  color: var(--accent);
  font-weight: 600;
}

/* 行内操作：悬停浮现，不挤压列布局 */
.row-actions {
  display: inline-flex;
  gap: 2px;
  opacity: 0;
  transform: translateX(4px);
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.row:hover .row-actions {
  opacity: 1;
  transform: none;
}

.act {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition:
    background-color 0.12s ease,
    color 0.12s ease,
    transform 0.12s ease;
}

.act svg {
  display: block;
}

.act:hover {
  background: var(--border);
  color: var(--text);
}

.act:active {
  transform: scale(0.88);
}

.act.on {
  color: var(--heart);
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 48px 16px;
}

.empty svg {
  color: var(--text-dim);
  opacity: 0.6;
}
</style>
