import { useCallback, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { upload } from "@hospital/plugin-audit";

export function usePatientUpload(patientId: string) {
  const [progress, setProgress] = useState(0);

  const start = useCallback(async () => {
    const onProgress = new Channel<number>();
    onProgress.onmessage = setProgress;
    await upload({ url: `/patients/${patientId}/upload`, onProgress });
  }, [patientId]);

  return { progress, start };
}
