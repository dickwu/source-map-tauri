import { useSearchPatientMutation } from '@/utils/apis/appointment'

export function PatientSearch() {
  const searchPatientMutation = useSearchPatientMutation()
  return <button onClick={() => searchPatientMutation}>Search Patient</button>
}
