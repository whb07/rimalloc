pub(super) fn source_has_inline_never_function(source: &str, signature: &str) -> bool {
    let mut saw_inline_never = false;
    for line in source.lines().map(str::trim_start) {
        if line == "#[inline(never)]" {
            saw_inline_never = true;
            continue;
        }
        if saw_inline_never && line.starts_with("//") {
            continue;
        }
        if saw_inline_never && line.starts_with(signature) {
            return true;
        }
        saw_inline_never = false;
    }
    false
}
