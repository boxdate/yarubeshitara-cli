fn main() {
    if let Err(e) = yarube_coach::run() {
        eprintln!("[ERROR] {e}");
        std::process::exit(1);
    }
}
