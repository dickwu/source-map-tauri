import { fetch as tauriFetch } from '@tauri-apps/plugin-http'

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://127.0.0.1:9501'

export const usePostApi = <T = any>(
  path: string,
  data: unknown,
  _isAuth = false,
  _enabled = true,
) => {
  return tauriFetch(`${API_URL}/${path}`, {
    method: 'POST',
    body: JSON.stringify(data),
    headers: {
      'Content-Type': 'application/json',
    },
  }) as Promise<T>
}
