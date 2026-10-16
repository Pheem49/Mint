fn main() {
    if let Err(error) = mint_mod_runtime::worker_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
