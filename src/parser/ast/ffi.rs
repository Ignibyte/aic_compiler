//! Purpose:
//! Defines AST records for elephantsc extern declarations and packed data layouts.
//! Represents C-facing scalar, pointer, buffer, function, global, and struct field metadata.
//!
//! Called from:
//! - `crate::parser::stmt::ffi` and downstream type/codegen FFI handling.
//!
//! Key details:
//! - These nodes describe compiler extensions, not PHP syntax, and must stay explicit in the AST.

use crate::span::Span;

use super::TypeExpr;

// --- FFI ---

/// Explicit C integer width in an extern declaration.
///
/// Plain `int` (and its alias `i64`) is the 64-bit default and stays `CType::Int`; these
/// variants let a declaration name the real width of a narrower or unsigned C integer so
/// the call boundary can normalize registers the C ABI leaves partially unspecified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CIntWidth {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    U64,
}

impl CIntWidth {
    /// Returns the bit width of the C integer.
    pub fn bits(self) -> u8 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::U64 => 64,
        }
    }

    /// Returns whether the C integer is signed.
    pub fn signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32)
    }
}

/// C type annotation for extern declarations.
#[derive(Debug, Clone, PartialEq)]
pub enum CType {
    Int,                    // int64_t / long, also spelled `i64`
    IntWidth(CIntWidth),    // explicit narrower or unsigned C integer (i8..u32, u64)
    Float,                  // double, also spelled `f64`
    F32,                    // C float; widened to a PHP double at the call boundary
    Str,        // char* (null-terminated)
    Bool,
    Void,
    Ptr,                    // opaque void*
    TypedPtr(String),       // ptr<ClassName>
    Callable,               // function pointer
}

/// A parameter in an extern function declaration, with a name and C type.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternParam {
    pub name: String,
    pub c_type: CType,
}

/// A field in an extern class (C struct) declaration, with a name and C type.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternField {
    pub name: String,
    pub c_type: CType,
}

/// A field within a `packed class` layout, with a name, type expression, and source span.
#[derive(Debug, Clone)]
pub struct PackedField {
    pub name: String,
    pub type_expr: TypeExpr,
    pub span: Span,
}

impl PartialEq for PackedField {
    /// Compares two `PackedField`s by name and type expression, ignoring span.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.type_expr == other.type_expr
    }
}
