import { computed, ref, watch, watchEffect } from "vue";
import { defineStore } from "pinia";
import { convertFileSrc } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { useDebounceFn, useThrottleFn } from "@vueuse/core";
import { libraryApi, type Track } from "../services/library";
import { overlayApi } from "../services/overlay";
import { settingsApi } from "../services/settings";

export type PlayMode = "sequential" | "repeat-all" | "repeat-one" | "shuffle";

const MODES: PlayMode[] = ["sequential", "repeat-all", "repeat-one", "shuffle"];

/** <audio> 引擎单例：播放状态唯一事实源在 store（技术设计文档 §2.3） */
const audio = new Audio();

/**
 * 播放状态 store（技术设计文档 §3 播放引擎）。
 * M2 起变更时 emit "playback:changed" 供迷你悬浮窗/托盘镜像。
 */
export const usePlayerStore = defineStore("player", () => {
  const currentTrack = ref<Track | null>(null);
  const queue = ref<Track[]>([]);
  /** 手动队列：「下一首播放/加入队列」插入的曲目，优先于上下文队列消耗 */
  const userQueue = ref<Track[]>([]);
  const isPlaying = ref(false);
  const volume = ref(0.8);
  const muted = ref(false);
  const mode = ref<PlayMode>("sequential");
  const positionSec = ref(0);
  const durationSec = ref(0);
  const pendingSeek = ref<number | null>(null);
  const consecutiveErrors = ref(0);

  function saveSettings() {
    void settingsApi.set(
      "player",
      JSON.stringify({ volume: volume.value, muted: muted.value, mode: mode.value }),
    );
  }
  const saveSettingsDebounced = useDebounceFn(saveSettings, 500);

  function savePosition() {
    if (!currentTrack.value) return;
    void settingsApi.set(
      "lastTrack",
      JSON.stringify({
        trackId: currentTrack.value.id,
        positionSec: Math.floor(positionSec.value),
      }),
    );
  }

  async function restore() {
    try {
      const raw = await settingsApi.get("player");
      if (raw) {
        const saved = JSON.parse(raw) as { volume?: number; muted?: boolean; mode?: PlayMode };
        volume.value = typeof saved.volume === "number" ? saved.volume : 0.8;
        muted.value = saved.muted === true;
        mode.value = MODES.includes(saved.mode as PlayMode)
          ? (saved.mode as PlayMode)
          : "sequential";
      }
      const last = await settingsApi.get("lastTrack");
      if (last) {
        const saved = JSON.parse(last) as { trackId?: number; positionSec?: number };
        if (typeof saved.trackId === "number") {
          const track = await libraryApi.trackGet(saved.trackId);
          if (track) {
            currentTrack.value = track;
            pendingSeek.value = saved.positionSec ?? 0;
            audio.src = convertFileSrc(track.path);
            audio.load();
          }
        }
      }
    } catch (err) {
      console.error("恢复播放状态失败", err);
    }
  }

  function bindAudio() {
    audio.addEventListener("loadedmetadata", () => {
      durationSec.value = audio.duration || 0;
      if (pendingSeek.value != null) {
        audio.currentTime = pendingSeek.value;
        pendingSeek.value = null;
      }
    });
    audio.addEventListener("timeupdate", () => {
      positionSec.value = audio.currentTime;
    });
    audio.addEventListener("playing", () => {
      isPlaying.value = true;
      consecutiveErrors.value = 0;
    });
    audio.addEventListener("pause", () => {
      isPlaying.value = false;
      savePosition();
    });
    audio.addEventListener("ended", () => next(true));
    audio.addEventListener("error", () => {
      isPlaying.value = false;
      if (currentTrack.value && consecutiveErrors.value < queue.value.length) {
        consecutiveErrors.value += 1;
        next(false);
      }
    });
  }

  function playAt(index: number) {
    const track = queue.value[index];
    if (!track) return;
    playTrack(track);
  }

  /** 播放指定曲目（可能来自上下文队列，也可能来自手动队列） */
  function playTrack(track: Track) {
    consecutiveErrors.value = 0;
    pendingSeek.value = 0;
    currentTrack.value = track;
    positionSec.value = 0;
    durationSec.value = 0;
    audio.src = convertFileSrc(track.path);
    void audio.play().catch(() => {});
    savePosition();
  }

  /** 以给定列表与起点开始播放（双击列表行时调用） */
  function play(tracks: Track[], index: number) {
    queue.value = tracks;
    userQueue.value = [];
    playAt(index);
  }

  /** 下一首播放：插入手动队列队首（当前曲目播完立即播放） */
  function playNext(track: Track) {
    userQueue.value.unshift(track);
  }

  /** 加入队列：追加到手动队列尾部 */
  function addToQueue(track: Track) {
    userQueue.value.push(track);
  }

  function removeFromQueue(index: number) {
    userQueue.value.splice(index, 1);
  }

  function clearUserQueue() {
    userQueue.value = [];
  }

  /** 拖拽重排手动队列 */
  function reorderUserQueue(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= userQueue.value.length) return;
    const [moved] = userQueue.value.splice(from, 1);
    if (moved) userQueue.value.splice(to, 0, moved);
  }

  function toggle() {
    if (!currentTrack.value) return;
    if (audio.paused) {
      if (!audio.src) audio.src = convertFileSrc(currentTrack.value.path);
      void audio.play().catch(() => {});
    } else {
      audio.pause();
    }
  }

  function next(auto = false) {
    const q = queue.value;
    if (!currentTrack.value) return;

    if (auto && mode.value === "repeat-one") {
      audio.currentTime = 0;
      void audio.play().catch(() => {});
      return;
    }

    // 手动队列优先：「下一首播放」的曲目最先出队
    if (userQueue.value.length > 0) {
      const [track] = userQueue.value.splice(0, 1);
      if (track) playTrack(track);
      return;
    }

    if (!q.length) return;
    if (mode.value === "shuffle" && q.length > 1) {
      let idx = Math.floor(Math.random() * q.length);
      while (q[idx].id === currentTrack.value.id) {
        idx = Math.floor(Math.random() * q.length);
      }
      playAt(idx);
      return;
    }

    const at = q.findIndex((t) => t.id === currentTrack.value?.id) + 1;
    if (at >= q.length) {
      if (mode.value === "repeat-all") {
        playAt(0);
      } else if (!auto) {
        playAt(q.length - 1);
      }
      return; // 顺序播放自然结束：停在最后一首
    }
    playAt(at);
  }

  function prev() {
    const q = queue.value;
    if (!q.length || !currentTrack.value) return;
    if (audio.currentTime > 3) {
      audio.currentTime = 0;
      return;
    }
    if (mode.value === "shuffle" && q.length > 1) {
      let idx = Math.floor(Math.random() * q.length);
      while (q[idx].id === currentTrack.value.id) {
        idx = Math.floor(Math.random() * q.length);
      }
      playAt(idx);
      return;
    }
    const idx = q.findIndex((t) => t.id === currentTrack.value?.id);
    playAt(idx <= 0 ? 0 : idx - 1);
  }

  function seek(sec: number) {
    if (!currentTrack.value) return;
    audio.currentTime = sec;
    positionSec.value = sec;
    pushSmtc(); // SMTC 时间轴即时校正
  }

  function setVolume(v: number) {
    volume.value = Math.min(1, Math.max(0, v));
    if (volume.value > 0) muted.value = false;
    saveSettingsDebounced();
  }

  function toggleMute() {
    muted.value = !muted.value;
    saveSettingsDebounced();
  }

  function cycleMode() {
    mode.value = MODES[(MODES.indexOf(mode.value) + 1) % MODES.length];
    saveSettingsDebounced();
  }

  /** 正在播放全屏页（AppShell 内切换） */
  const nowPlayingOpen = ref(false);
  function toggleNowPlaying() {
    nowPlayingOpen.value = !nowPlayingOpen.value;
  }

  /** SMTC 时间轴/元数据推送（M2 媒体键与系统媒体浮层） */
  function pushSmtc() {
    const t = currentTrack.value;
    void overlayApi
      .smtcUpdate({
        title: t?.title ?? "TinyMusic",
        artist: t?.artist ?? null,
        isPlaying: isPlaying.value,
        positionSec: positionSec.value,
        durationSec: durationSec.value,
        coverFile: t?.coverFile ?? null,
      })
      .catch(() => {});
  }
  const pushSmtcDebounced = useDebounceFn(pushSmtc, 300);

  /** 广播 playback:changed（迷你悬浮窗镜像，§2.3）：含播放态与进度供迷你窗渲染 */
  function broadcast() {
    const t = currentTrack.value;
    void emit(
      "playback:changed",
      t
        ? {
            id: t.id,
            title: t.title,
            artist: t.artist,
            coverFile: t.coverFile,
            isPlaying: isPlaying.value,
            mode: mode.value,
            positionSec: positionSec.value,
            durationSec: durationSec.value,
          }
        : null,
    ).catch(() => {});
  }

  /** 应用启动时调用一次：恢复设置与上次播放位置，并绑定引擎 */
  async function init() {
    bindAudio();
    await restore();
    watchEffect(() => {
      audio.volume = muted.value ? 0 : volume.value;
      audio.muted = muted.value;
    });
    watch(volume, saveSettingsDebounced);
    watch(muted, saveSettingsDebounced);
    // 播放历史入库：仅在曲目变化时记录一条（playback:changed 镜像之外的本地数据流）
    watch(
      () => currentTrack.value?.id,
      (id) => {
        if (id != null) void libraryApi.historyAdd(id).catch(() => {});
      },
    );
    // SMTC 推送：曲目/播放状态变化（去抖合并）+ 曲目时长就绪
    watch([currentTrack, isPlaying], pushSmtcDebounced);
    watch(durationSec, pushSmtcDebounced);
    // playback:changed 广播：曲目、播放状态、模式（即时）+ 播放位置（节流，迷你窗进度条）
    watch([currentTrack, isPlaying, mode], broadcast);
    const broadcastPositionThrottled = useThrottleFn(broadcast, 500, true);
    watch(positionSec, () => void broadcastPositionThrottled());
    // 事件回流：迷你悬浮窗控制指令 / 媒体键 / 托盘播放控制
    const applyAction = (action: string) => {
      switch (action) {
        case "toggle":
          toggle();
          break;
        case "next":
          next();
          break;
        case "prev":
          prev();
          break;
        default:
          break;
      }
    };
    void listen<string>("overlay:command", (e) => applyAction(e.payload));
    void listen<string>("smtc:command", (e) => applyAction(e.payload));
    void listen<string>("tray:action", (e) => applyAction(e.payload));
    // 迷你窗打开时主动补发当前快照（暂停中无位置广播，需按需拉取）
    void listen("playback:sync-request", () => broadcast());
    window.setInterval(() => {
      if (isPlaying.value) savePosition();
    }, 5000);
  }

  const modeLabel = computed(() => {
    switch (mode.value) {
      case "repeat-all":
        return "列表循环";
      case "repeat-one":
        return "单曲循环";
      case "shuffle":
        return "随机播放";
      default:
        return "顺序播放";
    }
  });

  return {
    currentTrack,
    queue,
    userQueue,
    isPlaying,
    volume,
    muted,
    mode,
    modeLabel,
    positionSec,
    durationSec,
    nowPlayingOpen,
    play,
    playNext,
    addToQueue,
    removeFromQueue,
    clearUserQueue,
    reorderUserQueue,
    toggle,
    toggleNowPlaying,
    next,
    prev,
    seek,
    setVolume,
    toggleMute,
    cycleMode,
    /** 直接读引擎时间（rAF 歌词同步用，绕过 timeupdate 的 ~250ms 粒度） */
    getTime: () => audio.currentTime,
    init,
  };
});
