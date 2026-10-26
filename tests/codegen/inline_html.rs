//! Purpose:
//! Integration or regression tests for end-to-end codegen of PHP files that leave PHP mode: the
//! `?>` close tag, inline HTML after it, and the `<?=` short echo tag, in a main file and in a
//! statically required file.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Every expected string is what `php` 8.5 prints for the same program, byte for byte.
//! - A close tag lowers to `;`, inline HTML to `echo "…";` and `<?=` to `echo`, so the tests
//!   also cover the empty statement and the `;` PHP allows before a switch's first case.

use crate::support::*;

/// The ticket's program: a close tag ends the statement, its newline is swallowed, and `<?=`
/// echoes inside HTML.
#[test]
fn test_close_tag_and_short_echo_print_inline_html() {
    let out = compile_and_run("<?php $x = 1; ?>\n<ul><?= $x ?></ul>\n");
    assert_eq!(out, "<ul>1</ul>\n");
}

/// Alternative syntax opened and closed across tags, for every control structure, with and
/// without a `;` before `?>`.
#[test]
fn test_alternative_syntax_spans_close_tags() {
    let out = compile_and_run(
        r#"<?php $rows = ['a', 'b<c', "d'e"]; $n = 2; ?>
<ul>
<?php foreach ($rows as $i => $r): ?>
  <li data-i="<?= $i ?>"><?= $r ?></li>
<?php endforeach; ?>
</ul>
<ol>
<?php foreach ($rows as $r): ?>
  <li><?= $r; ?></li>
<?php endforeach ?>
</ol>
<?php if ($n > 2): ?>big<?php elseif ($n > 1): ?>two<?php else: ?>small<?php endif ?>

<?php for ($i = 0; $i < 3; $i++): ?>[<?= $i ?>]<?php endfor; ?>

<?php switch ($n): ?>
<?php case 1: ?>one<?php break; ?>
<?php case 2: ?>TWO<?php break; ?>
<?php default: ?>many<?php endswitch ?>

<?php if ($n === 2) { ?>brace-if<?php } else { ?>brace-else<?php } ?>

<?php while ($n > 0): ?>w<?= $n-- ?><?php endwhile; ?>
end
"#,
    );
    assert_eq!(
        out,
        "<ul>\n  <li data-i=\"0\">a</li>\n  <li data-i=\"1\">b<c</li>\n  <li data-i=\"2\">d'e</li>\n</ul>\n\
         <ol>\n  <li>a</li>\n  <li>b<c</li>\n  <li>d'e</li>\n</ol>\n\
         two\n[0][1][2]\nTWO\nbrace-if\nw2w1end\n"
    );
}

/// One newline after `?>` is swallowed and a second one is printed; a file may end in HTML with
/// or without a final newline.
#[test]
fn test_close_tag_swallows_exactly_one_newline() {
    assert_eq!(compile_and_run("<?php echo \"a\"; ?>\n<p>tail one</p>\n"), "a<p>tail one</p>\n");
    assert_eq!(compile_and_run("<?php echo \"a\"; ?>\n\n<p>two newlines</p>"), "a\n<p>two newlines</p>");
}

/// `\r\n` and `\r` after `?>` are swallowed like `\n`; a `\r\n` inside HTML is printed.
#[test]
fn test_close_tag_swallows_carriage_return_newlines() {
    let out = compile_and_run("<?php echo \"a\";\n?>\r\nCRLF eaten\r\n<?php echo \"b\" ?>\r<?php echo \"c\";\n");
    assert_eq!(out, "aCRLF eaten\r\nbc");
}

/// HTML that looks like code stays text: XML declarations, `?>` and `<?` inside HTML, `<?phpX`,
/// variables, escapes and format specifiers; `<?PHP` and `<?Php` reopen code.
#[test]
fn test_inline_html_that_looks_like_code_stays_text() {
    let out = compile_and_run(
        "<?php $v = 'V'; ?>\n\
         <?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- a comment holding ?> and <? and <?xml, all text -->\n\
         <a href=\"x?y=1&z=2\" title='single \"double\"'>$v {$v} \\n \\t \\\\ \\' %s %d</a>\n\
         <script>if (a < b && c > d) { x = \"?>\"; }</script>\n\
         <?phpX not a tag <? short not a tag <?PHP echo 'UPPER'; ?>|<?Php echo 'mixed' ?>|é ü\n",
    );
    assert_eq!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- a comment holding ?> and <? and <?xml, all text -->\n\
         <a href=\"x?y=1&z=2\" title='single \"double\"'>$v {$v} \\n \\t \\\\ \\' %s %d</a>\n\
         <script>if (a < b && c > d) { x = \"?>\"; }</script>\n\
         <?phpX not a tag <? short not a tag UPPER|mixed|é ü\n"
    );
}

