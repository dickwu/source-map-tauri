import { useSearchPatientMutation } from '@/utils/apis/appointment'

export function PatientSearchContent() {
  const searchMutation = useSearchPatientMutation()
  return <button onClick={() => searchMutation}>Search</button>
}
