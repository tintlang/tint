// `app { js::"..." css::"..." }` metadata and host functions given to a
// `UiSession` before its `state` initializers run.

use std::rc::Rc;

use tint_evaluator::Value;
use tint_runtime::ui_session::{NativeFn, UiSession};

const SOURCE: &str = r#"
app {
    js::"./a.js"
    js::"./b.js"
    css::"./theme.css"
}

ui fn App() {
    state label = shout("hi")
    Column { Text { "{label}" } }
}
"#;

fn shout() -> (String, NativeFn) {
    let f: NativeFn = Rc::new(|args| match args.first() {
        Some(Value::String(s)) => Ok(Value::String(format!("{}!", s.to_uppercase()))),
        _ => Ok(Value::Unit),
    });
    ("shout".to_string(), f)
}

#[test]
fn app_block_lists_js_and_css_files() {
    let tokens = tint_lexer::collect_tokens(&mut tint_lexer::Lexer::new(SOURCE));
    let program = tint_parser::Parser::new(tokens).parse_program().unwrap();
    let meta = program.app_meta();
    assert_eq!(meta.js, vec!["./a.js", "./b.js"]);
    assert_eq!(meta.css, vec!["./theme.css"]);
}

#[test]
fn native_is_callable_from_a_state_initializer() {
    let mut session =
        UiSession::new_with_natives(SOURCE, "App", Default::default(), &[shout()]).unwrap();
    let tree = session.render().unwrap();
    assert!(format!("{:?}", tree).contains("HI!"));
}

#[test]
fn component_node_carries_name_and_canonical_props() {
    let source = r#"
struct P { n: number, label: string }

ui fn App() {
    state n = 2
    Column {
        Host { component||"Chart" props||{ P { n: n + 1, label: "a\"b" } } }
    }
}
"#;
    let mut session = UiSession::new(source, "App").unwrap();
    let tree = session.render().unwrap();
    let column = &tree[0];
    let host = &column.children[0];
    assert_eq!(host.component.as_deref(), Some("Chart"));
    assert_eq!(host.props.as_deref(), Some(r#"{"n":3,"label":"a\"b"}"#));
}

mod callbacks {
    use std::cell::RefCell;
    use std::rc::Rc;

    use tint_evaluator::{Callback, Value};
    use tint_runtime::ui_session::{NativeFn, UiSession};
    use tint_runtime::vm::set_deferred_runner;

    fn first_callback(args: &[Value]) -> Callback {
        match args.first() {
            Some(Value::Callback(cb)) => cb.clone(),
            other => panic!("expected a callback, got {other:?}"),
        }
    }

    #[test]
    fn native_calls_a_lambda_synchronously() {
        let each: NativeFn = Rc::new(|args| {
            let cb = first_callback(args);
            let mut sum = 0.0;
            for n in [1.0, 2.0, 3.0] {
                if let Value::Number(r) = (cb.0)(&[Value::Number(n)])? {
                    sum += r;
                }
            }
            Ok(Value::Number(sum))
        });
        let source = r#"ui fn App() { state total = each(|n| n * 10) Column { Text { "{total}" } } }"#;
        let mut session =
            UiSession::new_with_natives(source, "App", Default::default(), &[("each".into(), each)]).unwrap();
        assert!(format!("{:?}", session.render().unwrap()).contains("60"));
    }

    #[test]
    fn deferred_callback_updates_state_and_rerenders() {
        let stored: Rc<RefCell<Option<Callback>>> = Rc::default();
        let slot = Rc::clone(&stored);
        let later: NativeFn = Rc::new(move |args| {
            *slot.borrow_mut() = Some(first_callback(args));
            Ok(Value::Unit)
        });
        let source = r#"
ui fn App() {
    state label = "waiting"
    state started = later(|text| { label = text })
    Column { Text { "{label}" } }
}
"#;
        let mut session =
            UiSession::new_with_natives(source, "App", Default::default(), &[("later".into(), later)]).unwrap();
        assert!(format!("{:?}", session.render().unwrap()).contains("waiting"));

        // The host fires the stored callback after the native call returned.
        let session = Rc::new(RefCell::new(session));
        let seen = Rc::new(RefCell::new(String::new()));
        let (runner_session, runner_seen) = (Rc::clone(&session), Rc::clone(&seen));
        set_deferred_runner(Some(Rc::new(move |f, args| {
            let tree = runner_session.borrow_mut().call_value(f, &args).unwrap();
            *runner_seen.borrow_mut() = format!("{tree:?}");
        })));
        let cb = stored.borrow().clone().unwrap();
        (cb.0)(&[Value::String("done".into())]).unwrap();
        set_deferred_runner(None);
        assert!(seen.borrow().contains("done"), "{}", seen.borrow());
    }

    #[test]
    fn await_chains_deferred_callbacks() {
        let pending: Rc<RefCell<Vec<(f64, Callback)>>> = Rc::default();
        let queue = Rc::clone(&pending);
        let step: NativeFn = Rc::new(move |args| {
            let n = match &args[0] { Value::Number(n) => *n, other => panic!("{other:?}") };
            let cb = match args.last() { Some(Value::Callback(cb)) => cb.clone(), other => panic!("{other:?}") };
            queue.borrow_mut().push((n, cb));
            Ok(Value::Unit)
        });
        let source = r#"
async fn load(done) {
    let a = await step(1)
    let b = await step(10)
    done("{a}-{b}")
}
ui fn App() {
    state label = "waiting"
    state started = load(|s| { label = s })
    Column { Text { "{label}" } }
}
"#;
        let mut session =
            UiSession::new_with_natives(source, "App", Default::default(), &[("step".into(), step)]).unwrap();
        assert!(format!("{:?}", session.render().unwrap()).contains("waiting"));
        let session = Rc::new(RefCell::new(session));
        let seen = Rc::new(RefCell::new(String::new()));
        let (runner_session, runner_seen) = (Rc::clone(&session), Rc::clone(&seen));
        set_deferred_runner(Some(Rc::new(move |f, args| {
            let tree = runner_session.borrow_mut().call_value(f, &args).unwrap();
            *runner_seen.borrow_mut() = format!("{tree:?}");
        })));
        // Only the first step is pending; resolving it queues the second.
        assert_eq!(pending.borrow().len(), 1);
        let (n, cb) = pending.borrow_mut().remove(0);
        (cb.0)(&[Value::Number(n * 2.0)]).unwrap();
        assert_eq!(pending.borrow().len(), 1);
        let (n, cb) = pending.borrow_mut().remove(0);
        (cb.0)(&[Value::Number(n * 2.0)]).unwrap();
        set_deferred_runner(None);
        assert!(seen.borrow().contains("2-20"), "{}", seen.borrow());
    }
}
