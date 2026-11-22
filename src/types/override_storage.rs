//! Purpose:
//! Finds the overrides of a method that bind a parameter, or return their result, in other
//! storage than the signature a call through the receiver's declared class or interface uses.
//!
//! Called from:
//! - `crate::types::checker::inference::objects::methods`, to type such a call `mixed`.
//! - `crate::ir_lower::expr::method_calls`, to keep such arguments in their source form and to
//!   box such results.
//! - `crate::ir_lower::expr::static_method_calls`, the same for `static::` calls.
//! - `crate::codegen::lower_inst::method_dispatch` and `static_method_calls`, to call such
//!   overrides by runtime class.
//!
//! Key details:
//! - PHP lets an override widen a parameter (`int` to `int|string`, `string` to `?string`), and
//!   an implementing class widen an interface's. The wider body reads that argument in other
//!   storage, so a call bound by the receiver's declared signature hands it the wrong form.
//! - PHP also lets an override narrow its return type (`?int` to `int`, `mixed` to `string`), and
//!   an untyped method's overrides may return anything. The body then hands back its result in
//!   other storage than the receiver's signature reads.
//! - Object classes do not change pointer storage, so a covariant object return (`A` to `B`)
//!   is not listed. A `never` return hands nothing back, and a by-reference return passes a
//!   reference cell either way.
//! - An interface caller of a `mixed` return is left alone: the interface wrapper boxes it.
//! - Only concrete classes are listed for instance calls, since an abstract class has no
//!   instances, and a class whose implementation is the receiver's own is never listed. A
//!   private method is bound lexically, so it has no overrides.
//! - Parameter counts and by-reference flags already match: the checker refuses an override
//!   that changes either.

use std::collections::HashMap;

use super::{class_inherits_from, ClassInfo, FunctionSig, InterfaceInfo, PhpType};

/// A concrete class whose implementation of a method binds some parameters in other storage
/// than the receiver's declared signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DivergingOverride {
    /// The concrete class, as keyed in the class table.
    pub class_name: String,
    /// The parameter indexes whose storage differs, in order.
    pub params: Vec<usize>,
    /// Whether the implementation returns its result in other storage.
    pub returns: bool,
}

/// Returns the concrete classes reachable from `receiver` whose `method_key` implementation
/// binds a parameter in other storage than `receiver`'s own signature, sorted by name.
///
/// `receiver` is a class or an interface name; an unknown name, or one that does not declare
/// the method, has none.
pub(crate) fn diverging_overrides(
    classes: &HashMap<String, ClassInfo>,
    interfaces: &HashMap<String, InterfaceInfo>,
    receiver: &str,
    method_key: &str,
) -> Vec<DivergingOverride> {
    let receiver = receiver.trim_start_matches('\\');
    let (declared, receiver_impl) = if let Some(class) = classes.get(receiver) {
        let Some(sig) = class.methods.get(method_key) else {
            return Vec::new();
        };
        if !class.vtable_slots.contains_key(method_key)
            || class.is_final
            || class.final_methods.contains(method_key)
        {
            return Vec::new();
        }
        (sig, class.method_impl_classes.get(method_key).map(String::as_str))
    } else if let Some(interface) = interfaces.get(receiver) {
        let Some(sig) = interface.methods.get(method_key) else {
            return Vec::new();
        };
        (sig, None)
    } else {
        return Vec::new();
    };
    let is_interface = !classes.contains_key(receiver);
    let mut found = classes
        .iter()
        .filter_map(|(name, info)| {
            if info.is_abstract {
                return None;
            }
            let sig = info.methods.get(method_key)?;
            let implementation = info.method_impl_classes.get(method_key).map(String::as_str);
            if receiver_impl.is_some() && implementation == receiver_impl {
                return None;
            }
            let params = diverging_params(declared, sig);
            let returns = return_storage_differs(declared, sig, is_interface);
            if params.is_empty() && !returns {
                return None;
            }
            let reachable = if is_interface {
                info.interfaces.iter().any(|interface| interface == receiver)
            } else {
                name.as_str() == receiver || class_inherits_from(classes, name, receiver)
            };
            reachable.then(|| DivergingOverride { class_name: name.clone(), params, returns })
        })
        .collect::<Vec<_>>();
    found.sort_by(|left, right| left.class_name.cmp(&right.class_name));
    found
}

/// Returns the classes a `static::method_key()` call written in `receiver` can run whose
/// static implementation binds a parameter or returns in other storage than `receiver`'s own
/// static signature, sorted by name.
///
/// `receiver` itself and every class inheriting from it are candidates, abstract ones included:
/// a static method can be called on an abstract class. A final method has no overrides.
pub(crate) fn diverging_static_overrides(
    classes: &HashMap<String, ClassInfo>,
    receiver: &str,
    method_key: &str,
) -> Vec<DivergingOverride> {
    let receiver = receiver.trim_start_matches('\\');
    let Some(class) = classes.get(receiver) else {
        return Vec::new();
    };
    let Some(declared) = class.static_methods.get(method_key) else {
        return Vec::new();
    };
    if !class.static_vtable_slots.contains_key(method_key)
        || class.final_static_methods.contains(method_key)
    {
        return Vec::new();
    }
    let receiver_impl = class.static_method_impl_classes.get(method_key).map(String::as_str);
    let mut found = classes
        .iter()
        .filter(|(name, _)| class_inherits_from(classes, name, receiver))
        .filter_map(|(name, info)| {
            let sig = info.static_methods.get(method_key)?;
            let implementation = info.static_method_impl_classes.get(method_key).map(String::as_str);
            if implementation.is_none() || implementation == receiver_impl {
                return None;
            }
            let params = diverging_params(declared, sig);
            let returns = return_storage_differs(declared, sig, false);
            (!params.is_empty() || returns)
                .then(|| DivergingOverride { class_name: name.clone(), params, returns })
        })
        .collect::<Vec<_>>();
    found.sort_by(|left, right| left.class_name.cmp(&right.class_name));
    found
}

/// Returns whether `actual` hands its result back in other storage than `declared` reads it.
///
/// `interface_caller` marks a call through an interface, whose wrapper already boxes a result
/// the interface declares `mixed`.
fn return_storage_differs(declared: &FunctionSig, actual: &FunctionSig, interface_caller: bool) -> bool {
    if declared.by_ref_return
        || actual.by_ref_return
        || matches!(actual.return_type, PhpType::Never)
        || (interface_caller && declared.return_type.codegen_repr() == PhpType::Mixed)
    {
        return false;
    }
    !actual.return_type.reference_payload_compatible(&declared.return_type)
}

/// Returns the parameter indexes `actual` binds in other storage than `declared`.
fn diverging_params(declared: &FunctionSig, actual: &FunctionSig) -> Vec<usize> {
    if declared.params.len() != actual.params.len() {
        return Vec::new();
    }
    declared
        .params
        .iter()
        .zip(&actual.params)
        .enumerate()
        .filter(|(_, ((_, declared), (_, actual)))| declared.codegen_repr() != actual.codegen_repr())
        .map(|(index, _)| index)
        .collect()
}
