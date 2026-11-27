fn main() {
    if let Err(e) = yarube_coach_ja::run() {
        eprintln!("[ERROR] {e}");
        std::process::exit(1);
    }
}
