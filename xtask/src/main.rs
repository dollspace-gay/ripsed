fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("gen-schema") => gen_schema(),
        Some("gen-fixtures") => gen_fixtures(),
        _ => {
            eprintln!("Usage: cargo xtask <gen-schema|gen-fixtures>");
            std::process::exit(1);
        }
    }
}

fn gen_schema() {
    eprintln!("gen-schema: not yet implemented (planned for v0.2)");
    std::process::exit(1);
}

fn gen_fixtures() {
    eprintln!("gen-fixtures: not yet implemented (planned for v0.2)");
    std::process::exit(1);
}
