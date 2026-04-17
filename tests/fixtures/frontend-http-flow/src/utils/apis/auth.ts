import { usePostApi } from '@/utils/apis/api'

export const useLogin = (
  email: string,
  password: string,
  enabled: boolean,
) => {
  return usePostApi('auth/login', { email, password }, false, enabled)
}
