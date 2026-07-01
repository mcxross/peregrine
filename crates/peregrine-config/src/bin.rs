use std::path::Path;

#[tokio::main]
async fn main() {
    let path = Path::new("/Users/eieiron/dev/peregrine/.peregrine-dev");
    match peregrine_config::set_user_plugin_enabled(
        path,
        "peregrine-sui-move-knowledge".to_string(),
        true,
    )
    .await
    {
        Ok(_) => println!("Successfully enabled plugin!"),
        Err(e) => println!("Error: {e}"),
    }
}
