# PR #1700: caught compound append operations

- [x] Reserve compound append slots after the eager RHS and before the operation.
- [x] Retain selected ordinary cells across callbacks without PHP reference flags.
- [x] Preserve cancellation, COW, exception cleanup and target-aware lowering.
- [x] Integrate main through `9d76ca1c3cf49bda5349337601617fa54b41f1e6`.
- [x] Publish plain nested-write parents before runtime key conversion, without missing-key reads.
- [x] Validate the new plain-write callback regressions on the final candidate.
- [x] Continue abandoned intermediate selections without changing saved PHP copies.
- [x] Validate intermediate/final conversion and read loss, exceptions, ownership and all targets.
- [ ] Obtain clean Grok, Greptile 5/5 and complete green CI on the same published head.

## Implementation and historical validation

Original reviewed head: `5d6b9b41fd2c41973ae50609eedf72508937787c`.

The reviewed compiler evaluates a compound append before creating its slot. A caught
DivisionByZeroError therefore leaves an empty array where PHP preserves one null element.
The fix evaluates the eager RHS first, reserves the slot, then applies the operation.

Nested updates capture the ordinary stored Mixed cells before key diagnostics can invoke
user code. Compiler-only generated RefAssign markers describe these leases without setting
PHP reference-entry flags. Their EIR slots own the actual boxes, remain visible to exception
cleanup, and are retired in reverse order. Stores replace the selected box's payload instead
of replaying a path into a parent the callback may have replaced.

Explicit keys are normalized once through the ordinary array writer. Generated helper calls
have distinct synthetic spans so checked-call result caches cannot confuse their identities.
Intermediate stores preserve sparse keys; leaf reads keep missing-key diagnostics and restore
null after the existing writable-fetch autovivification, before the operation runs.

Two related ownership paths are necessary for the callback cases: array reads pin an owning
receiver over diagnostic callbacks, and nested assignments release every owned RHS temporary
after the setter retains/boxes its value (including fresh arrays, rather than strings alone).
String-offset branches share this final cleanup to avoid releasing an RHS twice.

Historical validation of the original reviewed implementation, not a substitute for the
new candidate's focused tests and exact-head CI:
- Warning-free `cargo build` and `git diff --check`.
- 102 focused append codegen tests; 7 append parser tests.
- 21 COW/cycle controls, 19 checked-operand regressions, 1 indexed-read lifetime control.
- All 3 new regression tests also pass with stack register allocation.
- 20 PHP fixtures, IR optimization on/off, executed on macOS ARM64, Linux x86_64 and
  Linux ARM64: 120 complete-stdout/warning-count comparisons, all heap-clean.
- Direct, nested and reentrant-handler fixtures with optimization on/off assemble as
  static libraries for iOS device and Simulator: 24 program/runtime objects; program runtime
  references all have emitted definitions. iOS binaries were not executed.

All local package entrypoints were forced fresh before freezing the baseline, avoiding shared
Cargo target artifacts from another worktree. Earlier prototypes and invalid runs remain in
`/private/tmp/elephc-1700-review`; their evidence does not substitute for the final checks above.
The full test suite and complete CI matrix were not run locally.

## Plain-write key diagnostics

Plain nested writes with runtime-converted keys reserve their slot and publish each parent
after the eager RHS, before converting the next key. A compiler-private unread guard probes
the selected cell after conversion; EIR consumes the marker without creating another owner.
An abandoned final selection yields null, while a surviving PHP copy receives the write. Plain
writes never read a missing child just to obtain this guard. Compound missing-key diagnostics
and the existing property-suffix Error path keep their separate behavior.

The published `b2f4f131bfbc5c3af3882ed3264e3647a4fa0eab` passed 118 focused append codegen tests, including plain callback
publication, throwing handlers, copy survival, cancellation and multilevel conversion with
IR optimization on and off. Five expanded target fixtures preserve the private guard through
AST optimization and emit valid assembly. Eight parser tests, a warning-free build and
`git diff --check` also pass. Final independent reviews and CI are still required after push.

## Intermediate continuation follow-up

Independent Grok review found that the published head cancels too early when a callback
abandons a non-final dimension. Six new regression tests fail on that head: the ordinary
value and later key diagnostics are lost, while final conversion loss warns once too often.

The local repair distinguishes unread pre-conversion probes from post-read probes, and
intermediate from final places. A first intermediate loss publishes a fresh Mixed array or
hash into the compiler lease before releasing the old box. A separate keeper protects that
scratch across child selection. The remaining expression bypasses owner probes and continues
on private storage. Conversion loss skips only the current intermediate read; final conversion
loss cancels before the missing-leaf warning. The owner-count predicate and selected-payload
runtime writers are unchanged. The new candidate passed all 128 append codegen tests,
including ten new strict-heap controls with IR optimization on and off, nine parser tests,
all five target fixtures, the macOS cross-atom dead-strip check, a warning-free build and
diff hygiene. Fresh independent review and complete CI are still required after publication.
