// Verifies Telemetry (Issue #53) and Devices (Issue #84) live under Settings on the web: the primary
// navigation has neither, and Settings links to both pages.

import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { PrimaryNav } from "@/components/primary-nav";
import { DevicesEntry } from "@/components/devices-entry";
import { TelemetryEntry } from "@/components/telemetry-entry";

test("the primary navigation has Projects after Meetings, then Chat, and no telemetry or devices entry", () => {
  render(<PrimaryNav />);

  const links = screen.getAllByRole("link").map((link) => [link.getAttribute("aria-label"), link.getAttribute("href")]);
  expect(links).toEqual([
    ["Meetings", "/meetings"],
    ["Projects", "/projects"],
    ["Chat", "/chat"],
    ["Voice gallery", "/voice-gallery"],
    ["Settings", "/settings"]
  ]);
});

test("Settings links to its Telemetry page", () => {
  render(<TelemetryEntry />);

  expect(screen.getByRole("link", { name: /Telemetry/ })).toHaveAttribute("href", "/settings/telemetry");
});

test("Settings links to the Devices page, which keeps its address", () => {
  render(<DevicesEntry />);

  expect(screen.getByRole("link", { name: /Devices/ })).toHaveAttribute("href", "/devices");
});
