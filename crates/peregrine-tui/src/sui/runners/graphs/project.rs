pub fn module_matches(requested: &str, address: Option<&str>, module_name: &str) -> bool {
    if requested == module_name {
        return true;
    }

    address.is_some_and(|address| requested == format!("{address}::{module_name}"))
}
