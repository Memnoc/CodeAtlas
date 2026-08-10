# 19 — The egress suite shares `dashboard/dist` with the build that wipes it

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** the spec's Testing Decisions require that "the security
posture is tested, not documented (ADR-0006)". Two tests in the dashboard
suite currently race for the same directory, and the loser is the guarantee:
one of the egress assertions **passes green having checked nothing at all**.

Two test files write and read `dashboard/dist` concurrently — vitest runs
files in parallel and `vite.config.ts` sets no `fileParallelism: false`:

- `dashboard/tests/zero-egress.test.ts:33` builds into `dist/` in `beforeAll`,
  then reads every file under it. Vite's `emptyOutDir` deletes the directory
  before rewriting it.
- `dashboard/tests/self-scan.test.tsx:18` shells out to
  `cargo run -q -p codeatlas -- scan .`, which runs `crates/codeatlas/build.rs`.
  That script watches `dashboard/src`, `index.html`, `vite.config.ts` and
  `package.json`, and `ensure_dist` re-runs `npm run build` whenever
  `dist/index.html` is older than any of them.

So any edit under `dashboard/src` arms the race, and the next `npx vitest run`
has two processes building into and reading from one directory.

**The part that matters is not the failure — it is the pass.**
`zero-egress.test.ts:61`, "build references no websocket endpoint and no
protocol-relative host", is a loop over `filesUnder(distDir)` with the
assertions in the body. When `dist` is empty the body never executes and the
test reports success:

```
✓ zero egress > build references no websocket endpoint and no protocol-relative host
  (0 files examined)
```

Verified directly on 2026-08-10 by pointing that test's logic at an empty
directory: **1 passed**. An ADR-0006 guarantee that cannot fail is not a
guarantee. The same shape sits in `zero-egress.test.ts:82`, the
`fonts.googleapis.com` / `fonts.gstatic.com` loop; it is currently masked only
because that test happens to read `index.html` first and dies on `ENOENT`.

**What it costs today**, measured 2026-08-10:

- With `dashboard/src` touched immediately beforehand, the race reproduced
  **3 runs out of 3**: `Tests 2 failed | 40 passed`, every time. Left alone,
  the suite is green.
- The two visible failures are `expected 0 to be greater than 0`
  (`files.length`) and `ENOENT … dist/index.html`.
- Because a second run is green — cargo's build script is satisfied by then —
  the whole thing reads as flakiness. That is the worst possible presentation:
  it trains the reflex to re-run rather than to look, and it fires precisely
  when someone is working on the dashboard, which is exactly when the egress
  guarantee most needs to hold.

**Why the suite never caught it:** nothing asserts that the egress checks
examined anything. `files.length > 0` is asserted in the *first* test only,
and vitest gives each test its own pass/fail — so the one assertion that
would have caught an empty `dist` protects only the test that carries it.

Found on 2026-08-10 while theming the dashboard (commit `a37ce60`): the first
full suite run after editing `dashboard/src` failed 2/42 and the next passed.
The theming did not cause it — any dashboard edit does.

**Blocked by:** none.

**Status:** done

- [x] The egress suite no longer shares a directory with the build script:
      either it builds somewhere of its own, or the two are prevented from
      running concurrently
- [x] Every egress assertion that iterates a file set first asserts the set is
      non-empty, so an empty or partial `dist` fails loudly instead of passing
      quietly — this holds even if the race is fixed some other way
- [x] The websocket / protocol-relative check and the web-font check both fail
      when handed an empty directory, asserted by a test that hands them one
- [x] `npx vitest run` is green on the first attempt with `dashboard/src`
      freshly edited — the condition that reproduces the race 3 times in 3
- [x] Whatever directory the egress build uses is gitignored and cleaned up,
      and does not become a second thing `build.rs` must know about

**How it landed.** The egress suite builds into `dist-egress/` of its own, so
`dashboard/dist` now has exactly one writer and one reader, both `build.rs`.
`filesUnder` became `nonEmptyFilesUnder` and refuses to return an empty list,
which covers every guarantee at one seam instead of three remembered guards.
The three checks were parameterised by directory so the tests and the vacuity
guard run the same code rather than a copy — a guard against a duplicate of a
check would prove nothing about the check — and a fourth check was split out,
because "assets are local" and "no web-font host" were two guarantees sharing
one name.

The guard test hands every check an empty directory, a partial one (files but
no `index.html`), and an `index.html` that references nothing, and requires a
failure from each. Both new refusals were mutation-tested: delete the throw in
`nonEmptyFilesUnder`, or the reference-count assertion, and the guard fails.
The race itself was re-run under the conditions that reproduced it 3 times in
3 — `dashboard/src` touched immediately beforehand — and is green 3 for 3.

**The fix opened a hole, and closing it is the more valuable half.** Scanning
a build of its own means the dashboard suite no longer scans the bytes that
ship: `build.rs` embeds `dashboard/dist`, and when `node_modules` is missing
it embeds a *stale* dist with only a `cargo:warning`. A green dashboard suite
was about to become a statement about a fresh build rather than about the
binary. `crates/codeatlas/tests/embedded.rs` now asserts the guarantee over
`serve::ASSETS` — the bytes actually compiled in, which is what `serve` hands
a browser and what the share artifact inlines — including its own
`ASSETS.is_empty()` guard, since it is loops all the way down. That check runs
in both feature configurations.

The allowlist of inert URLs now lives once, in `tests/common/mod.rs`, shared
by the share-artifact test that already had a copy and by the new one. It is
security policy; two copies drifting apart is how a check quietly stops
agreeing with the one beside it. The TypeScript side keeps its own copy by
necessity and says so.

**Decisions taken on the open questions:**

- **How to unshare — the separate `outDir`, as recommended.** `dist-egress/`
  is gitignored, removed in `afterAll`, and outside everything `build.rs`
  watches, so it costs the build script nothing.
- **`filesUnder` refusing to return nothing — yes**, and it turned out to be
  the load-bearing half exactly as predicted. The separate directory prevents
  this particular race; the refusal prevents the class.
- **Does the race run the other way? No longer possible, rather than merely
  unobserved.** The concern was `vite build` emptying `dist` while `build.rs`
  walked it for `include_bytes!`. With the egress suite moved, `dist` has a
  single writer — `build.rs` itself, via `npm run build` — so there is no
  second process to race. Verified by grep: nothing else in the test suites or
  `package.json` writes it.
- **Is anything else vacuous? Checked, and the Rust side was already
  careful.** `tests/share.rs` guards its schema walker with a companion test
  (`the_walker_itself_sees_the_contract`) written for this exact reason, and
  `tests/sealed.rs` asserts `!crates.is_empty()` plus a control test. The one
  remaining case is deliberate and different in kind: `tests/egress.rs` skips
  when `unshare -r -n` is unavailable, passing without asserting — but it
  prints a loud `SKIPPED:` to stderr, documents CI as the enforcing run, and
  says so in the module comment. Left as designed; confirmed it did not fire
  here (`unshare -r -n` works on this machine, and no run printed the skip),
  so the harden record's claim that all five genuinely asserted stands.
