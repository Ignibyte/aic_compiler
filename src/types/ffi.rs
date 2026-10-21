//! Purpose:
//! Defines FFI type and declaration metadata used by checking and native call lowering.
//! Maps compiler extension types into ABI-visible shapes for extern declarations.
//!
//! Called from:
//! - `crate::types::checker::extern_decl`
//! - `crate::codegen`
//!
//! Key details:
//! - FFI metadata must preserve target-independent type contracts while leaving register/stack layout to ABI helpers.
//! - Every C integer width maps to the PHP type `int` and C `float` to the PHP type `float`;
//!   `ExternAbiType` keeps the declared width so the call boundary can normalize registers
//!   without widening the PHP type system.
//! - Extern class fields use the natural C layout of the same widths (`ctype_field_layout`), so a
//!   struct declared with the header's field types has the header's offsets and `sizeof`.

use std::collections::HashMap;

use crate::parser::ast::CType;

use super::{PackedClassInfo, PhpType};

/// Register-level shape of one extern parameter or return slot, derived from the declared C type.
///
/// The PHP-visible type of every integer slot is `int`; the width only tells the call boundary
/// how to normalize the register, because both supported ABIs leave the bits above a narrow
/// return value unspecified and expect the caller to extend narrow arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbiType {
    /// Integer of `bits` width; `signed` selects sign or zero extension at the boundary.
    Int { bits: u8, signed: bool },
    /// C `double` in a floating-point register.
    Float64,
    /// C `float`: a single in the low half of the floating-point register, widened to the PHP double.
    Float32,
    /// C `bool`: only the low byte is defined.
    Bool,
    /// `char*` converted to and from a PHP string at the boundary.
    CStr,
    /// Any pointer, typed or opaque.
    Ptr,
    /// C function pointer bound from a PHP callable.
    Callable,
    /// No value (return only).
    Void,
}

impl ExternAbiType {
    /// The 64-bit signed integer shape of a plain `int`.
    pub const INT64: Self = Self::Int { bits: 64, signed: true };
}

/// Maps a parser CType to the ABI shape the call boundary normalizes.
pub fn ctype_to_abi_type(ct: &CType) -> ExternAbiType {
    match ct {
        CType::Int => ExternAbiType::INT64,
        CType::IntWidth(width) => ExternAbiType::Int { bits: width.bits(), signed: width.signed() },
        CType::Float => ExternAbiType::Float64,
        CType::F32 => ExternAbiType::Float32,
        CType::Str => ExternAbiType::CStr,
        CType::Bool => ExternAbiType::Bool,
        CType::Void => ExternAbiType::Void,
        CType::Ptr | CType::TypedPtr(_) => ExternAbiType::Ptr,
        CType::Callable => ExternAbiType::Callable,
    }
}

/// Maps a parser CType to its corresponding PhpType for FFI and extern declaration lowering.
///
/// Used when lowering `extern` declarations to connect the parser's C types (Int, IntWidth,
/// Float, F32, Str, Bool, Void, Ptr, TypedPtr, Callable) to the compiler's internal PhpType
/// representation. Every C integer width is a PHP `int` and both float widths are a PHP `float`.
pub fn ctype_to_php_type(ct: &CType) -> PhpType {
    match ct {
        CType::Int | CType::IntWidth(_) => PhpType::Int,
        CType::Float | CType::F32 => PhpType::Float,
        CType::Str => PhpType::Str,
        CType::Bool => PhpType::Bool,
        CType::Void => PhpType::Void,
        CType::Ptr => PhpType::Pointer(None),
        CType::TypedPtr(name) => PhpType::Pointer(Some(name.clone())),
        CType::Callable => PhpType::Callable,
    }
}

/// Returns the natural C size and alignment, in bytes, of an extern class field type.
///
/// Every supported target (macOS and iOS arm64, Linux aarch64 and x86_64) is LP64 with
/// natural alignment for these scalars, so one table serves them all: `int`, `i64`, `u64`,
/// `float`/`f64`, pointers and `string` (`char*`) are 8 bytes, `i32`/`u32`/`f32` 4, `i16`/`u16` 2,
/// and `i8`/`u8`/`bool` (C `_Bool`) one byte, each aligned to its own size. Returns `None` for
/// `void` and `callable`, which cannot be stored in a struct field.
pub fn ctype_field_layout(ct: &CType) -> Option<(usize, usize)> {
    match ct {
        CType::Int | CType::Float | CType::Ptr | CType::TypedPtr(_) | CType::Str => Some((8, 8)),
        CType::IntWidth(width) => {
            let bytes = usize::from(width.bits() / 8);
            Some((bytes, bytes))
        }
        CType::F32 => Some((4, 4)),
        CType::Bool => Some((1, 1)),
        CType::Void | CType::Callable => None,
    }
}

/// Rounds `offset` up to the next multiple of `align` (a power of two, at least one).
pub fn align_up(offset: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two(), "C alignments are powers of two");
    (offset + align - 1) & !(align - 1)
}

/// Returns the ABI size for a PhpType, or None if the type has no fixed size.
pub fn packed_type_size(
    ty: &PhpType,
    packed_classes: &HashMap<String, PackedClassInfo>,
) -> Option<usize> {
    match ty {
        PhpType::Int
        | PhpType::Float
        | PhpType::Bool
        | PhpType::False
        | PhpType::Pointer(_)
        | PhpType::Resource(_) => Some(8),
        PhpType::Packed(name) => packed_classes.get(name).map(|info| info.total_size),
        _ => None,
    }
}
