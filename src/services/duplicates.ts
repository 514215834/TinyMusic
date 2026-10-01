import {
  commands,
  type DuplicateGroup,
  type DuplicateResolve,
} from "../bindings";
import { unwrap } from "./ipc";

export type { DuplicateGroup, DuplicateResolve };

/** 重复曲目检测与清理（M6）：源文件移入回收站 + 曲库记录删除 */
export const duplicatesApi = {
  /** 按归一化标题+艺人+时长±2s 分组（items 首个为保留候选，按添加时间升序） */
  scan: async () => unwrap(await commands.duplicatesScan()),
  /** 清理：keep_id 之外的曲目源文件进回收站并删除曲库记录 */
  resolve: async (keepId: number, removeIds: number[]) =>
    unwrap(await commands.duplicateResolve(keepId, removeIds)),
};
