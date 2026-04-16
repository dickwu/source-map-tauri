import { create } from 'zustand'
import { useSavedAccounts } from '../hooks/useSavedAccounts'

const getInvoke = async () => {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke
}

export const useAccountStore = create(() => ({
  loadAccounts: async () => {
    const invoke = await getInvoke()
    await invoke('get_all_accounts')
  },
  openDevtools: async () => {
    await window.__TAURI__.invoke('open_devtools')
  },
  useSavedAccounts,
}))
