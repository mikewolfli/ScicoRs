// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Plugin API versioning, capability declaration and compatibility policy.
//!
//! # Why this exists
//!
//! A plugin built against one version of the host API cannot be assumed to run
//! against another. Rather than discovering that through a crash, every plugin
//! manifest declares its **API version**, an **entry point**, the
//! **capabilities** it provides, its **platform requirements** and its
//! **dependencies**, and the loader runs them all through a
//! [`CompatibilityPolicy`] *before* the plugin is loaded.
//!
//! # Deprecation and compatibility strategy
//!
//! The host API is versioned `major.minor`:
//!
//! * A **major** mismatch is always incompatible.
//! * A plugin built against a **newer minor** than the host is incompatible
//!   (the plugin may use APIs the host does not have).
//! * A plugin built against a **current or older minor** of the same major is
//!   compatible.
//!
//! When a minor version is deprecated, the host keeps accepting it until the
//! deprecation window closes (see [`CompatibilityPolicy::deprecated_minors`]).
//! Deprecated-but-accepted plugins are reported through
//! [`CompatDecision::Deprecated`] so a caller can log a migration warning
//! without failing the load. Existing public paths are never broken by
//! reshuffling private modules.

use std::collections::BTreeSet;

use super::PluginManifest;

/// A `major.minor` API version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApiVersion {
    /// Major version; a change here is a breaking change.
    pub major: u32,
    /// Minor version; additive, backwards-compatible within a major.
    pub minor: u32,
}

impl ApiVersion {
    /// Create a version.
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Parse `"major.minor"`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let trimmed = text.trim();
        let mut parts = trimmed.split('.');
        let major = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| format!("invalid API version '{}': expected 'major.minor'", text))?;
        let minor = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| format!("invalid API version '{}': expected 'major.minor'", text))?;
        if parts.next().is_some() {
            return Err(format!(
                "invalid API version '{}': too many components",
                text
            ));
        }
        Ok(Self { major, minor })
    }

    /// Whether `self` (the plugin) is compatible with `host` under the default
    /// policy: same major and not newer than the host.
    pub fn is_compatible_with(self, host: ApiVersion) -> bool {
        self.major == host.major && self.minor <= host.minor
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A capability a plugin declares it provides.
///
/// Declaring capabilities lets the host decide up front whether a plugin is
/// relevant and lets two plugins be checked for conflicts, without loading
/// either.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Capability {
    /// Namespaced capability name, e.g. `"blocks.thermal"`.
    pub name: String,
    /// Optional API version this capability conforms to.
    pub version: Option<ApiVersion>,
}

impl Capability {
    /// Create a capability with no explicit version.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: None,
        }
    }

    /// Create a capability with an explicit API version.
    pub fn with_version(name: &str, version: ApiVersion) -> Self {
        Self {
            name: name.to_string(),
            version: Some(version),
        }
    }

    /// Whether this capability is compatible with a host API version: an
    /// unversioned capability is always compatible; a versioned one follows
    /// [`ApiVersion::is_compatible_with`].
    pub fn is_compatible_with(&self, host: ApiVersion) -> bool {
        match self.version {
            Some(v) => v.is_compatible_with(host),
            None => true,
        }
    }
}

/// A platform requirement declared by a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRequirement {
    /// Target operating system (`"linux"`, `"macos"`, `"windows"`, ...); empty
    /// means any.
    pub os: String,
    /// Target CPU architecture (`"x86_64"`, `"aarch64"`, ...); empty means any.
    pub arch: String,
}

impl PlatformRequirement {
    /// A requirement matching any platform.
    pub fn any() -> Self {
        Self {
            os: String::new(),
            arch: String::new(),
        }
    }

    /// Build a requirement for a specific OS/arch pair.
    pub fn new(os: &str, arch: &str) -> Self {
        Self {
            os: os.to_string(),
            arch: arch.to_string(),
        }
    }

