import { ref, watch } from "vue";
import { coverUrl, type Track } from "../services/library";

/** coverFile → 封面 URL 的解析缓存：同一封面文件（同专辑）只发起一次 IPC */
const promiseCache = new Map<string, Promise<string | null>>();
const CACHE_MAX = 2000;

/**
 * 按需解析封面：仅对 getter 返回的（可视窗口内的）曲目发起解析，
 * 结果按 track.id 记录；无封面记为 null，避免重复解析。
 * getter 返回新数组时触发（虚拟列表滚动 / 列表内容变化）。
 */
export function useCovers(visible: () => Track[]) {
  const urls = ref<Record<number, string | null>>({});
  watch(
    visible,
    (list) => {
      for (const track of list) {
        if (track.coverFile == null || urls.value[track.id] !== undefined) continue;
        urls.value[track.id] = null; // 解析中占位，防止同帧重复入队
        let p = promiseCache.get(track.coverFile);
        if (!p) {
          p = coverUrl(track.coverFile).catch(() => null);
          promiseCache.set(track.coverFile, p);
          const oldest = promiseCache.keys().next().value;
          if (promiseCache.size > CACHE_MAX && oldest) promiseCache.delete(oldest);
        }
        void p.then((url) => {
          urls.value[track.id] = url;
        });
      }
    },
    { immediate: true },
  );
  return urls;
}
