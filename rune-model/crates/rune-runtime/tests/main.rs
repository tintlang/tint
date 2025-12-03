use rune_parser::Parser;
use rune_lexer::Lexer;
use rune_runtime::vm::RuneVM;
use rune_ast::{Item, FnBody, Stmt, Expr};
use rune_evaluator::EvalHost;

fn format_expr(e: &Expr) -> String {
    use Expr::*;
    match e {
        Ident(name, _) => name.clone(),
        Number(n, _) => n.clone(),
        Binary { left, op, right, .. } =>
            format!("{} {} {}", format_expr(left), op, format_expr(right)),
        _ => format!("<expr {:?}>", e),
    }
}

fn print_fn_source(item: &Item) {
    if let Item::Fn(func) = item {
        println!("EXECUTED CODE:");
        println!("fn {}() {{", func.name);

        match &func.body {
            FnBody::Block(block) => {
                for stmt in &block.stmts {
                    match stmt {
                        Stmt::Let { name, expr, .. } =>
                            println!("    let {} = {};", name, format_expr(expr)),
                        Stmt::Return(expr, _) =>
                            println!("    return {};", format_expr(expr)),
                        _ => println!("    {:?}", stmt),
                    }
                }
            }
            FnBody::Expr(expr) =>
                println!("    {}", format_expr(expr)),
        }

        println!("}}\n");
    }
}

#[test]
fn test_complex_math_plus_ui() {
    let code = r#"
        ui fn Show(msg: string) {
            <Text padding{12} color{blue}> "OUT: {msg}" </Text>
        }

        ui fn Status(label: string, value: number) {
            <Row gap.x{8} padding{4}>
                <Text>"{label}"</Text>
                <Text color{green}>"{value}"</Text>
            </Row>
        }

        ui fn Card(title: string, msg: string) {
            <Panel padding{16} radius{12}>
                <Text weight{700}> "{title}" </Text>
                <Text>"{msg}"</Text>
            </Panel>
        }

        fn Calc2() {
            let x = 12;
            let y = 4;
            let z = 3;

            let r = (x * y - 6) + (z * (y + 2)) - 5;

            Show("Result is {r}");
            Status("Value:", r);
            Card("Final", "Computed value is {r}");

            return r;
        }
    "#;

    println!("================ CODE ================");
    println!("{code}");
    println!("======================================");

    // ---------------- TOKEN DUMP ----------------
    let mut lexer = Lexer::new(code);
    let tokens = rune_lexer::collect_tokens(&mut lexer);

    println!("\n============= TOKENS =============");
    for t in &tokens {
        println!("{:?}", t);
    }

    // ---------------- PARSER ----------------
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parser failed");

    println!("\n============= AST =============");
    for item in &program.items {
        println!("{:#?}", item);
    }

    // ---------------- RUNTIME ----------------
    let mut vm = RuneVM::new();
    vm.run_program(&program);

    println!("\n============= RUN Calc2() =============");

    let out = vm.call_fn("Calc2", &[], rune_ast::Span::dummy())
        .expect("Call failed");

    let result = match out {
        rune_evaluator::value::Value::Number(n) => n as i64,
        _ => panic!("Calc2 must return number"),
    };

    println!("RESULT = {}", result);
    assert_eq!(result, 55);

    println!("============= TEST PASSED =============");
}
