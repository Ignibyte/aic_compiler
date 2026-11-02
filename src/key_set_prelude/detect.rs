//! Purpose:
//! Decides whether a parsed program calls `array_intersect_key()` or `array_diff_key()`, so the
//! key-set prelude is injected only for programs that can need it.
//!
//! Called from:
//! - `crate::key_set_prelude::inject_if_used`.
//!
//! Key details:
//! - Adapted from `hash_prelude::detect`: it runs before name resolution, so a call may be
//!   written `array_intersect_key`, `\array_intersect_key` or in any case, and the walk matches
//!   the unqualified last segment case-insensitively. Only function-call positions count; the
//!   prelude declares no class.
//! - Soundness over precision, as there: the `match`es are exhaustive, and a false positive only
//!   injects two declarations, which reachability drops when no call is retargeted to them.

use crate::names::Name;
use crate::parser::ast::{
    CallableTarget, ClassConst, ClassMethod, ClassProperty, EnumCaseDecl, Expr, ExprKind,
    InstanceOfTarget, PackedField, StaticReceiver, Stmt, StmtKind, TraitAdaptation, TraitUse,
    TypeExpr,
};

/// The builtins whose boxed or list operands call the key-set prelude.
const HASH_CONTEXT_FUNCTIONS: &[&str] = &["array_intersect_key", "array_diff_key"];

/// Returns whether any top-level statement references the incremental-hashing
/// surface, so the prelude must be injected ahead of user code.
pub(super) fn program_calls_key_set_builtin(program: &[Stmt]) -> bool {
    program.iter().any(stmt_refs_hash)
}

/// Returns whether `name`'s unqualified last segment is one of the four streaming
/// functions, compared case-insensitively to match PHP's case-insensitive function
/// names and any namespace/leading-backslash form (`hash_init`, `\hash_init`,
/// `\Some\hash_init`). Whole-segment equality, never a prefix — see the module
/// preamble.
fn name_is_hash_function(name: &Name) -> bool {
    name.last_segment().is_some_and(|segment| {
        HASH_CONTEXT_FUNCTIONS
            .iter()
            .any(|candidate| segment.eq_ignore_ascii_case(candidate))
    })
}

/// Returns whether `name`'s unqualified last segment is `HashContext`, compared
/// case-insensitively and tolerant of any namespace/leading-backslash form
/// (`HashContext`, `\HashContext`, `\Foo\HashContext`).
fn name_is_hash_class(name: &Name) -> bool {
    // The key-set prelude declares no class, so no class-name position triggers injection.
    let _ = name;
    false
}

/// Returns whether a static receiver names the hash class (`HashContext::...`).
/// `self`, `static`, and `parent` never resolve to `HashContext` at this position.
fn receiver_refs_hash(receiver: &StaticReceiver) -> bool {
    matches!(receiver, StaticReceiver::Named(name) if name_is_hash_class(name))
}

/// Returns whether an `instanceof` target references the hash class, recursing into
/// the operand when the target is a runtime expression.
fn instanceof_target_refs_hash(target: &InstanceOfTarget) -> bool {
    match target {
        InstanceOfTarget::Name(name) => name_is_hash_class(name),
        InstanceOfTarget::Generic(class_type) => type_refs_hash(class_type),
        InstanceOfTarget::Expr(expr) => expr_refs_hash(expr),
    }
}

/// Returns whether a first-class-callable target references the hashing surface: a
/// free function (`hash_update(...)`), a static-method receiver, or an
/// instance-method object expression.
fn callable_target_refs_hash(target: &CallableTarget) -> bool {
    match target {
        CallableTarget::Function(name) => name_is_hash_function(name),
        CallableTarget::StaticMethod { receiver, .. } => receiver_refs_hash(receiver),
        CallableTarget::Method { object, .. } => expr_refs_hash(object),
    }
}

/// Returns whether a type expression names the hash class, recursing through
/// nullable/union/array/buffer wrappers and `ptr<Class>` targets.
fn type_refs_hash(type_expr: &TypeExpr) -> bool {
    match type_expr {
        TypeExpr::Int
        | TypeExpr::Float
        | TypeExpr::Bool
        | TypeExpr::False
        | TypeExpr::Str
        | TypeExpr::Void
        | TypeExpr::Never
        | TypeExpr::Iterable => false,
        TypeExpr::Ptr(target) => target.as_ref().is_some_and(|name| name_is_hash_class(&crate::names::pointer_target_leaf_name(name))),
        TypeExpr::Array(inner) | TypeExpr::Buffer(inner) | TypeExpr::Nullable(inner) => {
            type_refs_hash(inner)
        }
        TypeExpr::AssocArray { key, value } => type_refs_hash(key) || type_refs_hash(value),
        // A declared signature can name one of these classes, and a program
        // that mentions it only through a callback still needs the prelude.
        TypeExpr::CallableSig { params, ret } => {
            params.iter().any(type_refs_hash) || type_refs_hash(ret)
        }
        TypeExpr::Named(name) => name_is_hash_class(name),
        TypeExpr::GenericClass { name, args } => {
            name_is_hash_class(name) || args.iter().any(type_refs_hash)
        }
        TypeExpr::Union(members) | TypeExpr::Intersection(members) => {
            members.iter().any(type_refs_hash)
        }
    }
}

