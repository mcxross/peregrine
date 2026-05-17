use crate::core::{logical_id, PackageInfo, PackageRole, PackageStatus};
use crate::sui::model::LoadedPackage;

pub fn package_info(package: &LoadedPackage, indexed_at: i64) -> PackageInfo {
    PackageInfo {
        id: logical_id(
            "package",
            [
                package.package_name.as_str(),
                package
                    .manifest_hash
                    .get(..16)
                    .unwrap_or(&package.manifest_hash),
            ],
        ),
        name: package.package_name.clone(),
        root_path: package.root.to_string_lossy().into_owned(),
        manifest_path: package.manifest_path.to_string_lossy().into_owned(),
        role: PackageRole::Root,
        compiler_version: None,
        package_hash: package.manifest_hash.clone(),
        status: PackageStatus::Indexed,
        indexed_at,
        metadata_json: None,
    }
}
