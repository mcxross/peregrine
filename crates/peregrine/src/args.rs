use crate::sui::args::{
    AnalyzeArgs, CheckAllArgs, FuzzArgs, ImportPackageArgs, NewPackageArgs, VerifyArgs,
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
    pub pretty: bool,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum CliCommand {
    Build,
    Test,
    Coverage,
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
}
