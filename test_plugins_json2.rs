use std::fs;
fn main() {
    let _ = fs::read_to_string("/tmp/plugins.json");
}
