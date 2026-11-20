// Verifies the web Settings page's AI connections (#95): listing connections without their keys,
// adding one (an online service with a write-only API key, #138), testing it, the default, the prompt, automatic summaries (#101), and the
// connection that answers paired devices' summary requests (#107).

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { AiSettings } from "@/components/ai-settings";
import * as api from "@/lib/llm-api";

vi.mock("@/lib/llm-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/llm-api")),
  getLlmSettings: vi.fn(),
  saveLlmConnection: vi.fn(),
  testLlmConnection: vi.fn(),
  updateLlmSettings: vi.fn(),
  deleteLlmConnection: vi.fn()
}));

const office: api.LlmConnection = {
  id: "c-1",
  label: "Office server",
  kind: "open_ai_compatible",
  base_url: "http://192.168.1.5:11434/v1",
  model: "llama3.1:8b",
  context_tokens: 8192,
  plain_http: true,
  is_default: true
};

const settings: api.LlmSettings = {
  connections: [office],
  default_connection: "c-1",
  template: "Summarize {{title}}.",
  default_template: "Built in",
  custom_template: true,
  auto_summarize: true,
  paired_summary_connection: null,
  placeholders: ["title", "date"]
};

beforeEach(() => {
  vi.mocked(api.getLlmSettings).mockReset();
  vi.mocked(api.saveLlmConnection).mockReset();
  vi.mocked(api.testLlmConnection).mockReset();
  vi.mocked(api.updateLlmSettings).mockReset();
});

test("lists connections with their default, a plain-http warning and no key", () => {
  render(<AiSettings initial={settings} />);

  // Once in the list, once in the menu of the connection that answers paired devices (#107).
  expect(screen.getAllByText("Office server")).toHaveLength(2);
  expect(screen.getByRole("option", { name: "Office server" })).toBeInTheDocument();
  expect(screen.getByText("Default")).toBeInTheDocument();
  expect(screen.getByText(/Unencrypted http/)).toBeInTheDocument();
  expect(screen.queryByText(/API key/)).not.toBeInTheDocument();
  expect(screen.getByLabelText("Summary prompt")).toHaveValue("Summarize {{title}}.");
  expect(screen.getByLabelText(/Summarize meetings automatically/)).toBeChecked();
});

