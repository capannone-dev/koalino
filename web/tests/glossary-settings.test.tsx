// Verifies the web Settings page's glossary: the saved terms shown one per line, saving them, and
// the reasons a save fails.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { GlossarySettings } from "@/components/glossary-settings";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/api")),
  setGlossary: vi.fn()
}));

beforeEach(() => {
  vi.mocked(api.setGlossary).mockReset();
});

test("shows the saved terms one per line", () => {
  render(<GlossarySettings initialTerms={["Marco Rossi", "OKR"]} />);

  expect(screen.getByLabelText("Glossary")).toHaveValue("Marco Rossi\nOKR");
});

test("saves one term per line, without blank lines, and shows what was saved", async () => {
  vi.mocked(api.setGlossary).mockResolvedValue(["Marco Rossi", "Kubernetes"]);
  render(<GlossarySettings initialTerms={[]} />);

  fireEvent.change(screen.getByLabelText("Glossary"), { target: { value: " Marco Rossi \n\nKubernetes\n" } });
  fireEvent.click(screen.getByRole("button", { name: "Save glossary" }));

  expect(await screen.findByText("Saved. The next recording uses these names and terms.")).toBeInTheDocument();
  expect(api.setGlossary).toHaveBeenCalledWith(["Marco Rossi", "Kubernetes"]);
  expect(screen.getByLabelText("Glossary")).toHaveValue("Marco Rossi\nKubernetes");
});

test("a refused glossary explains the limits and keeps the text", async () => {
  vi.mocked(api.setGlossary).mockRejectedValue(new api.GlossaryError("invalid_glossary", 400));
  render(<GlossarySettings initialTerms={[]} />);

  fireEvent.change(screen.getByLabelText("Glossary"), { target: { value: "x".repeat(61) } });
  fireEvent.click(screen.getByRole("button", { name: "Save glossary" }));

  expect(await screen.findByText(/at most 60 characters/)).toBeInTheDocument();
  expect(screen.getByLabelText("Glossary")).toHaveValue("x".repeat(61));
});

test("without the server the glossary cannot be edited", () => {
  render(<GlossarySettings initialTerms={null} />);

  expect(screen.getByLabelText("Glossary")).toBeDisabled();
  expect(screen.getByText(/Can't read the glossary/)).toBeInTheDocument();
});
