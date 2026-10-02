import { computed, ref, watch, watchEffect } from "vue";
import { defineStore } from "pinia";
import { convertFileSrc } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { useDebounceFn, useThrottleFn } from "@vueuse/core";
import { libraryApi, type Track } from "../services/library";
import { overlayApi } from "../services/overlay";
import { settingsApi } from "../services/settings";
import { crossfadeGains } from "../utils/crossfade";

export type PlayMode = "sequential" | "repeat-all" | "repeat-one" | "shuffle";

const MODES: PlayMode[] = ["sequential", "repeat-all", "repeat-one", "shuffle"];

/**
 * 双 <audio> 引擎（M8，技术设计文档 §3）：A/B 两元素轮流承担播放。
 * 待命元素提前预载下一曲（gapless），曲终换切只做角色交换；
 * crossfade 时两元素同时发声，经各自 GainNode 等功率交叉渐变（Web Audio）。
 * 引擎与其状态全部在模块层（单例），store 仅负责暴露、持久化与事件接线。
 */
function createEngineEl(): HTMLAudioElement {
  const el = new Audio();
  el.preload = "auto";
  // asset 协议响应恒带 Access-Control-Allow-Origin（tauri asset.rs），anonymous 模式
  // 让媒体走 CORS 清洁通道，MediaElementSource（EQ）不会被跨域污染输出静音
  el.crossOrigin = "anonymous";
  return el;
}
const elA = createEngineEl();
const elB = createEngineEl();
/** 当前承担播放的元素；换切 = 指针交换，事件绑定按元素身份过滤 */
let active: HTMLAudioElement = elA;

function otherOf(el: HTMLAudioElement): HTMLAudioElement {
  return el === elA ? elB : elA;
}

/** EQ 十频段（Hz）：首尾 shelf，中间 peaking（技术设计文档 §3 M3） */
export const EQ_FREQS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const EQ_MIN = -12;
export const EQ_MAX = 12;
/** 倍速档位（M3） */
export const SPEEDS = [0.75, 1, 1.25, 1.5, 2] as const;
/** crossfade 上限（秒） */
export const CROSSFADE_MAX = 6;

/* ---- 引擎状态（模块级单例） ---- */
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
const speed = ref(1);
const eqEnabled = ref(false);
const eqGains = ref<number[]>(Array<number>(EQ_FREQS.length).fill(0));
const outputDeviceId = ref("");
/** gapless 无缝播放（M8）：待命元素预载 + 曲终换切 */
const gapless = ref(true);
/** crossfade 时长（秒），0 = 关闭（默认） */
const crossfadeSec = ref(0);

let audioCtx: AudioContext | null = null;
let eqFilters: BiquadFilterNode[] = [];
/** 两元素音频汇聚点（EQ 链入口） */
let eqInput: GainNode | null = null;
/** 每元素独立增益：crossfade 渐变作用于此，主音量仍在元素 volume 上 */
const elGains = new Map<HTMLAudioElement, GainNode>();

/**
 * 建 Web Audio 图：A/B 两路 MediaElementSource → 各自 GainNode → EQ 链 → 输出。
 * 惰性创建（首次启用 EQ 或 crossfade）；创建后元素声音只走图内，必须保证 ctx resume。
 */
function ensureGraph(): boolean {
  if (audioCtx) return true;
  try {
    const ctx = new AudioContext();
    eqInput = ctx.createGain();
    let node: AudioNode = eqInput;
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
    for (const el of [elA, elB]) {
      const src = ctx.createMediaElementSource(el);
      const gain = ctx.createGain();
      src.connect(gain);
      gain.connect(eqInput);
      elGains.set(el, gain);
    }
    audioCtx = ctx;
    applyEq();
    return true;
  } catch (err) {
    console.error("创建 Web Audio 图失败，crossfade 不可用", err);
    return false;
  }
}

