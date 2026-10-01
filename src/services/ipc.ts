import { invoke } from "@tauri-apps/api/core";

/** 归一化的 IPC 错误（Rust 侧 AppErrorDto 统一转为此类型） */
export class IpcError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "IpcError";
  }
}

/** tauri-specta 生成的 Result 风格返回值 */
export type SpectaResult<T> =
  { status: "ok"; data: T } | { status: "error"; error: { message: string } };

/** 解包 specta 命令返回值：error 时抛出 IpcError */
export function unwrap<T>(result: SpectaResult<T>): T {
  if (result.status === "error") {
    throw new IpcError(result.error.message);
  }
  return result.data;
}

/**
 * IPC 唯一入口（技术设计文档 §8）：services/ 之外禁止直接 invoke。
 * M0 各命令封装在 library.ts / settings.ts，经 unwrap 解包。
 */
export async function ipc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw new IpcError(typeof err === "string" ? err : JSON.stringify(err));
  }
}
