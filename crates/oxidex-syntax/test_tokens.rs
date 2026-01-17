use oxidex_syntax::lexer::Lexer;

fn main() {
    let source = "List<Int>";
    let mut lexer = Lexer::new(source);
    let result = lexer.lex();
    match result {
        Ok(tokens) => {
            for token in &tokens {
                println!("{:?}", token.kind);
            }
        }
        Err(e) => {
            eprintln!("Lexer error: {:?}", e);
        }
    }
}
