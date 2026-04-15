fn main() {
    if let Err(e) = ail_cli::run() {
        eprintln!("{:?}", miette::Report::new(e));
        std::process::exit(1);
    }
}
