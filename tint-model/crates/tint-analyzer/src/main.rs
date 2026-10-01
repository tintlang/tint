mod analyzer;
mod project;
mod protocol;

use analyzer::Analyzer;
use protocol::{handle, read_message, write_message};
use serde_json::Value;
use std::io::{self, BufReader};

fn main() {
    let stdin = io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let mut analyzer = Analyzer::default();
    let stdout = io::stdout();
    let mut output = stdout.lock();

    while let Some(message) = read_message(&mut input) {
        let Ok(request) = serde_json::from_slice::<Value>(&message) else {
            continue;
        };
        if let Some(response) = handle(&mut analyzer, &request) {
            write_message(&mut output, &response);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn fragments_are_checked_inside_their_project() {
        let root = std::env::temp_dir().join(format!("tint-analyzer-{}", std::process::id()));
        fs::create_dir_all(root.join("parts")).unwrap();
        fs::write(root.join("fns.tn"), "fn greet() { 1 }\n").unwrap();
        fs::write(
            root.join("main.tn"),
            "import \"./fns.tn\";\nui fn App() {\n    Column {\n        import \"./parts/bar.tn\";\n    }\n}\n",
        )
        .unwrap();
        let bar = root.join("parts/bar.tn");
        fs::write(&bar, "Text { click||greet \"hi\" }\n").unwrap();

        let uri = format!("file://{}", bar.display());
        let mut analyzer = Analyzer::default();
        analyzer
            .documents
            .insert(uri.clone(), fs::read_to_string(&bar).unwrap());
        assert_eq!(analyzer.diagnostics(&uri), json!([]));

        // A real error is still reported, on the fragment's own line.
        analyzer
            .documents
            .insert(uri.clone(), "\nText { click||nope \"hi\" }\n".into());
        let found = analyzer.diagnostics(&uri);
        assert_eq!(found[0]["range"]["start"]["line"], 1);
        fs::remove_dir_all(&root).ok();
    }
}
