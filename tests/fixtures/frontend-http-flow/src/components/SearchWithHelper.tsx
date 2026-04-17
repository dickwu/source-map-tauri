import { useSearchPatientMutation } from '@/utils/apis/appointment'

export function useSearchHelper() {
  const searchMutation = useSearchPatientMutation()
  return searchMutation
}

export function SearchWithHelper() {
  const searchMutation = useSearchHelper()
  return <button onClick={() => searchMutation}>Helper Search</button>
}
