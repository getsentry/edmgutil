fn main() {
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {}
        _ => {
            panic!("unsupported operating system. edmgutil only works on macos");
        }
    }
}
