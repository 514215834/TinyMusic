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
// asset 协议响应恒带 Access-Control-Allow-Origin（tauri asset.rs），anonymous 模式
// 让媒体走 CORS 清洁通道，MediaElementSource（EQ）不会被跨域污染输出静音
audio.crossOrigin = "anonymous";

/** EQ 十频段（Hz）：首尾 shelf，中间 peaking（技术设计文档 §3 M3） */
export const EQ_FREQS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const EQ_MIN = -12;
export const EQ_MAX = 12;
/** 倍速档位（M3） */
export const SPEEDS = [0.75, 1, 1.25, 1.5, 2] as const;

/* 引擎级设置状态：与 audio 单例同属模块级，store 负责暴露与持久化 */
const speed = ref(1);
const eqEnabled = ref(false);
const eqGains = ref<number[]>(Array<number>(EQ_FREQS.length).fill(0));
const outputDeviceId = ref("");

let audioCtx: AudioContext | null = null;
let eqFilters: BiquadFilterNode[] = [];

/** 懒建 EQ 链：首次启用 EQ 才把 <audio> 路由进 Web Audio（此后常驻，旁路=增益归零） */
function ensureEqGraph() {
  if (audioCtx) return;
  const ctx = new AudioContext();
  let node: AudioNode = ctx.createMediaElementSource(audio);
  eqFilters = EQ_FREQS.map((freq, i) => {
    const filter = ctx.createBiquadFilter();
    filter.type = i === 0 ? "lowshelf" : i === EQ_FREQS.length - 1 ? "highshelf" : "peaking";
    filter.frequency.value = freq;
    filter.Q.value = 1.1;
    node.connect(filter);
    node = filter;
    return filter;
  });
  node.connect(ctx.destination);
  audioCtx = ctx;
}

/** 应用当前 EQ 状态；autoplay 策略下 AudioContext 可能在手势后才可 resume */
function applyEq() {
  if (!audioCtx) {
    if (!eqEnabled.value) return;
    ensureEqGraph();
  }
  if (!audioCtx) return;
  if (audioCtx.state === "suspended") void audioCtx.resume().catch(() => {});
  eqFilters.forEach((filter, i) => {
    filter.gain.value = eqEnabled.value ? (eqGains.value[i] ?? 0) : 0;
  });
}

function resumeEqCtx() {
  if (audioCtx && audioCtx.state === "suspended") void audioCtx.resume().catch(() => {});
}

type SinkIdCapable = HTMLMediaElement & { setSinkId?: (id: string) => Promise<void> };