/// Returns whether any parameter's type hint or default value references the
/// hashing surface. Shared by function, method, and closure parameter lists.
fn params_ref_hash(params: &[(String, Option<TypeExpr>, Option<Expr>, bool)]) -> bool {
    params.iter().any(|(_, type_expr, default, _)| {
        type_expr.as_ref().is_some_and(type_refs_hash)
            || default.as_ref().is_some_and(expr_refs_hash)
    })
}

/// Returns whether a `use Trait` clause names the hash class through its trait list
/// or any conflict-resolution adaptation.
fn trait_use_refs_hash(trait_use: &TraitUse) -> bool {
    trait_use.trait_names.iter().any(name_is_hash_class)
        || trait_use.adaptations.iter().any(|adaptation| match adaptation {
            TraitAdaptation::Alias { trait_name, .. } => {
                trait_name.as_ref().is_some_and(name_is_hash_class)
            }
            TraitAdaptation::InsteadOf {
                trait_name,
                instead_of,
                ..
            } => {
                trait_name.as_ref().is_some_and(name_is_hash_class)
                    || instead_of.iter().any(name_is_hash_class)
            }
        })
}

/// Returns whether a class property's type hint or default value references the
/// hashing surface.
fn class_property_refs_hash(property: &ClassProperty) -> bool {
    property.type_expr.as_ref().is_some_and(type_refs_hash)
        || property.default.as_ref().is_some_and(expr_refs_hash)
}

/// Returns whether a method's parameters, return type, or body reference the
/// hashing surface.
fn class_method_refs_hash(method: &ClassMethod) -> bool {
    params_ref_hash(&method.params)
        || method.return_type.as_ref().is_some_and(type_refs_hash)
        || method.body.iter().any(stmt_refs_hash)
}

/// Returns whether a class constant's initializer references the hashing surface.
fn class_const_refs_hash(constant: &ClassConst) -> bool {
    expr_refs_hash(&constant.value)
}

/// Returns whether an enum case's backing-value expression references the hashing
/// surface.
fn enum_case_refs_hash(case: &EnumCaseDecl) -> bool {
    case.value.as_ref().is_some_and(expr_refs_hash)
}

/// Returns whether a `packed class` field's type references the hash class.
/// `HashContext` is never a valid packed field type, but the field is walked for
/// completeness.
fn packed_field_refs_hash(field: &PackedField) -> bool {
    type_refs_hash(&field.type_expr)
}

