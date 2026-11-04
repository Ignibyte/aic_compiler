# Fixes not upstream yet

This fork carries 242 fix and feature commits that upstream elephc (`9267616a00`) does not have.
18 of them are in an open pull request on [illegalstudio/elephc](https://github.com/illegalstudio/elephc/pulls); the rest are not sent.
The list should shrink: a row leaves when upstream merges the fix, or fixes the same bug its own way,
and the fork drops its copy at the next sync.

Generated from `git log upstream/main..main` and our upstream pull requests. Do not edit by hand.

| Commit | Kind | Area | Change | Upstream |
|---|---|---|---|---|
| [`5c880ac9f1`](https://github.com/Ignibyte/aic_compiler/commit/5c880ac9f1) | feat | - | parse typed class constants (PHP 8.3 const TYPE NAME) — re-port of #465 onto v0.26.1 | not sent |
| [`f89d2b345d`](https://github.com/Ignibyte/aic_compiler/commit/f89d2b345d) | fix | - | mb_ereg_match/mb_strlen-style coercible subject accept (re-port R2, elephc #591) | not sent |
| [`6626ed1288`](https://github.com/Ignibyte/aic_compiler/commit/6626ed1288) | feat | - | re-port mb_strlen() builtin onto v0.26.1 (R3, elephc #592) | not sent |
| [`5052070743`](https://github.com/Ignibyte/aic_compiler/commit/5052070743) | feat | checker | re-port comparison + return-type completeness onto v0.26.1 | not sent |
| [`439bd52b50`](https://github.com/Ignibyte/aic_compiler/commit/439bd52b50) | feat | parser,checker | re-port enum + use-const parse edges onto v0.26.1 (R6/#595) | not sent |
| [`e052354053`](https://github.com/Ignibyte/aic_compiler/commit/e052354053) | feat | checker | re-port covariance family onto v0.26.1 (R5/#594) | not sent |
| [`2a03280119`](https://github.com/Ignibyte/aic_compiler/commit/2a03280119) | feat | types | re-port object/Closure pseudo-type hints onto v0.26.1 (R4/#593, #467) | not sent |
| [`3caed1f84e`](https://github.com/Ignibyte/aic_compiler/commit/3caed1f84e) | feat | - | re-port PHP clone onto v0.26.1 — parser, checker, EIR shallow copy (R9/#599, #472) | not sent |
| [`3899314d3e`](https://github.com/Ignibyte/aic_compiler/commit/3899314d3e) | feat | checker | unknown-array foreach keys + Mixed array_merge arg (R9 tail) | not sent |
| [`23c6f46460`](https://github.com/Ignibyte/aic_compiler/commit/23c6f46460) | feat | prelude | re-port the stdlib prelude onto v0.26.1 (R3/#592) | not sent |
| [`061aaab0d8`](https://github.com/Ignibyte/aic_compiler/commit/061aaab0d8) | feat | prelude,types | complete R3 stdlib surface — get_debug_type, filter/url fns + constants (#592) | not sent |
| [`13a3cdcd51`](https://github.com/Ignibyte/aic_compiler/commit/13a3cdcd51) | feat | checker | self/static return normalization + exception $previous (R7 tail) | not sent |
| [`1196ca147e`](https://github.com/Ignibyte/aic_compiler/commit/1196ca147e) | feat | checker,traits | static-widened ancestor return + PHP 8.2 trait constants (R7 tail) | not sent |
| [`20ca5bdf6d`](https://github.com/Ignibyte/aic_compiler/commit/20ca5bdf6d) | feat | builtins | preg_match_all accepts the $matches out-param + flags (R7 tail, #558-adjacent) | not sent |
| [`4985249f4d`](https://github.com/Ignibyte/aic_compiler/commit/4985249f4d) | feat | oop | re-port `$expr::class` dynamic class-name-of onto v0.26.1 (W1/#600) | not sent |
| [`22d434a6c6`](https://github.com/Ignibyte/aic_compiler/commit/22d434a6c6) | feat | checker | covariance family batch 2 (whole-codebase) — array_all/any + array<Object> param + self/static property assign (W2/#601) | not sent |
| [`eb3d5a660c`](https://github.com/Ignibyte/aic_compiler/commit/eb3d5a660c) | feat | checker | array<Object> param accepts any-object-element array (W2 tail) | not sent |
| [`8356da8b83`](https://github.com/Ignibyte/aic_compiler/commit/8356da8b83) | feat | prelude | add strspn + mb_substr_count (W3 prelude adds) | not sent |
| [`88adfa0f5a`](https://github.com/Ignibyte/aic_compiler/commit/88adfa0f5a) | feat | builtins | re-port strtr/random_bytes/getmypid/assert/mb_strpos + SORT_*/PASSWORD_* (W3/#602) | not sent |
| [`f516338b2a`](https://github.com/Ignibyte/aic_compiler/commit/f516338b2a) | feat | parser | use const PHP_EOL + trailing comma in closure use() list (W5/#604) | not sent |
| [`0bfe7794d0`](https://github.com/Ignibyte/aic_compiler/commit/0bfe7794d0) | feat | checker | register UnitEnum + BackedEnum builtin interfaces (W4/#603) | not sent |
| [`10c6d4ce99`](https://github.com/Ignibyte/aic_compiler/commit/10c6d4ce99) | feat | checker | W6 tail — array_filter Mixed source, yield-from iterable, generator return-path (W6/#605) | not sent |
| [`c0fa969fd0`](https://github.com/Ignibyte/aic_compiler/commit/c0fa969fd0) | fix | runtime | recognize boxed callable descriptors (tag 10) in is_callable(mixed) | not sent |
| [`6c0f1d924b`](https://github.com/Ignibyte/aic_compiler/commit/6c0f1d924b) | fix | codegen | stamp callable-array value_type 10 + narrow is_callable() to Callable | not sent |
| [`9f52b0d194`](https://github.com/Ignibyte/aic_compiler/commit/9f52b0d194) | fix | checker | treat break/continue as diverging for guard complement narrowing | not sent |
| [`5b4ce3cf25`](https://github.com/Ignibyte/aic_compiler/commit/5b4ce3cf25) | feat | checker+codegen | is_array() narrowing + array_keys() on boxed Mixed | not sent |
| [`5264499d42`](https://github.com/Ignibyte/aic_compiler/commit/5264499d42) | feat | checker | narrow through compound `A \|\| B` type guards | not sent |
| [`128aa0858f`](https://github.com/Ignibyte/aic_compiler/commit/128aa0858f) | feat | checker | narrow match(true) arms guarded by instanceof / is_* predicates | not sent |
| [`1661d4680d`](https://github.com/Ignibyte/aic_compiler/commit/1661d4680d) | fix | checker | allow `?Closure` (nullable Closure) as a property type | not sent |
| [`444e4e6043`](https://github.com/Ignibyte/aic_compiler/commit/444e4e6043) | feat | codegen | support class_exists() with a runtime (dynamic) class name | not sent |
| [`00e918dd1b`](https://github.com/Ignibyte/aic_compiler/commit/00e918dd1b) | fix | checker | enums implicitly implement UnitEnum / BackedEnum | not sent |
| [`6c77e00b45`](https://github.com/Ignibyte/aic_compiler/commit/6c77e00b45) | feat | pdo | PDO::ATTR_DEFAULT_FETCH_MODE + PDOStatement::closeCursor | not sent |
| [`da324bc910`](https://github.com/Ignibyte/aic_compiler/commit/da324bc910) | fix | parser | accept eager-tokenized constants in `use const` / `use function` | not sent |
| [`0820a3934d`](https://github.com/Ignibyte/aic_compiler/commit/0820a3934d) | feat | parser | long `array(...)` syntax + comma-operator `for` clauses | not sent |
| [`b30d681277`](https://github.com/Ignibyte/aic_compiler/commit/b30d681277) | feat | pdo | PDO::ATTR_STRINGIFY_FETCHES constant | not sent |
| [`749fa70ad7`](https://github.com/Ignibyte/aic_compiler/commit/749fa70ad7) | feat | strtr | two-argument replacement-map form strtr($str, $pairs) | not sent |
| [`388060d64c`](https://github.com/Ignibyte/aic_compiler/commit/388060d64c) | feat | explode | three-argument element-cap form explode($sep, $str, $limit) | not sent |
| [`a353fa5a68`](https://github.com/Ignibyte/aic_compiler/commit/a353fa5a68) | feat | builtins | array_search 3-arg (strict) + strpos 3-arg (offset) | not sent |
| [`573f574740`](https://github.com/Ignibyte/aic_compiler/commit/573f574740) | feat | strcspn | add the missing strcspn($subject, $characters) builtin | not sent |
| [`dcbc81908e`](https://github.com/Ignibyte/aic_compiler/commit/dcbc81908e) | feat | base64 | two-argument base64_decode($string, $strict) | not sent |
| [`72e3142e51`](https://github.com/Ignibyte/aic_compiler/commit/72e3142e51) | fix | checker | type builtin Exception::__construct $previous as ?Throwable | not sent |
| [`6d8e13faa2`](https://github.com/Ignibyte/aic_compiler/commit/6d8e13faa2) | feat | constants | fold PHP_VERSION and PHP_BINARY magic constants | not sent |
| [`0a9e4e6ca1`](https://github.com/Ignibyte/aic_compiler/commit/0a9e4e6ca1) | feat | name-resolver | desugar pack('H*', $data) to hex2bin | not sent |
| [`ffc88aecec`](https://github.com/Ignibyte/aic_compiler/commit/ffc88aecec) | feat | prelude | provide substr_compare() via a dedicated prelude | not sent |
| [`4d6055f0d6`](https://github.com/Ignibyte/aic_compiler/commit/4d6055f0d6) | feat | prelude | extended-arity mkdir($dir, $mode, $recursive) | not sent |
| [`c3ed34a3c1`](https://github.com/Ignibyte/aic_compiler/commit/c3ed34a3c1) | feat | checker | resolve ->value/->name on BackedEnum/UnitEnum-typed values | not sent |
| [`7eb743ba3a`](https://github.com/Ignibyte/aic_compiler/commit/7eb743ba3a) | feat | prelude | provide preg_quote($str[, $delimiter]) | not sent |
| [`c071ffc9c2`](https://github.com/Ignibyte/aic_compiler/commit/c071ffc9c2) | feat | prelude | provide a minimal Random\Randomizer class | not sent |
| [`ae6cd03731`](https://github.com/Ignibyte/aic_compiler/commit/ae6cd03731) | feat | prelude | provide strncmp() and strncasecmp() | not sent |
| [`7e1d561ca7`](https://github.com/Ignibyte/aic_compiler/commit/7e1d561ca7) | fix | ir_lower | box enum-case array-literal elements as Mixed, not Str | not sent |
| [`d75cc87648`](https://github.com/Ignibyte/aic_compiler/commit/d75cc87648) | fix | codegen | accept the padded const-null $previous in builtin-throwable ctors | not sent |
| [`4af6dc8fe3`](https://github.com/Ignibyte/aic_compiler/commit/4af6dc8fe3) | fix | ir_lower | box match results as Mixed when a null arm mixes with value arms | not sent |
| [`4a58a0c876`](https://github.com/Ignibyte/aic_compiler/commit/4a58a0c876) | feat | prelude | provide addcslashes($str, $characters) | not sent |
| [`8544b67c57`](https://github.com/Ignibyte/aic_compiler/commit/8544b67c57) | fix | types | widen preg_match_all legacy golden to the 4-param registry shape | not sent |
| [`e49910105a`](https://github.com/Ignibyte/aic_compiler/commit/e49910105a) | feat | types+codegen | P5 keystone — untyped params default Mixed + Mixed-param callable/stream ABI (#621) | not sent |
| [`8fea2a7e4a`](https://github.com/Ignibyte/aic_compiler/commit/8fea2a7e4a) | fix | codegen | interface dispatch must not clobber r8/r9 arg registers (#627) | not sent |
| [`7f81dfb192`](https://github.com/Ignibyte/aic_compiler/commit/7f81dfb192) | fix | types | @return static resolves to receiver + local reassign-to-supertype (#622) | not sent |
| [`a4eb4873be`](https://github.com/Ignibyte/aic_compiler/commit/a4eb4873be) | fix | types | exception-subclass ctor param types from child decl, not parent by position (#624) | not sent |
| [`48f438c1f5`](https://github.com/Ignibyte/aic_compiler/commit/48f438c1f5) | fix | name-resolver | resolve use-aliases in dynamic `new $class(...)` arguments (#625) | not sent |
| [`c16a68d735`](https://github.com/Ignibyte/aic_compiler/commit/c16a68d735) | fix | name-resolver | resolve use-aliases in typed variadic parameters (#625) | not sent |
| [`7bc2bc98fb`](https://github.com/Ignibyte/aic_compiler/commit/7bc2bc98fb) | feat | codegen | dispatch method calls on the checker's instanceof-narrowed receiver type (#632) | not sent |
| [`5e2bc4a2f8`](https://github.com/Ignibyte/aic_compiler/commit/5e2bc4a2f8) | fix | ir_lower | type ?:/match result from call arms, not syntactic Int (#635) | not sent |
| [`aef031e398`](https://github.com/Ignibyte/aic_compiler/commit/aef031e398) | feat | parser | first-class callable from a value callee — <expr>(...) (#613) | not sent |
| [`0ab851ab3b`](https://github.com/Ignibyte/aic_compiler/commit/0ab851ab3b) | feat | builtins | re-port memory_get_usage / memory_get_peak_usage (#555) | not sent |
| [`ec388f8059`](https://github.com/Ignibyte/aic_compiler/commit/ec388f8059) | feat | parser | array-append []= — auto-vivify + expression position (#552) | not sent |
| [`76c088365f`](https://github.com/Ignibyte/aic_compiler/commit/76c088365f) | feat | closures | Closure::fromCallable() for every callable form (#605) | not sent |
| [`671b721fc0`](https://github.com/Ignibyte/aic_compiler/commit/671b721fc0) | fix | types | is_subclass_of/is_a resolve a ::class target (#636) | not sent |
| [`894dab379a`](https://github.com/Ignibyte/aic_compiler/commit/894dab379a) | feat | parser | foreach with a [...] list-destructuring value pattern (#613) | not sent |
| [`b3ea78f3c0`](https://github.com/Ignibyte/aic_compiler/commit/b3ea78f3c0) | fix | types | runtime is_subclass_of($clsName, $targetName) hierarchy walk (#636) | not sent |
| [`8e185f94f4`](https://github.com/Ignibyte/aic_compiler/commit/8e185f94f4) | fix | types | declared generic `array` property adopts AssocArray storage on plain assign (#607) | not sent |
| [`fb8da1cb3b`](https://github.com/Ignibyte/aic_compiler/commit/fb8da1cb3b) | fix | types | undeclared return on a body-less method infers Mixed, not Int (#607) | not sent |
| [`28cad1f1cd`](https://github.com/Ignibyte/aic_compiler/commit/28cad1f1cd) | fix | parser | assignment binds to its trailing target inside a binary expression (#613) | not sent |
| [`98bf4a2668`](https://github.com/Ignibyte/aic_compiler/commit/98bf4a2668) | fix | prelude | get_debug_type uses $value::class, not get_class($mixed) (#638) | not sent |
| [`340bcc8450`](https://github.com/Ignibyte/aic_compiler/commit/340bcc8450) | fix | codegen | mb_ereg_match / preg_* accept a Mixed subject (array keys) (#639) | not sent |
| [`9642d40cec`](https://github.com/Ignibyte/aic_compiler/commit/9642d40cec) | fix | parser | a leading backslash on a global constant is a namespace no-op (#647) | not sent |
| [`48e100d141`](https://github.com/Ignibyte/aic_compiler/commit/48e100d141) | fix | codegen | store/load a nullable typed property (?int) (#641) | not sent |
| [`af58b2aedb`](https://github.com/Ignibyte/aic_compiler/commit/af58b2aedb) | fix | codegen | runtime_call converts a Mixed value to a nullable scalar (#644) | not sent |
| [`a073ebe8a0`](https://github.com/Ignibyte/aic_compiler/commit/a073ebe8a0) | fix | codegen | string-backed enum from/tryFrom accepts a Mixed input (#646) | not sent |
| [`d75f759726`](https://github.com/Ignibyte/aic_compiler/commit/d75f759726) | fix | checker | empty-array `[]` parameter specialization — no-pin + map-arg compat (#645) | not sent |
| [`3a8590ea1d`](https://github.com/Ignibyte/aic_compiler/commit/3a8590ea1d) | fix | codegen | array_key_exists on Array(Mixed) via runtime kind dispatch (#640) | not sent |
| [`010ac66484`](https://github.com/Ignibyte/aic_compiler/commit/010ac66484) | fix | codegen | array === [] compares by emptiness, not pointer identity (#642) | not sent |
| [`63905616cc`](https://github.com/Ignibyte/aic_compiler/commit/63905616cc) | fix | codegen | inline builtin-throwable parent::__construct (#628) | not sent |
| [`1eaaa90d87`](https://github.com/Ignibyte/aic_compiler/commit/1eaaa90d87) | fix | codegen | deep === for populated Array(Mixed) via __rt_array_strict_eq (#642) | not sent |
| [`62a10747a5`](https://github.com/Ignibyte/aic_compiler/commit/62a10747a5) | fix | codegen | coerce array/Traversable-object to iterable in runtime_call (#644) | not sent |
| [`66f9317237`](https://github.com/Ignibyte/aic_compiler/commit/66f9317237) | fix | codegen | coerce indexed array to AssocArray storage in runtime_call (#648) | not sent |
| [`6073166322`](https://github.com/Ignibyte/aic_compiler/commit/6073166322) | fix | usort | sort array/iterable/Mixed-element arrays + validate array-typed comparator params (#643) | not sent |
| [`bd7ecb480e`](https://github.com/Ignibyte/aic_compiler/commit/bd7ecb480e) | fix | spread | unpack a Mixed spread source into a variadic pack instead of passing the box (#623 spread half) | not sent |
| [`1e45574fe9`](https://github.com/Ignibyte/aic_compiler/commit/1e45574fe9) | fix | callable | dispatch a Mixed callee by runtime type to string/array/closure handlers (#623 callable half) | not sent |
| [`bd489de1cd`](https://github.com/Ignibyte/aic_compiler/commit/bd489de1cd) | fix | checker | admit a Mixed/Union callee, backed by the runtime callable dispatch (#623) | not sent |
| [`3b9f898ba0`](https://github.com/Ignibyte/aic_compiler/commit/3b9f898ba0) | fix | array-predicate | allow Mixed-element indexed arrays in array_any/array_all (#648) | not sent |
| [`b49fe2e61e`](https://github.com/Ignibyte/aic_compiler/commit/b49fe2e61e) | fix | array_slice | support string-element indexed arrays via a persisting Str slicer (#648) | not sent |
| [`820e64da0d`](https://github.com/Ignibyte/aic_compiler/commit/820e64da0d) | fix | array_merge | support string-element arrays + coerce a Mixed operand (#648) | not sent |
| [`b76da1496e`](https://github.com/Ignibyte/aic_compiler/commit/b76da1496e) | fix | str_replace | support an array `search` over a scalar string subject (#648) | not sent |
| [`0f3d78fd04`](https://github.com/Ignibyte/aic_compiler/commit/0f3d78fd04) | fix | runtime_call | coerce an associative array to array<mixed> as a representation-identity widening (#648) | not sent |
| [`8342a52bee`](https://github.com/Ignibyte/aic_compiler/commit/8342a52bee) | fix | ir-lower | lower an unreachable array-return fall-through to an empty array, not a 0-operand runtime_call (#648) | not sent |
| [`e23615bd2d`](https://github.com/Ignibyte/aic_compiler/commit/e23615bd2d) | fix | ir-lower | type an associative-array literal's method-call values as Mixed, not the Int fallback (#648) | not sent |
| [`17032df469`](https://github.com/Ignibyte/aic_compiler/commit/17032df469) | fix | ir-lower | type a static-call arm by its declared return type in match/ternary merges (#648) | not sent |
| [`f6fd1a48a6`](https://github.com/Ignibyte/aic_compiler/commit/f6fd1a48a6) | fix | spaceship | order string operands lexicographically instead of rejecting them (#648) | not sent |
| [`d7fba4d5bb`](https://github.com/Ignibyte/aic_compiler/commit/d7fba4d5bb) | fix | foreach | yield typed object elements by value instead of erasing to Mixed (#648) | not sent |
| [`2033eeb6e9`](https://github.com/Ignibyte/aic_compiler/commit/2033eeb6e9) | fix | array_map | type the result by the callback return, not the source element (#643) | not sent |
| [`bac01bf822`](https://github.com/Ignibyte/aic_compiler/commit/bac01bf822) | fix | dynamic-new | exclude type-incompatible ctor candidates instead of emitting impossible arg conversions (#644) | not sent |
| [`f18f985768`](https://github.com/Ignibyte/aic_compiler/commit/f18f985768) | fix | ir-lower | type a backed-enum-case match/ternary arm as the case object, not its backing string (#644) | not sent |
| [`f81b1110c1`](https://github.com/Ignibyte/aic_compiler/commit/f81b1110c1) | fix | ir-lower | type an indexed array-literal method-call element by the callee return, not the Int default (#644) | not sent |
| [`83e0dd21a6`](https://github.com/Ignibyte/aic_compiler/commit/83e0dd21a6) | fix | ir-lower | lower an object-returning function's unreachable fall-through to a null sentinel, not a 0-operand runtime_call (#644) | not sent |
| [`4d000624e7`](https://github.com/Ignibyte/aic_compiler/commit/4d000624e7) | fix | ir-lower | resolve a property-access method-call receiver's class for array-element typing (#644) | not sent |
| [`fcbc8449cf`](https://github.com/Ignibyte/aic_compiler/commit/fcbc8449cf) | fix | usort | sort a boxed-Mixed source by unboxing to its underlying indexed array (#643) | not sent |
| [`40e52f4d63`](https://github.com/Ignibyte/aic_compiler/commit/40e52f4d63) | fix | in_array | support a boxed-Mixed needle under strict membership (#623) | not sent |
| [`d205a0d52a`](https://github.com/Ignibyte/aic_compiler/commit/d205a0d52a) | fix | arrays | correct by-key reads and isset() on a hash-promoted array<Mixed> (#640) | not sent |
| [`6990fb0c45`](https://github.com/Ignibyte/aic_compiler/commit/6990fb0c45) | feat | array_fill_keys | support boxed-Mixed key elements with PHP's own key cast (#648) | not sent |
| [`2f74c278ae`](https://github.com/Ignibyte/aic_compiler/commit/2f74c278ae) | fix | checker | flow-narrow `!== null` guards and asserts; fix return-collection env leak (#652) | not sent |
| [`6966afd41b`](https://github.com/Ignibyte/aic_compiler/commit/6966afd41b) | fix | checker,codegen | short-circuit narrowing + unbox a narrowed nullable object arg (#652) | not sent |
| [`b9947f00c8`](https://github.com/Ignibyte/aic_compiler/commit/b9947f00c8) | fix | checker | narrow a `while` loop body by its condition guard (#652) | not sent |
| [`6082a7ba4e`](https://github.com/Ignibyte/aic_compiler/commit/6082a7ba4e) | fix | checker | preserve a property narrowing read as a nested-call argument (#653) | not sent |
| [`c4cc63bb1f`](https://github.com/Ignibyte/aic_compiler/commit/c4cc63bb1f) | fix | runtime | array_combine() throws ValueError on a length mismatch (#651 P1) | not sent |
| [`e6e49f1b37`](https://github.com/Ignibyte/aic_compiler/commit/e6e49f1b37) | fix | codegen | unbox a Mixed argument passed to an `array` parameter (#650) | not sent |
| [`faa3662405`](https://github.com/Ignibyte/aic_compiler/commit/faa3662405) | fix | checker | degrade a Mixed-keyed map's value type across the return boundary (#649) | not sent |
| [`c9f5b4473a`](https://github.com/Ignibyte/aic_compiler/commit/c9f5b4473a) | feat | throwable | store Exception/Error $previous on the compact payload (#628 part A) | not sent |
| [`96dc8f4828`](https://github.com/Ignibyte/aic_compiler/commit/96dc8f4828) | fix | ir | write nested $a[$i][$j] through the parent cell — no longer silently lost (#529) | not sent |
| [`f71a9074c4`](https://github.com/Ignibyte/aic_compiler/commit/f71a9074c4) | feat | array_filter | filter a boxed-Mixed source by unboxing to its indexed array (#648) | not sent |
| [`76b5bb6bba`](https://github.com/Ignibyte/aic_compiler/commit/76b5bb6bba) | feat | array_filter | filter an associative-array (hash) source, preserving keys (#648) | not sent |
| [`fb1311d11e`](https://github.com/Ignibyte/aic_compiler/commit/fb1311d11e) | feat | checker+ir | bridge instanceof-narrowing to property access — resolve slots on the narrowed concrete class (#654) | not sent |
| [`721f9f70f8`](https://github.com/Ignibyte/aic_compiler/commit/721f9f70f8) | feat | ir | unset() an associative-array element of an object property — publish the mutated table back (#655) | not sent |
| [`411573442a`](https://github.com/Ignibyte/aic_compiler/commit/411573442a) | feat | checker+codegen | usort() with an opaque runtime Closure comparator + instanceof Closure (#664) | not sent |
| [`30e517b07f`](https://github.com/Ignibyte/aic_compiler/commit/30e517b07f) | fix | ir-lower | release the boxed Mixed result of `$arr[k]` reads on array<mixed> receivers (#665) | not sent |
| [`6a26035ff3`](https://github.com/Ignibyte/aic_compiler/commit/6a26035ff3) | fix | ir-lower | release the intermediate receiver of a chained container read (#667) | not sent |
| [`4b44413436`](https://github.com/Ignibyte/aic_compiler/commit/4b44413436) | fix | codegen | stop leaking callback array-element args — borrow for scalar-return callbacks (#666) | not sent |
| [`79537c4a7e`](https://github.com/Ignibyte/aic_compiler/commit/79537c4a7e) | fix | codegen | var_dump an Array-typed null-container as NULL, not SIGSEGV (#581) | not sent |
| [`026075d9c7`](https://github.com/Ignibyte/aic_compiler/commit/026075d9c7) | fix | codegen | guard by-reference/by-value foreach over a null-container source (#556) | not sent |
| [`14a7baf4c8`](https://github.com/Ignibyte/aic_compiler/commit/14a7baf4c8) | fix | ir-lower | autovivify a missing intermediate on a chained array write (#555) | not sent |
| [`5a6b0d9abd`](https://github.com/Ignibyte/aic_compiler/commit/5a6b0d9abd) | fix | ir-lower | persist by-reference foreach writes through an array-element source (#580) | not sent |
| [`a41027aaa1`](https://github.com/Ignibyte/aic_compiler/commit/a41027aaa1) | fix | ir-lower | resolve self::/parent:: static calls in closures + first-class callables (#657) | not sent |
| [`6ec4f92454`](https://github.com/Ignibyte/aic_compiler/commit/6ec4f92454) | feat | builtins | sort() accepts the optional $flags argument (#668, sort half) | not sent |
| [`4c324043b7`](https://github.com/Ignibyte/aic_compiler/commit/4c324043b7) | fix | codegen | resolve get_parent_class() of a runtime class-name string (#658) | not sent |
| [`32129ed378`](https://github.com/Ignibyte/aic_compiler/commit/32129ed378) | fix | codegen | unify the Throwable $previous slot on the Mixed cell (#628) | not sent |
| [`df275f6599`](https://github.com/Ignibyte/aic_compiler/commit/df275f6599) | fix | codegen | unbox narrowed Mixed at string registers, ?int slice length, element-typed usort comparator (#669, #644, #648) | not sent |
| [`c17be7637f`](https://github.com/Ignibyte/aic_compiler/commit/c17be7637f) | fix | codegen | store an indexed array literal into a hash-backed property (#670) | not sent |
| [`a0f0b972a2`](https://github.com/Ignibyte/aic_compiler/commit/a0f0b972a2) | feat | resolver | fold __DIR__ in an include path (#582 H1) | not sent |
| [`746e93a13e`](https://github.com/Ignibyte/aic_compiler/commit/746e93a13e) | fix | ir_lower | stamp array_slice/array_keys results with the checker's element type (#671) | not sent |
| [`ff2e3ac87a`](https://github.com/Ignibyte/aic_compiler/commit/ff2e3ac87a) | fix | ir_lower | stop fabricating Int for unresolvable ternary-branch types (#648) | not sent |
| [`e58a4a6705`](https://github.com/Ignibyte/aic_compiler/commit/e58a4a6705) | fix | ir_lower | reconcile local type facts across if branches (#672, partial) | not sent |
| [`3677da5f11`](https://github.com/Ignibyte/aic_compiler/commit/3677da5f11) | fix | parser | reject array unpacking mixed with keyed elements instead of dropping it (#673) | not sent |
| [`11b9ae88b7`](https://github.com/Ignibyte/aic_compiler/commit/11b9ae88b7) | fix | checker | let a bare-`array` property adopt assoc storage from a typed assignment (#670) | not sent |
| [`4cfaf971fd`](https://github.com/Ignibyte/aic_compiler/commit/4cfaf971fd) | fix | ir_lower | type a promoted hash's keys like the checker does (#676) | not sent |
| [`888e1b9570`](https://github.com/Ignibyte/aic_compiler/commit/888e1b9570) | feat | codegen | allow an enum case as a property default (#675) | not sent |
| [`42d741cd0c`](https://github.com/Ignibyte/aic_compiler/commit/42d741cd0c) | feat | codegen | accept object elements in array_map (#674 arm 1) | not sent |
| [`383415a9ec`](https://github.com/Ignibyte/aic_compiler/commit/383415a9ec) | fix | ir_lower | branch `??` over an array offset on presence, not on the read value (#672) | not sent |
| [`4b4a1b3c06`](https://github.com/Ignibyte/aic_compiler/commit/4b4a1b3c06) | fix | linker | reconcile duplicate archive members for any two linked bridges (#677) | not sent |
| [`f20e1909e5`](https://github.com/Ignibyte/aic_compiler/commit/f20e1909e5) | fix | checker | derive the strict array_search return type from the haystack's keys (#679) | not sent |
| [`ea1ee8ec39`](https://github.com/Ignibyte/aic_compiler/commit/ea1ee8ec39) | fix | checker | keep a readonly property's narrowing across calls (#680 P1) | not sent |
| [`0f6e2b5322`](https://github.com/Ignibyte/aic_compiler/commit/0f6e2b5322) | feat | parser | accept include/require as an expression, delivered to a property assign (#613 Gap A) | not sent |
| [`b59afa6ab8`](https://github.com/Ignibyte/aic_compiler/commit/b59afa6ab8) | fix | codegen | select the usort wrapper by the source array's runtime cell format (#678) | not sent |
| [`f2319bf0e1`](https://github.com/Ignibyte/aic_compiler/commit/f2319bf0e1) | fix | codegen | implode() over a boxed-Mixed operand handles raw int-element arrays (#688) | not sent |
| [`e1c7285df5`](https://github.com/Ignibyte/aic_compiler/commit/e1c7285df5) | feat | array_merge | variadic + runtime spread array_merge() (#668) | not sent |
| [`1230df2aa4`](https://github.com/Ignibyte/aic_compiler/commit/1230df2aa4) | feat | diagnostics | name the function in location-less EIR backend errors | not sent |
| [`6a06569f14`](https://github.com/Ignibyte/aic_compiler/commit/6a06569f14) | fix | array_map | stamp the IR result element type for inline-closure callbacks | not sent |
| [`5e572ef5a1`](https://github.com/Ignibyte/aic_compiler/commit/5e572ef5a1) | feat | array_find | support string-element indexed arrays in array_find/any/all | not sent |
| [`ccee0a434f`](https://github.com/Ignibyte/aic_compiler/commit/ccee0a434f) | fix | compare | PHP 8 numeric-string semantics for string <,<=,>,>=,<=> (#691) | not sent |
| [`c19c54cc71`](https://github.com/Ignibyte/aic_compiler/commit/c19c54cc71) | fix | ctype | accept a guard-narrowed Mixed operand (#695 D2) | not sent |
| [`55e8f0ea1b`](https://github.com/Ignibyte/aic_compiler/commit/55e8f0ea1b) | fix | checker | instanceof narrows by intersection, not replacement (#695 D1) | not sent |
| [`f891d11016`](https://github.com/Ignibyte/aic_compiler/commit/f891d11016) | fix | checker+codegen | getCode() types int\|string; ctor code arg tag-check unboxes (#695 D4) | not sent |
| [`4bedc388a0`](https://github.com/Ignibyte/aic_compiler/commit/4bedc388a0) | fix | checker | array-element guard narrowing — literal-indexed keys on array roots (#695 D3) | not sent |
| [`9aa1921b91`](https://github.com/Ignibyte/aic_compiler/commit/9aa1921b91) | fix | checker | multi-condition match(true) arms narrow one receiver to the union; bridge shared-slot union property reads (#695 D5) | not sent |
| [`a05e1d7a3d`](https://github.com/Ignibyte/aic_compiler/commit/a05e1d7a3d) | fix | checker | array_keys over a Mixed-element array stamps Array(Mixed), mirroring ir_lower (#696) | not sent |
| [`d5eb4a2cf0`](https://github.com/Ignibyte/aic_compiler/commit/d5eb4a2cf0) | fix | checker | compound && conditions apply guard-conjunct narrowing to the branch (#695 drift pin) | not sent |
| [`716ca3e6e5`](https://github.com/Ignibyte/aic_compiler/commit/716ca3e6e5) | fix | - | widen a function-local hash before a write of another value type | not sent |
| [`641817e575`](https://github.com/Ignibyte/aic_compiler/commit/641817e575) | fix | - | keep x86_64 interface dispatch off the fifth and sixth argument registers | not sent |
| [`504c760634`](https://github.com/Ignibyte/aic_compiler/commit/504c760634) | fix | - | accept every PHP regex delimiter, not only the slash | [#1929](https://github.com/illegalstudio/elephc/pull/1929) |
| [`a9f1423e9b`](https://github.com/Ignibyte/aic_compiler/commit/a9f1423e9b) | fix | - | expand the braced ${n} backreferences in preg_replace replacements | [#1939](https://github.com/illegalstudio/elephc/pull/1939) |
| [`c40a3fc97c`](https://github.com/Ignibyte/aic_compiler/commit/c40a3fc97c) | fix | - | vivify a nested append's missing bucket without the undefined-key warning | not sent |
| [`77b1cd6c39`](https://github.com/Ignibyte/aic_compiler/commit/77b1cd6c39) | feat | - | add flush(), a no-op over elephc's unbuffered writes | [#1942](https://github.com/illegalstudio/elephc/pull/1942) |
| [`3af2ba7d5e`](https://github.com/Ignibyte/aic_compiler/commit/3af2ba7d5e) | fix | - | dispatch a method only the narrowed class declares on the runtime class | not sent |
| [`209e45918e`](https://github.com/Ignibyte/aic_compiler/commit/209e45918e) | fix | - | keep WeakMap's retire loop off a user get() method | [#1947](https://github.com/illegalstudio/elephc/pull/1947) |
| [`188469c005`](https://github.com/Ignibyte/aic_compiler/commit/188469c005) | fix | - | accept control and non-ASCII regex delimiters as PHP does | [#1929](https://github.com/illegalstudio/elephc/pull/1929) |
| [`f19e4ed777`](https://github.com/Ignibyte/aic_compiler/commit/f19e4ed777) | fix | - | warn for a nested append's key reads once and before the right-hand side | not sent |
| [`9dab31e876`](https://github.com/Ignibyte/aic_compiler/commit/9dab31e876) | fix | - | send queued headers on flush() under --web | [#1942](https://github.com/illegalstudio/elephc/pull/1942) |
| [`35a34cc4fd`](https://github.com/Ignibyte/aic_compiler/commit/35a34cc4fd) | fix | - | narrow logical operands as PHP flows them, and merge [] into any array | not sent |
| [`c585e1ae2c`](https://github.com/Ignibyte/aic_compiler/commit/c585e1ae2c) | fix | - | dispatch a narrowed receiver's method and property reads on the runtime class | not sent |
| [`69c99d1471`](https://github.com/Ignibyte/aic_compiler/commit/69c99d1471) | fix | - | move a file across filesystems in rename(), as PHP does | not sent |
| [`1e9952854b`](https://github.com/Ignibyte/aic_compiler/commit/1e9952854b) | feat | - | is_uploaded_file() and move_uploaded_file() under --web | not sent |
| [`1b00395274`](https://github.com/Ignibyte/aic_compiler/commit/1b00395274) | fix | ir-lower | store a tagged nullable int in an associative literal as Mixed | not sent |
| [`16800331cb`](https://github.com/Ignibyte/aic_compiler/commit/16800331cb) | fix | codegen | unbox a Mixed value into the tagged-scalar return shape | not sent |
| [`ed2b9f6508`](https://github.com/Ignibyte/aic_compiler/commit/ed2b9f6508) | fix | - | store a nullable int written into a hash element as Mixed | not sent |
| [`9c20a3a6f1`](https://github.com/Ignibyte/aic_compiler/commit/9c20a3a6f1) | fix | - | type a ?? arm of a ternary or match from both of its sides | not sent |
| [`dd3ea35e10`](https://github.com/Ignibyte/aic_compiler/commit/dd3ea35e10) | fix | - | accept array_merge() with any number of arrays | not sent |
| [`911bf91cb4`](https://github.com/Ignibyte/aic_compiler/commit/911bf91cb4) | fix | - | type a throw arm of a ternary or ?? as never | not sent |
| [`f2d7b7acc5`](https://github.com/Ignibyte/aic_compiler/commit/f2d7b7acc5) | fix | - | keep an early-exit narrowing inside its enclosing block | [#1964](https://github.com/illegalstudio/elephc/pull/1964) |
| [`e2cda0dc9b`](https://github.com/Ignibyte/aic_compiler/commit/e2cda0dc9b) | fix | - | narrow mixed with is_array() and a for body by its condition | not sent |
| [`9535ad1a5a`](https://github.com/Ignibyte/aic_compiler/commit/9535ad1a5a) | fix | - | keep an early-exit narrowing inside its enclosing block | [#1964](https://github.com/illegalstudio/elephc/pull/1964) |
| [`497adec117`](https://github.com/Ignibyte/aic_compiler/commit/497adec117) | fix | - | narrow a mixed value with is_array() | [#1964](https://github.com/illegalstudio/elephc/pull/1964) |
| [`43d5ba2f9a`](https://github.com/Ignibyte/aic_compiler/commit/43d5ba2f9a) | fix | - | check a mixed value at array and class parameter and return boundaries | not sent |
| [`18ad2bd538`](https://github.com/Ignibyte/aic_compiler/commit/18ad2bd538) | fix | - | undo kept narrowings per switch case and try body; keep them after a do-while body | not sent |
| [`1957c203a7`](https://github.com/Ignibyte/aic_compiler/commit/1957c203a7) | fix | - | keep kept narrowings after a try or switch when every path to its end shares them | not sent |
| [`3491166409`](https://github.com/Ignibyte/aic_compiler/commit/3491166409) | fix | - | preserve every callee-saved register across the --web handler | not sent |
| [`096a8008fa`](https://github.com/Ignibyte/aic_compiler/commit/096a8008fa) | fix | - | store a nullable int in an indexed array as Mixed | not sent |
| [`cfcd2f3b70`](https://github.com/Ignibyte/aic_compiler/commit/cfcd2f3b70) | fix | - | check catches and finally without a try body's kept facts | [#1964](https://github.com/illegalstudio/elephc/pull/1964) |
| [`2ca93b5254`](https://github.com/Ignibyte/aic_compiler/commit/2ca93b5254) | fix | - | keep the switch tail after a case with a nested break | not sent |
| [`64f64d1b20`](https://github.com/Ignibyte/aic_compiler/commit/64f64d1b20) | fix | - | call the runtime class's __toString() and count() implicitly | not sent |
| [`17eeb1bd04`](https://github.com/Ignibyte/aic_compiler/commit/17eeb1bd04) | fix | - | round number_format() halves away from zero | not sent |
| [`e0d52608be`](https://github.com/Ignibyte/aic_compiler/commit/e0d52608be) | fix | - | accept a mixed or nullable number in number_format() | not sent |
| [`469cb9af60`](https://github.com/Ignibyte/aic_compiler/commit/469cb9af60) | fix | - | re-check implode's destination room before copying a cast element on x86_64 | not sent |
| [`351d32bfe0`](https://github.com/Ignibyte/aic_compiler/commit/351d32bfe0) | fix | - | drop the second undo_kept_narrowings_since the sync merge brought in | not sent |
| [`2c00950f84`](https://github.com/Ignibyte/aic_compiler/commit/2c00950f84) | fix | - | decide a container's TaggedScalar lowering without lowering its element type | not sent |
| [`4da0671223`](https://github.com/Ignibyte/aic_compiler/commit/4da0671223) | fix | - | type an unresolved property read in an array literal as mixed | not sent |
| [`ae18dba8d6`](https://github.com/Ignibyte/aic_compiler/commit/ae18dba8d6) | fix | checker | narrow match(true) arms by their instanceof condition | not sent |
| [`b6bfd1d4ad`](https://github.com/Ignibyte/aic_compiler/commit/b6bfd1d4ad) | fix | checker | do not narrow property guards in match(true) arms | not sent |
| [`9f8dec58b5`](https://github.com/Ignibyte/aic_compiler/commit/9f8dec58b5) | fix | checker | keep match(true) narrowing for an arm that passes the guarded local to a call | not sent |
| [`d3616c01d0`](https://github.com/Ignibyte/aic_compiler/commit/d3616c01d0) | fix | - | lex ?> close tags, inline HTML and <?= echo tags in PHP files | not sent |
| [`9f6ddd7d8b`](https://github.com/Ignibyte/aic_compiler/commit/9f6ddd7d8b) | fix | - | htmlspecialchars applies ENT_SUBSTITUTE and ENT_IGNORE to invalid UTF-8 | not sent |
| [`f7981cf171`](https://github.com/Ignibyte/aic_compiler/commit/f7981cf171) | fix | - | bound the per-descriptor stream tables | not sent |
| [`f98c528837`](https://github.com/Ignibyte/aic_compiler/commit/f98c528837) | fix | - | give the EOF flags a slot for every openable descriptor | not sent |
| [`73b843e4fc`](https://github.com/Ignibyte/aic_compiler/commit/73b843e4fc) | fix | - | pass the fork's callee-saved flag to upstream's shared-helper frame layout | not sent |
| [`bad0bafed0`](https://github.com/Ignibyte/aic_compiler/commit/bad0bafed0) | fix | - | convert a mixed pattern or subject for preg_match, preg_match_all and preg_split | not sent |
| [`975e2eac07`](https://github.com/Ignibyte/aic_compiler/commit/975e2eac07) | fix | - | accept array_merge() with spread arguments | not sent |
| [`9f64cbca5a`](https://github.com/Ignibyte/aic_compiler/commit/9f64cbca5a) | fix | - | merge concrete associative and string arrays through the boxed array_merge path | not sent |
| [`b851e3bd00`](https://github.com/Ignibyte/aic_compiler/commit/b851e3bd00) | fix | - | reach the code after a loop that breaks before its body returns | not sent |
| [`1c9ef659d9`](https://github.com/Ignibyte/aic_compiler/commit/1c9ef659d9) | fix | - | type a ternary or match of enum cases by the cases, not as a string | not sent |
| [`8d7f576fc2`](https://github.com/Ignibyte/aic_compiler/commit/8d7f576fc2) | fix | - | keep a guard chain's first type, and no facts past a writing finally or an outer break | not sent |
| [`2401459073`](https://github.com/Ignibyte/aic_compiler/commit/2401459073) | fix | - | type a literal element called on a dynamic receiver by the shared return | not sent |
| [`81052a18f2`](https://github.com/Ignibyte/aic_compiler/commit/81052a18f2) | fix | - | keep a readonly narrowing across a call only for $this outside a cloning hierarchy | not sent |
| [`296a9abac9`](https://github.com/Ignibyte/aic_compiler/commit/296a9abac9) | fix | - | keep a promoted parameter's Core attribute only on the owner it targets | not sent |
| [`b7939ead35`](https://github.com/Ignibyte/aic_compiler/commit/b7939ead35) | fix | - | unite or refuse the arrays a mixed + - * meets | [#2217](https://github.com/illegalstudio/elephc/pull/2217) |
| [`e37d048cd7`](https://github.com/Ignibyte/aic_compiler/commit/e37d048cd7) | fix | - | check a value returned through a union with a scalar member | not sent |
| [`abc9baa2b1`](https://github.com/Ignibyte/aic_compiler/commit/abc9baa2b1) | fix | - | check a mixed argument at a declared scalar parameter | not sent |
| [`cbe1c00c9d`](https://github.com/Ignibyte/aic_compiler/commit/cbe1c00c9d) | fix | - | check a closure's nullable return at its declared boundary | not sent |
| [`e4264a8851`](https://github.com/Ignibyte/aic_compiler/commit/e4264a8851) | fix | - | leave a bare object parameter to the mixed boundary guard | not sent |
| [`ce82c9391a`](https://github.com/Ignibyte/aic_compiler/commit/ce82c9391a) | fix | - | check a strict caller's string argument without staging it | not sent |
| [`efe8f97792`](https://github.com/Ignibyte/aic_compiler/commit/efe8f97792) | fix | - | satisfy a declared Stringable with an implicit Stringable class | [#2213](https://github.com/illegalstudio/elephc/pull/2213) |
| [`7e00df7dcc`](https://github.com/Ignibyte/aic_compiler/commit/7e00df7dcc) | fix | - | unite a declared array operand of + with any array | [#2217](https://github.com/illegalstudio/elephc/pull/2217) |
| [`1e12f37684`](https://github.com/Ignibyte/aic_compiler/commit/1e12f37684) | fix | - | release an owned sprintf format after the call | [#2209](https://github.com/illegalstudio/elephc/pull/2209) |
| [`4e976b5a02`](https://github.com/Ignibyte/aic_compiler/commit/4e976b5a02) | fix | - | warn when a NAN is coerced to string | [#2210](https://github.com/illegalstudio/elephc/pull/2210) |
| [`dd95495052`](https://github.com/Ignibyte/aic_compiler/commit/dd95495052) | fix | - | make --heap-debug work under --web | [#2211](https://github.com/illegalstudio/elephc/pull/2211) |
| [`089c152e31`](https://github.com/Ignibyte/aic_compiler/commit/089c152e31) | fix | - | map a hash of objects or mixed values, and a list of objects | [#2212](https://github.com/illegalstudio/elephc/pull/2212) |
| [`6774b2aaef`](https://github.com/Ignibyte/aic_compiler/commit/6774b2aaef) | fix | - | convert a weak scalar argument at a builtin's string parameter | [#2214](https://github.com/illegalstudio/elephc/pull/2214) |
| [`bcbc9866fe`](https://github.com/Ignibyte/aic_compiler/commit/bcbc9866fe) | fix | - | take any array operand in array_intersect_key() and array_diff_key() | not sent |
| [`22b1a150d4`](https://github.com/Ignibyte/aic_compiler/commit/22b1a150d4) | fix | - | store a literal pattern's named groups in preg_match()'s matches | not sent |
| [`79745bef69`](https://github.com/Ignibyte/aic_compiler/commit/79745bef69) | fix | - | join local types after try/catch | not sent |
| [`d123c3386b`](https://github.com/Ignibyte/aic_compiler/commit/d123c3386b) | fix | - | widen an array local reassigned in a branch | not sent |
| [`2da6f91568`](https://github.com/Ignibyte/aic_compiler/commit/2da6f91568) | fix | - | compare array_diff() and array_intersect() values by string and keep keys | not sent |
| [`f48dd49a3f`](https://github.com/Ignibyte/aic_compiler/commit/f48dd49a3f) | fix | - | return false for malformed UTF-8 under /u and add preg_last_error() | not sent |
| [`87c0ee928a`](https://github.com/Ignibyte/aic_compiler/commit/87c0ee928a) | fix | - | bind widened override parameters by the body that runs (#52) | not sent |
| [`a3640af3d6`](https://github.com/Ignibyte/aic_compiler/commit/a3640af3d6) | fix | - | preg_replace() and preg_split() handle empty subjects and zero-length matches (#54) | not sent |
| [`0c07509a58`](https://github.com/Ignibyte/aic_compiler/commit/0c07509a58) | fix | - | preg_replace_callback() zero-length matches and preg_split() no-empty limits (#56) | not sent |
