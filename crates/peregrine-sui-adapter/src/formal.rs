#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiFormalVerificationOptions {
    pub module_name: String,
    pub file_path: String,
    pub timeout_seconds: Option<usize>,
    pub verbose: bool,
    pub trace: bool,
    pub keep_temp: bool,
}

impl SuiFormalVerificationOptions {
    pub fn new(module_name: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            file_path: file_path.into(),
            timeout_seconds: Some(DEFAULT_FORMAL_VERIFICATION_TIMEOUT_SECONDS),
            verbose: false,
            trace: false,
            keep_temp: false,
        }
    }
}

pub const DEFAULT_FORMAL_VERIFICATION_TIMEOUT_SECONDS: usize = 45;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiFormalVerificationCommand {
    pub display: String,
    pub module_name: String,
    pub file_path: String,
    pub timeout_seconds: usize,
}

impl SuiFormalVerificationCommand {
    pub fn new(options: &SuiFormalVerificationOptions) -> Self {
        let timeout_seconds = options
            .timeout_seconds
            .unwrap_or(DEFAULT_FORMAL_VERIFICATION_TIMEOUT_SECONDS);
        let display = format!(
            "bundled sui-prover --path <package> --modules {} --timeout {}",
            options.module_name, timeout_seconds
        );

        Self {
            display,
            module_name: options.module_name.clone(),
            file_path: options.file_path.clone(),
            timeout_seconds,
        }
    }
}