/// `?>` inside strings, heredoc, nowdoc, array keys and block comments is data; a `//` or `#`
/// line comment ends at `?>`, which then closes PHP.
#[test]
fn test_close_tag_in_strings_is_data_and_ends_line_comments() {
    let out = compile_and_run(
        r#"<?php
$a = "dq ?> inside";
$b = 'sq ?> inside';
$h = <<<EOT
heredoc ?> inside $a
EOT;
$n = <<<'EOT'
nowdoc ?> inside
EOT;
$arr = ['?>' => 'key'];
/* block ?> comment */
echo $a, "|", $b, "|", $h, "|", $n, "|", $arr['?>'], "|{$arr['?>']}|\n";
echo "line-comment:"; // ends here ?> HTML after line comment
<?php echo "hash:"; # hash ?> HTML after hash comment
<?php echo "\n";
"#,
    );
    assert_eq!(
        out,
        "dq ?> inside|sq ?> inside|heredoc ?> inside dq ?> inside|nowdoc ?> inside|key|key|\n\
         line-comment: HTML after line comment\nhash: HTML after hash comment\n\n"
    );
}

/// A line comment holding `?>` no longer runs to the newline: the rest of the line is HTML.
#[test]
fn test_line_comment_close_tag_switches_to_html() {
    let out = compile_and_run("<?php echo \"a\"; // note ?> B\necho \"c\";\n");
    assert_eq!(out, "a B\necho \"c\";\n");
}

/// `__LINE__` keeps counting lines through inline HTML.
#[test]
fn test_line_numbers_count_through_inline_html() {
    let out = compile_and_run("<?php echo __LINE__; ?>\nline two\nline three <?= __LINE__ ?>\n\n<?php echo __LINE__, \"\\n\";\n");
    assert_eq!(out, "1line two\nline three 3\n5\n");
}

/// Empty statements: `<?php ?>`, `;;;`, adjacent tags, `<?=` with a list, and a block before `?>`.
#[test]
fn test_empty_statements_between_tags() {
    let out = compile_and_run("<?php ?>A<?php ?><?php ;;; ?>B<?php echo 1 ?><?= 2, 3 ?><?php if (1) {} ?>C\n");
    assert_eq!(out, "AB123C\n");
}

/// A private-use character in inline HTML prints its own UTF-8 bytes.
#[test]
fn test_private_use_character_in_inline_html_prints_its_bytes() {
    let out = compile_and_run("<?php $m = \"m\"; ?>\n<p>\u{e001} private-use, é accent</p>\n");
    assert_eq!(out, "<p>\u{e001} private-use, é accent</p>\n");
}

/// `declare(strict_types=1)` before a close tag keeps strict mode for the code after the tag:
/// PHP throws `TypeError` for `f("5")`, and elephc refuses the call when it type-checks.
#[test]
fn test_strict_types_survives_a_close_tag() {
    let error = compile_expect_type_error(
        "<?php declare(strict_types=1); ?>\n<?php\nfunction f(int $i): int { return $i; }\necho f(\"5\");\n",
    );
    assert!(error.contains("strict_types=1"), "{error}");
}

/// A brace switch and a class declared across tags.
#[test]
fn test_brace_switch_and_class_across_close_tags() {
    let out = compile_and_run(
        "<?php switch (2) { ?>\n<?php case 1: ?>one<?php break; case 2: ?>two<?php break; } ?>\n\
         <?php class A { public function n(): int { return 1; } } ?>\n<?= (new A())->n() ?>\n",
    );
    assert_eq!(out, "two1");
}

/// A required file that starts in PHP and ends in HTML, with alternative syntax inside.
#[test]
fn test_required_file_with_inline_html() {
    let out = compile_and_run_files(
        &[
            ("main.php", "<?php\n$items = ['x', 'y'];\nrequire __DIR__ . '/part.php';\necho \"after require\\n\";\n"),
            (
                "part.php",
                "<?php $count = count($items); ?>\n<div><?= $count ?> items:<?php foreach ($items as $it): ?> \
                 <b><?= $it ?></b><?php endforeach ?></div>\ntrailing html, no close tag\n",
            ),
        ],
        "main.php",
    );
    assert_eq!(out, "<div>2 items: <b>x</b> <b>y</b></div>\ntrailing html, no close tag\nafter require\n");
}

/// A literal `eval()` fragment holding `?>` and `<?=` still matches PHP now that it tokenizes.
#[test]
fn test_eval_literal_with_close_tags_matches_php() {
    let out = compile_and_run(
        "<?php\neval('echo \"a\"; ?>HTML<?php echo \"b\";');\neval('?>[<?= 1 + 2 ?>]' . \"\\n\");\n\
         $x = 'X';\neval('if (true) { ?>in-<?= $x ?><?php } ?>' . \"\\n\" . 'after');\necho \"\\n\";\n",
    );
    assert_eq!(out, "aHTMLb[3]\nin-Xafter\n");
}

