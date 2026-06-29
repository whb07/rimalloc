use mechanical_mimalloc::{GlobalOptions, OptionKey};

fn main() {
    let max_warnings = GlobalOptions::get(OptionKey::MaxWarnings);
    assert!(max_warnings >= 0);
}
