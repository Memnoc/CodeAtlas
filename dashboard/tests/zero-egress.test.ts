// ADR-0006: rendering the dashboard makes zero external requests — asserted,
// not assumed. This test builds the production bundle and scans every emitted
// byte for references to external hosts. Anything fetchable (scripts, styles,
// fonts, images, XHR targets) would appear here as an http(s) URL.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { beforeAll, describe, expect, it } from "vitest";

const dashboardDir = path.resolve(import.meta.dirname, "..");
const distDir = path.join(dashboardDir, "dist");

// URLs that are string literals by construction, never requests:
// - XML/SVG/MathML namespace URIs — DOM API identifiers, nothing is fetched.
// - react.dev/errors — text of React's minified-error message.
// - reactflow.dev — React Flow's attribution link href and error-doc strings.
// Everything else fails the test.
const INERT = [
  /^https?:\/\/www\.w3\.org\//,
  /^https:\/\/react\.dev\/errors\//,
  // reactflow.dev doc links, including the minified `https://${x}flow.dev/error#…`
  // template inside React Flow's error-message helper.
  /^https:\/\/(\$\{[^}]*\})?(react)?flow\.dev([/?#]|$)/,
];

function filesUnder(dir: string): string[] {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.join(entry.parentPath, entry.name));
}

describe("zero egress", () => {
  beforeAll(() => {
    execFileSync("npx", ["vite", "build"], {
      cwd: dashboardDir,
      stdio: "pipe",
      timeout: 180_000,
      // Vitest exports NODE_ENV=test, which would make Vite bundle
      // development React; the shipped artifact is the production build.
      env: { ...process.env, NODE_ENV: "production" },
    });
  }, 200_000);

  it("production build references no external URL except inert string literals", () => {
    const files = filesUnder(distDir);
    expect(files.length).toBeGreaterThan(0);

    const offenders: string[] = [];
    for (const file of files) {
      const content = readFileSync(file, "utf8");
      for (const match of content.matchAll(/https?:\/\/[^\s"'`\\)<>]+/g)) {
        const url = match[0];
        if (!INERT.some((pattern) => pattern.test(url))) {
          offenders.push(`${path.relative(distDir, file)}: ${url}`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  it("index.html loads every asset from a local relative path", () => {
    const html = readFileSync(path.join(distDir, "index.html"), "utf8");
    for (const match of html.matchAll(/(?:src|href)="([^"]*)"/g)) {
      const ref = match[1] ?? "";
      // Local paths only — no absolute URLs, no protocol-relative //host.
      expect(ref).toMatch(/^(?!\/\/)[./]/);
    }
    // No externally loaded fonts anywhere in the build.
    expect(html).not.toContain("fonts.googleapis.com");
    for (const file of filesUnder(distDir)) {
      const content = readFileSync(file, "utf8");
      expect(content).not.toContain("fonts.googleapis.com");
      expect(content).not.toContain("fonts.gstatic.com");
    }
  });
});
