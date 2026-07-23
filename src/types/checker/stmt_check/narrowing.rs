//! Purpose:
//! Flow-sensitive type narrowing for `if`/`else` branches guarded by type predicates.
//! Narrows a union- or mixed-typed variable to the guarded type in the matching branch.
//!
//! Called from:
//! - `crate::types::checker::stmt_check::control_flow` when checking `StmtKind::If`.
//!
//! Key details:
//! - Recognizes `is_int`/`is_float`/`is_string`/`is_bool`/`is_callable($var)` (and aliases) and `$var instanceof
//!   Class` guards, optionally negated with a leading `!`. Narrowing is applied to each clause in an
//!   if/elseif*/else chain (each subsequent clause, and the else, see the accumulated complement
//!   from previous guards). For a chain with no else where *every* clause body always diverges
//!   (return/throw/exit/die/never-function), the accumulated complement is applied to the statements
//!   after the entire if construct.
//! - Conservative: a concrete (non-union, non-mixed) type is left unchanged, and an empty narrowing
//!   result falls back to the original type, so valid code is never narrowed away to `Never`.

use crate::errors::CompileError;
use crate::names::{php_symbol_key, property_hook_get_method};
use crate::parser::ast::{BinOp, Expr, ExprKind, InstanceOfTarget, Stmt, StmtKind};
use crate::types::{PhpType, TypeEnv};

use super::super::Checker;

/// A detected type-guard narrowing: the guarded binding's env key and the types it takes in the
/// then-branch (guard true) and else-branch (guard false).
pub(crate) struct GuardNarrowing {
    /// `TypeEnv` key of the guarded binding: a variable name (without the leading `$`) or the
    /// synthetic property key from `narrowed_property_env_key`.
    pub var: String,
    /// Type the binding has where the guard is true.
    pub then_ty: PhpType,
    /// Type the binding has where the guard is false.
    pub else_ty: PhpType,
}

impl Checker {
    /// Detects a type-predicate guard in an `if`/ternary condition and computes the then/else
    /// narrowing for the guarded binding against the current environment. Handles the scalar
    /// `is_*` predicates, `is_null`, `instanceof Class`, and `===`/`!==` against `false` / `null`, each with an
    /// optional leading `!` that swaps the branches. The guarded receiver may be a variable
    /// (narrowed under its name) or a simple property access `$var->prop` / `$this->prop`
    /// (narrowed under a synthetic key that `infer_property_access_type` consults). Returns
    /// `Ok(None)` when the condition is not a recognized guard or the receiver's current type is
    /// unknown.
    pub(crate) fn guard_narrowing(
        &mut self,
        condition: &Expr,
        env: &TypeEnv,
    ) -> Result<Option<GuardNarrowing>, CompileError> {
        // Compound `A || B` guard: `if (A || B) { ... }` leaves the fall-through env with `!A && !B`.
        // When exactly one operand is a recognized type guard (the common
        // `!is_array($v) || array_is_list($v)` shape — ward-config Config::getSection), narrow the
        // fall-through by that operand's complement (its own `else_ty`); the guard-true body keeps
        // the binding's current (un-narrowed) type, which is sound whether or not the body diverges —
        // `keep_complement_after_if` in `control_flow` only carries the else type past the `if` when
        // every clause body diverges. Both-guards / neither-guard fall back to no narrowing.
        if let ExprKind::BinaryOp { left, op: BinOp::Or, right } = &condition.kind {
            let left_guard = self.guard_narrowing(left, env)?;
            let right_guard = self.guard_narrowing(right, env)?;
            return Ok(match (left_guard, right_guard) {
                (Some(guard), None) | (None, Some(guard)) => {
                    let current = env.get(&guard.var).cloned().unwrap_or_else(|| guard.then_ty.clone());
                    Some(GuardNarrowing { var: guard.var, then_ty: current, else_ty: guard.else_ty })
                }
                _ => None,
            });
        }
        let (cond, negated) = match &condition.kind {
            ExprKind::Not(inner) => (inner.as_ref(), true),
            _ => (condition, false),
        };
        let Some((receiver, target, inverted)) = guard_receiver_and_type(cond) else {
            return Ok(None);
        };
        // `!==` is the complement of `===`, so it composes with a leading `!` by XOR:
        // `!($x !== null)` narrows the then-branch exactly like `$x === null`.
        let negated = negated ^ inverted;
        let Some(key) = self.guard_env_key(receiver, env)? else {
            return Ok(None);
        };
        if self.property_guard_receiver_is_unstable(receiver, env)? {
            return Ok(None);
        }
        // A prior narrowing (or a variable binding) wins; otherwise a property receiver falls back
        // to its declared field type. An unbound plain variable stays un-narrowed.
        let current = match env.get(&key) {
            Some(ty) => ty.clone(),
            None if matches!(receiver.kind, ExprKind::PropertyAccess { .. }) => {
                self.infer_type(receiver, env)?
            }
            None => return Ok(None),
        };
        let matched = self.narrow_to(&current, &target);
        let complement = self.narrow_complement(&current, &target);
        let (then_ty, else_ty) = if negated {
            (complement, matched)
        } else {
            (matched, complement)
        };
        Ok(Some(GuardNarrowing { var: key, then_ty, else_ty }))
    }

