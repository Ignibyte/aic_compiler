//! Purpose:
//! Gives `preg_match()` php-src's named-group keys for a literal pattern: rewrites
//! `preg_match('/(?<year>\d+)/', $s, $m)` to
//! `__elephc_preg_named_result($m = __elephc_preg_named_matches('/(?<year>\d+)/', $s, [1 => 'year']))`,
//! whose prelude helpers run the ordinary call and store each named group under its name too.
//!
//! Called from:
//! - `crate::pipeline::compile()` and the integration-test frontend, before name resolution.
//!
//! Key details:
//! - The compiled regex functions build a numbered array only. The group NAMES and their numbers
//!   are fixed by a literal pattern, so they are read here, at compile time, and handed to the
//!   prelude as an `[number => name]` map.
//! - The matches reach the variable through an ordinary assignment, not a by-reference
//!   parameter, so an undefined `$m` is created as php-src creates it, at the top level too. The
//!   status helper answers what `preg_match()` returned (1, 0 or false).
//! - Only the three-argument form with a string-literal pattern and a plain variable is
//!   rewritten. A pattern this reader cannot number with certainty (the `x` or `J` modifier, an
//!   inline `(?x)`, a branch reset `(?|`, a `\Q...\E` quote, a duplicate name) is left alone, so
//!   it keeps today's numbered result rather than a wrong name.
//! - The prelude is injected only when a call was rewritten.

use crate::magic_constants::walker::{walk_program, Pass};
use crate::names::Name;
use crate::parser::ast::{Expr, ExprKind, MagicConstant, Program};
use crate::span::Span;

/// The prelude function that runs the call and returns the named matches.
const MATCHES: &str = "__elephc_preg_named_matches";
/// The prelude function that answers what the call returned.
const RESULT: &str = "__elephc_preg_named_result";

/// The prelude source, ordinary PHP.
const SOURCE: &str = include_str!("preg_named_groups/named.php");

/// Rewrites each eligible `preg_match()` call and prepends the prelude when one was rewritten.
pub fn rewrite(
    program: Program,
    inventory: &mut crate::optimize::reachability::PreludeInventory,
) -> Program {
    let mut pass = NamedGroups { rewrote: false };
    let program = walk_program(program, &mut pass);
    if !pass.rewrote {
        return program;
    }
    let tokens = crate::lexer::tokenize(SOURCE).expect("valid named-group prelude tokens");
    let mut combined = crate::parser::parse(&tokens).expect("valid named-group prelude");
    inventory.record_program("preg_named_groups", &combined);
    combined.extend(program);
    combined
}

/// The walker pass that rewrites `preg_match()` calls.
struct NamedGroups {
    /// Whether any call was rewritten, which decides the prelude injection.
    rewrote: bool,
}

impl Pass for NamedGroups {
    /// Leaves magic constants as they are.
    fn transform_magic(&self, _span: Span, mc: MagicConstant) -> ExprKind {
        ExprKind::MagicConstant(mc)
    }

    /// Rewrites `preg_match(LITERAL, $subject, $matches)` when the literal has named groups.
    fn transform_expr(&mut self, expr: Expr) -> Expr {
        let ExprKind::FunctionCall { name, args } = &expr.kind else {
            return expr;
        };
        let is_preg_match = name
            .as_str()
            .rsplit('\\')
            .next()
            .is_some_and(|segment| segment.eq_ignore_ascii_case("preg_match"));
        if !is_preg_match || args.len() != 3 {
            return expr;
        }
        let ExprKind::StringLiteral(pattern) = &args[0].kind else {
            return expr;
        };
        if args.iter().any(|arg| matches!(arg.kind, ExprKind::Spread(_) | ExprKind::NamedArg { .. }))
            || !matches!(args[2].kind, ExprKind::Variable(_))
        {
            return expr;
        }
        let Some(groups) = named_groups(pattern).filter(|groups| !groups.is_empty()) else {
            return expr;
        };
        let span = expr.span;
        let names = Expr::new(
            ExprKind::ArrayLiteralAssoc(
                groups
                    .into_iter()
                    .map(|(number, group)| {
                        (
                            Expr::new(ExprKind::IntLiteral(number as i64), span),
                            Expr::new(ExprKind::StringLiteral(group), span),
                        )
                    })
                    .collect(),
            ),
            span,
        );
        let matches = Expr::new(
            ExprKind::FunctionCall {
                name: Name::unqualified(MATCHES),
                args: vec![args[0].clone(), args[1].clone(), names],
            },
            span,
        );
        let assignment = Expr::new(
            ExprKind::Assignment {
                target: Box::new(args[2].clone()),
                value: Box::new(matches),
                result_target: None,
                prelude: Vec::new(),
                conditional_value_temp: None,
            },
            span,
        );
        self.rewrote = true;
        Expr::new(
            ExprKind::FunctionCall { name: Name::unqualified(RESULT), args: vec![assignment] },
            span,
        )
    }
}

