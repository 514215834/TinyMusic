import { onBeforeUnmount, ref, type Ref } from "vue";

/**
 * 指针事件拖拽重排。
 *
 * 为什么不用 HTML5 DnD：Windows 上 Tauri dragDropEnabled（默认开启，M7 文件拖入导入
 * 依赖它）会 RevokeDragDrop + RegisterDragDrop 把 WebView2 原生 IDropTarget 换成
 * 只认 CF_HDROP 的处理器，而 Chromium 的 HTML5 拖拽（含页面内部）走 OS OLE 拖拽循环
 * ——落点永远命中 wry 的处理器，DOM 收不到 dragover/drop，页面内拖拽全部失效。
 * 指针实现不启动 OS 拖拽循环，完全绕开该限制。
 *
 * 用法：行元素携带 data-index（虚拟列表为绝对下标）；行上绑定 onPointerDown(index, e)；
 * 视觉反馈读 dragIndex（拖动中）/ overIndex（落点）。
 * 提交语义为 splice(from, 1) 后 splice(to, 0, moved) 的"移动到第 to 位"。
 */
export function usePointerReorder(options: {
  /** 滚动容器：拖到上下边缘自动滚动；拖拽确认后作为指针捕获元素 */
  scroller: Ref<HTMLElement | null>;
  /** 行元素根容器（querySelectorAll 范围） */
  container: Ref<HTMLElement | null>;
  itemSelector: string;
  onReorder: (from: number, to: number) => void;
  /** 触发拖拽的位移阈值（px），默认 5——小于阈值的位移仍是普通点击 */
  threshold?: number;
}) {
  const { scroller, container, itemSelector, onReorder } = options;
  const threshold = options.threshold ?? 5;
  const EDGE = 32;
  const SCROLL_SPEED = 12;

  const dragIndex = ref<number | null>(null);
  const overIndex = ref<number | null>(null);
  const dragging = ref(false);

  let press: { index: number; x: number; y: number; pointerId: number } | null = null;
  let lastY = 0;
  let scrollRaf = 0;
  let escHandler: ((e: KeyboardEvent) => void) | null = null;
  let clickCapture: ((e: MouseEvent) => void) | null = null;

  function isInteractive(e: PointerEvent): boolean {
    const el = e.target as HTMLElement | null;
    return !!el?.closest?.("button, input, textarea, select, [contenteditable]");
  }

  function rows(): { index: number; rect: DOMRect }[] {
    const root = container.value;
    if (!root) return [];
    return Array.from(root.querySelectorAll<HTMLElement>(itemSelector))
      .map((el) => ({ index: Number(el.dataset.index), rect: el.getBoundingClientRect() }))
      .filter((r) => r.rect.height > 0 && Number.isFinite(r.index));
  }

  /** 光标所在行；落在行间隙时取最近的一行 */
  function hitIndex(y: number): number | null {
    const list = rows();
    if (!list.length) return null;
    let best = list[0];
    let bestDist = Infinity;
    for (const row of list) {
      if (y >= row.rect.top && y < row.rect.bottom) return row.index;
      const dist = y < row.rect.top ? row.rect.top - y : y - row.rect.bottom;
      if (dist < bestDist) {
        bestDist = dist;
        best = row;
      }
    }
    return best.index;
  }

  /** 位移超阈值才进入拖拽态：普通点击（导航/选中）完全不受影响 */
  function confirmDrag(e: PointerEvent) {
    dragging.value = true;
    dragIndex.value = press!.index;
    overIndex.value = press!.index;
    // 捕获到滚动容器：指针移出窗口仍能收到 move/up；捕获事件重定向后照常冒泡到 document
    try {
      scroller.value?.setPointerCapture(e.pointerId);
    } catch {
      /* 指针可能已失效（如点按期间弹窗），退化为窗口内拖拽 */
    }
    document.body.style.userSelect = "none";
    escHandler = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") cancelDrag();
    };
    window.addEventListener("keydown", escHandler, true);
    lastY = e.clientY;
    scrollRaf = requestAnimationFrame(autoScroll);
  }

  /** 拖拽期逐帧：边缘自动滚动 + 重算落点（滚动后行矩形变化，光标不动落点也变） */
  function autoScroll() {
    if (!dragging.value) return;
    const el = scroller.value;
    if (el) {
      const rect = el.getBoundingClientRect();
      if (lastY < rect.top + EDGE) el.scrollTop -= SCROLL_SPEED;
      else if (lastY > rect.bottom - EDGE) el.scrollTop += SCROLL_SPEED;
    }
    overIndex.value = hitIndex(lastY);
    scrollRaf = requestAnimationFrame(autoScroll);
  }

  function onDocumentPointerMove(e: PointerEvent) {
    if (!press) return;
    if (!dragging.value) {
      const dist = Math.hypot(e.clientX - press.x, e.clientY - press.y);
      if (dist < threshold) return;
      confirmDrag(e);
    }
    lastY = e.clientY;
    overIndex.value = hitIndex(lastY);
    e.preventDefault();
  }

  /** 拖拽结束后吞掉紧接着的合成 click（避免误触发行导航/选中） */
  function suppressNextClick() {
    clickCapture = (e: MouseEvent) => {
      e.stopPropagation();
      e.preventDefault();
      releaseClickCapture();
    };
    document.addEventListener("click", clickCapture, true);
  }

  function releaseClickCapture() {
    if (clickCapture) {
      document.removeEventListener("click", clickCapture, true);
      clickCapture = null;
    }
  }

  function removeDocumentListeners() {
    document.removeEventListener("pointermove", onDocumentPointerMove);
    document.removeEventListener("pointerup", onDocumentPointerUp);
    document.removeEventListener("pointercancel", onDocumentCancel);
  }

  function cleanup() {
    dragging.value = false;
    press = null;
    dragIndex.value = null;
    overIndex.value = null;
    cancelAnimationFrame(scrollRaf);
    document.body.style.userSelect = "";
    if (escHandler) {
      window.removeEventListener("keydown", escHandler, true);
      escHandler = null;
    }
    removeDocumentListeners();
  }

  function onDocumentPointerUp(e: PointerEvent) {
    if (!dragging.value) {
      press = null;
      removeDocumentListeners();
      return;
    }
    const from = dragIndex.value;
    const to = overIndex.value;
    try {
      scroller.value?.releasePointerCapture(e.pointerId);
    } catch {
      /* 已释放 */
    }
    cleanup();
    if (from != null && to != null && from !== to) {
      suppressNextClick();
      onReorder(from, to);
    }
  }

  function onDocumentCancel() {
    if (press || dragging.value) cleanup();
  }

  function cancelDrag() {
    cleanup();
  }

  /** 行元素 pointerdown 绑定入口（左键、无修饰键、非交互子元素才可能进入拖拽） */
  function onPointerDown(index: number, e: PointerEvent) {
    if (e.button !== 0 || e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return;
    if (isInteractive(e)) return;
    removeDocumentListeners();
    press = { index, x: e.clientX, y: e.clientY, pointerId: e.pointerId };
    document.addEventListener("pointermove", onDocumentPointerMove, { passive: false });
    document.addEventListener("pointerup", onDocumentPointerUp);
    document.addEventListener("pointercancel", onDocumentCancel);
  }

  onBeforeUnmount(cleanup);

  return { dragIndex, overIndex, dragging, onPointerDown };
}
