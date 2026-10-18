# About this fork

This repository is a private fork of [illegalstudio/elephc](https://github.com/illegalstudio/elephc).
Since 2026-10-17 its `main` is upstream `main` plus carried commits. The goal (AIC ADR 0064,
2026-10-18) is AIC's whole runtime surface compiled natively: our carries stay where they get
AIC there, and each one leaves as soon as plain upstream does the same job.

## History

- `aic/fork-final-0360718` tags the last commit of the earlier independent fork: 173 commits on
  top of upstream `release/0.26.1`. That line is also the second parent of the sync merge
  `61265dffe`, so `git log` still reaches it. None of its code is in the tree unless a carry
  commit brought it back.
- `61265dffe` is the sync merge. Its tree is upstream `ecc48af08` exactly.

## Carry commits

Every commit on top of upstream is a carry commit. Each one:

1. fixes a behaviour upstream gets wrong or refuses, with a PHP program that shows it;
2. adds tests in upstream's own layout and passes upstream's suite.

A carry does not have to be written as an upstream pull request. It may re-port the earlier
fork's own fix (AIC's `docs/planning/elephc-upstream-sync/carry-inventory.md` lists them by
cluster) or carry a policy upstream has not adopted. The tests are what let a sync tell when
upstream covers a carry: a carry whose program passes on plain upstream is dropped at that sync,
not merged around. Where upstream changed the same code but still fails the program, the carry
is re-applied on top of upstream's version.

Only a fix that is solid (`just check` passes, with a byte-parity program) and about PHP rather
than shaped around AIC goes upstream, one narrow fix per pull request, each with the owner's OK.

Carry commits still follow upstream's conventions (`AGENTS.md`, "Conventions"), so any of them
can go upstream later without a rewrite: a `fix:` or `feat:` prefix, a short message, no
`Co-Authored-By` line, and no `CHANGELOG.md` entry. When the change alters what a page in
`docs/internals/` says, the same commit updates it.

## Carry workflow

A carry fix goes through four phases: Plan, Code, Quality checks and Documentation. The checks
are `just` recipes in
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
     fork holds the commit as written. A branch made from a newer `upstream/main` brings those
     upstream commits along, so that merge also counts as a sync.
   - Only for a solid, PHP-generic fix (see "Carry commits") and with the owner's OK, push the
     branch to the public fork `chadmandoo/elephc` and open the PR on `illegalstudio/elephc`:
     the repro, php's output and ours, the cause, the fix, and the check results.
   - Record the outcome on the AIC ticket. Keep the run as a ledger with `just record <sha>`
     when it will serve as a baseline.

## Syncing with upstream

Sync on every upstream release, or monthly, whichever comes first. Each sync is also the
re-evaluation of every carry.

```sh
git fetch upstream
git switch -c sync/upstream-<sha> main
git merge upstream/main          # take upstream's side, then re-apply a carry it still needs
```

Then:

1. `just baseline base=upstream/main` records plain upstream's ledger. Run each carry's PHP
   program on that build.
2. Drop every carry whose program now passes on plain upstream. Re-apply any carry upstream's
   change displaced but did not replace.
3. `just check base=<new upstream base>` until it prints `check: PASS`.
4. Add a row (date, upstream base, carry count, survey) to the carry ledger in AIC's
   `docs/planning/native-compilation-roadmap.md`.

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
