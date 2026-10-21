//! Purpose:
//! Pins executable shutdown ownership for emitted class static properties.
//!
//! Called from:
//! - The AST-to-EIR unit-test module through Rust's test harness.
//!
//! Key details:
//! - All five targets must skip uninitialized slots and detach inherited owners once.
//! - Destructor boundaries must finish remaining slots before propagating exceptions.

/// Verifies mixed static roots use storage-aware hash writes and captured prefix results.
fn assert_mixed_static_update_storage(name: &str) {
    let source = r#"<?php
class ReviewMixedStorage {
    public static array<mixed> $items = [[5], [7]];
    public static array<string, mixed> $flat = ['k' => 5];
    public static array $boxed = [[10]];
}
function bump(int $key): void { ++ReviewMixedStorage::$items[$key][0]; }
++ReviewMixedStorage::$items[null][0];
++ReviewMixedStorage::$items[null][0]; bump(0);
++ReviewMixedStorage::$flat['k'];
echo ++ReviewMixedStorage::$boxed[1.5][0];
"#;
    let target = crate::codegen::platform::Target::parse(name).unwrap();
    let module = super::lower_source_at_for_target(source,
        std::path::Path::new("main.php"), std::path::Path::new("."), target);
    let ir = crate::ir::print_module(&module);
    assert!(ir.contains("array_to_hash"), "{name}: {ir}");
    assert!(!ir.contains("array_get_for_write"), "{name}: {ir}");
    crate::codegen::generate_user_asm_from_ir(&module, false, false).unwrap();
}

/// macOS emits storage-aware concrete mixed-static updates.
#[test]
fn mixed_static_update_storage_macos_aarch64() {
    assert_mixed_static_update_storage("macos-aarch64");
}

/// iOS devices emit storage-aware concrete mixed-static updates.
#[test]
fn mixed_static_update_storage_ios_arm64() {
    assert_mixed_static_update_storage("ios-arm64");
}

/// iOS simulators emit storage-aware concrete mixed-static updates.
#[test]
fn mixed_static_update_storage_ios_sim_arm64() {
    assert_mixed_static_update_storage("ios-sim-arm64");
}

/// Linux ARM64 emits storage-aware concrete mixed-static updates.
#[test]
fn mixed_static_update_storage_linux_aarch64() {
    assert_mixed_static_update_storage("linux-aarch64");
}

/// Linux x86_64 emits storage-aware concrete mixed-static updates.
#[test]
fn mixed_static_update_storage_linux_x86_64() {
    assert_mixed_static_update_storage("linux-x86_64");
}

/// Nested updates capture keys once and reuse the read's normalization on every target.
#[test]
fn nested_static_update_keys_are_captured_on_all_targets() {
    let source = r#"<?php
class ReviewNestedKeys { public static array $items = [[1 => 10, 2 => 20]]; }
$key = 1.9;
set_error_handler(function($level, $message) use (&$key) { $key = 2.9; return true; });
++ReviewNestedKeys::$items[0][$key];
restore_error_handler();
echo json_encode(ReviewNestedKeys::$items);
"#;
    for name in ["macos-aarch64", "ios-arm64", "ios-sim-arm64", "linux-aarch64", "linux-x86_64"] {
        let target = crate::codegen::platform::Target::parse(name).unwrap();
        let module = super::lower_source_at_for_target(source,
            std::path::Path::new("main.php"), std::path::Path::new("."), target);
        let ir = crate::ir::print_module(&module);
        assert!(ir.contains("array.fetch_for_write_already_diagnosed"), "{name}: {ir}");
        crate::codegen::generate_user_asm_from_ir(&module, false, false).unwrap();
    }
}

