/**
 * Specifies the meeting sites the extension knows (#174): which service a page belongs to, the
 * name shown for a site, and the meeting title taken from a page's title.
 */
import { describe, expect, it } from "vitest";
import { MEETING_SERVICES, knownSitePatterns, meetingTitle, serviceForHost, siteKey, siteName } from "../src/lib/sites";

describe("serviceForHost", () => {
  it("knows the meeting services by their hosts", () => {
    expect(serviceForHost("teams.microsoft.com")?.id).toBe("teams");
    expect(serviceForHost("teams.live.com")?.id).toBe("teams");
    expect(serviceForHost("meet.google.com")?.id).toBe("meet");
    expect(serviceForHost("app.zoom.us")?.id).toBe("zoom");
    expect(serviceForHost("us02web.zoom.us")?.id).toBe("zoom");
    expect(serviceForHost("meet.jit.si")?.id).toBe("jitsi");
    expect(serviceForHost("8x8.vc")?.id).toBe("jitsi");
    expect(serviceForHost("acme.webex.com")?.id).toBe("webex");
    expect(serviceForHost("whereby.com")?.id).toBe("whereby");
  });

  it("does not take look-alike hosts for a service", () => {
    expect(serviceForHost("zoom.us.example.com")).toBeNull();
    expect(serviceForHost("notzoom.us")).toBeNull();
    expect(serviceForHost("meet.google.com.evil.test")).toBeNull();
    expect(serviceForHost("example.org")).toBeNull();
  });
});

describe("knownSitePatterns", () => {
  it("gives a match pattern for every service host, https only", () => {
    const patterns = knownSitePatterns();
    expect(patterns).toContain("https://meet.google.com/*");
    expect(patterns).toContain("https://*.zoom.us/*");
    expect(patterns.every((pattern) => pattern.startsWith("https://") && pattern.endsWith("/*"))).toBe(true);
    expect(new Set(patterns).size).toBe(patterns.length);
  });

  it("covers each service's hosts", () => {
    for (const service of MEETING_SERVICES) expect(service.patterns.length).toBeGreaterThan(0);
  });
});

describe("siteName and siteKey", () => {
  it("names a known service, else the host without www", () => {
    expect(siteName("meet.google.com")).toBe("Google Meet");
    expect(siteName("www.example.org")).toBe("example.org");
    expect(siteName("calls.example.org")).toBe("calls.example.org");
  });

  it("keys a site by its service or its host, for per-site choices", () => {
    expect(siteKey("us02web.zoom.us")).toBe("zoom");
    expect(siteKey("app.zoom.us")).toBe("zoom");
    expect(siteKey("www.Example.org")).toBe("example.org");
  });
});

describe("meetingTitle", () => {
  it("drops the service's name and unread counts from the page title", () => {
    expect(meetingTitle("whereby.com", "(2) Design review | Whereby")).toBe("Design review");
    expect(meetingTitle("meet.jit.si", "Weekly sync | Jitsi Meet")).toBe("Weekly sync");
    expect(meetingTitle("acme.webex.com", "Cisco Webex - Budget 2027")).toBe("Budget 2027");
  });

  it("names a Google Meet call by its code", () => {
    expect(meetingTitle("meet.google.com", "Meet – abc-defg-hij")).toBe("Google Meet abc-defg-hij");
    expect(meetingTitle("meet.google.com", "Meet - Project kickoff")).toBe("Project kickoff");
  });

  it("gives nothing when the title only names the service", () => {
    expect(meetingTitle("app.zoom.us", "Zoom")).toBe("");
    expect(meetingTitle("meet.google.com", "Google Meet")).toBe("");
    expect(meetingTitle("example.org", "   ")).toBe("");
  });

  it("keeps the title of an unknown site", () => {
    expect(meetingTitle("calls.example.org", "Team call – Example Calls")).toBe("Team call – Example Calls");
  });
});
