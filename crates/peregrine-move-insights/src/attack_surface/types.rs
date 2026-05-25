use serde::Serialize;

use crate::object_lifecycle::ObjectLifecycleMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageSurface {
    pub entry_function_count: usize,
    pub capability_count: usize,
    pub shared_object_count: usize,
    pub address_owned_object_count: usize,
    pub immutable_object_count: usize,
    pub wrapped_object_count: usize,
    pub party_object_count: usize,
    pub admin_control_count: usize,
    pub external_call_count: usize,
    pub public_package_relationship_count: usize,
    pub capability_structs: Vec<String>,
    pub capability_findings: Vec<CapabilityFinding>,
    pub shared_object_structs: Vec<String>,
    pub object_lifecycle_maps: Vec<ObjectLifecycleMap>,
    pub object_ownership_findings: Vec<ObjectOwnershipFinding>,
    pub admin_control_findings: Vec<AdminControlFinding>,
    pub external_call_findings: Vec<ExternalCallFinding>,
    pub public_package_relationships: Vec<PublicPackageRelationship>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub protected_functions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOwnershipFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub ownership_kind: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub related_functions: Vec<String>,
    pub wrapped_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminControlFinding {
    pub function_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub guarding_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCallFinding {
    pub caller_module: String,
    pub caller_function: String,
    pub target_module: String,
    pub target_function: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPackageRelationship {
    pub source_module: String,
    pub source_function: String,
    pub target_module: String,
    pub target_function: String,
}
