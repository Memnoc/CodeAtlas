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

**Status:** ready

- [ ] The egress suite no longer shares a directory with the build script:
      either it builds somewhere of its own, or the two are prevented from
      running concurrently
- [ ] Every egress assertion that iterates a file set first asserts the set is
      non-empty, so an empty or partial `dist` fails loudly instead of passing
      quietly — this holds even if the race is fixed some other way
- [ ] The websocket / protocol-relative check and the web-font check both fail
      when handed an empty directory, asserted by a test that hands them one
- [ ] `npx vitest run` is green on the first attempt with `dashboard/src`
      freshly edited — the condition that reproduces the race 3 times in 3
- [ ] Whatever directory the egress build uses is gitignored and cleaned up,
      and does not become a second thing `build.rs` must know about

**Worth deciding while in here:**

- **How to unshare the directory.** Giving the egress test its own
  `--outDir` (say `dist-egress/`) removes the sharing outright and keeps both
  test files parallel, which is why it looks better than the alternative.
  Serialising with `fileParallelism: false` slows the whole suite to fix two
  files and — the real objection — leaves the vacuous pass in place, since an
  empty `dist` can arise any other way too. Recommend the separate outDir,
  and treat the non-emptiness assertions as the load-bearing half.
- **Whether `filesUnder` should refuse to return nothing.** Making the helper
  itself throw on an empty result would fix every present and future caller in
  one line, rather than relying on each test to remember. Cheaper than three
  separate guards and harder to regress.
- **Does the race also run the other way?** Both processes *write* `dist`, so
  in principle `vite build` could empty it while `build.rs` is walking it to
  generate `include_bytes!` entries, embedding a partial asset set into the
  binary. Not observed, and not investigated — worth ten minutes before
  closing, because a binary that serves half a dashboard would be a far
  quieter failure than a red test.
- **Is anything else vacuous?** This ticket found one guarantee that asserts
  nothing when its input is empty. The same question is worth asking once of
  the Rust egress suite and the redaction-exhaustiveness test, both of which
  are also loops over a discovered set.
