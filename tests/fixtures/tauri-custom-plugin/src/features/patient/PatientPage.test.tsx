import { render, screen } from "@testing-library/react";
import { PatientPage } from "./PatientPage";

test("renders upload button", () => {
  render(<PatientPage patientId="fixture-patient" />);
  expect(screen.getByText("Upload patient file")).toBeTruthy();
});
