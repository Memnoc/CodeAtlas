// ADR-0006: rendering the dashboard makes zero external requests — asserted,
// not assumed. This builds the production bundle and scans every emitted byte
// for references to external hosts. Anything fetchable (scripts, styles,
// fonts, images, XHR targets) would appear here as an http(s) URL.
//
// The build goes to a directory of this test's own rather than to `dist/`
// (ticket 19). `dist/` belongs to crates/codeatlas/build.rs, which rebuilds it
// whenever the dashboard sources change; since vitest runs test files in
// parallel and self-scan.test.tsx shells out to cargo, both processes were
// emptying and reading one directory. The visible symptom was two tests
// failing on every dashboard edit; the real one was silent, because these
// guarantees are loops and a loop over an empty directory examines nothing
// and reports success.
//
// Scanning a build of our own leaves one thing this file cannot see: whether
// the bytes actually compiled into the binary say the same. That is asserted
// from inside the artifact, in crates/codeatlas/tests/embedded.rs.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const dashboardDir = path.resolve(import.meta.dirname, "..");
/** Relative to the dashboard root, which is what `vite build --outDir` takes. */
const EGRESS_OUT_DIR = "dist-egress";
const buildDir = path.join(dashboardDir, EGRESS_OUT_DIR);

/**
 * The phrase every "there was nothing to check" refusal carries. Shared so the
 * vacuity test can recognise a refusal without pinning the prose around it.
 */
const EXAMINED_NOTHING = "examined nothing";

// URLs that are string literals by construction, never requests:
// - XML/SVG/MathML namespace URIs — DOM API identifiers, nothing is fetched.
// - react.dev/errors — text of React's minified-error message.
// - reactflow.dev — React Flow's attribution link href and error-doc strings.
// Everything else fails the test. Kept in step with the Rust-side copy in
// crates/codeatlas/tests/common/mod.rs.
const INERT = [
  /^https?:\/\/www\.w3\.org\//,
  /^https:\/\/react\.dev\/errors\//,
  // reactflow.dev doc links, including the minified `https://${x}flow.dev/error#…`
  // template inside React Flow's error-message helper.
  /^https:\/\/(\$\{[^}]*\})?(react)?flow\.dev([/?#]|$)/,
];

/**
 * Every file under `dir` — and never an empty list.
 *
 * The refusal is the substance of ticket 19. Each guarantee below is a loop
 * with its assertions in the body, so over an empty directory it examines
 * nothing and passes; that is how a build which never ran once reported zero
 * egress. Throwing here covers every caller, present and future, which is
 * cheaper and harder to regress than a guard remembered in each test.
 */
function nonEmptyFilesUnder(dir: string): string[] {
  const files = readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.join(entry.parentPath, entry.name));
  if (files.length === 0) {
    throw new Error(
      `no files under ${dir}: the build produced nothing, so this guarantee ` +
        `${EXAMINED_NOTHING}. Failing loudly beats a vacuous pass.`,
    );
  }
  return files;
}

/** Hands `visit` the text of every file under `dir`, or refuses. */
function forEachFileContent(
  dir: string,
  visit: (content: string, file: string) => void,
): void {
  for (const file of nonEmptyFilesUnder(dir)) {
    visit(readFileSync(file, "utf8"), file);
  }
}

// The guarantees, each parameterised by the directory to check, so the tests
// below and the vacuity guard run exactly this code rather than a copy of it.
// A guard against a duplicate of a check proves nothing about the check.

