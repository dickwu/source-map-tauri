import { usePatientUpload } from "../../hooks/usePatientUpload";

export function PatientPage({ patientId }: { patientId: string }) {
  const upload = usePatientUpload(patientId);
  return <button onClick={upload.start}>Upload patient file</button>;
}
