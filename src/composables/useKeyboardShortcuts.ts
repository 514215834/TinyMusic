import { onBeforeUnmount, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useFavoritesStore } from "../stores/favorites";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";

/**
 * 应用内快捷键（功能设计文档 §4.3 应用内部分）：
 * Space 播放暂停、←/→ 快退快进 5s、Ctrl+←/→ 上下曲、↑/↓ 音量、M 静音、L 收藏、Ctrl+F 聚焦搜索。
 * 输入框聚焦时仅保留 Ctrl 组合键，避免打字冲突。
 */
export function useKeyboardShortcuts() {
  const router = useRouter();
  const player = usePlayerStore();
  const library = useLibraryStore();
  const favorites = useFavoritesStore();

  function isTyping(target: EventTarget | null) {
    const el = target as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
  }

  function onKeydown(e: KeyboardEvent) {
    const typing = isTyping(e.target);
    if (e.ctrlKey && e.code === "KeyF") {
      e.preventDefault();
      if (router.currentRoute.value.name !== "library") void router.push({ name: "library" });
      library.focusSearch();
      return;
    }
    if (typing || e.metaKey || e.altKey) return;
    if (e.ctrlKey) {
      if (e.code === "ArrowLeft") {
        e.preventDefault();
        player.prev();
      } else if (e.code === "ArrowRight") {
        e.preventDefault();
        player.next();
      }
      return;
    }

    switch (e.code) {
      case "Space":
        e.preventDefault();
        player.toggle();
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (player.currentTrack) player.seek(Math.max(0, player.positionSec - 5));
        break;
      case "ArrowRight":
        e.preventDefault();
        if (player.currentTrack) player.seek(player.positionSec + 5);
        break;
      case "ArrowUp":
        e.preventDefault();
        player.setVolume(player.volume + 0.05);
        break;
      case "ArrowDown":
        e.preventDefault();
        player.setVolume(player.volume - 0.05);
        break;
      case "KeyM":
        player.toggleMute();
        break;
      case "KeyL":
        if (player.currentTrack) void favorites.toggle(player.currentTrack.id);
        break;
      default:
        break;
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeydown));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
}
