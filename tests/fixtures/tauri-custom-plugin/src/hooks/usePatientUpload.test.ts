import { mockIPC } from "@tauri-apps/api/mocks";
import { upload } from "@hospital/plugin-audit";

test("uses the audit upload plugin", async () => {
  mockIPC((cmd) => {
    if (cmd === "plugin:audit|upload") {
      return Promise.resolve(null);
    }

    return Promise.reject(new Error("unexpected command"));
  });

  await upload({ url: "/patients/fixture/upload" });
});