    /// Whether this requirement is satisfied by the given `os`/`arch`.
    ///
    /// An empty field matches anything.
    pub fn is_satisfied_by(&self, os: &str, arch: &str) -> bool {
        let os_ok = self.os.is_empty() || self.os.eq_ignore_ascii_case(os);
        let arch_ok = self.arch.is_empty() || self.arch.eq_ignore_ascii_case(arch);
        os_ok && arch_ok
    }

    /// The current platform as `(os, arch)` from the running build.
    pub fn current() -> (String, String) {
        (
            std::env::consts::OS.to_string(),
            std::env::consts::ARCH.to_string(),
        )
    }
}

/// An extended, self-describing plugin manifest.
///
/// This wraps the base [`PluginManifest`] with the compatibility metadata the
/// loader needs: declared capabilities, platform requirements, dependencies and
/// the source/signature hash strategy.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtendedManifest {
    /// The base manifest (name, version, api_version, entry_point, ...).
    pub base: PluginManifest,
    /// Declared capabilities.
    pub capabilities: Vec<Capability>,
    /// Platform requirement.
    pub platform: PlatformRequirement,
    /// Names of plugins this one depends on (`name@version` or just `name`).
    pub dependencies: Vec<String>,
    /// Hash of the plugin artifact, used for provenance/verification.
    pub signature_hash: Option<String>,
    /// Strategy used to verify the artifact's source.
    pub source_policy: SourcePolicy,
}

/// The strategy used to establish that a plugin artifact is the one its
/// manifest claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePolicy {
    /// No verification (development only). Recorded, never treated as verified.
    None,
    /// The `signature_hash` must match the artifact bytes.
    Sha256Hash,
}

impl SourcePolicy {
    /// Stable name for messages and files.
    pub fn name(self) -> &'static str {
        match self {
            SourcePolicy::None => "none",
            SourcePolicy::Sha256Hash => "sha256",
        }
    }

    /// Whether this policy actually verifies anything.
    pub fn is_verifying(self) -> bool {
        matches!(self, SourcePolicy::Sha256Hash)
    }
}

impl ExtendedManifest {
    /// Build an extended manifest from a base manifest with no capabilities and
    /// no platform restriction.
    pub fn from_base(base: PluginManifest) -> Self {
        Self {
            base,
            capabilities: Vec::new(),
            platform: PlatformRequirement::any(),
            dependencies: Vec::new(),
            signature_hash: None,
            source_policy: SourcePolicy::None,
        }
    }

    /// Parse an extended manifest from JSON.
    ///
    /// Accepts the base manifest's fields plus optional `capabilities`
    /// (array of strings or `{"name":..., "version":"m.n"}`),
    /// `platform_os`, `platform_arch`, `dependencies` and `signature_hash`.
    /// `source_policy` is `sha256` when `signature_hash` is present, else `none`.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let base = PluginManifest::from_json(json)?;
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid manifest JSON: {}", e))?;
        let obj = value
            .as_object()
            .ok_or_else(|| "manifest JSON must be an object".to_string())?;

