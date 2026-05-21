use peregrine_move_model::MovePackageModel;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub(super) struct SummaryLocation {
    pub(super) path: PathBuf,
}

pub(super) fn resolve_summary_location(
    root: &Path,
    packages: &[MovePackageModel],
) -> Option<SummaryLocation> {
    let root_summary = root.join("package_summaries");

    if root_summary.is_dir() {
        return Some(SummaryLocation { path: root_summary });
    }

    packages
        .iter()
        .filter_map(|move_package| {
            let package_summary = root.join(&move_package.path).join("package_summaries");

            package_summary.is_dir().then_some(SummaryLocation {
                path: package_summary,
            })
        })
        .min_by(|left, right| {
            left.path
                .components()
                .count()
                .cmp(&right.path.components().count())
                .then_with(|| left.path.cmp(&right.path))
        })
}

pub(super) fn read_address_mapping(path: &Path) -> BTreeMap<String, String> {
    let Ok(source) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };

    serde_json::from_str(&source).unwrap_or_default()
}

#[derive(Deserialize)]
pub(super) struct SummaryModule {
    pub(super) id: SummaryModuleId,
    #[serde(default)]
    pub(super) functions: BTreeMap<String, Value>,
    #[serde(default)]
    pub(super) structs: BTreeMap<String, Value>,
    #[serde(default)]
    pub(super) enums: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
pub(super) struct SummaryModuleId {
    pub(super) address: String,
    pub(super) name: String,
}

pub(super) fn read_summary_modules(
    directory: &Path,
    target_ids: &BTreeSet<String>,
) -> Vec<SummaryModule> {
    let mut modules = Vec::new();
    collect_summary_modules(directory, directory, target_ids, &mut modules);
    modules.sort_by(|left, right| {
        left.id
            .address
            .cmp(&right.id.address)
            .then_with(|| left.id.name.cmp(&right.id.name))
    });
    modules
}

fn collect_summary_modules(
    root: &Path,
    directory: &Path,
    target_ids: &BTreeSet<String>,
    modules: &mut Vec<SummaryModule>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if should_enter_summary_directory(root, &path, target_ids) {
                collect_summary_modules(root, &path, target_ids, modules);
            }
            continue;
        }

        if !file_type.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            || path
                .file_stem()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "address_mapping" | "root_package_metadata"))
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(summary) = serde_json::from_str::<SummaryModule>(&source) else {
            continue;
        };

        modules.push(summary);
    }
}

fn should_enter_summary_directory(
    root: &Path,
    directory: &Path,
    target_ids: &BTreeSet<String>,
) -> bool {
    if target_ids.is_empty() {
        return true;
    }

    let Ok(relative) = directory.strip_prefix(root) else {
        return false;
    };
    let mut components = relative.components();
    let Some(first_component) = components.next() else {
        return true;
    };
    let Some(first_component) = first_component.as_os_str().to_str() else {
        return false;
    };

    target_ids.contains(first_component)
}

pub(super) fn summary_fields(value: &Value) -> Vec<(String, Value)> {
    let Some(fields) = value
        .get("fields")
        .and_then(|fields| fields.get("fields"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    let mut result = fields
        .iter()
        .filter_map(|(field_name, field)| {
            field
                .get("type_")
                .cloned()
                .map(|field_type| (field_name.clone(), field_type))
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}
