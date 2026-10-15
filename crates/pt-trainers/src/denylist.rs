//! The anti-cheat denylist: parse, validate, and check targets.
//!
//! proton-trainer is for single-player games. Whenever the target's
//! executable name or any of its mapped file paths contains a
//! denylist pattern, attach is REFUSED — never bypassed.

use serde::{Deserialize, Serialize};

use crate::error::TrainerError;

/// The embedded default denylist, shipped with the binary.
pub const EMBEDDED_DENYLIST: &str = include_str!("../denylist.toml");

/// The only denylist schema version this release understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// The kind of protected component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An anti-cheat system.
    Anticheat,
}

/// One denylist component and the patterns that identify it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Human-readable component name (e.g. `EasyAntiCheat`).
    pub name: String,
    /// The component kind.
    pub kind: Kind,
    /// Lowercase patterns matched (substring, case-insensitive)
    /// against the target's exe name and mapped file paths.
    #[serde(rename = "match")]
    pub patterns: Vec<String>,
}

/// A parsed denylist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denylist {
    /// Schema version; must be [`SUPPORTED_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The denylist components; written as `[[component]]`.
    #[serde(rename = "component")]
    pub components: Vec<Component>,
}

/// A denylist match against a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The component that matched.
    pub component: String,
    /// The kind of the matched component.
    pub kind: Kind,
    /// The denylist pattern that matched.
    pub pattern: String,
    /// The target string (exe name or mapped path) that matched.
    pub matched: String,
}

impl Refusal {
    /// The plain-English explanation shown to the user on refusal.
    pub fn explanation(&self) -> String {
        format!(
            "refusing to attach: the target matches a known {} ({}: pattern {:?} found in {:?}). \
             proton-trainer supports single-player games only: it never attaches to \
             anti-cheat protected processes and never bypasses anti-cheat.",
            match self.kind {
                Kind::Anticheat => "anti-cheat component",
            },
            self.component,
            self.pattern,
            self.matched,
        )
    }
}

impl Denylist {
    /// Parses a denylist from TOML text and validates it.
    ///
    /// # Errors
    ///
    /// Returns [`TrainerError::Denylist`] for parse or validation
    /// failures.
    pub fn parse(text: &str) -> Result<Self, TrainerError> {
        let denylist: Denylist =
            toml::from_str(text).map_err(|source| TrainerError::Denylist(source.to_string()))?;
        denylist.validate()
    }

    /// Returns the denylist compiled into this binary.
    ///
    /// # Errors
    ///
    /// Returns [`TrainerError::Denylist`] if the embedded file is
    /// invalid (a build-time authoring mistake).
    pub fn embedded() -> Result<Self, TrainerError> {
        Self::parse(EMBEDDED_DENYLIST)
    }

    /// Checks a target against the denylist.
    ///
    /// Matching is case-insensitive substring matching of every
    /// pattern against the target's executable name and every mapped
    /// file path reported by `/proc/<pid>/maps`.
    ///
    /// # Errors
    ///
    /// Returns an error if the denylist itself is invalid.
    pub fn check(
        &self,
        exe_name: &str,
        mapped_paths: &[String],
    ) -> Result<Option<Refusal>, TrainerError> {
        let exe_name = exe_name.to_ascii_lowercase();
        for component in &self.components {
            for pattern in &component.patterns {
                if exe_name.contains(pattern) {
                    return Ok(Some(Refusal {
                        component: component.name.clone(),
                        kind: component.kind,
                        pattern: pattern.clone(),
                        matched: exe_name.clone(),
                    }));
                }
                for path in mapped_paths {
                    let path = path.to_ascii_lowercase();
                    if path.contains(pattern) {
                        return Ok(Some(Refusal {
                            component: component.name.clone(),
                            kind: component.kind,
                            pattern: pattern.clone(),
                            matched: path.clone(),
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Validates structure and normalizes patterns to lowercase.
    fn validate(mut self) -> Result<Self, TrainerError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(TrainerError::Denylist(format!(
                "unsupported schema_version {} (this build supports {})",
                self.schema_version, SUPPORTED_SCHEMA_VERSION
            )));
        }
        for component in &mut self.components {
            if component.name.trim().is_empty() {
                return Err(TrainerError::Denylist(
                    "a component has an empty \"name\"".to_string(),
                ));
            }
            if component.patterns.is_empty() {
                return Err(TrainerError::Denylist(format!(
                    "component {:?} has an empty \"match\" list",
                    component.name
                )));
            }
            for pattern in &mut component.patterns {
                if pattern.trim().is_empty() {
                    return Err(TrainerError::Denylist(format!(
                        "component {:?} has an empty match pattern",
                        component.name
                    )));
                }
                *pattern = pattern.to_ascii_lowercase();
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded() -> Denylist {
        Denylist::embedded().expect("the embedded denylist must be valid")
    }

    #[test]
    fn embedded_denylist_loads() {
        let denylist = embedded();
        let names: Vec<&str> = denylist
            .components
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        for expected in ["EasyAntiCheat", "BattlEye", "Riot Vanguard"] {
            assert!(names.contains(&expected), "missing component {expected}");
        }
    }

    #[test]
    fn refuses_by_exe_name_case_insensitively() {
        let denial = embedded()
            .check("EasyAntiCheat.exe", &[])
            .unwrap()
            .expect("must refuse EAC");
        assert_eq!(denial.component, "EasyAntiCheat");
        assert!(denial.explanation().contains("never bypasses anti-cheat"));
    }

    #[test]
    fn refuses_by_mapped_path() {
        let paths = vec![
            "/usr/lib/wine/EasyAntiCheat_x64.dll".to_string(),
            "/game/game.exe".to_string(),
        ];
        let denial = embedded()
            .check("game.exe", &paths)
            .unwrap()
            .expect("must refuse when a mapped path matches");
        assert_eq!(denial.component, "EasyAntiCheat");
    }

    #[test]
    fn allows_clean_targets() {
        let paths = vec!["/home/user/game/game.exe".to_string()];
        assert!(embedded().check("game.exe", &paths).unwrap().is_none());
        assert!(embedded()
            .check("hollow_knight.exe", &[])
            .unwrap()
            .is_none());
    }

    #[test]
    fn refuses_battleye_and_vanguard() {
        assert!(embedded().check("BattlEye_x64.exe", &[]).unwrap().is_some());
        assert!(embedded().check("vgtray.exe", &[]).unwrap().is_some());
        assert!(embedded().check("vgc.exe", &[]).unwrap().is_some());
    }

    #[test]
    fn clean_word_does_not_falsely_match() {
        // "eac" would be too broad ("breach"); the real patterns are not.
        assert!(embedded().check("breach.exe", &[]).unwrap().is_none());
        assert!(embedded().check("treasure.exe", &[]).unwrap().is_none());
    }

    #[test]
    fn rejects_invalid_denylists() {
        assert!(Denylist::parse("schema_version = 9").is_err());
        let no_patterns = "\
schema_version = 1
[[component]]
name = \"X\"
kind = \"anticheat\"
match = []
";
        assert!(Denylist::parse(no_patterns).is_err());
        assert!(Denylist::parse("bogus = true").is_err());
    }
}
