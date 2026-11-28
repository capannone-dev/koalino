// Verifies the web's navigation (#205): the current page is marked for sight and screen readers,
// every link carries an icon beside its word, and the home page sends people to their meetings.

import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { PrimaryNav } from "@/components/primary-nav";

const navigation = vi.hoisted(() => ({ pathname: "/meetings/m-1", redirect: vi.fn() }));
vi.mock("next/navigation", () => ({ usePathname: () => navigation.pathname, redirect: navigation.redirect }));

test("the current section is marked, including on its sub-pages", () => {
  render(<PrimaryNav />);
  expect(screen.getByRole("link", { name: "Meetings" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("link", { name: "Projects" })).not.toHaveAttribute("aria-current");
});

test("every link shows an icon beside its word", () => {
  render(<PrimaryNav />);
  for (const link of screen.getAllByRole("link")) expect(link.querySelector("svg")).not.toBeNull();
});

test("the home page opens the meetings", async () => {
  const { default: HomePage } = await import("@/app/page");
  HomePage();
  expect(navigation.redirect).toHaveBeenCalledWith("/meetings");
});
