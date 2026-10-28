// Verifies Telemetry lives under Settings on the web (Issue #53): the primary navigation has no
// Dashboard entry, and Settings links to its Telemetry page.

import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { PrimaryNav } from "@/components/primary-nav";
import { TelemetryEntry } from "@/components/telemetry-entry";

test("the primary navigation has no telemetry dashboard entry", () => {
  render(<PrimaryNav />);

  const links = screen.getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")]);
  expect(links).toEqual([
    ["Meetings", "/meetings"],
    ["Voice gallery", "/voice-gallery"],
    ["Settings", "/settings"]
  ]);
});

test("Settings links to its Telemetry page", () => {
  render(<TelemetryEntry />);

  expect(screen.getByRole("link", { name: /Telemetry/ })).toHaveAttribute("href", "/settings/telemetry");
});