function checkNoExternalUrls(dir: string): void {
  const offenders: string[] = [];
  forEachFileContent(dir, (content, file) => {
    for (const match of content.matchAll(/https?:\/\/[^\s"'`\\)<>]+/g)) {
      const url = match[0];
      if (!INERT.some((pattern) => pattern.test(url))) {
        offenders.push(`${path.relative(dir, file)}: ${url}`);
      }
    }
  });
  expect(offenders).toEqual([]);
}

function checkNoWebsocketOrProtocolRelativeHost(dir: string): void {
  forEachFileContent(dir, (content) => {
    // ws:// and wss:// are as much egress as http(s); nothing may reference
    // them, in any context, with no allowlist.
    expect(content).not.toMatch(/wss?:\/\//);
    // Protocol-relative //host in a markup src/href context resolves to an
    // external origin when served over http(s).
    expect(content).not.toMatch(/(?:src|href)\s*=\s*["']\/\//);
  });
}

function checkNoWebFontHosts(dir: string): void {
  forEachFileContent(dir, (content) => {
    expect(content).not.toContain("fonts.googleapis.com");
    expect(content).not.toContain("fonts.gstatic.com");
  });
}

function checkIndexLoadsAssetsLocally(dir: string): void {
  nonEmptyFilesUnder(dir); // refuses an empty build before anything else
  const index = path.join(dir, "index.html");
  if (!existsSync(index)) {
    // A partial build — files present, entry point absent — would otherwise
    // fail here with a bare ENOENT that reads like a broken test rather than
    // an unchecked guarantee.
    throw new Error(
      `${index} is missing: the build is incomplete, so this guarantee ` +
        `${EXAMINED_NOTHING}.`,
    );
  }
  const references = [...readFileSync(index, "utf8").matchAll(
    /(?:src|href)="([^"]*)"/g,
  )].map((match) => match[1] ?? "");
  // index.html carries at least a script and a stylesheet. Without this the
  // loop below is one more assertion that passes over an empty set.
  expect(references.length).toBeGreaterThan(0);
  for (const reference of references) {
    // Local paths only — no absolute URLs, no protocol-relative //host.
    expect(reference).toMatch(/^(?!\/\/)[./]/);
  }
}

describe("zero egress", () => {
  beforeAll(() => {
    execFileSync("npx", ["vite", "build", "--outDir", EGRESS_OUT_DIR], {
      cwd: dashboardDir,
      stdio: "pipe",
      timeout: 180_000,
      // Vitest exports NODE_ENV=test, which would make Vite bundle
      // development React; the shipped artifact is the production build.
      env: { ...process.env, NODE_ENV: "production" },
    });
  }, 200_000);

  afterAll(() => {
    rmSync(buildDir, { recursive: true, force: true });
  });

  it("production build references no external URL except inert string literals", () => {
    checkNoExternalUrls(buildDir);
  });

  it("build references no websocket endpoint and no protocol-relative host", () => {
    checkNoWebsocketOrProtocolRelativeHost(buildDir);
  });

  it("build references no web-font host", () => {
    checkNoWebFontHosts(buildDir);
  });

  it("index.html loads every asset from a local relative path", () => {
    checkIndexLoadsAssetsLocally(buildDir);
  });

  it("every guarantee above refuses an empty or partial build", () => {
    // The regression this file exists for. Before ticket 19 the websocket
    // check passed green over an empty directory, so the guarantee that
    // mattered most was the one that could not fail.
    const empty = mkdtempSync(path.join(tmpdir(), "codeatlas-egress-empty-"));
    const partial = mkdtempSync(path.join(tmpdir(), "codeatlas-egress-part-"));
    writeFileSync(path.join(partial, "assets.js"), "export const x = 1;\n");
    try {
      for (const check of [
        checkNoExternalUrls,
        checkNoWebsocketOrProtocolRelativeHost,
        checkNoWebFontHosts,
        checkIndexLoadsAssetsLocally,
      ]) {
        expect(() => check(empty), check.name).toThrow(EXAMINED_NOTHING);
      }
      // Files but no entry point: the case a half-finished build leaves.
      expect(() => checkIndexLoadsAssetsLocally(partial)).toThrow(
        EXAMINED_NOTHING,
      );
      // And an index.html that references nothing at all. It cannot refuse
      // with EXAMINED_NOTHING — the build is real — but it must still fail
      // rather than walk an empty reference list and call that a pass.
      writeFileSync(path.join(partial, "index.html"), "<!doctype html><html>");
      expect(() => checkIndexLoadsAssetsLocally(partial)).toThrow();
    } finally {
      rmSync(empty, { recursive: true, force: true });
      rmSync(partial, { recursive: true, force: true });
    }
  });
});
