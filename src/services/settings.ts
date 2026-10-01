import { commands } from "../bindings";
import { unwrap } from "./ipc";

/** 设置读写：value 为 JSON 字符串，由调用方序列化/反序列化 */
export const settingsApi = {
  get: async (key: string) => unwrap(await commands.settingsGet(key)),
  set: async (key: string, value: string) => unwrap(await commands.settingsSet(key, value)),
};