/** 切换输出设备：setSinkId 即时生效且不中断播放（WebView2 支持） */
async function applySink(deviceId: string): Promise<boolean> {
  const el = audio as SinkIdCapable;
  if (typeof el.setSinkId !== "function") return false;
  try {
    if (deviceId) await el.setSinkId(deviceId);
    return true;
  } catch {
    return false;
  }
}

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
      JSON.stringify({
        volume: volume.value,
        muted: muted.value,
        mode: mode.value,
        speed: speed.value,
      }),
    );
  }
  const saveSettingsDebounced = useDebounceFn(saveSettings, 500);

  /** 引擎级音频设置（M3）：输出设备 + EQ 状态，独立于播放进度设置 */
  function saveAudioSettings() {
    void settingsApi
      .set(
        "audio",
        JSON.stringify({
          outputDeviceId: outputDeviceId.value,
          eq: { enabled: eqEnabled.value, gains: eqGains.value },
        }),
      )
      .catch(() => {});
  }
  const saveAudioSettingsDebounced = useDebounceFn(saveAudioSettings, 300);

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
        const saved = JSON.parse(raw) as {
          volume?: number;
          muted?: boolean;
          mode?: PlayMode;
          speed?: number;
        };
        volume.value = typeof saved.volume === "number" ? saved.volume : 0.8;
        muted.value = saved.muted === true;
        mode.value = MODES.includes(saved.mode as PlayMode)
          ? (saved.mode as PlayMode)
          : "sequential";
        if (typeof saved.speed === "number" && saved.speed > 0) speed.value = saved.speed;
      }
      /* M3 引擎级设置：输出设备切换 + EQ 恢复（EQ 需在 src 加载后重建路由） */
      const rawAudio = await settingsApi.get("audio");
      if (rawAudio) {
        const savedAudio = JSON.parse(rawAudio) as {
          outputDeviceId?: string;
          eq?: { enabled?: boolean; gains?: number[] };
        };
        if (typeof savedAudio.outputDeviceId === "string") {
          outputDeviceId.value = savedAudio.outputDeviceId;
          await applySink(savedAudio.outputDeviceId);
        }
        if (savedAudio.eq) {
          eqEnabled.value = savedAudio.eq.enabled === true;
          if (
            Array.isArray(savedAudio.eq.gains) &&
            savedAudio.eq.gains.length === EQ_FREQS.length
          ) {
            eqGains.value = savedAudio.eq.gains.map((g) =>
              Math.min(EQ_MAX, Math.max(EQ_MIN, Number(g) || 0)),
            );
          }
        }
      }
      if (eqEnabled.value) applyEq();
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
    resumeEqCtx(); // autoplay 策略：用户手势路径上恢复被挂起的 AudioContext
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

  /**
   * 播放全部（PlayAllButton）：首曲立即播放，其余整表自动加入队列——
   * 手动队列即待播序列（面板可见、可移除/拖拽重排/清空），next() 天然按此消耗。
   * 随机模式下对入队部分预洗牌，保证消耗顺序即随机顺序。
   */
  function playAll(tracks: Track[]) {
    if (!tracks.length) return;
    const [first, ...rest] = tracks;
    queue.value = tracks;
    userQueue.value = mode.value === "shuffle" ? shuffleSlice(rest) : rest;
    if (first) playTrack(first);
  }

  function shuffleSlice(arr: Track[]): Track[] {
    const out = [...arr];
    for (let i = out.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [out[i], out[j]] = [out[j]!, out[i]!];
    }
    return out;
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
    // 清空 = 播完当前即停：截断上下文队列中当前曲目之后的部分，
    // 避免 next() 回落到索引续播把已"清空"的曲目重新播出来（prev 回退不受影响）
    const idx = queue.value.findIndex((t) => t.id === currentTrack.value?.id);
    if (idx >= 0) queue.value = queue.value.slice(0, idx + 1);
  }

  /** 拖拽重排手动队列 */
  function reorderUserQueue(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= userQueue.value.length) return;
    const [moved] = userQueue.value.splice(from, 1);
    if (moved) userQueue.value.splice(to, 0, moved);
  }

  function toggle() {
    if (!currentTrack.value) return;
    resumeEqCtx();
    if (audio.paused) {
      if (!audio.src) audio.src = convertFileSrc(currentTrack.value.path);
      void audio.play().catch(() => {});
    } else {
      audio.pause();
    }
  }

  /* ---- M3：倍速 / EQ / 输出设备 ---- */

  function setSpeed(v: number) {
    if (!Number.isFinite(v) || v <= 0) return;
    speed.value = v;
    saveSettings();
  }

  /** 播放条倍速按钮：在 SPEEDS 档位间循环 */
  function cycleSpeed() {
    const idx = SPEEDS.indexOf(speed.value as (typeof SPEEDS)[number]);
    setSpeed(SPEEDS[(idx + 1) % SPEEDS.length] ?? 1);
  }

  function setEqEnabled(enabled: boolean) {
    eqEnabled.value = enabled;
    applyEq();
    saveAudioSettings();
  }

  function setEqGain(index: number, gain: number) {
    if (index < 0 || index >= EQ_FREQS.length) return;
    eqGains.value = eqGains.value.map((g, i) =>
      i === index ? Math.min(EQ_MAX, Math.max(EQ_MIN, gain)) : g,
    );
    applyEq();
    saveAudioSettingsDebounced();
  }

  function resetEq() {
    eqGains.value = eqGains.value.map(() => 0);
    applyEq();
    saveAudioSettings();
  }

  /** 切换输出设备；返回 false 表示引擎不支持或拒绝（设备失效等） */
  async function setOutputDevice(deviceId: string): Promise<boolean> {
    const ok = await applySink(deviceId);
    outputDeviceId.value = deviceId;
    saveAudioSettings();
    return ok;
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
    // 倍速（M3）：defaultPlaybackRate 保证换曲后保持
    watchEffect(() => {
      audio.playbackRate = speed.value;
      audio.defaultPlaybackRate = speed.value;
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
    speed,
    eqEnabled,
    eqGains,
    outputDeviceId,
    play,
    playAll,
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
    setSpeed,
    cycleSpeed,
    setEqEnabled,
    setEqGain,
    resetEq,
    setOutputDevice,
    /** 直接读引擎时间（rAF 歌词同步用，绕过 timeupdate 的 ~250ms 粒度） */
    getTime: () => audio.currentTime,
    init,
  };
});
