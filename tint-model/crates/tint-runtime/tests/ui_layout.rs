// Proves ui/layout.rs computes REAL flexbox pixel positions from a
// UiTree -- the same tree the DOM backend turns into CSS strings, now
// fed through taffy instead. This is the first slice of a second
// (GPU-bound) rendering backend: no DOM/browser involved, no CSS
// string parsing by a browser -- just Rust computing where every node
// actually lands, which a future vello/wgpu painter would use directly.

use tint_ast::Item;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::ui::builder::UiBuilder;
use tint_runtime::ui::layout::compute_layout;
use tint_runtime::vm::TintVM;

fn parse_ui_fn(code: &str, name: &str) -> tint_ast::UiFnDecl {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    for item in program.items {
        if let Item::UiFn(f) = item {
            if f.name == name {
                return f;
            }
        }
    }

    panic!("ui fn `{}` not found", name);
}

// Mirrors the sandbox demo's account-cards row: three fixed-width cards
// in a `direction::row` with `justify::space-between`, exactly what
// Round 3 asked for ("categories more even, no dead space"). Proves it
// with real numbers instead of trusting the browser will do it right.
#[test]
fn justify_space_between_spreads_cards_evenly_across_the_row() {
    let code = r#"
ui fn App() {
    Row {
        direction::row
        justify::space-between

        MiniCard { padding::12 }
        MiniCard { padding::12 }
        MiniCard { padding::12 }
    }
}
"#;
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    let root = builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let layout = compute_layout(&tree, root, 600.0, 200.0);

    let row = tree.nodes.iter().find(|n| n.tag == "Row").unwrap();
    let cards: Vec<_> = tree
        .nodes
        .iter()
        .filter(|n| n.tag == "MiniCard")
        .collect();
    assert_eq!(cards.len(), 3);

    let row_rect = layout[&row.id];
    let rects: Vec<_> = cards.iter().map(|c| layout[&c.id]).collect();

    // First card flush against the row's left edge, last card flush
    // against the right edge -- not all three clumped on the left with
    // dead space after them (the exact bug the screenshots showed).
    assert!((rects[0].x - row_rect.x).abs() < 0.01);
    let last = &rects[2];
    assert!(((last.x + last.width) - (row_rect.x + row_rect.width)).abs() < 0.01);

    // The gap after card 1 and after card 2 should match (evenly
    // spread), and be clearly nonzero -- not all three touching.
    let gap1 = rects[1].x - (rects[0].x + rects[0].width);
    let gap2 = rects[2].x - (rects[1].x + rects[1].width);
    assert!(gap1 > 1.0, "expected a real gap after card 1, got {gap1}");
    assert!((gap1 - gap2).abs() < 0.5, "gaps should be even: {gap1} vs {gap2}");
}

// Mirrors the balance card's clustered spacing (Round 3): a column
// where two pairs of rows are pulled close via `margin::{bottom::4}`,
// separated by one bigger gap via `margin::{bottom::28}` on the middle
// item -- proving the CLUSTERING is real pixel distance, not just "some
// margin got applied somewhere".
#[test]
fn margin_bottom_clusters_rows_unevenly() {
    let code = r#"
ui fn App() {
    Column {
        direction::column

        Row { margin::{bottom::4} "a" }
        Row { margin::{bottom::28} "b" }
        Row { margin::{bottom::4} "c" }
        Row { "d" }
    }
}
"#;
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    let root = builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let layout = compute_layout(&tree, root, 400.0, 400.0);

    let column = tree.nodes.iter().find(|n| n.tag == "Column").unwrap();
    let rows: Vec<_> = column
        .children
        .iter()
        .map(|&id| layout[&id])
        .collect();
    assert_eq!(rows.len(), 4);

    let gap_ab = rows[1].y - (rows[0].y + rows[0].height);
    let gap_bc = rows[2].y - (rows[1].y + rows[1].height);
    let gap_cd = rows[3].y - (rows[2].y + rows[2].height);

    // a-b close, b-c far (the big separator), c-d close again.
    assert!(gap_ab < gap_bc, "a-b ({gap_ab}) should be tighter than b-c ({gap_bc})");
    assert!(gap_cd < gap_bc, "c-d ({gap_cd}) should be tighter than b-c ({gap_bc})");
    assert!((gap_ab - 4.0).abs() < 0.01);
    assert!((gap_cd - 4.0).abs() < 0.01);
}

// Mirrors the mini-cards row after Mark asked for "full width, with real
// gaps between them" instead of content-sized cards spread apart by
// `justify::space-between`: three `MiniCard`s each carrying `grow::1`
// inside a `direction::row gap::n` container should split the row's
// full width EQUALLY between them, with a real (non-content) gap after
// each one except the last.
#[test]
fn grow_splits_the_row_width_evenly_with_real_gaps() {
    let code = r#"
ui fn App() {
    Row {
        direction::row
        gap::12

        MiniCard { grow::1 padding::12 }
        MiniCard { grow::1 padding::12 }
        MiniCard { grow::1 padding::12 }
    }
}
"#;
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    let root = builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let layout = compute_layout(&tree, root, 600.0, 200.0);

    let row = tree.nodes.iter().find(|n| n.tag == "Row").unwrap();
    let cards: Vec<_> = tree
        .nodes
        .iter()
        .filter(|n| n.tag == "MiniCard")
        .map(|c| layout[&c.id])
        .collect();
    assert_eq!(cards.len(), 3);

    let row_rect = layout[&row.id];

    // Flush against both edges of the row -- filling its full width,
    // not sitting at content size with dead space after the last card.
    assert!((cards[0].x - row_rect.x).abs() < 0.01);
    let last = &cards[2];
    assert!(((last.x + last.width) - (row_rect.x + row_rect.width)).abs() < 0.01);

    // All three the same width (equal `grow::1` share of the row, minus
    // the two 12px gaps split between them).
    assert!((cards[0].width - cards[1].width).abs() < 0.01);
    assert!((cards[1].width - cards[2].width).abs() < 0.01);
    let expected_width = (600.0 - 2.0 * 12.0) / 3.0;
    assert!((cards[0].width - expected_width).abs() < 0.01, "{} vs {}", cards[0].width, expected_width);

    // Real 12px gaps between them, not zero.
    let gap1 = cards[1].x - (cards[0].x + cards[0].width);
    let gap2 = cards[2].x - (cards[1].x + cards[1].width);
    assert!((gap1 - 12.0).abs() < 0.01);
    assert!((gap2 - 12.0).abs() < 0.01);
}
