import { useSearchPatientMutation } from '@/utils/apis/appointment'

export function PatientSearch() {
  const searchPatientMutation = useSearchPatientMutation()
  const duplicateSearchPatientMutation = useSearchPatientMutation()
  return (
    <button onClick={() => [searchPatientMutation, duplicateSearchPatientMutation]}>
      Search Patient
    </button>
  )
}