/// The template shape AIC renders: a class method requires a template between `ob_start()` and
/// `ob_get_clean()`, the template types its variables in a docblock before `?>`, renders a child
/// template through `$pass`, and is required again for each render.
#[test]
fn test_templates_required_from_methods_render_like_php() {
    let out = compile_and_run_files(
        &[
            (
                "main.php",
                r#"<?php
declare(strict_types=1);

require __DIR__ . '/Status.php';
require __DIR__ . '/Greetings.php';
require __DIR__ . '/Renderer.php';

use App\Site\Greetings;
use App\Site\Renderer;
use App\Site\Status;

$renderer = new Renderer();
$next = new Greetings([], 'Second', Status::Open);
echo $renderer->page(new Greetings(['Hello, &lt;Chad&gt;', 'Hola, O&#039;Brien'], 'Hi &amp; bye', Status::Sold, $next), 'Greetings');
echo $renderer->greetings($next);
for ($i = 0; $i < 3; $i++) {
    echo strlen($renderer->greetings(new Greetings(['x' . $i], null, Status::Open))), "\n";
}
"#,
            ),
            (
                "Status.php",
                "<?php\ndeclare(strict_types=1);\n\nnamespace App\\Site;\n\nenum Status: string\n{\n    case Open = 'open';\n    case Sold = 'sold';\n}\n",
            ),
            (
                "Greetings.php",
                r#"<?php
declare(strict_types=1);

namespace App\Site;

final readonly class Greetings
{
    public const string LABEL = 'greetings';

    /** @param list<string> $greetings */
    public function __construct(
        public array $greetings,
        public ?string $title,
        public Status $status,
        public ?Greetings $next = null,
    ) {
    }
}
"#,
            ),
            (
                "Renderer.php",
                r#"<?php
declare(strict_types=1);

namespace App\Site;

final class Renderer
{
    public function greetings(Greetings $greetings): string
    {
        ob_start();
        require __DIR__ . '/greetings.tpl.php';
        return (string) ob_get_clean();
    }

    public function page(Greetings $greetings, string $heading): string
    {
        $pass = $this;
        ob_start();
        require __DIR__ . '/page.tpl.php';
        return (string) ob_get_clean();
    }
}
"#,
            ),
            (
                "greetings.tpl.php",
                r#"<?php /** @var \App\Site\Greetings $greetings */ ?>
<?php if ($greetings->title !== null): ?>
<h2 class="<?= \App\Site\Greetings::LABEL ?>"><?= $greetings->title ?></h2>
<?php endif ?>
<ul id="greetings"><!--t:b-->
<?php foreach ($greetings->greetings as $i => $greeting): ?>
  <li data-n="<?= $i + 1 ?>"<?php if ($i === 0): ?> class="first"<?php endif; ?>><?= $greeting ?></li>
<?php endforeach; ?>
<?php if (empty($greetings->greetings)): ?>  <li>none</li>
<?php endif ?>
<!--/t:b--></ul>
<p><?= $greetings->status === \App\Site\Status::Sold ? 'sold' : 'open' ?> / <?= $greetings->next?->title ?? 'no next' ?> / <?= match ($greetings->status) { \App\Site\Status::Open => 'O', \App\Site\Status::Sold => 'S' } ?></p>
"#,
            ),
            (
                "page.tpl.php",
                r#"<?php /** @var \App\Site\Greetings $greetings */ /** @var string $heading */ /** @var \App\Site\Renderer $pass */ ?>
<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title><?= $heading ?></title></head>
<body>
<main data-region="content"><?= $pass->greetings($greetings) ?></main>
<?php for ($k = 0; $k < 2; $k++): ?><span><?= $k ?></span><?php endfor ?>

</body>
</html>
"#,
            ),
        ],
        "main.php",
    );
    assert_eq!(
        out,
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Greetings</title></head>\n<body>\n\
         <main data-region=\"content\"><h2 class=\"greetings\">Hi &amp; bye</h2>\n<ul id=\"greetings\"><!--t:b-->\n\
         \u{20}\u{20}<li data-n=\"1\" class=\"first\">Hello, &lt;Chad&gt;</li>\n\
         \u{20}\u{20}<li data-n=\"2\">Hola, O&#039;Brien</li>\n<!--/t:b--></ul>\n<p>sold / Second / S</p>\n</main>\n\
         <span>0</span><span>1</span>\n</body>\n</html>\n\
         <h2 class=\"greetings\">Second</h2>\n<ul id=\"greetings\"><!--t:b-->\n  <li>none</li>\n<!--/t:b--></ul>\n\
         <p>open / no next / O</p>\n112\n112\n112\n"
    );
}
