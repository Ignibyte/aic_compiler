# About this fork

This repository is a private fork of [illegalstudio/elephc](https://github.com/illegalstudio/elephc).
Since 2026-10-17 its `main` is upstream `main` plus a small set of carried commits. Upstream's
code wins wherever both sides touched the same thing.

## History

- `aic/fork-final-0360718` tags the last commit of the earlier independent fork: 173 commits on
  top of upstream `release/0.26.1`. That line is also the second parent of the sync merge
  `61265dffe`, so `git log` still reaches it. None of its code is in the tree unless a carry
  commit brought it back.
- `61265dffe` is the sync merge. Its tree is upstream `ecc48af08` exactly.

## Carry commits

Every commit on top of upstream is a carry commit. Each one:

1. fixes a behaviour upstream gets wrong or refuses, with a PHP program that shows it;
2. adds tests in upstream's own layout and passes upstream's suite;
3. is written so it could be sent upstream as a pull request, and is sent when it is ready.

A carry commit that upstream later supersedes is dropped at the next sync, not merged around.

Carry commits follow upstream's conventions (`AGENTS.md`, "Conventions"), because each one is
meant to go upstream: a `fix:` or `feat:` prefix, a short message, no `Co-Authored-By` line, and
no `CHANGELOG.md` entry. When the change alters what a page in `docs/internals/` says, the same
commit updates it.

## Carry workflow

A carry fix goes through four phases, the compiler's version of AIC's Plan, Code, Quality
checks and Documentation (AIC ADR 0063). The checks are `just` recipes in
`/srv/stacks/elephc-bundler/justfile`, run as `just -f /srv/stacks/elephc-bundler/justfile
<recipe>`. They live outside this repository because a carry branch starts from upstream and
would not contain them. The AIC skill `elephc-carry` walks through the same steps.

1. **Plan.** After `git fetch upstream`, reproduce the bug on plain upstream with a PHP program,
   and run the program under `php` for the expected output. Find the cause. Post the repro, the
   cause and the approach as a comment on the AIC ticket, and wait for the owner's OK.
2. **Code.** Branch `fix/<slug>` from `upstream/main`. Write the fix and a regression test in
   upstream's layout (`tests/codegen/`, `compile_and_run`, the heap-debug helpers when ownership
   is involved), following `AGENTS.md` and `CONTRIBUTING.md`. Iterate with `just test <filter>`.
   Commit, then run `just bite <filter>`: it reverts `src/` to the merge base and fails unless at
   least one of the new tests fails.
3. **Quality checks.** Run `just check` until it prints `check: PASS`. It runs:
   - the debug build, which must have no warnings (upstream CI's gate), and `git diff --check`;
   - the AIC survey with forced lowering, the carry battery and the #712 repros, each diffed
     against the ledger of the branch's upstream base. A row that was OK and is not any more
     fails the check;
   - the full debug suite. Each failure is rerun alone and reported as `flake` (passes alone),
     `env` (listed in the bundler's `known-env-failures.txt`) or `FAIL`.

   A check takes about an hour, nearly all of it the suite. The first check on a new upstream
   base, or after AIC's `packages/` has changed, first builds that base in
   `/mnt/fast/build/elephc-upstream` and records its ledger, which adds about five minutes.
4. **Documentation and delivery.**
   - Open a PR from the branch into this fork's `main` and merge it with a merge commit, so the
     fork holds the same commit upstream gets. A branch made from a newer `upstream/main` brings
     those upstream commits along, so that merge also counts as a sync.
   - With the owner's OK, push the branch to the public fork `chadmandoo/elephc` and open the PR
     on `illegalstudio/elephc`: the repro, php's output and ours, the cause, the fix, and the
     check results.
   - Record the outcome on the AIC ticket. Keep the run as a ledger with `just record <sha>`
     when it will serve as a baseline.

## Syncing with upstream

```sh
git fetch upstream
git switch -c sync/upstream-<sha> main
git merge upstream/main          # resolve conflicts in upstream's favour
```

Then run `just check base=<previous upstream base>`, and drop any carry commit whose program
now passes on plain upstream.

Build with a per-checkout target directory, so the bridge archives sit beside the binary:

```sh
CARGO_TARGET_DIR=/mnt/fast/target/elephc cargo build --release
```

Run the suite in the debug profile. `cargo test --release` hangs on a `#[should_panic]` test
whose `debug_assert!` is compiled out. Run it through `just suite` rather than a bare `cargo
test`: the recipe builds the bridge crates together first and sets
`ELEPHC_TEST_PREBUILT_BRIDGES=1`, as upstream CI does. Otherwise the harness rebuilds a stale
bridge on its own, that copy resolves other features than the one inside
`libelephc_magician.a`, and the `pcntl` tests fail at link time with duplicate symbols.