    /// Synthetic `TypeEnv` key for a narrowed simple property access `$var->prop` (`None` for a
    /// more complex receiver). The `\x01` sigil bytes cannot appear in a real variable name, so
    /// this key never collides with a variable binding — a normal property read only picks it up
    /// when a narrowing has explicitly inserted it.
    pub(crate) fn narrowed_property_env_key(object: &Expr, property: &str) -> Option<String> {
        match &object.kind {
            ExprKind::Variable(var) => Some(format!("\u{1}prop\u{1}{var}->{property}")),
            ExprKind::This => Some(format!("\u{1}prop\u{1}$this->{property}")),
            _ => None,
        }
    }

    /// Synthetic `TypeEnv` key for a narrowed `readonly` property access. Same shape as
    /// [`Self::narrowed_property_env_key`] but under a distinct `\x02` sigil, so
    /// [`Self::purge_property_narrowings`] can keep these while dropping the mutable ones without
    /// needing the receiver's class at purge time (it has no `&self` to look one up with).
    pub(crate) fn narrowed_readonly_property_env_key(
        object: &Expr,
        property: &str,
    ) -> Option<String> {
        match &object.kind {
            ExprKind::Variable(var) => Some(format!("\u{2}roprop\u{2}{var}->{property}")),
            ExprKind::This => Some(format!("\u{2}roprop\u{2}$this->{property}")),
            _ => None,
        }
    }

    /// Synthetic `TypeEnv` key for a narrowed array-element access `$var['k']` / `$var[0]` —
    /// `None` for anything else (non-variable roots, dynamic indexes), which keeps those paths
    /// un-narrowed rather than unsoundly keyed (#695 D3).
    ///
    /// Same `\x01` sigil family as the MUTABLE property keys on purpose: the blanket call/loop
    /// purge ([`Self::purge_property_narrowings`]) then invalidates element refinements for
    /// free, which is the required conservative posture — arrays can be aliased via references,
    /// so any call may mutate the root. A numeric-string literal (`$a['3']`) and its int twin
    /// (`$a[3]`) name the same PHP element but format as distinct keys; that costs precision
    /// (a mixed-spelling guard/read pair misses the refinement), never soundness, because every
    /// element WRITE purges by root, not by key.
    pub(crate) fn narrowed_array_element_env_key(array: &Expr, index: &Expr) -> Option<String> {
        let ExprKind::Variable(var) = &array.kind else {
            return None;
        };
        match &index.kind {
            ExprKind::StringLiteral(key) => Some(format!("\u{1}elem\u{1}{var}['{key}']")),
            ExprKind::IntLiteral(key) => Some(format!("\u{1}elem\u{1}{var}[{key}]")),
            _ => None,
        }
    }

    /// Whether a refinement on `$object->property` stays true across a call.
    ///
    /// A `readonly` property cannot be written after initialization, so no callee and no aliased
    /// write can change the slot — the refinement survives any call. That guarantee does NOT hold
    /// inside `__construct` or `__clone` of the declaring class, where PHP still permits
    /// (re)initialization, so those are excluded.
    ///
    /// Only a single concrete receiver class qualifies. For a union the property would have to be
    /// `readonly` in every member, and that extra precision is not worth the soundness surface.
    fn property_refinement_survives_calls(
        &mut self,
        object: &Expr,
        property: &str,
        env: &TypeEnv,
    ) -> Result<bool, CompileError> {
        let PhpType::Object(class) = self.infer_type(object, env)? else {
            return Ok(false);
        };
        let in_initializer = matches!(
            self.current_method.as_deref(),
            Some("__construct") | Some("__clone")
        ) && self
            .current_class
            .as_deref()
            .is_some_and(|current| current.eq_ignore_ascii_case(&class));
        if in_initializer {
            return Ok(false);
        }
        Ok(self.classes.get(&class).is_some_and(|info| {
            info.is_readonly_class || info.readonly_properties.contains(property)
        }))
    }

