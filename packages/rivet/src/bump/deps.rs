use crate::bump::plan::{InternalDepUpdateInfo, PlannedBump};
use crate::config::{InternalDepUpdate, RivetConfig};
use crate::error::Result;
use crate::release_file::BumpType;
use crate::workspace::Workspace;
use indexmap::IndexMap;

/// Compute which internal dependency ranges need updating.
pub(crate) fn compute_internal_dep_updates(
  workspace: &Workspace,
  bumps: &IndexMap<String, PlannedBump>,
  config: &RivetConfig,
) -> Result<Vec<InternalDepUpdateInfo>> {
  let mut updates = Vec::new();

  // Check all workspace packages for dependencies on bumped packages
  for (dep_name, pkg) in &workspace.packages {
    // If this package is itself bumped, no need to update self-dependency
    // Check all dependency fields
    let dep_fields: Vec<(&str, &Option<IndexMap<String, String>>)> = vec![
      ("dependencies", &pkg.package_json.dependencies),
      ("devDependencies", &pkg.package_json.dev_dependencies),
      ("peerDependencies", &pkg.package_json.peer_dependencies),
      (
        "optionalDependencies",
        &pkg.package_json.optional_dependencies,
      ),
    ];

    for (field_name, field) in &dep_fields {
      let deps = match field {
        Some(d) => d,
        None => continue,
      };

      for (dep_name_in_range, _range) in deps {
        // Check if this dependency is being bumped
        if let Some(bump) = bumps.get(dep_name_in_range) {
          // Check if we should update based on config
          if should_update_dependency(&config.update_internal_dependencies, &bump.bump_type) {
            // We'll compute the new range when applying
            updates.push(InternalDepUpdateInfo {
              dependent_package_path: pkg.dir.join("package.json"),
              dependent_package_name: dep_name.clone(),
              dep_name: dep_name_in_range.clone(),
              dep_type: field_name.to_string(),
              old_range: _range.clone(),
              new_range: String::new(), // filled in during apply
            });
          }
        }
      }
    }
  }

  Ok(updates)
}

/// Determine if we should update internal dependencies based on config.
fn should_update_dependency(config: &InternalDepUpdate, bump_type: &BumpType) -> bool {
  match config {
    InternalDepUpdate::Always => true,
    InternalDepUpdate::Never => false,
    InternalDepUpdate::Patch => true,
    InternalDepUpdate::Minor => bump_type.priority() >= BumpType::Minor.priority(),
    InternalDepUpdate::Major => bump_type.priority() >= BumpType::Major.priority(),
  }
}
