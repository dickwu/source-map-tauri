import { usePostMutation } from '@/utils/apis/api'

interface PatientSearchResponse {
  total: number
}

export const useSearchPatientMutation = () => {
  return usePostMutation<PatientSearchResponse>('appointment/home/search', true)
}