/// Returns each named group of a delimited PCRE pattern as `(number, name)`, or `None` when the
/// pattern cannot be numbered with certainty (see the module notes).
fn named_groups(pattern: &str) -> Option<Vec<(usize, String)>> {
    let (body, modifiers) = split_delimiters(pattern)?;
    if modifiers.contains('x') || modifiers.contains('J') {
        return None;
    }
    let bytes = body.as_bytes();
    let mut groups = Vec::new();
    let mut number = 0usize;
    let mut in_class = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'\\' {
            if bytes.get(i + 1) == Some(&b'Q') {
                return None;
            }
            i += 2;
            continue;
        }
        if in_class {
            in_class = byte != b']';
            i += 1;
            continue;
        }
        if byte == b'[' {
            in_class = true;
            i += 1;
            if bytes.get(i) == Some(&b'^') {
                i += 1;
            }
            if bytes.get(i) == Some(&b']') {
                i += 1;
            }
            continue;
        }
        if byte != b'(' {
            i += 1;
            continue;
        }
        match (bytes.get(i + 1), bytes.get(i + 2)) {
            (Some(b'*'), _) => i += 2,
            (Some(b'?'), next) => {
                let rest = &body[i + 2..];
                if let Some((name, consumed)) = group_name(rest) {
                    number += 1;
                    if groups.iter().any(|(_, existing)| existing == &name) {
                        return None;
                    }
                    groups.push((number, name));
                    i += 2 + consumed;
                } else if next == Some(&b'|') || inline_flags_set_extended(rest) {
                    return None;
                } else {
                    i += 2;
                }
            }
            _ => {
                number += 1;
                i += 1;
            }
        }
    }
    Some(groups)
}

/// Reads a group name after `(?`: `<name>`, `P<name>` or `'name'`. Returns the name and the bytes
/// it spans, or `None` for any other `(?` construct (lookbehind `<=`/`<!` included).
fn group_name(rest: &str) -> Option<(String, usize)> {
    let (open, close) = if rest.starts_with("P<") {
        (2, '>')
    } else if rest.starts_with('<') && !rest.starts_with("<=") && !rest.starts_with("<!") {
        (1, '>')
    } else if rest.starts_with('\'') {
        (1, '\'')
    } else {
        return None;
    };
    let name: String = rest[open..].chars().take_while(|c| *c != close).collect();
    let valid = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let closed = rest[open + name.len()..].starts_with(close);
    (valid && closed).then(|| {
        let consumed = open + name.len() + 1;
        (name, consumed)
    })
}

/// Returns whether an inline option group after `(?` (`x)`, `ix:`, `-x)`) turns extended mode on.
fn inline_flags_set_extended(rest: &str) -> bool {
    let flags: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '-' || *c == '^')
        .collect();
    let terminated = rest[flags.len()..].starts_with(')') || rest[flags.len()..].starts_with(':');
    terminated && flags.split('-').next().is_some_and(|enabled| enabled.contains('x'))
}

/// Splits a PHP regex into its body and modifiers: any delimiter but an alphanumeric, a
/// backslash or whitespace, with the four bracket pairs closing on their partner.
fn split_delimiters(pattern: &str) -> Option<(&str, &str)> {
    let open = pattern.chars().next()?;
    if open.is_ascii_alphanumeric() || open == '\\' || open.is_whitespace() {
        return None;
    }
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        other => other,
    };
    let end = pattern.rfind(close).filter(|end| *end > 0)?;
    Some((&pattern[open.len_utf8()..end], &pattern[end + close.len_utf8()..]))
}

#[cfg(test)]
mod tests {
    use super::named_groups;

    /// Named groups are numbered with the plain groups around them; other `(?` forms are not.
    #[test]
    fn numbers_named_groups_among_plain_ones() {
        assert_eq!(
            named_groups(r"/(\d+)-(?<year>\d{4})(?:x)(?P<mon>\d\d)(?'day'\d\d)(?=z)/i"),
            Some(vec![(2, "year".into()), (3, "mon".into()), (4, "day".into())])
        );
        assert_eq!(named_groups(r"#[(](?<a>x)\(#"), Some(vec![(1, "a".into())]));
        assert_eq!(named_groups(r"/(?<=a)(b)/"), Some(vec![]));
    }

    /// Patterns this reader cannot number with certainty are refused.
    #[test]
    fn refuses_uncertain_patterns() {
        assert_eq!(named_groups(r"/(?<a>x) # c/x"), None);
        assert_eq!(named_groups(r"/(?x)(?<a>x)/"), None);
        assert_eq!(named_groups(r"/(?|(?<a>x)|(?<a>y))/"), None);
        assert_eq!(named_groups(r"/(?<a>x)(?<a>y)/J"), None);
        assert_eq!(named_groups(r"/\Q(\E(?<a>x)/"), None);
    }
}
