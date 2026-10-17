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

## Syncing with upstream

```sh
git fetch upstream
git switch -c sync/upstream-<sha> main
git merge upstream/main          # resolve conflicts in upstream's favour
```

Then rebuild, rerun the suite and the downstream survey, and drop any carry commit whose
program now passes on plain upstream.

Build with a per-checkout target directory, so the bridge archives sit beside the binary:

```sh
CARGO_TARGET_DIR=/mnt/fast/target/elephc cargo build --release
```

Run the suite in the debug profile. `cargo test --release` hangs on a `#[should_panic]` test
whose `debug_assert!` is compiled out.