/// Every target roots the nullable receiver after the delayed RHS and retires it after the store.
#[test]
fn nullable_static_property_write_receiver_is_retained_on_all_targets() {
    let source = r#"<?php
class ReviewPinnedObject { public int $v = 1; }
class ReviewPinnedHolder { public static ?ReviewPinnedObject $o = null; }
function replaceReviewPinnedObject(): int {
    ReviewPinnedHolder::$o = new ReviewPinnedObject();
    return 9;
}
function writeReviewPinnedObject(): void {
    ReviewPinnedHolder::$o->v = replaceReviewPinnedObject();
}
ReviewPinnedHolder::$o = new ReviewPinnedObject();
writeReviewPinnedObject();
"#;
    for name in ["macos-aarch64", "ios-arm64", "ios-sim-arm64", "linux-aarch64", "linux-x86_64"] {
        let target = crate::codegen::platform::Target::parse(name).unwrap();
        let module = super::lower_source_at_for_target(source,
            std::path::Path::new("main.php"), std::path::Path::new("."), target);
        let function = module.functions.iter().find(|function| function.name == "writeReviewPinnedObject").unwrap();
        let store = function.instructions.iter().position(|inst| inst.op == crate::ir::Op::PropSet).unwrap();
        let rhs = function.instructions.iter().position(|inst| inst.op == crate::ir::Op::Call).unwrap();
        let load = function.instructions.iter().position(|inst| inst.op == crate::ir::Op::LoadStaticProperty).unwrap();
        assert!(rhs < load && load < store, "{name}: PHP delays the static receiver fetch");
        let object = function.value(function.instructions[store].operands[0]).unwrap();
        let crate::ir::ValueDef::Instruction { inst, .. } = object.def else {
            panic!("{name}: the property receiver must be an acquired value");
        };
        let narrowed = function.instruction(inst).unwrap();
        assert_eq!(narrowed.op, crate::ir::Op::MixedUnbox, "{name}");
        assert_eq!(object.ownership, crate::ir::Ownership::Owned, "{name}: the object projection owns its lease");
        let boxed = function.value(narrowed.operands[0]).unwrap();
        let crate::ir::ValueDef::Instruction { inst, .. } = boxed.def else {
            panic!("{name}: the nullable static slot must be retained before narrowing");
        };
        let acquired = function.instruction(inst).unwrap();
        assert_eq!(acquired.op, crate::ir::Op::Acquire, "{name}");
        assert_eq!(acquired.operands, vec![function.instructions[load].result.unwrap()], "{name}");
        assert!(function.instructions[store + 1..].iter().any(|inst|
            inst.op == crate::ir::Op::Release && inst.operands == [function.instructions[store].operands[0]]), "{name}");
        assert!(function.instructions[..store].iter().any(|inst| inst.op == crate::ir::Op::PushCallOperandOwner), "{name}");
        assert!(function.instructions[store + 1..].iter().any(|inst| inst.op == crate::ir::Op::PopCallOperandOwner), "{name}");
        crate::codegen::generate_user_asm_from_ir(&module, false, false).unwrap();
    }
}

/// Every target retires inherited class-static owners before bounded shutdown release.
#[test]
fn class_static_shutdown_ownership_is_emitted_on_all_targets() {
    let source = r#"<?php
class ShutdownEmitterOwner {
    public static string $text = "";
    public static mixed $payload = null;
    public static string $uninitialized;
}
class ShutdownEmitterChild extends ShutdownEmitterOwner {}
ShutdownEmitterChild::$text = str_repeat("x", $argc);
ShutdownEmitterChild::$payload = ["key" => $argc];
echo isset(ShutdownEmitterChild::$uninitialized);
"#;
    for name in ["macos-aarch64", "ios-arm64", "ios-sim-arm64", "linux-aarch64", "linux-x86_64"] {
        let target = crate::codegen::platform::Target::parse(name).unwrap();
        let module = super::lower_source_at_for_target(
            source, std::path::Path::new("main.php"), std::path::Path::new("."), target,
        );
        let asm = crate::codegen::generate_user_asm_from_ir(&module, false, false).unwrap();
        for property in ["text", "payload", "uninitialized"] {
            let symbol = crate::names::static_property_symbol("ShutdownEmitterOwner", property);
            let comment = format!("epilogue cleanup static property {symbol}");
            assert_eq!(asm.matches(&comment).count(), 1, "{name}: {property}");
            let cleanup = asm.split_once(&comment).unwrap().1;
            let retired = cleanup.find("retire the static property").unwrap();
            let released = cleanup.find("__rt_cleanup_preserve_exception").unwrap();
            assert!(retired < released, "{name}: {property} must retire before destructor entry");
            let inherited = crate::names::static_property_symbol("ShutdownEmitterChild", property);
            assert!(!asm.contains(&format!("epilogue cleanup static property {inherited}")), "{name}");
        }
        let tail = asm.rsplit_once("retire the static property").unwrap().1;
        assert!(tail.find("__rt_cleanup_preserve_exception").unwrap() < tail.find("__rt_throw_current").unwrap(), "{name}");
    }
}
