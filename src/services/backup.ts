import { commands, type BackupExportResult, type BackupRestoreResult } from "../bindings";
import { unwrap } from "./ipc";

export type { BackupExportResult, BackupRestoreResult };

/** 全量备份（M7）：导出/恢复 JSON 文件，曲目按路径匹配，未命中跳过并计数 */
export const backupApi = {
  export: async (path: string) => unwrap(await commands.backupExport(path)),
  restore: async (path: string) => unwrap(await commands.backupRestore(path)),
};