        let mut capabilities = Vec::new();
        if let Some(caps) = obj.get("capabilities") {
            let arr = caps
                .as_array()
                .ok_or_else(|| "'capabilities' must be an array".to_string())?;
            for entry in arr {
                match entry {
                    serde_json::Value::String(name) => {
                        if name.trim().is_empty() {
                            return Err("capability name must not be empty".to_string());
                        }
                        capabilities.push(Capability::new(name));
                    }
                    serde_json::Value::Object(map) => {
                        let name = map
                            .get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| "capability object needs a 'name'".to_string())?;
                        if name.trim().is_empty() {
                            return Err("capability name must not be empty".to_string());
                        }
                        let version = match map.get("version").and_then(|v| v.as_str()) {
                            Some(v) => Some(ApiVersion::parse(v)?),
                            None => None,
                        };
                        capabilities.push(Capability {
                            name: name.to_string(),
                            version,
                        });
                    }
                    _ => return Err("capability must be a string or an object".to_string()),
                }
            }
        }

        let platform = PlatformRequirement {
            os: obj
                .get("platform_os")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            arch: obj
                .get("platform_arch")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };

        let mut dependencies = Vec::new();
        if let Some(deps) = obj.get("dependencies") {
            let arr = deps
                .as_array()
                .ok_or_else(|| "'dependencies' must be an array".to_string())?;
            for dep in arr {
                let name = dep
                    .as_str()
                    .ok_or_else(|| "dependency must be a string".to_string())?;
                dependencies.push(name.to_string());
            }
        }

        let signature_hash = obj
            .get("signature_hash")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let source_policy = if signature_hash.is_some() {
            SourcePolicy::Sha256Hash
        } else {
            SourcePolicy::None
        };

        Ok(Self {
            base,
            capabilities,
            platform,
            dependencies,
            signature_hash,
            source_policy,
        })
    }

    /// The plugin's declared capabilities as a set of names.
    pub fn capability_names(&self) -> BTreeSet<&str> {
        self.capabilities.iter().map(|c| c.name.as_str()).collect()
    }

    /// Whether this plugin declares `name` as a capability.
    pub fn declares(&self, name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == name)
    }
}

/// The compatibility policy applied before a plugin is loaded.
#[derive(Debug, Clone)]
pub struct CompatibilityPolicy {
    /// The host API version a plugin must be compatible with.
    pub host_api: ApiVersion,
    /// Minor versions of `host_api.major` that are still accepted but
    /// deprecated, with a migration note.
    pub deprecated_minors: Vec<(u32, String)>,
    /// Require a non-empty entry point.
    pub require_entry_point: bool,
    /// Require a verifying source policy (`sha256`).
    pub require_verified_source: bool,
}

impl CompatibilityPolicy {
    /// A default policy: host API `1.0`, entry point required, source
    /// verification *not* required (so un-hashed development plugins load).
    pub fn new(host_api: ApiVersion) -> Self {
        Self {
            host_api,
            deprecated_minors: Vec::new(),
            require_entry_point: true,
            require_verified_source: false,
        }
    }

    /// Mark a minor version (of the host major) as deprecated with a note.
    pub fn deprecate_minor(mut self, minor: u32, note: &str) -> Self {
        self.deprecated_minors.push((minor, note.to_string()));
        self
    }

    /// Require plugins to carry a verifying `signature_hash`.
    pub fn require_verified_source(mut self, required: bool) -> Self {
        self.require_verified_source = required;
        self
    }

    /// The migration note for a deprecated minor, if any.
    pub fn deprecation_note(&self, minor: u32) -> Option<&str> {
        self.deprecated_minors
            .iter()
            .find(|(m, _)| *m == minor)
            .map(|(_, note)| note.as_str())
    }
}

impl Default for CompatibilityPolicy {
    /// Host API `1.0`, entry point required, source verification optional.
    fn default() -> Self {
        Self::new(ApiVersion::new(1, 0))
    }
}

/// The decision reached by compatibility checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatDecision {
    /// The plugin is compatible and should be loaded.
    Compatible,
    /// The plugin is compatible but its API version is deprecated; load it and
    /// log the migration note.
    Deprecated {
        /// Minor version that is deprecated.
        minor: u32,
        /// Migration guidance.
        note: String,
    },
    /// The plugin cannot be loaded; see [`CompatDecision::reason`].
    Incompatible {
        /// Why the plugin was rejected.
        reason: String,
    },
}

impl CompatDecision {
    /// Whether the decision permits loading.
    pub fn is_compatible(&self) -> bool {
        !matches!(self, CompatDecision::Incompatible { .. })
    }

    /// A human-readable reason, non-empty for every variant.
    pub fn reason(&self) -> String {
        match self {
            CompatDecision::Compatible => "compatible".to_string(),
            CompatDecision::Deprecated { minor, note } => {
                format!("compatible but API minor {} is deprecated: {}", minor, note)
            }
            CompatDecision::Incompatible { reason } => reason.clone(),
        }
    }
}