test("adding a local server from a preset sends no key and lists it", async () => {
  const added: api.LlmConnection = { ...office, id: "c-2", label: "Studio", base_url: "http://192.168.1.10:1234/v1", model: "qwen3-8b", is_default: false };
  vi.mocked(api.saveLlmConnection).mockResolvedValue(added);
  vi.mocked(api.getLlmSettings).mockResolvedValue({ ...settings, connections: [office, added] });
  render(<AiSettings initial={settings} />);

  fireEvent.click(screen.getByRole("button", { name: "Add connection" }));
  expect(screen.queryByRole("option", { name: "Anthropic" })).not.toBeInTheDocument();
  expect(screen.queryByRole("option", { name: "OpenAI" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Start from"), { target: { value: "LM Studio" } });
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Studio" } });
  expect(screen.getByText(/desktop app to pick models/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save connection" }));

  expect(await screen.findByText("Saved Studio.")).toBeInTheDocument();
  expect(api.saveLlmConnection).toHaveBeenCalledWith(
    { label: "Studio", kind: "open_ai_compatible", base_url: "http://192.168.1.10:1234/v1", model: "qwen3-8b", context_tokens: 32768 },
    undefined
  );
});

test("testing a connection shows its reply, and failures are plain", async () => {
  vi.mocked(api.testLlmConnection).mockResolvedValueOnce("OK");
  render(<AiSettings initial={settings} />);

  fireEvent.click(screen.getByRole("button", { name: "Test" }));
  expect(await screen.findByText("Office server answered: OK")).toBeInTheDocument();

  vi.mocked(api.testLlmConnection).mockRejectedValueOnce(new api.LlmApiError("llm_unreachable", 502));
  fireEvent.click(screen.getByRole("button", { name: "Test" }));
  expect(await screen.findByText("The AI server could not be reached.")).toBeInTheDocument();
});

test("automatic summaries can be turned off", async () => {
  vi.mocked(api.updateLlmSettings).mockResolvedValue({ ...settings, auto_summarize: false });
  render(<AiSettings initial={settings} />);

  fireEvent.click(screen.getByLabelText(/Summarize meetings automatically/));

  expect(await screen.findByText("Automatic summaries are off.")).toBeInTheDocument();
  expect(api.updateLlmSettings).toHaveBeenCalledWith({ auto_summarize: false });
});

test("without the server the settings say so", () => {
  render(<AiSettings initial={null} />);
  expect(screen.getByText(/Can't read the AI settings/)).toBeInTheDocument();
});

test("Claude Code and Codex show whether the tool is installed, with no address or key (#105)", () => {
  const claude: api.LlmConnection = { ...office, id: "c-2", label: "Claude Pro", kind: "claude_code", base_url: "", model: "", plain_http: false, tool_installed: true, is_default: false };
  const codex: api.LlmConnection = { ...office, id: "c-3", label: "ChatGPT", kind: "codex", base_url: "", model: "gpt-5.5", plain_http: false, tool_installed: false, is_default: false };
  render(<AiSettings initial={{ ...settings, connections: [claude, codex] }} />);

  expect(screen.getByText(/Claude Code · default model/)).toBeInTheDocument();
  expect(screen.getByText(/Installed on the server's computer/)).toBeInTheDocument();
  expect(screen.getByText(/Codex · gpt-5.5/)).toBeInTheDocument();
  expect(screen.getByText(/Not found on the server's computer/)).toBeInTheDocument();
  expect(screen.queryByText(/API key/)).not.toBeInTheDocument();
});

test("a Claude Code connection is saved without an address or a key (#105)", async () => {
  vi.mocked(api.saveLlmConnection).mockResolvedValue({ ...office, id: "c-2", label: "Claude Pro", kind: "claude_code", base_url: "", model: "" });
  vi.mocked(api.getLlmSettings).mockResolvedValue(settings);
  render(<AiSettings initial={settings} />);

  fireEvent.click(screen.getByRole("button", { name: "Add connection" }));
  fireEvent.change(screen.getByLabelText("Start from"), { target: { value: "Claude Code (Claude plan)" } });
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Claude Pro" } });
  expect(screen.queryByLabelText("Address")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save connection" }));
  await screen.findByText("Saved Claude Pro.");

  const [body] = vi.mocked(api.saveLlmConnection).mock.calls[0];
  expect(body).toEqual({ label: "Claude Pro", kind: "claude_code", base_url: "", model: "", context_tokens: 200000 });
});

test("a connection can answer the summaries paired devices ask for (#107)", async () => {
  vi.mocked(api.updateLlmSettings).mockResolvedValue({ ...settings, paired_summary_connection: "c-1" });
  render(<AiSettings initial={settings} />);

  const menu = screen.getByLabelText(/Summaries for my other devices/);
  expect(menu).toHaveValue("");
  fireEvent.change(menu, { target: { value: "c-1" } });

  expect(
    await screen.findByText("Office server now writes the summaries and answers the chats your other devices ask for.")
  ).toBeInTheDocument();
  expect(api.updateLlmSettings).toHaveBeenCalledWith({ paired_summary_connection: "c-1" });
});

test("answering paired devices can be turned off (#107)", async () => {
  vi.mocked(api.updateLlmSettings).mockResolvedValue(settings);
  render(<AiSettings initial={{ ...settings, paired_summary_connection: "c-1" }} />);

  fireEvent.change(screen.getByLabelText(/Summaries for my other devices/), { target: { value: "" } });

  expect(
    await screen.findByText("This computer no longer writes summaries or answers chats for your other devices.")
  ).toBeInTheDocument();
  expect(api.updateLlmSettings).toHaveBeenCalledWith({ paired_summary_connection: null });
});

test("an online service is described and its key is sent once, never shown (#138)", async () => {
  const added: api.LlmConnection = { ...office, id: "c-2", label: "Gemini", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", plain_http: false, has_key: true, is_default: false };
  vi.mocked(api.saveLlmConnection).mockResolvedValue(added);
  vi.mocked(api.getLlmSettings).mockResolvedValue({ ...settings, connections: [office, added] });
  render(<AiSettings initial={settings} />);

  fireEvent.click(screen.getByRole("button", { name: "Add connection" }));
  fireEvent.change(screen.getByLabelText("Start from"), { target: { value: "Gemini" } });
  expect(screen.getByText(/aistudio\.google\.com/)).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Gemini" } });
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: " AIza-test " } });
  fireEvent.click(screen.getByRole("button", { name: "Save connection" }));
  await screen.findByText("Saved Gemini.");

  const [body] = vi.mocked(api.saveLlmConnection).mock.calls[0];
  expect(body.api_key).toBe("AIza-test");
  expect(body.base_url).toBe("https://generativelanguage.googleapis.com/v1beta/openai");
  expect(screen.queryByDisplayValue(/AIza-test/)).not.toBeInTheDocument();
});

test("a saved key is kept unless removed (#138)", async () => {
  const keyed: api.LlmConnection = { ...office, base_url: "https://api.deepseek.com", plain_http: false, has_key: true };
  vi.mocked(api.saveLlmConnection).mockResolvedValue(keyed);
  vi.mocked(api.getLlmSettings).mockResolvedValue({ ...settings, connections: [keyed] });
  render(<AiSettings initial={{ ...settings, connections: [keyed] }} />);

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByText("A key is saved. Leave empty to keep it.")).toBeInTheDocument();
  fireEvent.click(screen.getByLabelText("Remove the saved key"));
  fireEvent.click(screen.getByRole("button", { name: "Save connection" }));
  await screen.findByText(/Saved/);

  const [body] = vi.mocked(api.saveLlmConnection).mock.calls[0];
  expect(body.clear_key).toBe(true);
  expect(body.api_key).toBeUndefined();
});
