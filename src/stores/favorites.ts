import { defineStore } from "pinia";
import { libraryApi } from "../services/library";

/** 收藏状态：行内红心与收藏视图共用一份 id 集合 */
export const useFavoritesStore = defineStore("favorites", {
  state: () => ({
    ids: [] as number[],
    loaded: false,
  }),
  getters: {
    has: (state) => (trackId: number) => state.ids.includes(trackId),
  },
  actions: {
    async load() {
      if (this.loaded) return;
      this.ids = await libraryApi.favoritesIds();
      this.loaded = true;
    },
    /** 乐观更新：先翻转 UI，失败时回滚 */
    async toggle(trackId: number) {
      const wasFav = this.ids.includes(trackId);
      this.ids = wasFav ? this.ids.filter((id) => id !== trackId) : [...this.ids, trackId];
      try {
        const nowFav = await libraryApi.favoriteToggle(trackId);
        if (nowFav !== !wasFav) {
          this.ids = nowFav
            ? [...new Set([...this.ids, trackId])]
            : this.ids.filter((id) => id !== trackId);
        }
      } catch (e) {
        this.ids = wasFav ? [...this.ids, trackId] : this.ids.filter((id) => id !== trackId);
        throw e;
      }
    },
  },
});
