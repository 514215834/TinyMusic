import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { libraryApi, type Folder, type Track } from "../services/library";

interface ScanState {
  running: boolean;
  done: number;
  total: number;
}

/** 曲库状态：分页加载 + 扫描进度镜像（scan:progress / scan:done 事件） */
export const useLibraryStore = defineStore("library", {
  state: () => ({
    tracks: [] as Track[],
    total: 0,
    page: 0,
    pageSize: 200,
    loading: false,
    folders: [] as Folder[],
    scan: { running: false, done: 0, total: 0 } as ScanState,
    lastError: "",
    initialized: false,
    /** Ctrl+F 聚焦搜索框：自增 tick 驱动 LibraryView watch 聚焦 */
    searchFocusTick: 0,
  }),
  actions: {
    async init() {
      if (this.initialized) return;
      this.initialized = true;

      await listen<{ done: number; total: number }>("scan:progress", (e) => {
        this.scan = { running: true, done: e.payload.done, total: e.payload.total };
      });
      await listen<{
        added: number;
        updated: number;
        removed: number;
        unchanged: number;
        errors: number;
        durationMs: number;
      }>("scan:done", async (e) => {
        this.scan = { running: false, done: 0, total: 0 };
        // 部分失败时后端会另行发 scan:error 给出更具体提示，这里只在正常完成时提示
        if (e.payload.errors === 0) this.lastError = "";
        await this.reload();
      });
      await listen<string>("scan:error", (e) => {
        this.scan = { running: false, done: 0, total: 0 };
        this.lastError = e.payload;
      });

      await this.loadFolders();
      await this.reload();
    },
    async loadFolders() {
      this.folders = await libraryApi.folderList();
    },
    async reload() {
      this.page = 0;
      this.tracks = [];
      this.total = 0;
      await this.loadMore();
    },
    async loadMore() {
      if (this.loading) return;
      if (this.tracks.length > 0 && this.tracks.length >= this.total) return;
      this.loading = true;
      try {
        const result = await libraryApi.tracksQuery(this.page + 1, this.pageSize);
        this.tracks.push(...result.items);
        this.total = result.total;
        this.page += 1;
      } finally {
        this.loading = false;
      }
    },
    async addFolder(path: string) {
      await libraryApi.folderAdd(path);
      this.lastError = "";
      await this.loadFolders();
      this.scan = { running: true, done: 0, total: 0 };
    },
    async removeFolder(id: number) {
      await libraryApi.folderRemove(id);
      await this.loadFolders();
      await this.rescan();
    },
    async rescan() {
      this.scan = { running: true, done: 0, total: 0 };
      await libraryApi.rescan();
    },
    focusSearch() {
      this.searchFocusTick += 1;
    },
  },
});
