# About this fork

This repository is Ignibyte's fork of [illegalstudio/elephc](https://github.com/illegalstudio/elephc),
the compiler the [AIC framework](https://github.com/Ignibyte/aic-nextgen) builds its native site
with. Its `main` is upstream `main` plus carry commits: fixes and features AIC needs that upstream
does not have yet. [FIXES.md](FIXES.md) lists every one, with the upstream pull request that
carries it where there is one. The goal is a fork with nothing left in that list.

Use upstream elephc for anything else. This fork follows upstream closely, but it is tested
against AIC's code, not released on its own.

## Build

The requirements are upstream's (README, "Requirements"): a Rust toolchain, and a native
assembler, linker and C toolchain for the host.

```sh
git clone https://github.com/Ignibyte/aic_compiler.git elephc
cd elephc
cargo build --release
ln -s "$PWD/target/release/elephc" ~/.local/bin/elephc   # or anywhere on PATH
```

Keep the binary in `target/release/`, beside the bridge archives the build writes there, and
link to it rather than copying it. AIC's gates look for `elephc` on `PATH` (or `ELEPHC_BIN`) and
install the native packages they pin (PCRE2, Oniguruma) on first use.

## History

- `aic/fork-final-0360718` tags the last commit of the earlier independent fork: 173 commits on
  top of upstream `release/0.26.1`. That line is also the second parent of the sync merge
  `61265dffe`, so `git log` still reaches it. None of its code is in the tree unless a carry
  commit brought it back.
- `61265dffe` is the sync merge of 2026-10-17. Its tree is upstream `ecc48af08` exactly. Since
  then `main` has been upstream `main` plus carries, synced again on every upstream release.

## Carry commits

Every commit on top of upstream is a carry commit. Each one:

1. fixes a behaviour upstream gets wrong or refuses, with a PHP program that shows it;
2. adds tests in upstream's own layout and passes upstream's suite.

A carry does not have to be written as an upstream pull request. It may re-port the earlier
fork's own fix or carry a policy upstream has not adopted. The tests are what let a sync tell
when upstream covers a carry: a carry whose program passes on plain upstream is dropped at that
sync, not merged around. Where upstream changed the same code but still fails the program, the
carry is re-applied on top of upstream's version.

Only a fix that is solid (the full check passes, with a program whose output matches `php`'s
byte for byte) and about PHP rather than shaped around AIC goes upstream, one narrow fix per pull
request. Upstream PRs come from [chadmandoo/elephc](https://github.com/chadmandoo/elephc).

Carry commits still follow upstream's conventions (`AGENTS.md`, "Conventions"), so any of them
can go upstream later without a rewrite: a `fix:` or `feat:` prefix, a short message, no
`Co-Authored-By` line, and no `CHANGELOG.md` entry. When the change alters what a page in
`docs/internals/` says, the same commit updates it.

## How a carry is made

A carry goes through four phases: Plan, Code, Quality checks and Documentation. The checks are
scripts kept outside this repository, because a carry branch starts from upstream and would not
contain them.

1. **Plan.** After `git fetch upstream`, reproduce the bug on plain upstream with a PHP program,
   and run the program under `php` for the expected output. Find the cause.
2. **Code.** Branch `fix/<slug>` from `upstream/main`. Write the fix and a regression test in
   upstream's layout (`tests/codegen/`, `compile_and_run`, the heap-debug helpers when ownership
   is involved), following `AGENTS.md` and `CONTRIBUTING.md`. Prove the test bites: with `src/`
   reverted to the merge base, at least one new test must fail.
3. **Quality checks.** All of these must pass:
   - the debug build, with no warnings (upstream CI's gate), and `git diff --check`;
   - three ledgers diffed against the ledger of the branch's upstream base: a survey that compiles
     each of AIC's classes, a battery of earlier carries' programs, and a set of older repros. A
     row that was OK and is not any more fails the check;
   - the full debug suite. Each failure is rerun alone and judged a flake (passes alone), an
     environment failure (a known list, such as tests that need network) or a real failure.
4. **Documentation and delivery.**
   - Open a PR from the branch into this fork's `main` and merge it with a merge commit, so the
     fork holds the commit as written. A branch made from a newer `upstream/main` brings those
     upstream commits along, so that merge also counts as a sync.
   - Regenerate [FIXES.md](FIXES.md).
   - For a solid, PHP-generic fix, push the branch to `chadmandoo/elephc` and open the PR on
     `illegalstudio/elephc`: the repro, `php`'s output and ours, the cause, the fix, and the check
     results.

## Syncing with upstream

Sync on every upstream release, or monthly, whichever comes first. Each sync is also the
re-evaluation of every carry.

```sh
git fetch upstream
git switch -c sync/upstream-<sha> main
git merge upstream/main          # take upstream's side, then re-apply a carry it still needs
```

Then build plain upstream, run each carry's PHP program on it, drop every carry whose program now
passes, re-apply any carry upstream's change displaced but did not replace, run the full check
against the new base, and regenerate [FIXES.md](FIXES.md).

Run the suite in the debug profile. `cargo test --release` hangs on a `#[should_panic]` test
whose `debug_assert!` is compiled out. Build the bridge crates together first and set
`ELEPHC_TEST_PREBUILT_BRIDGES=1`, as upstream CI does. Otherwise the harness rebuilds a stale
bridge on its own, that copy resolves other features than the one inside
`libelephc_magician.a`, and the `pcntl` tests fail at link time with duplicate symbols.