/** 应用当前 EQ 状态；autoplay 策略下 AudioContext 可能在手势后才可 resume */
function applyEq() {
  if (!audioCtx) {
    if (!eqEnabled.value) return;
    ensureGraph();
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

/** 设置元素增益（graph 就绪时经 GainNode；未建图时无需渐变能力） */
function setElGain(el: HTMLAudioElement, v: number) {
  const gain = elGains.get(el);
  if (gain && audioCtx) gain.gain.value = v;
}

type SinkIdCapable = HTMLMediaElement & { setSinkId?: (id: string) => Promise<void> };

/** 切换输出设备：setSinkId 即时生效且不中断播放（WebView2 支持）；两元素同步 */
async function applySink(deviceId: string): Promise<boolean> {
  for (const raw of [elA, elB]) {
    const el = raw as SinkIdCapable;
    if (typeof el.setSinkId !== "function") return false;
    try {
      await el.setSinkId(deviceId);
    } catch {
      return false;
    }
  }
  return true;
}

/* ---- M8 引擎运行态：下一曲决策 / 待命预载 / 渐变 ---- */

/** 下一曲决策结果：source/index 用于换切时精确消耗与校验（防队列变更后错播） */
interface NextInfo {
  track: Track;
  trackId: number;
  source: "userQueue" | "queue";
  index: number;
}

interface StandbyState {
  el: HTMLAudioElement;
  info: NextInfo;
}

interface FadeState {
  out: HTMLAudioElement;
  enter: HTMLAudioElement;
  info: NextInfo;
  /** 渐变起点（out 元素自身时间轴），进度随其 currentTime 推进——暂停自然冻结 */
  startPos: number;
  duration: number;
  raf: number;
}

let standby: StandbyState | null = null;
let fading: FadeState | null = null;
/** 提前预载窗口：剩余播放时间低于该值时准备待命元素 */
const PRELOAD_LEAD_SEC = 30;

function sameInfo(a: NextInfo, b: NextInfo): boolean {
  return a.trackId === b.trackId && a.source === b.source && a.index === b.index;
}

/**
 * 下一曲决策（纯查询不消耗）：手动队列优先 → 随机 → 顺序下一条/列表循环。
 * auto=false 时与旧版 next() 行为逐分支对齐（顺序模式末尾手动下一曲 = 重播最后一首）。
 */
function nextTrackInfo(auto: boolean): NextInfo | null {
  const q = queue.value;
  const cur = currentTrack.value;
  if (!cur) return null;
  if (userQueue.value.length > 0) {
    const track = userQueue.value[0];
    return { track, trackId: track.id, source: "userQueue", index: 0 };
  }
  if (!q.length) return null;
  if (mode.value === "shuffle" && q.length > 1) {
    let idx = Math.floor(Math.random() * q.length);
    while (q[idx].id === cur.id) {
      idx = Math.floor(Math.random() * q.length);
    }
    return { track: q[idx], trackId: q[idx].id, source: "queue", index: idx };
  }
  const at = q.findIndex((t) => t.id === cur.id) + 1;
  if (at < q.length) {
    return { track: q[at], trackId: q[at].id, source: "queue", index: at };
  }
  if (mode.value === "repeat-all") {
    return { track: q[0], trackId: q[0].id, source: "queue", index: 0 };
  }
  if (!auto) {
    const last = q[q.length - 1];
    return { track: last, trackId: last.id, source: "queue", index: q.length - 1 };
  }
  return null; // 顺序播放自然结束：停在最后一首
}

/** 换切/渐变落定时消耗决策：仅手动队列需要出队（头部校验防御队列变更） */
function consumeNextInfo(info: NextInfo) {
  if (info.source === "userQueue" && userQueue.value[0]?.id === info.trackId) {
    userQueue.value.shift();
  }
}

/** 待命元素预载的决策是否仍然成立（队列/模式/手动队列变化后自愈） */
function standbyStillValid(): boolean {
  if (!standby || mode.value === "repeat-one") return false;
  const s = standby.info;
  const curId = currentTrack.value?.id;
  if (s.source === "userQueue") return userQueue.value[0]?.id === s.trackId;
  if (userQueue.value.length > 0) return false; // 手动队列出现，优先级改变
  if (mode.value === "shuffle") {
    // 随机选择不可重放（重掷会得到不同结果），仅验证预定槽位仍指向同一首
    return queue.value[s.index]?.id === s.trackId;
  }
  const at = queue.value.findIndex((t) => t.id === curId) + 1;
  if (at < queue.value.length) return queue.value[at]?.id === s.trackId;
  if (mode.value === "repeat-all") return queue.value[0]?.id === s.trackId;
  return false;
}

function prepareStandby(info: NextInfo) {
  const el = otherOf(active);
  el.src = convertFileSrc(info.track.path);
  el.load();
  standby = { el, info };
}

function discardStandby() {
  if (!standby) return;
  standby.el.pause();
  standby.el.removeAttribute("src");
  standby.el.load(); // 重置元素，释放已缓冲数据
  standby = null;
}

/** timeupdate 驱动：校验待命决策 + 临近曲尾时预载 */
function scheduleStandby() {
  if (fading) return;
  if (!standbyStillValid()) discardStandby();
  if (standby || mode.value === "repeat-one") return;
  if (!gapless.value && crossfadeSec.value <= 0) return; // 双开关都关：不预载
  if (durationSec.value <= 0) return;
  if (durationSec.value - positionSec.value > PRELOAD_LEAD_SEC) return;
  const info = nextTrackInfo(true);
  if (info && info.trackId !== currentTrack.value?.id) prepareStandby(info);
}

/**
 * 换切核心：待命元素转正当刻。旧曲目进度先落库再停用清空；
 * 新元素从 0 开始（预载完成，启动延迟仅为 play() 调用开销）。
 */
function commitSwap(info: NextInfo) {
  const st = standby;
  if (!st) {
    playTrack(info.track);
    return;
  }
  savePosition(); // 换切后旧元素的 pause 事件被身份过滤，不会自动落库
  const oldEl = active;
  active = st.el;
  standby = null;
  currentTrack.value = st.info.track;
  positionSec.value = 0;
  durationSec.value = st.el.duration || 0;
  setElGain(active, 1);
  setElGain(oldEl, 1);
  oldEl.pause();
  oldEl.removeAttribute("src");
  oldEl.load();
  void active.play().catch(() => {});
}

/** 启动 crossfade：enter 元素开始发声，rAF 逐帧推 equal-power 增益 */
function startFade(info: NextInfo): boolean {
  if (fading || !ensureGraph()) return false;
  resumeEqCtx();
  let enterEl: HTMLAudioElement;
  if (standby && sameInfo(standby.info, info)) {
    enterEl = standby.el;
    standby = null; // 所有权移交 fading
  } else {
    discardStandby();
    enterEl = otherOf(active);
    enterEl.src = convertFileSrc(info.track.path);
    enterEl.load();
  }
  const duration = Math.min(crossfadeSec.value, Math.max(0, durationSec.value - positionSec.value));
  if (duration <= 0) return false;
  enterEl.currentTime = 0;
  setElGain(enterEl, 0);
  setElGain(active, 1);
  void enterEl.play().catch(() => {});
  fading = { out: active, enter: enterEl, info, startPos: active.currentTime, duration, raf: 0 };
  fading.raf = requestAnimationFrame(fadeFrame);
  return true;
}

function fadeFrame() {
  const f = fading;
  if (!f) return;
  const progress = f.duration > 0 ? (f.out.currentTime - f.startPos) / f.duration : 1;
  if (progress >= 1 || f.out.ended) {
    finalizeFade();
    return;
  }
  const gains = crossfadeGains(progress);
  setElGain(f.out, gains.out);
  setElGain(f.enter, gains.enter);
  f.raf = requestAnimationFrame(fadeFrame);
}

/** 渐变完成（out 播完/进度到 1）：enter 转正 */
function finalizeFade() {
  const f = fading;
  if (!f) return;
  cancelAnimationFrame(f.raf);
  fading = null;
  savePosition();
  const oldEl = f.out;
  active = f.enter;
  standby = null; // 渐变已接管待命元素（若曾预载）
  currentTrack.value = f.info.track;
  consumeNextInfo(f.info);
  positionSec.value = 0;
  durationSec.value = active.duration || 0;
  setElGain(active, 1);
  setElGain(oldEl, 1);
  oldEl.pause();
  oldEl.removeAttribute("src");
  oldEl.load();
}

/** 中止渐变（seek/手动切歌/关 crossfade）：out 增益复位，enter 丢弃 */
function cancelFade() {
  const f = fading;
  if (!f) return;
  cancelAnimationFrame(f.raf);
  fading = null;
  setElGain(f.out, 1);
  f.enter.pause();
  f.enter.removeAttribute("src");
  f.enter.load();
}

/** 临近曲尾且开启 crossfade 时触发双曲交叉 */
function maybeStartFade() {
  if (crossfadeSec.value <= 0 || fading || !isPlaying.value) return;
  if (durationSec.value <= 0) return;
  if (durationSec.value - positionSec.value > crossfadeSec.value) return;
  if (mode.value === "repeat-one") return;
  const info = nextTrackInfo(true);
  if (!info || info.trackId === currentTrack.value?.id) return;
  startFade(info);
}

/* ---- 持久化 ---- */

function saveSettings() {
  void settingsApi.set(
    "player",
    JSON.stringify({
      volume: volume.value,
      muted: muted.value,
      mode: mode.value,
      speed: speed.value,
      gapless: gapless.value,
      crossfadeSec: crossfadeSec.value,
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

/* ---- 引擎事件绑定：按元素身份过滤，只有 active 元素驱动状态 ---- */

function bindEngineEl(el: HTMLAudioElement) {
  el.addEventListener("loadedmetadata", () => {
    if (el !== active) return;
    durationSec.value = el.duration || 0;
    if (pendingSeek.value != null) {
      el.currentTime = pendingSeek.value;
      pendingSeek.value = null;
    }
  });
  el.addEventListener("timeupdate", () => {
    if (el !== active) return;
    positionSec.value = el.currentTime;
    scheduleStandby();
    maybeStartFade();
  });
  el.addEventListener("playing", () => {
    if (el !== active) return;
    isPlaying.value = true;
    consecutiveErrors.value = 0;
  });
  el.addEventListener("pause", () => {
    if (el !== active) return;
    isPlaying.value = false;
    savePosition();
  });
  el.addEventListener("ended", () => {
    if (el !== active) return;
    if (fading && fading.out === el) {
      finalizeFade();
      return;
    }
    next(true);
  });
  el.addEventListener("error", () => {
    // 待命/渐变元素加载失败：静默放弃，不打断当前播放
    if (standby && el === standby.el) {
      discardStandby();
      return;
    }
    if (fading && el === fading.enter) {
      cancelFade();
      return;
    }
    if (el !== active) return;
    isPlaying.value = false;
    if (currentTrack.value && consecutiveErrors.value < queue.value.length) {
      consecutiveErrors.value += 1;
      next(false);
    }
  });
}

/* ---- 播放控制（store 暴露的方法本体） ---- */

function playAt(index: number) {
  const track = queue.value[index];
  if (!track) return;
  playTrack(track);
}

/** 播放指定曲目（可能来自上下文队列，也可能来自手动队列）：丢弃待命/渐变状态全新加载 */
function playTrack(track: Track) {
  consecutiveErrors.value = 0;
  pendingSeek.value = 0;
  cancelFade();
  discardStandby();
  currentTrack.value = track;
  positionSec.value = 0;
  durationSec.value = 0;
  resumeEqCtx(); // autoplay 策略：用户手势路径上恢复被挂起的 AudioContext
  if (crossfadeSec.value > 0) ensureGraph(); // 渐变需要 GainNode 通路，趁手势路径就绪
  active.src = convertFileSrc(track.path);
  void active.play().catch(() => {});
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

/** 双击待播曲目（M7+）：立即播放该曲；其之前的待播曲目一并出队，后续曲目按原顺序接续 */
function playQueueAt(index: number) {
  if (index < 0 || index >= userQueue.value.length) return;
  const track = userQueue.value.splice(index, 1)[0];
  userQueue.value.splice(0, index);
  if (track) playTrack(track);
}

function clearUserQueue() {
  userQueue.value = [];
  // 清空 = 播完当前即停：截断上下文队列中当前曲目之后的部分，
  // 避免 next() 回落到索引续播把已"清空"的曲目重新播出来（prev 回退不受影响）
  const idx = queue.value.findIndex((t) => t.id === currentTrack.value?.id);
  if (idx >= 0) queue.value = queue.value.slice(0, idx + 1);
  // 已预载/渐变中的"下一曲"决策随之失效
  cancelFade();
  discardStandby();
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
  if (fading) {
    // 渐变中两元素同步暂停/继续：进度随 out.currentTime 冻结，rAF 增益随之停驻
    if (fading.out.paused) {
      void fading.out.play().catch(() => {});
      void fading.enter.play().catch(() => {});
    } else {
      fading.out.pause();
      fading.enter.pause();
    }
    return;
  }
  if (active.paused) {
    if (!active.src) active.src = convertFileSrc(currentTrack.value.path);
    void active.play().catch(() => {});
  } else {
    active.pause();
  }
}

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

/** gapless 开关（M8）：关闭且无 crossfade 时丢弃预载，回到"曲终重载"行为 */
function setGapless(on: boolean) {
  gapless.value = on;
  if (!on && crossfadeSec.value <= 0) discardStandby();
  saveSettings();
}

function setCrossfade(sec: number) {
  const v = Number.isFinite(sec) ? Math.min(CROSSFADE_MAX, Math.max(0, sec)) : 0;
  crossfadeSec.value = v;
  if (v <= 0) cancelFade();
  saveSettings();
}

function next(auto = false) {
  if (!currentTrack.value) return;

  if (auto && mode.value === "repeat-one") {
    active.currentTime = 0;
    void active.play().catch(() => {});
    return;
  }

  const info = nextTrackInfo(auto);
  // gapless 快路径：待命元素已预载决策曲目 → 直接换切角色，省去重新拉流
  if (info && standby && sameInfo(standby.info, info)) {
    consumeNextInfo(info);
    commitSwap(info);
    return;
  }
  discardStandby();
  if (info) {
    consumeNextInfo(info);
    playTrack(info.track);
    return;
  }
  // 顺序播放自然结束：停在最后一首（isPlaying 由 ended 事件收敛）
}

function prev() {
  const q = queue.value;
  if (!q.length || !currentTrack.value) return;
  if (active.currentTime > 3) {
    active.currentTime = 0;
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
  // 渐变中 seek 语义不明：中止渐变回到普通播放
  if (fading) {
    cancelFade();
    discardStandby();
  }
  active.currentTime = sec;
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

async function restore() {
  try {
    const raw = await settingsApi.get("player");
    if (raw) {
      const saved = JSON.parse(raw) as {
        volume?: number;
        muted?: boolean;
        mode?: PlayMode;
        speed?: number;
        gapless?: boolean;
        crossfadeSec?: number;
      };
      volume.value = typeof saved.volume === "number" ? saved.volume : 0.8;
      muted.value = saved.muted === true;
      mode.value = MODES.includes(saved.mode as PlayMode)
        ? (saved.mode as PlayMode)
        : "sequential";
      if (typeof saved.speed === "number" && saved.speed > 0) speed.value = saved.speed;
      if (typeof saved.gapless === "boolean") gapless.value = saved.gapless;
      if (
        typeof saved.crossfadeSec === "number" &&
        saved.crossfadeSec >= 0 &&
        saved.crossfadeSec <= CROSSFADE_MAX
      ) {
        crossfadeSec.value = saved.crossfadeSec;
      }
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
        if (Array.isArray(savedAudio.eq.gains) && savedAudio.eq.gains.length === EQ_FREQS.length) {
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
          active.src = convertFileSrc(track.path);
          active.load();
        }
      }
    }
  } catch (err) {
    console.error("恢复播放状态失败", err);
  }
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

/**
 * 播放状态 store（技术设计文档 §3 播放引擎）。
 * M2 起变更时 emit "playback:changed" 供迷你悬浮窗/托盘镜像。
 */
export const usePlayerStore = defineStore("player", () => {
  /** 应用启动时调用一次：恢复设置与上次播放位置，并绑定引擎 */
  async function init() {
    bindEngineEl(elA);
    bindEngineEl(elB);
    await restore();
    watchEffect(() => {
      const v = muted.value ? 0 : volume.value;
      for (const el of [elA, elB]) {
        el.volume = v;
        el.muted = muted.value;
      }
    });
    // 倍速（M3）：defaultPlaybackRate 保证换曲后保持；两元素同步（待命预载含倍速语义）
    watchEffect(() => {
      for (const el of [elA, elB]) {
        el.playbackRate = speed.value;
        el.defaultPlaybackRate = speed.value;
      }
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
    // SMTC 推送：曲目/播放状态变化（去抖合并）+ 曲目时长就绪；
    // currentTrack 是浅比较，就地刮削封面要靠 coverFile 源才能补推
    watch([currentTrack, isPlaying, () => currentTrack.value?.coverFile], pushSmtcDebounced);
    watch(durationSec, pushSmtcDebounced);
    // playback:changed 广播：曲目、播放状态、模式（即时）+ 封面就地更新（迷你窗即时换图）
    // + 播放位置（节流，迷你窗进度条）
    watch([currentTrack, isPlaying, mode, () => currentTrack.value?.coverFile], broadcast);
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
    gapless,
    crossfadeSec,
    play,
    playAll,
    playNext,
    addToQueue,
    removeFromQueue,
    playQueueAt,
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
    setGapless,
    setCrossfade,
    /** 直接读引擎时间（rAF 歌词同步用，绕过 timeupdate 的 ~250ms 粒度） */
    getTime: () => active.currentTime,
    init,
  };
});