/// Check a manifest's compatibility against a policy.
///
/// The checks are ordered from the most fundamental (parseable API version) to
/// the most specific (platform, capabilities), so the first failure describes
/// the real blocker.
pub fn check_compatibility(
    manifest: &ExtendedManifest,
    policy: &CompatibilityPolicy,
) -> CompatDecision {
    if policy.require_entry_point && manifest.base.entry_point.trim().is_empty() {
        return CompatDecision::Incompatible {
            reason: "manifest declares no entry point".to_string(),
        };
    }
    if policy.require_verified_source && !manifest.source_policy.is_verifying() {
        return CompatDecision::Incompatible {
            reason: format!(
                "source policy '{}' does not verify the artifact (a signature hash is required)",
                manifest.source_policy.name()
            ),
        };
    }
    if manifest.source_policy.is_verifying() && manifest.signature_hash.is_none() {
        return CompatDecision::Incompatible {
            reason: "source policy is 'sha256' but no signature_hash is present".to_string(),
        };
    }

    let plugin_api = match ApiVersion::parse(&manifest.base.api_version) {
        Ok(v) => v,
        Err(e) => {
            return CompatDecision::Incompatible { reason: e };
        }
    };
    if plugin_api.major != policy.host_api.major {
        return CompatDecision::Incompatible {
            reason: format!(
                "API major mismatch: plugin {} vs host {}",
                plugin_api, policy.host_api
            ),
        };
    }
    if plugin_api.minor > policy.host_api.minor {
        return CompatDecision::Incompatible {
            reason: format!(
                "plugin API {} is newer than host API {}",
                plugin_api, policy.host_api
            ),
        };
    }

    let (os, arch) = PlatformRequirement::current();
    if !manifest.platform.is_satisfied_by(&os, &arch) {
        return CompatDecision::Incompatible {
            reason: format!(
                "platform requirement (os='{}', arch='{}') not satisfied by {}/{}",
                manifest.platform.os, manifest.platform.arch, os, arch
            ),
        };
    }

    for cap in &manifest.capabilities {
        if !cap.is_compatible_with(policy.host_api) {
            return CompatDecision::Incompatible {
                reason: format!(
                    "capability '{}' declares an incompatible API version",
                    cap.name
                ),
            };
        }
    }

    if let Some(note) = policy.deprecation_note(plugin_api.minor) {
        return CompatDecision::Deprecated {
            minor: plugin_api.minor,
            note: note.to_string(),
        };
    }
    CompatDecision::Compatible
}

/// Convenience: build an [`ExtendedManifest`] from a base manifest (no extra
/// metadata) and check it against a policy. Used by the CLI, which accepts a
/// plain manifest file.
pub fn compatibility_of_manifest(
    manifest: &PluginManifest,
    host_api: ApiVersion,
    policy: &CompatibilityPolicy,
) -> CompatDecision {
    let extended = ExtendedManifest::from_base(manifest.clone());
    // A plain manifest still carries an entry point, so keep the entry-point
    // requirement but re-key the policy onto the caller-provided host version.
    let mut effective = policy.clone();
    effective.host_api = host_api;
    check_compatibility(&extended, &effective)
}

