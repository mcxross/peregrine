use clap::{Args, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Debug, Args)]
pub struct FuzzArgs {
    #[arg(long, default_value_t = 30)]
    pub time_limit_seconds: u64,

    #[arg(long, default_value_t = 1)]
    pub seed: u64,
}

#[derive(Clone, Debug, Args)]
pub struct VerifyArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub modules: Vec<String>,

    #[arg(long, value_name = "PATH")]
    pub file: Option<String>,

    #[arg(long, default_value_t = 45)]
    pub timeout_seconds: usize,

    #[arg(long)]
    pub trace: bool,

    #[arg(long)]
    pub keep_temp: bool,
}

#[derive(Clone, Debug, Args)]
pub struct BytecodeArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub module: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub file: Option<String>,

    #[arg(long)]
    pub interactive: bool,

    #[arg(long = "bytecode-map")]
    pub bytecode_map: bool,

    #[arg(long = "Xdebug")]
    pub debug: bool,
}

#[derive(Clone, Debug, Args)]
pub struct SignaturesArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub modules: Vec<String>,

    #[arg(long, value_name = "PATH")]
    pub file: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct GraphOutputArgs {
    #[arg(long)]
    pub dot: bool,

    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, Args)]
pub struct CallGraphArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub modules: Vec<String>,

    #[arg(long)]
    pub include_external: bool,

    #[command(flatten)]
    pub output: GraphOutputArgs,
}

#[derive(Clone, Debug, Args)]
pub struct ObjectGraphArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub modules: Vec<String>,

    #[arg(long)]
    pub include_external: bool,

    #[command(flatten)]
    pub output: GraphOutputArgs,
}

#[derive(Clone, Debug, Args)]
pub struct CfgArgs {
    #[arg(long = "module", value_name = "NAME")]
    pub module: Option<String>,

    #[arg(long = "function", value_name = "NAME")]
    pub function: Option<String>,

    #[command(flatten)]
    pub output: GraphOutputArgs,
}

#[derive(Clone, Debug, Args)]
pub struct AnalyzeArgs {
    #[arg(long)]
    pub fail_on_findings: bool,
}

#[derive(Clone, Debug, Args)]
pub struct CheckAllArgs {
    #[arg(long)]
    pub skip_fuzz: bool,

    #[arg(long)]
    pub skip_verify: bool,

    #[arg(long)]
    pub fail_on_findings: bool,

    #[arg(long, default_value_t = 30)]
    pub fuzz_time_limit_seconds: u64,

    #[arg(long, default_value_t = 1)]
    pub fuzz_seed: u64,

    #[arg(long = "module", value_name = "NAME")]
    pub verify_modules: Vec<String>,

    #[arg(long = "file", value_name = "PATH")]
    pub verify_file: Option<String>,

    #[arg(long, default_value_t = 45)]
    pub verify_timeout_seconds: usize,
}

impl CheckAllArgs {
    pub fn fuzz_args(&self) -> FuzzArgs {
        FuzzArgs {
            time_limit_seconds: self.fuzz_time_limit_seconds,
            seed: self.fuzz_seed,
        }
    }

    pub fn verify_args(&self) -> VerifyArgs {
        VerifyArgs {
            modules: self.verify_modules.clone(),
            file: self.verify_file.clone(),
            timeout_seconds: self.verify_timeout_seconds,
            trace: false,
            keep_temp: false,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct ImportPackageArgs {
    #[arg(long, value_enum)]
    pub network: ImportNetwork,

    #[arg(long, value_name = "PACKAGE_ID")]
    pub package_id: String,

    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub raw_only: bool,

    #[arg(long, default_value_t = 3)]
    pub max_dependency_depth: usize,

    #[arg(long, default_value_t = 64)]
    pub max_dependency_packages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ImportNetwork {
    Testnet,
    Mainnet,
    Devnet,
}

impl ImportNetwork {
    pub fn id(self) -> &'static str {
        match self {
            Self::Testnet => "testnet",
            Self::Mainnet => "mainnet",
            Self::Devnet => "devnet",
        }
    }

    pub fn graph_ql_url(self) -> &'static str {
        match self {
            Self::Testnet => "https://graphql.testnet.sui.io/graphql",
            Self::Mainnet => "https://graphql.mainnet.sui.io/graphql",
            Self::Devnet => "https://graphql.devnet.sui.io/graphql",
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct NewPackageArgs {
    pub package_name: String,
}
