import { useSearchPatientMutation } from '@/utils/apis/appointment'

export default function SearchDefaultPanel() {
  const searchMutation = useSearchPatientMutation()
  return <button onClick={() => searchMutation}>Default Search</button>
}
