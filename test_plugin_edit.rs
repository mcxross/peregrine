use peregrine_config::plugin_edit::*;
use std::path::Path;

#[tokio::main]
async fn main() {
    let path = Path::new("/Users/eieiron/dev/peregrine/.peregrine-dev");
    set_user_plugin_enabled(path, "peregrine-sui-move-knowledge".to_string(), true).await.unwrap();
    println!("Done");
}
