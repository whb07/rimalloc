use mechanical_mimalloc::{OptionKey, Options};

fn main() {
    let max_warnings = Options::get(OptionKey::MaxWarnings);
    assert!(max_warnings >= 0);
}