/// Returns whether an expression references the hashing surface at any
/// function-call or class-name position, recursing into every child expression and
/// statement. The `match` is exhaustive so a newly added `ExprKind` cannot silently
/// bypass detection.
fn expr_refs_hash(expr: &Expr) -> bool {
    match &expr.kind {
        // Leaves and identifier-only forms carry no hash reference.
        ExprKind::StringLiteral(_)
        | ExprKind::IntLiteral(_)
        | ExprKind::FloatLiteral(_)
        | ExprKind::Variable(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::Null
        | ExprKind::This
        | ExprKind::PreIncrement(_)
        | ExprKind::PostIncrement(_)
        | ExprKind::PreDecrement(_)
        | ExprKind::PostDecrement(_)
        | ExprKind::ConstRef(_)
        | ExprKind::MagicConstant(_) => false,

        ExprKind::BinaryOp { left, right, .. } => expr_refs_hash(left) || expr_refs_hash(right),
        ExprKind::InstanceOf { value, target } => {
            expr_refs_hash(value) || instanceof_target_refs_hash(target)
        }
        ExprKind::Negate(inner)
        | ExprKind::Not(inner)
        | ExprKind::BitNot(inner)
        | ExprKind::Throw(inner)
        | ExprKind::Clone(inner)
        | ExprKind::ErrorSuppress(inner)
        | ExprKind::Print(inner)
        | ExprKind::Spread(inner)
        | ExprKind::YieldFrom(inner) => expr_refs_hash(inner),
        ExprKind::NullCoalesce { value, default }
        | ExprKind::ShortTernary { value, default } => {
            expr_refs_hash(value) || expr_refs_hash(default)
        }
        ExprKind::Pipe { value, callable } => expr_refs_hash(value) || expr_refs_hash(callable),
        ExprKind::Assignment {
            target,
            value,
            result_target,
            prelude,
            ..
        } => {
            expr_refs_hash(target)
                || expr_refs_hash(value)
                || result_target.as_deref().is_some_and(expr_refs_hash)
                || prelude.iter().any(stmt_refs_hash)
        }
        // A free-function call is the dominant hash-context position: `hash_init()`
        // is the only way to obtain a `HashContext` in the first place, so a program
        // may stream a hash without ever naming the class.
        ExprKind::FunctionCall { name, args } => {
            name_is_hash_function(name) || args.iter().any(expr_refs_hash)
        }
        ExprKind::ClosureCall { args, .. } => args.iter().any(expr_refs_hash),
        ExprKind::ArrayLiteral(items) => items.iter().any(expr_refs_hash),
        ExprKind::ArrayLiteralAssoc(pairs) => pairs
            .iter()
            .any(|(key, value)| expr_refs_hash(key) || expr_refs_hash(value)),
        ExprKind::ArrayLiteralMixed(entries) => entries
            .iter()
            .flat_map(|entry| entry.exprs())
            .any(expr_refs_hash),
        ExprKind::Match {
            subject,
            arms,
            default,
        } => {
            expr_refs_hash(subject)
                || arms.iter().any(|(conditions, body)| {
                    conditions.iter().any(expr_refs_hash) || expr_refs_hash(body)
                })
                || default.as_deref().is_some_and(expr_refs_hash)
        }
        ExprKind::ArrayAccess { array, index } => expr_refs_hash(array) || expr_refs_hash(index),
        ExprKind::Ternary {
            condition,
            then_expr,
            else_expr,
        } => expr_refs_hash(condition) || expr_refs_hash(then_expr) || expr_refs_hash(else_expr),
        ExprKind::Cast { expr, .. } | ExprKind::PtrCast { expr, .. } => expr_refs_hash(expr),
        ExprKind::Closure {
            params,
            return_type,
            body,
            ..
        } => {
            params_ref_hash(params)
                || return_type.as_ref().is_some_and(type_refs_hash)
                || body.iter().any(stmt_refs_hash)
        }
        ExprKind::NamedArg { value, .. } => expr_refs_hash(value),
        ExprKind::ExprCall { callee, args } => {
            expr_refs_hash(callee) || args.iter().any(expr_refs_hash)
        }
        ExprKind::NewObject { class_name, args } => {
            name_is_hash_class(class_name) || args.iter().any(expr_refs_hash)
        }
        ExprKind::NewGeneric { class_type, args } => {
            type_refs_hash(class_type) || args.iter().any(expr_refs_hash)
        }
        ExprKind::NewDynamic { name_expr, args } => {
            expr_refs_hash(name_expr) || args.iter().any(expr_refs_hash)
        }
        ExprKind::NewDynamicObject {
            class_name,
            fallback_class,
            required_parent,
            args,
        } => {
            expr_refs_hash(class_name)
                || name_is_hash_class(fallback_class)
                || name_is_hash_class(required_parent)
                || args.iter().any(expr_refs_hash)
        }
        ExprKind::PropertyAccess { object, .. }
        | ExprKind::NullsafePropertyAccess { object, .. } => expr_refs_hash(object),
        ExprKind::DynamicPropertyAccess { object, property }
        | ExprKind::NullsafeDynamicPropertyAccess { object, property } => {
            expr_refs_hash(object) || expr_refs_hash(property)
        }
        ExprKind::StaticPropertyAccess { receiver, .. } => receiver_refs_hash(receiver),
        ExprKind::MethodCall { object, args, .. }
        | ExprKind::NullsafeMethodCall { object, args, .. } => {
            expr_refs_hash(object) || args.iter().any(expr_refs_hash)
        }
        ExprKind::NullsafeDynamicMethodCall {
            object,
            method,
            args,
        } => expr_refs_hash(object) || expr_refs_hash(method) || args.iter().any(expr_refs_hash),
        ExprKind::StaticMethodCall { receiver, args, .. } => {
            receiver_refs_hash(receiver) || args.iter().any(expr_refs_hash)
        }
        ExprKind::FirstClassCallable(target) => callable_target_refs_hash(target),
        ExprKind::BufferNew { element_type, len } => {
            type_refs_hash(element_type) || expr_refs_hash(len)
        }
        ExprKind::ClassConstant { receiver }
        | ExprKind::ScopedConstantAccess { receiver, .. } => receiver_refs_hash(receiver),
        ExprKind::ObjectClassName { object } => expr_refs_hash(object),
        ExprKind::NewScopedObject { receiver, args } => {
            receiver_refs_hash(receiver) || args.iter().any(expr_refs_hash)
        }
        ExprKind::Yield { key, value } => {
            key.as_deref().is_some_and(expr_refs_hash)
                || value.as_deref().is_some_and(expr_refs_hash)
        }
        // Transient: the resolver expands this into the included file's statements
        // before hash detection runs, so it should never reach here. Recurse into the
        // path expression defensively to keep detection exhaustive and correct.
        ExprKind::IncludeValue { path, .. } => expr_refs_hash(path),
    }
}

/// Returns whether a statement references the hashing surface at any function-call
/// or class-name position, recursing into nested statements, expressions, and class
/// members. The `match` is exhaustive so a newly added `StmtKind` cannot silently
/// bypass detection.
fn stmt_refs_hash(stmt: &Stmt) -> bool {
    match &stmt.kind {
        // Statements with no hash-name position and no child expr/stmt.
        StmtKind::RefAssign { .. }
        | StmtKind::IncludeOnceMark { .. }
        | StmtKind::Break(_)
        | StmtKind::Continue(_)
        | StmtKind::NamespaceDecl { .. }
        | StmtKind::FunctionVariantGroup { .. }
        | StmtKind::FunctionVariantMark { .. }
        | StmtKind::Global { .. }
        | StmtKind::ExternFunctionDecl { .. }
        | StmtKind::ExternClassDecl { .. }
        | StmtKind::ExternGlobalDecl { .. } => false,

        // An aliased import (`use HashContext as Ctx;`) names the class only here;
        // the later `new Ctx()` / `Ctx $c` carries the alias, which the walk cannot
        // otherwise connect back — so skipping imports would be a false negative.
        // Function imports (`use function hash_init as hi;`) land in the same list
        // and are caught by the same check, which is why both name tests are applied.
        StmtKind::UseDecl { imports } => imports
            .iter()
            .any(|item| name_is_hash_class(&item.name) || name_is_hash_function(&item.name)),

        StmtKind::Echo(expr) | StmtKind::Throw(expr) | StmtKind::ExprStmt(expr) => {
            expr_refs_hash(expr)
        }
        StmtKind::Assign { value, .. } => expr_refs_hash(value),
        StmtKind::If {
            condition,
            then_body,
            elseif_clauses,
            else_body,
        } => {
            expr_refs_hash(condition)
                || then_body.iter().any(stmt_refs_hash)
                || elseif_clauses
                    .iter()
                    .any(|(cond, body)| expr_refs_hash(cond) || body.iter().any(stmt_refs_hash))
                || else_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(stmt_refs_hash))
        }
        StmtKind::IfDef {
            then_body,
            else_body,
            ..
        } => {
            then_body.iter().any(stmt_refs_hash)
                || else_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(stmt_refs_hash))
        }
        StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
            expr_refs_hash(condition) || body.iter().any(stmt_refs_hash)
        }
        StmtKind::For {
            init,
            condition,
            update,
            body,
        } => {
            init.as_deref().is_some_and(stmt_refs_hash)
                || condition.as_ref().is_some_and(expr_refs_hash)
                || update.as_deref().is_some_and(stmt_refs_hash)
                || body.iter().any(stmt_refs_hash)
        }
        StmtKind::ArrayAssign { index, value, .. } => {
            expr_refs_hash(index) || expr_refs_hash(value)
        }
        StmtKind::NestedArrayAssign { target, value } => {
            expr_refs_hash(target) || expr_refs_hash(value)
        }
        StmtKind::ArrayPush { value, .. } => expr_refs_hash(value),
        StmtKind::TypedAssign {
            type_expr, value, ..
        } => type_refs_hash(type_expr) || expr_refs_hash(value),
        StmtKind::Foreach { array, body, .. } => {
            expr_refs_hash(array) || body.iter().any(stmt_refs_hash)
        }
        StmtKind::Switch {
            subject,
            cases,
            default,
        } => {
            expr_refs_hash(subject)
                || cases.iter().any(|(conditions, body)| {
                    conditions.iter().any(expr_refs_hash) || body.iter().any(stmt_refs_hash)
                })
                || default
                    .as_ref()
                    .is_some_and(|body| body.iter().any(stmt_refs_hash))
        }
        StmtKind::Include { path, .. } => expr_refs_hash(path),
        StmtKind::IncludeOnceGuard { body, .. }
        | StmtKind::Synthetic(body)
        | StmtKind::NamespaceBlock { body, .. } => body.iter().any(stmt_refs_hash),
        StmtKind::Try {
            try_body,
            catches,
            finally_body,
        } => {
            try_body.iter().any(stmt_refs_hash)
                || catches.iter().any(|catch| {
                    catch.exception_types.iter().any(name_is_hash_class)
                        || catch.body.iter().any(stmt_refs_hash)
                })
                || finally_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(stmt_refs_hash))
        }
        StmtKind::FunctionDecl {
            params,
            return_type,
            body,
            ..
        } => {
            params_ref_hash(params)
                || return_type.as_ref().is_some_and(type_refs_hash)
                || body.iter().any(stmt_refs_hash)
        }
        StmtKind::Return(value) => value.as_ref().is_some_and(expr_refs_hash),
        StmtKind::ConstDecl { value, .. } => expr_refs_hash(value),
        StmtKind::ListUnpack { value, .. } => expr_refs_hash(value),
        StmtKind::StaticVar { init, .. } => expr_refs_hash(init),
        StmtKind::ClassDecl {
            extends,
            implements,
            trait_uses,
            properties,
            methods,
            constants,
            ..
        } => {
            extends.as_ref().is_some_and(name_is_hash_class)
                || implements.iter().any(name_is_hash_class)
                || trait_uses.iter().any(trait_use_refs_hash)
                || properties.iter().any(class_property_refs_hash)
                || methods.iter().any(class_method_refs_hash)
                || constants.iter().any(class_const_refs_hash)
        }
        StmtKind::EnumDecl {
            backing_type,
            cases,
            ..
        } => {
            backing_type.as_ref().is_some_and(type_refs_hash)
                || cases.iter().any(enum_case_refs_hash)
        }
        StmtKind::PackedClassDecl { fields, .. } => fields.iter().any(packed_field_refs_hash),
        StmtKind::InterfaceDecl {
            extends,
            properties,
            methods,
            constants,
            ..
        } => {
            extends.iter().any(name_is_hash_class)
                || properties.iter().any(class_property_refs_hash)
                || methods.iter().any(class_method_refs_hash)
                || constants.iter().any(class_const_refs_hash)
        }
        StmtKind::TraitDecl {
            trait_uses,
            properties,
            methods,
            constants,
            ..
        } => {
            trait_uses.iter().any(trait_use_refs_hash)
                || properties.iter().any(class_property_refs_hash)
                || methods.iter().any(class_method_refs_hash)
                || constants.iter().any(class_const_refs_hash)
        }
        StmtKind::PropertyAssign { object, value, .. } => {
            expr_refs_hash(object) || expr_refs_hash(value)
        }
        StmtKind::StaticPropertyAssign {
            receiver, value, ..
        }
        | StmtKind::StaticPropertyArrayPush {
            receiver, value, ..
        } => receiver_refs_hash(receiver) || expr_refs_hash(value),
        StmtKind::StaticPropertyArrayAssign {
            receiver,
            index,
            value,
            ..
        } => receiver_refs_hash(receiver) || expr_refs_hash(index) || expr_refs_hash(value),
        StmtKind::PropertyArrayPush { object, value, .. } => {
            expr_refs_hash(object) || expr_refs_hash(value)
        }
        StmtKind::PropertyArrayAssign {
            object,
            index,
            value,
            ..
        } => expr_refs_hash(object) || expr_refs_hash(index) || expr_refs_hash(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses a PHP snippet into a program for the detector.
    fn parse(src: &str) -> Vec<Stmt> {
        let tokens = crate::lexer::tokenize(src).expect("tokenize");
        crate::parser::parse(&tokens).expect("parse")
    }

    /// A call in any case or namespace spelling, nested in a function, counts.
    #[test]
    fn detects_key_set_calls() {
        assert!(program_calls_key_set_builtin(&parse(
            r#"<?php function f(array $a) { return \ARRAY_INTERSECT_KEY($a, ['x' => 1]); }"#
        )));
        assert!(program_calls_key_set_builtin(&parse(r#"<?php echo count(array_diff_key([1], []));"#)));
    }

    /// Other array builtins and a mention inside a string do not.
    #[test]
    fn ignores_other_calls_and_strings() {
        assert!(!program_calls_key_set_builtin(&parse(
            r#"<?php echo count(array_intersect([1], [1])), "array_diff_key";"#
        )));
    }
}
