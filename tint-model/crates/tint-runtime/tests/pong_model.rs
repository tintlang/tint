use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

#[test]
fn pong_event_dispatch_updates_a_persistent_game_model() {
    let source = r#"
enum GameState { Menu, Playing }
enum GameEvent { MoveUp, Tick(f32) }

struct Ball {
    position{Vec2},
    velocity{Vec2},
}

struct Paddle {
    position{Vec2},
    speed{f32},
}

struct Game {
    mode{GameState},
    ball{Ball},
    player{Paddle},
}

impl Game {
    fn move_player(&mut self, delta) {
        self.player.position.y = clamp(self.player.position.y + delta * self.player.speed, 1, 8)
        match self.mode {
            Menu => { self.mode = GameState::Playing },
            _ => {},
        }
    }

    fn update(&mut self, dt) {
        let advance = |velocity| velocity * dt;
        self.ball.position.x = self.ball.position.x + advance(self.ball.velocity.x)
        self.ball.position.y = self.ball.position.y + advance(self.ball.velocity.y)
    }

    fn handle(&mut self, event) {
        match event {
            MoveUp => self.move_player(0 - 1),
            Tick(dt) => self.update(dt),
        }
    }
}

fn test() {
    let mut game = Game {
        mode{GameState::Menu},
        ball{Ball { position{vec2(0, 0)}, velocity{vec2(8, 2)} }},
        player{Paddle { position{vec2(0, 4.5)}, speed{0.8} }},
    }
    game.handle(GameEvent::MoveUp)
    game.handle(GameEvent::MoveUp)
    game.handle(GameEvent::Tick(0.5))
    game.ball.position.x + game.player.position.y
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");

    let mut vm = TintVM::new();
    vm.run_program(&program);
    let result = vm
        .call_fn("test", &[], tint_ast::Span::dummy())
        .expect("model should run");

    assert!(matches!(result, Value::Number(value) if value == 6.9));
}