/// Detect conflicts between a set of extended manifests: two plugins declaring
/// the same capability name conflict, and a dependency naming a plugin that is
/// not present in the set is reported as missing.
///
/// Returns a list of `(plugin_name, problem)` pairs; empty means no conflicts.
pub fn detect_conflicts(manifests: &[ExtendedManifest]) -> Vec<(String, String)> {
    let mut problems = Vec::new();
    let mut owners: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for m in manifests {
        for cap in &m.capabilities {
            match owners.get(cap.name.as_str()) {
                Some(existing) if *existing != m.base.name => {
                    problems.push((
                        m.base.name.clone(),
                        format!(
                            "capability '{}' is already declared by plugin '{}'",
                            cap.name, existing
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    owners.insert(cap.name.as_str(), m.base.name.as_str());
                }
            }
        }
    }

    let present: BTreeSet<&str> = manifests.iter().map(|m| m.base.name.as_str()).collect();
    for m in manifests {
        for dep in &m.dependencies {
            let dep_name = dep.split('@').next().unwrap_or(dep);
            if !present.contains(dep_name) {
                problems.push((
                    m.base.name.clone(),
                    format!("dependency '{}' is not present", dep),
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(name: &str, api: &str, entry: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            version: "1.0".to_string(),
            author: "a".to_string(),
            description: String::new(),
            api_version: api.to_string(),
            entry_point: entry.to_string(),
        }
    }

    #[test]
    fn api_version_parses_and_compares() {
        assert_eq!(ApiVersion::parse("1.0").unwrap(), ApiVersion::new(1, 0));
        assert_eq!(ApiVersion::parse(" 2.5 ").unwrap(), ApiVersion::new(2, 5));
        assert!(ApiVersion::parse("1").is_err());
        assert!(ApiVersion::parse("1.2.3").is_err());
        assert!(ApiVersion::new(1, 2) > ApiVersion::new(1, 1));
        assert!(ApiVersion::new(1, 0).is_compatible_with(ApiVersion::new(1, 1)));
        assert!(!ApiVersion::new(1, 2).is_compatible_with(ApiVersion::new(1, 1)));
        assert!(!ApiVersion::new(2, 0).is_compatible_with(ApiVersion::new(1, 9)));
        assert_eq!(ApiVersion::new(1, 3).to_string(), "1.3");
    }

    #[test]
    fn compatible_manifest_is_accepted() {
        let m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        assert_eq!(
            check_compatibility(&m, &CompatibilityPolicy::default()),
            CompatDecision::Compatible
        );
    }

    #[test]
    fn major_version_mismatch_is_incompatible() {
        let m = ExtendedManifest::from_base(manifest("p", "2.0", "libp.so"));
        let decision = check_compatibility(&m, &CompatibilityPolicy::default());
        assert!(!decision.is_compatible());
        assert!(decision.reason().contains("major mismatch"));
    }

    #[test]
    fn newer_minor_is_incompatible_older_is_ok() {
        let newer = ExtendedManifest::from_base(manifest("p", "1.9", "libp.so"));
        assert!(!check_compatibility(&newer, &CompatibilityPolicy::default()).is_compatible());

        let policy = CompatibilityPolicy::new(ApiVersion::new(1, 5));
        let older = ExtendedManifest::from_base(manifest("p", "1.2", "libp.so"));
        assert!(check_compatibility(&older, &policy).is_compatible());
    }

    #[test]
    fn deprecated_minor_is_compatible_with_note() {
        let policy = CompatibilityPolicy::default().deprecate_minor(0, "use 1.1");
        let m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        let decision = check_compatibility(&m, &policy);
        assert!(decision.is_compatible());
        assert!(matches!(
            decision,
            CompatDecision::Deprecated { minor: 0, .. }
        ));
        assert!(decision.reason().contains("use 1.1"));
    }

    #[test]
    fn missing_entry_point_is_incompatible() {
        let m = ExtendedManifest::from_base(manifest("p", "1.0", ""));
        let decision = check_compatibility(&m, &CompatibilityPolicy::default());
        assert!(!decision.is_compatible());
        assert!(decision.reason().contains("entry point"));
    }

    #[test]
    fn unparseable_api_version_is_incompatible() {
        let m = ExtendedManifest::from_base(manifest("p", "not.a.version", "libp.so"));
        assert!(!check_compatibility(&m, &CompatibilityPolicy::default()).is_compatible());
    }

    #[test]
    fn platform_requirement_is_checked_against_current_platform() {
        let (os, arch) = PlatformRequirement::current();
        let mut m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        m.platform = PlatformRequirement::new(&os, &arch);
        assert!(check_compatibility(&m, &CompatibilityPolicy::default()).is_compatible());

        m.platform = PlatformRequirement::new("plan9", &arch);
        assert!(!check_compatibility(&m, &CompatibilityPolicy::default()).is_compatible());
    }

    #[test]
    fn capability_declaration_is_checked() {
        let mut m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        m.capabilities = vec![
            Capability::new("blocks.thermal"),
            Capability::with_version("solvers.stiff", ApiVersion::new(1, 0)),
        ];
        assert!(m.declares("blocks.thermal"));
        assert!(m.declares("solvers.stiff"));
        assert!(!m.declares("blocks.fluid"));
        assert_eq!(
            check_compatibility(&m, &CompatibilityPolicy::default()),
            CompatDecision::Compatible
        );

        // A capability that declares a future API version is incompatible.
        m.capabilities = vec![Capability::with_version("x", ApiVersion::new(1, 99))];
        assert!(!check_compatibility(&m, &CompatibilityPolicy::default()).is_compatible());
    }

    #[test]
    fn source_policy_verification_can_be_required() {
        let mut m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        m.signature_hash = Some("abc".to_string());
        m.source_policy = SourcePolicy::Sha256Hash;
        assert!(
            check_compatibility(
                &m,
                &CompatibilityPolicy::default().require_verified_source(true)
            )
            .is_compatible()
        );

        let unsigned = ExtendedManifest::from_base(manifest("q", "1.0", "libq.so"));
        let decision = check_compatibility(
            &unsigned,
            &CompatibilityPolicy::default().require_verified_source(true),
        );
        assert!(!decision.is_compatible());
        assert!(decision.reason().contains("signature hash"));
    }

    #[test]
    fn extended_manifest_parses_from_json() {
        let json = r#"{
            "name": "thermal",
            "version": "2.1",
            "api_version": "1.0",
            "entry_point": "libthermal.so",
            "capabilities": ["blocks.thermal", {"name": "solvers.stiff", "version": "1.0"}],
            "platform_os": "",
            "dependencies": ["core@1.0"],
            "signature_hash": "deadbeef"
        }"#;
        let m = ExtendedManifest::from_json(json).unwrap();
        assert_eq!(m.base.name, "thermal");
        assert_eq!(m.capabilities.len(), 2);
        assert_eq!(m.source_policy, SourcePolicy::Sha256Hash);
        assert_eq!(m.dependencies, vec!["core@1.0".to_string()]);
        assert!(m.declares("solvers.stiff"));
    }

    #[test]
    fn extended_manifest_rejects_bad_capabilities() {
        let bad = r#"{"name":"p","version":"1.0","entry_point":"l","capabilities":[123]}"#;
        assert!(ExtendedManifest::from_json(bad).is_err());
        let empty = r#"{"name":"p","version":"1.0","entry_point":"l","capabilities":[""]}"#;
        assert!(ExtendedManifest::from_json(empty).is_err());
    }

    #[test]
    fn conflict_detection_finds_duplicate_capabilities_and_missing_deps() {
        let mut a = ExtendedManifest::from_base(manifest("a", "1.0", "la.so"));
        a.capabilities = vec![Capability::new("blocks.x")];
        let mut b = ExtendedManifest::from_base(manifest("b", "1.0", "lb.so"));
        b.capabilities = vec![Capability::new("blocks.x")];
        b.dependencies = vec!["missing@1.0".to_string()];

        let conflicts = detect_conflicts(&[a, b]);
        assert_eq!(conflicts.len(), 2);
        assert!(
            conflicts
                .iter()
                .any(|(_, p)| p.contains("already declared"))
        );
        assert!(conflicts.iter().any(|(_, p)| p.contains("not present")));
    }

    #[test]
    fn compatibility_of_manifest_helper_uses_supplied_host() {
        let m = manifest("p", "1.0", "libp.so");
        let policy = CompatibilityPolicy::default();
        assert!(compatibility_of_manifest(&m, ApiVersion::new(1, 0), &policy).is_compatible());
        assert!(!compatibility_of_manifest(&m, ApiVersion::new(2, 0), &policy).is_compatible());
    }
}
