import { invoke, Channel } from "@tauri-apps/api/core";

export type UploadArgs = {
  url: string;
  onProgress?: Channel<number>;
};

export async function upload(args: UploadArgs) {
  return await invoke("plugin:audit|upload", args);
}
