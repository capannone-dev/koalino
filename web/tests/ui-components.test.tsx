// Specifies the dashboard's shared building blocks (#207): the empty state and the banner, and
// that icons come only from the shared Material set.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { Banner, EmptyState, buttonStyles } from "@/components/ui";

test("an empty state says what is missing and offers the next step", () => {
  const { container } = render(
    <EmptyState icon="meetings" title="No meetings yet" action={<a href="/devices" className={buttonStyles.filled}>Pair a device</a>}>
      They appear once they sync.
    </EmptyState>
  );
  expect(screen.getByText("No meetings yet")).toBeInTheDocument();
  expect(screen.getByText("They appear once they sync.")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Pair a device" })).toHaveAttribute("href", "/devices");
  expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
});

test("a danger banner is announced as an alert; others as status", () => {
  render(
    <>
      <Banner tone="danger" title="Can't load this page">Koalino is not responding.</Banner>
      <Banner tone="info" title="Tip" />
    </>
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Can't load this page");
  expect(screen.getByRole("status")).toHaveTextContent("Tip");
  expect(screen.getByRole("heading", { name: "Can't load this page" })).toBeInTheDocument();
});

test("only the shared icon set draws SVG icons", () => {
  const files: string[] = [];
  const walk = (folder: string) => {
    for (const name of readdirSync(folder)) {
      const path = join(folder, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (path.endsWith(".tsx")) files.push(path);
    }
  };
  walk("components");
  walk("app");
  const drawing = files.filter((path) => !path.endsWith("icons.tsx") && readFileSync(path, "utf8").includes("<svg"));
  expect(drawing).toEqual([]);
});