    /// `TypeEnv` key for a guard receiver: a variable's name, or the synthetic property key for a
    /// simple property access. `None` for receivers narrowing can't key (complex chains).
    ///
    /// A property whose refinement survives calls is keyed under the immutable sigil so the
    /// call purge leaves it alone.
    fn guard_env_key(
        &mut self,
        receiver: &Expr,
        env: &TypeEnv,
    ) -> Result<Option<String>, CompileError> {
        Ok(match &receiver.kind {
            ExprKind::Variable(var) => Some(var.clone()),
            ExprKind::PropertyAccess { object, property } => {
                if self.property_refinement_survives_calls(object, property, env)? {
                    Self::narrowed_readonly_property_env_key(object, property)
                } else {
                    Self::narrowed_property_env_key(object, property)
                }
            }
            // #695 D3: key a literal-indexed element read of a genuinely-array root.
            // The type gate matters: an `ArrayAccess` OBJECT's `offsetGet` may run user
            // code (two reads can differ — the hooked-property analogue), and a `Mixed`
            // root could hold such an object at runtime. A known array's literal-indexed
            // read is a stable flow binding.
            ExprKind::ArrayAccess { array, index } => {
                if matches!(
                    self.infer_type(array, env)?,
                    PhpType::Array(_) | PhpType::AssocArray { .. }
                ) {
                    Self::narrowed_array_element_env_key(array, index)
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    /// Drops every MUTABLE synthetic property narrowing from the environment. Called after effects
    /// that may write a property (property assignments, any call — a callee can mutate the object),
    /// and at loop-body entry (a later iteration may observe an earlier iteration's write), so a
    /// stale narrowing never survives a potential mutation. Variable narrowings are unaffected —
    /// visible assignments already update those bindings directly.
    ///
    /// `readonly` refinements (the `\x02` sigil) are deliberately KEPT: PHP forbids writing the
    /// slot after initialization, so no callee can invalidate them. That exemption is why a
    /// `readonly` property survives a nested call in an argument list (#680).
    pub(crate) fn purge_property_narrowings(env: &mut TypeEnv) {
        env.retain(|key, _| !key.starts_with('\u{1}'));
    }

    /// Drops synthetic property narrowings rooted at one local variable after that local is
    /// rebound. Other receivers remain valid and keep their precision.
    ///
    /// Both sigils are dropped, `readonly` included: `readonly` means the SLOT cannot change, not
    /// that the local still names the same object. Rebinding the root changes which object the
    /// path refers to, so the refinement no longer describes anything.
    pub(crate) fn purge_property_narrowings_for_root(env: &mut TypeEnv, root: &str) {
        let mutable_prefix = format!("\u{1}prop\u{1}{root}->");
        let readonly_prefix = format!("\u{2}roprop\u{2}{root}->");
        let element_prefix = format!("\u{1}elem\u{1}{root}[");
        env.retain(|key, _| {
            !key.starts_with(&mutable_prefix)
                && !key.starts_with(&readonly_prefix)
                && !key.starts_with(&element_prefix)
        });
    }

    /// Drops array-element narrowings rooted at one local after an element of that local is
    /// written (`$root[...] = ...`) or removed (`unset($root[...])`). A dynamic index can hit
    /// any element, and even a different-literal write mutates the same root value — so the
    /// purge is by ROOT, never by exact key (#695 D3).
    pub(crate) fn purge_array_element_narrowings_for_root(env: &mut TypeEnv, root: &str) {
        let element_prefix = format!("\u{1}elem\u{1}{root}[");
        env.retain(|key, _| !key.starts_with(&element_prefix));
    }

    /// Returns whether a property guard can invoke user code on either read. Hooked or magic
    /// properties are not stable flow bindings because two reads may produce different values.
    fn property_guard_receiver_is_unstable(
        &mut self,
        receiver: &Expr,
        env: &TypeEnv,
    ) -> Result<bool, CompileError> {
        let ExprKind::PropertyAccess { object, property } = &receiver.kind else {
            return Ok(false);
        };
        let object_ty = self.infer_type(object, env)?;
        let classes = match object_ty {
            PhpType::Object(class) => vec![class],
            PhpType::Union(_) => self.union_object_classes(&object_ty),
            _ => return Ok(false),
        };
        let get_hook = php_symbol_key(&property_hook_get_method(property));
        Ok(classes.iter().any(|class| {
            self.classes.get(class).is_some_and(|info| {
                info.methods.contains_key(&get_hook)
                    || (!info.properties.iter().any(|(name, _)| name == property)
                        && info.methods.contains_key("__get"))
            })
        }))
    }

    /// Narrows `current` to the guard-true type. Inside the branch the guard guarantees the target,
    /// so `Mixed` and any incompatible concrete type become `target`; a `Union` keeps only its
    /// matching members (falling back to `target` if none match); a concrete type already matching
    /// the guard is kept as-is (preserving a more specific class for `instanceof`).
    fn narrow_to(&self, current: &PhpType, target: &PhpType) -> PhpType {
        match current {
            PhpType::Union(members) => {
                let kept: Vec<PhpType> =
                    members.iter().filter(|m| guard_matches(m, target)).cloned().collect();
                if kept.is_empty() {
                    target.clone()
                } else {
                    self.normalize_union_type(kept)
                }
            }
            _ if guard_matches(current, target) => current.clone(),
            // A guard by an interface the value already satisfies keeps the more-informative
            // `current` type instead of discarding it for the guard target. PHP's `$x instanceof I`
            // yields `typeof($x) & I`; with no intersection type, `current` is the sound
            // approximation whenever `current` is a subtype of the interface (the intersection IS
            // `current`) or the interface is an empty marker adding no callable surface. Without
            // this, `AdminResource` guarded by the empty `StaticAdminResource` marker was narrowed
            // to the marker and lost `AdminResource`'s methods (mis-reported "Undefined method").
            _ if self.keep_current_over_interface_guard(current, target) => current.clone(),
            _ => target.clone(),
        }
    }

    /// Returns true when an `instanceof <interface>` guard should keep the pre-guard `current`
    /// object type rather than replace it with the interface target, because the guard adds no
    /// callable surface the value did not already carry. Two sound cases (`current` is never a
    /// supertype claim): (a) `current` already implements/extends the target interface, so
    /// `current & target` == the more-specific `current`; (b) the target is an empty marker
    /// interface (no methods, no static methods, no parents), so `current & target` exposes exactly
    /// `current`'s methods. A class target, or a non-empty unrelated interface, falls through to the
    /// caller's `target.clone()` — a real intersection type is future work.
    fn keep_current_over_interface_guard(&self, current: &PhpType, target: &PhpType) -> bool {
        let (PhpType::Object(current_class), PhpType::Object(target_class)) = (current, target)
        else {
            return false;
        };
        let Some(target_info) = self.interfaces.get(target_class) else {
            return false;
        };
        if self.object_type_implements_interface(current_class, target_class) {
            return true;
        }
        target_info.methods.is_empty()
            && target_info.static_methods.is_empty()
            && target_info.parents.is_empty()
    }

    /// Narrows `current` to the subset incompatible with `target` (the guard-false type): a `Union`
    /// drops its matching members, while `Mixed` and concrete types are returned unchanged (the
    /// complement of `Mixed` is not representable). An empty result falls back to `current`.
    fn narrow_complement(&self, current: &PhpType, target: &PhpType) -> PhpType {
        match current {
            PhpType::Union(members) => {
                let kept: Vec<PhpType> =
                    members.iter().filter(|m| !guard_matches(m, target)).cloned().collect();
                if kept.is_empty() {
                    current.clone()
                } else {
                    self.normalize_union_type(kept)
                }
            }
            _ => current.clone(),
        }
    }

    /// Returns true when a statement body always diverges.
    ///
    /// A body is considered diverging if its last statement is:
    /// - `return` or `throw`
    /// - `break` or `continue` (of any level) — both skip the statements that follow the enclosing
    ///   `if` within the current loop body, so reaching them proves the guard was false. This makes
    ///   the common loop guard `if (!is_callable($x)) { continue; } $x(...)` (PSR-14 dispatch)
    ///   narrow `$x` on the fall-through, exactly as a `return` guard does. The complement is scoped
    ///   to the loop body's env, so it never leaks to statements after the loop.
    /// - a call to `exit()` or `die()`
    /// - a call to a user function whose declared return type is `never`
    ///
    /// This is used by type narrowing so that an `if (guard) { ... diverging ... }` (with no else)
    /// allows the statements *after* the if to be narrowed to the complement type.
    pub(crate) fn body_always_diverges(&self, body: &[Stmt]) -> bool {
        let Some(last) = body.last() else {
            return false;
        };

        match &last.kind {
            StmtKind::Return(_)
            | StmtKind::Throw(_)
            | StmtKind::Break(_)
            | StmtKind::Continue(_) => true,
            StmtKind::ExprStmt(expr) => self.expr_always_diverges(expr),
            _ => false,
        }
    }

    /// Returns true if the expression is known to never return normally: a call to `exit()` or
    /// `die()` (recognized by name), or a call to a user function whose declared return type is
    /// `never`. The function name is resolved case-insensitively against the checker's function
    /// table, matching PHP's call semantics.
    fn expr_always_diverges(&self, expr: &Expr) -> bool {
        let ExprKind::FunctionCall { name, .. } = &expr.kind else {
            return false;
        };
        let lowered = name.to_ascii_lowercase();
        if lowered == "exit" || lowered == "die" {
            return true;
        }
        self.canonical_function_name_folded(name)
            .and_then(|canonical| self.functions.get(&canonical))
            .map(|sig| sig.return_type == PhpType::Never)
            .unwrap_or(false)
    }
}

/// Extracts the guarded receiver expression and the target type from a (non-negated) guard
/// expression. Recognizes the scalar `is_*` predicates, `is_callable`, `is_null`, `instanceof <Name>`, and
/// `===`/`!==` against `false` / `null` (the third tuple element reports comparison polarity).
/// The receiver may be any expression here — `guard_env_key` decides
/// which receivers narrowing can actually key (variables and simple property accesses).
fn guard_receiver_and_type(cond: &Expr) -> Option<(&Expr, PhpType, bool)> {
    match &cond.kind {
        ExprKind::FunctionCall { name, args } if args.len() == 1 => {
            let target = match name.as_str().to_ascii_lowercase().as_str() {
                "is_int" | "is_integer" | "is_long" => PhpType::Int,
                "is_float" | "is_double" => PhpType::Float,
                "is_string" => PhpType::Str,
                "is_bool" => PhpType::Bool,
                // `is_callable($x)`: narrow to Callable in the then-branch (and, via the negated
                // early-return complement, on the fall-through). Sound because every value for which
                // `is_callable` is true — closure, function-name string, `[obj, method]` /
                // `[class, staticMethod]` array, or an `__invoke` object — is a PHP callable. Lets
                // the `if (!is_callable($x)) { continue; } $x(...)` guard (PSR-14 EventDispatcher)
                // type-check; the boxed-closure runtime (is_callable tag-10 reader + callable-array
                // value_type stamp) makes the guarded dispatch work natively, not just at --check.
                "is_callable" => PhpType::Callable,
                // `is_array($x)`: narrow to `array<mixed>` — the array shape accepted by the widest
                // set of array builtins (array_filter/array_map take `Array` but reject `AssocArray`;
                // array_keys handles it at runtime by heap kind). Its codegen repr stays `Mixed`, so
                // a genuine `mixed` value is served natively by the array-builtin Mixed-unbox path
                // (array_keys), while `array_keys($x)` still yields mixed keys so a following
                // `is_string($key)` guard type-checks (ward-config Config::getSection). `guard_matches`
                // keeps an already-array-typed value at its precise shape rather than widening it.
                "is_array" => PhpType::Array(Box::new(PhpType::Mixed)),
                // `is_null($x)`: same narrowing as `$x === null` — elephc models a `?T` value's
                // null as Void, so the complement strips it (`if (is_null($x)) { throw; }` leaves
                // ?int as int on the fall-through path).
                "is_null" => PhpType::Void,
                _ => return None,
            };
            Some((&args[0], target, false))
        }
        ExprKind::InstanceOf { value, target } => {
            let InstanceOfTarget::Name(class) = target else {
                return None;
            };
            Some((value, PhpType::Object(class.as_str().to_string()), false))
        }
        // `$var === false` / `false === $var`: narrow to the literal False subtype in the
        // then-branch; the else-branch strips only that member (e.g. int|false → int) while a full
        // `bool` member remains. Enables the common
        // `if ($x === false) { throw; } return $x;` guard (ward-http StreamGuards::requireInt etc.).
        // `$var === false` / `false === $var`: narrow to the literal False subtype in the
        // then-branch; the else-branch strips only that member (e.g. int|false → int) while a full
        // `bool` member remains.
        //
        // `!==` is accepted as the same guard with inverted polarity, which is what makes the
        // POSITIVE form work: `if ($error !== null) { f($error); }` strips the null-ish member
        // inside the body, so a `?string` parameter satisfies a `string` parameter there. Without
        // it only the early-return form (`if ($x === null) { throw; }`) narrowed, and the positive
        // form was the shape real-world code actually writes.
        ExprKind::BinaryOp { left, op: op @ (BinOp::StrictEq | BinOp::StrictNotEq), right } => {
            let (receiver, lit) = match (&left.kind, &right.kind) {
                (ExprKind::Variable(_) | ExprKind::PropertyAccess { .. }, _) => {
                    (left.as_ref(), &right.kind)
                }
                (_, ExprKind::Variable(_) | ExprKind::PropertyAccess { .. }) => {
                    (right.as_ref(), &left.kind)
                }
                _ => return None,
            };
            let inverted = matches!(op, BinOp::StrictNotEq);
            match lit {
                ExprKind::BoolLiteral(false) => Some((receiver, PhpType::False, inverted)),
                // `$x === null`: strip the null-ish member (elephc models a `?T` value's null as
                // Void), e.g. `?self` / self|null → self after `if ($x === null) { throw; }`.
                ExprKind::Null => Some((receiver, PhpType::Void, inverted)),
                _ => None,
            }
        }
        _ => None,
    }
}

impl Checker {
    /// Applies the narrowing implied by a bare `assert(<guard>)` statement to the rest of the
    /// scope, so `assert(is_string($x)); return $x;` type-checks against a `string` return.
    ///
    /// PHP's `assert()` is a developer contract rather than a runtime guarantee — it is compiled
    /// out under `zend.assertions=-1`, and elephc's own `assert` builtin is documented as a no-op
    /// under production settings — so this trusts the declaration rather than proving it. That is
    /// the same posture PHPStan takes, and it is what the assertion is written to express. Only
    /// the guard forms `guard_narrowing` already recognizes narrow; anything else is left alone.
    ///
    /// # Errors
    /// Propagates inference errors raised while evaluating the guard condition.
    pub(crate) fn apply_assert_narrowing(
        &mut self,
        expr: &Expr,
        env: &mut TypeEnv,
    ) -> Result<(), CompileError> {
        let ExprKind::FunctionCall { name, args } = &expr.kind else {
            return Ok(());
        };
        if !name.eq_ignore_ascii_case("assert") || args.is_empty() {
            return Ok(());
        }
        if let Some(narrowing) = self.guard_narrowing(&args[0], env)? {
            env.insert(narrowing.var, narrowing.then_ty);
        }
        Ok(())
    }
}

/// Returns true when a union member is compatible with a guard target, used to keep (then) or drop
/// (else) members. Scalar targets require an exact variant match; an `Object` target matches an
/// object member with the same class name (inheritance-aware narrowing is left for the future).
fn guard_matches(member: &PhpType, target: &PhpType) -> bool {
    match (member, target) {
        (PhpType::Object(member_class), PhpType::Object(target_class)) => member_class == target_class,
        (PhpType::False, PhpType::Bool) => true,
        // `is_array` narrows to a generic array target; any concrete array — indexed `Array` or
        // associative `AssocArray`, with any element/key refinement — already is an array, so it
        // matches. Narrowing therefore keeps the precise current/member type instead of widening it
        // to `array<mixed, mixed>` (which would spuriously reject a later `array_filter`/`array_map`
        // that only accepts a concrete `Array`), while a `mixed` value still widens to the target.
        (
            PhpType::Array(_) | PhpType::AssocArray { .. },
            PhpType::Array(_) | PhpType::AssocArray { .. },
        ) => true,
        _ => member == target,
    }
}
