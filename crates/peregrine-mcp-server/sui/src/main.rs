fn main() -> anyhow::Result<()> {
    // Basic argument parsing without clap for minimal overhead
    let mut args = std::env::args().skip(1);
    let mut transport = peregrine_sui_mcp_server::TransportKind::Stdio;

    while let Some(arg) = args.next() {
        if arg == "--transport" {
            if let Some(kind) = args.next() {
                if kind == "sse" {
                    let port = 8765; // Default or parsed from config
                    transport = peregrine_sui_mcp_server::TransportKind::Sse { port };
                } else if kind == "stdio" {
                    transport = peregrine_sui_mcp_server::TransportKind::Stdio;
                } else {
                    anyhow::bail!("Unknown transport: {}", kind);
                }
            }
        }
    }

    peregrine_sui_mcp_server::run_server(transport)
}
