use crate::sui::args::{
    AnalyzeArgs, BytecodeArgs, CallGraphArgs, CfgArgs, CheckAllArgs, FuzzArgs, ImportPackageArgs,
    NewPackageArgs, ObjectGraphArgs, SignaturesArgs, VerifyArgs,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "peregrine",
    version,
    about = "Peregrine Move security workflow CLI"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = ".", value_name = "PATH")]
    pub project: PathBuf,

    #[arg(long, global = true, default_value = ".", value_name = "PATH")]
    pub package: String,

    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum CliCommand {
    Build,
    Test,
    Coverage,
    #[command(name = "bytecode", visible_alias = "bytecode-viewer")]
    Bytecode(BytecodeArgs),
    #[command(name = "signatures", visible_alias = "function-signatures")]
    Signatures(SignaturesArgs),
    #[command(name = "call-graph", visible_alias = "callgraph")]
    CallGraph(CallGraphArgs),
    #[command(name = "object-graph", visible_alias = "objectgraph")]
    ObjectGraph(ObjectGraphArgs),
    #[command(name = "cfg", visible_alias = "control-flow-graph")]
    Cfg(CfgArgs),
    Fuzz(FuzzArgs),
    Verify(VerifyArgs),
    Analyze(AnalyzeArgs),
    #[command(name = "check-all")]
    CheckAll(CheckAllArgs),
    #[command(name = "import-package")]
    ImportPackage(ImportPackageArgs),
    #[command(name = "new-package")]
    NewPackage(NewPackageArgs),
}

impl CliCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Coverage => "coverage",
            Self::Bytecode(_) => "bytecode",
            Self::Signatures(_) => "signatures",
            Self::CallGraph(_) => "call-graph",
            Self::ObjectGraph(_) => "object-graph",
            Self::Cfg(_) => "cfg",
            Self::Fuzz(_) => "fuzz",
            Self::Verify(_) => "verify",
            Self::Analyze(_) => "analyze",
            Self::CheckAll(_) => "check-all",
            Self::ImportPackage(_) => "import-package",
            Self::NewPackage(_) => "new-package",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sui::args::ImportNetwork;
    use clap::Parser;

    #[test]
    fn parses_global_project_and_package_for_check_all() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "--project",
            "/workspace",
            "--package",
            "packages/vault",
            "check-all",
            "--skip-fuzz",
        ])
        .expect("cli args");

        assert_eq!(cli.project, PathBuf::from("/workspace"));
        assert_eq!(cli.package, "packages/vault");
        assert!(matches!(cli.command, CliCommand::CheckAll(_)));
    }

    #[test]
    fn parses_multiple_verify_modules() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "verify",
            "--module",
            "vault",
            "--module",
            "admin",
        ])
        .expect("cli args");

        let CliCommand::Verify(args) = cli.command else {
            panic!("expected verify command");
        };
        assert_eq!(args.modules, ["vault", "admin"]);
    }

    #[test]
    fn parses_import_package_network_and_id() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "import-package",
            "--network",
            "testnet",
            "--package-id",
            "0x2",
        ])
        .expect("cli args");

        let CliCommand::ImportPackage(args) = cli.command else {
            panic!("expected import-package command");
        };
        assert_eq!(args.network, ImportNetwork::Testnet);
        assert_eq!(
            args.network.graph_ql_url(),
            "https://graphql.testnet.sui.io/graphql"
        );
        assert_eq!(args.package_id, "0x2");
    }

    #[test]
    fn parses_new_package_name() {
        let cli = Cli::try_parse_from(["peregrine", "new-package", "vault"]).expect("cli args");

        let CliCommand::NewPackage(args) = cli.command else {
            panic!("expected new-package command");
        };
        assert_eq!(args.package_name, "vault");
    }

    #[test]
    fn parses_bytecode_viewer_target() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "bytecode",
            "--module",
            "vault",
            "--interactive",
        ])
        .expect("cli args");

        let CliCommand::Bytecode(args) = cli.command else {
            panic!("expected bytecode command");
        };
        assert_eq!(args.module.as_deref(), Some("vault"));
        assert!(args.interactive);
    }

    #[test]
    fn parses_signature_filters() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "signatures",
            "--module",
            "orders",
            "--module",
            "policy",
            "--file",
            "sources/orders.move",
        ])
        .expect("cli args");

        let CliCommand::Signatures(args) = cli.command else {
            panic!("expected signatures command");
        };
        assert_eq!(args.modules, ["orders", "policy"]);
        assert_eq!(args.file.as_deref(), Some("sources/orders.move"));
    }

    #[test]
    fn parses_graph_output_options() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "call-graph",
            "--module",
            "orders",
            "--include-external",
            "--dot",
            "--output",
            "graphs/calls.dot",
        ])
        .expect("cli args");

        let CliCommand::CallGraph(args) = cli.command else {
            panic!("expected call-graph command");
        };
        assert_eq!(args.modules, ["orders"]);
        assert!(args.include_external);
        assert!(args.output.dot);
        assert_eq!(
            args.output.output.as_deref(),
            Some(std::path::Path::new("graphs/calls.dot"))
        );
    }

    #[test]
    fn parses_cfg_target() {
        let cli = Cli::try_parse_from([
            "peregrine",
            "cfg",
            "--module",
            "vault",
            "--function",
            "deposit",
            "--dot",
        ])
        .expect("cli args");

        let CliCommand::Cfg(args) = cli.command else {
            panic!("expected cfg command");
        };
        assert_eq!(args.module.as_deref(), Some("vault"));
        assert_eq!(args.function.as_deref(), Some("deposit"));
        assert!(args.output.dot);
    }

    #[test]
    fn parses_json_output_flag_after_subcommand() {
        let cli = Cli::try_parse_from(["peregrine", "analyze", "--json"]).expect("cli args");

        assert!(cli.json);
        assert!(matches!(cli.command, CliCommand::Analyze(_)));
    }
}
