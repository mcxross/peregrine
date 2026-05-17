pub const SCHEMA_VERSION: i64 = 1;

pub const CREATE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_metadata (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS packages (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  root_path TEXT NOT NULL,
  manifest_path TEXT NOT NULL,
  role TEXT NOT NULL,
  compiler_version TEXT,
  package_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  indexed_at INTEGER NOT NULL,
  metadata_json TEXT
);

CREATE TABLE IF NOT EXISTS files (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  path TEXT NOT NULL,
  content_hash TEXT,
  kind TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS summary_artifacts (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  package_alias TEXT NOT NULL,
  module_name TEXT NOT NULL,
  summary_path TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  schema_version TEXT,
  role TEXT NOT NULL,
  materialized_status TEXT NOT NULL,
  last_seen_at INTEGER NOT NULL,
  card_json TEXT,
  raw_summary_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS address_mappings (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  alias TEXT NOT NULL,
  address TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS modules (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  summary_artifact_id TEXT,
  file_id TEXT,
  address TEXT NOT NULL,
  name TEXT NOT NULL,
  full_name TEXT NOT NULL,
  immediate_dependencies_json TEXT NOT NULL,
  docs TEXT,
  attributes_json TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS dependencies (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  source_package_alias TEXT NOT NULL,
  source_module TEXT NOT NULL,
  target_package_alias TEXT NOT NULL,
  target_module TEXT NOT NULL,
  dependency_kind TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS types (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  module_id TEXT NOT NULL,
  name TEXT NOT NULL,
  full_name TEXT NOT NULL,
  kind TEXT NOT NULL,
  abilities_json TEXT NOT NULL,
  type_parameters_json TEXT NOT NULL,
  docs TEXT,
  attributes_json TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(module_id) REFERENCES modules(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fields (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  module_id TEXT NOT NULL,
  type_id TEXT NOT NULL,
  name TEXT NOT NULL,
  type_name TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(type_id) REFERENCES types(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS functions (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  module_id TEXT NOT NULL,
  name TEXT NOT NULL,
  full_name TEXT NOT NULL,
  visibility TEXT NOT NULL,
  is_entry INTEGER NOT NULL,
  is_native INTEGER NOT NULL,
  type_parameters_json TEXT NOT NULL,
  parameters_json TEXT NOT NULL,
  returns_json TEXT NOT NULL,
  acquires_json TEXT NOT NULL,
  docs TEXT,
  attributes_json TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(module_id) REFERENCES modules(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS locals (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  function_id TEXT NOT NULL,
  name TEXT NOT NULL,
  type_name TEXT,
  index_in_function INTEGER,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(function_id) REFERENCES functions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS basic_blocks (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  function_id TEXT NOT NULL,
  index_in_function INTEGER NOT NULL,
  label TEXT NOT NULL,
  start_operation_index INTEGER,
  end_operation_index INTEGER,
  source_span_json TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(function_id) REFERENCES functions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS operations (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  function_id TEXT NOT NULL,
  index_in_function INTEGER NOT NULL,
  kind TEXT NOT NULL,
  display TEXT NOT NULL,
  target TEXT,
  source_span_json TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
  FOREIGN KEY(function_id) REFERENCES functions(id) ON DELETE CASCADE,
  UNIQUE(function_id, index_in_function)
);

CREATE TABLE IF NOT EXISTS edges (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  from_id TEXT NOT NULL,
  to_id TEXT NOT NULL,
  edge_type TEXT NOT NULL,
  operation_id TEXT,
  source_span_json TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS semantic_tags (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS diagnostics (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  severity TEXT NOT NULL,
  source TEXT NOT NULL,
  message TEXT NOT NULL,
  source_span_json TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS chunks (
  id TEXT PRIMARY KEY,
  package_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  level TEXT NOT NULL,
  estimated_tokens INTEGER NOT NULL,
  text TEXT NOT NULL,
  metadata_json TEXT,
  FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
CREATE INDEX IF NOT EXISTS idx_summary_artifacts_module ON summary_artifacts(package_id, package_alias, module_name);
CREATE INDEX IF NOT EXISTS idx_summary_artifacts_hash ON summary_artifacts(content_hash);
CREATE INDEX IF NOT EXISTS idx_modules_full_name ON modules(package_id, full_name);
CREATE INDEX IF NOT EXISTS idx_types_full_name ON types(package_id, full_name);
CREATE INDEX IF NOT EXISTS idx_functions_full_name ON functions(package_id, full_name);
CREATE INDEX IF NOT EXISTS idx_functions_entry_visibility ON functions(package_id, is_entry, visibility);
CREATE INDEX IF NOT EXISTS idx_operations_function_index ON operations(package_id, function_id, index_in_function);
CREATE INDEX IF NOT EXISTS idx_operations_kind ON operations(package_id, kind);
CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(package_id, from_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(package_id, to_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_semantic_tags_tag ON semantic_tags(package_id, tag);
CREATE INDEX IF NOT EXISTS idx_diagnostics_severity ON diagnostics(package_id, severity);
"#;
