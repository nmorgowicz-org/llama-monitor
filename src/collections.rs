use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCollection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub preset_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetCollections {
    #[serde(default)]
    pub collections: Vec<PresetCollection>,
}

pub fn collections_path(config_dir: &Path) -> PathBuf {
    config_dir.join("collections.json")
}

pub fn load_collections(config_dir: &Path) -> PresetCollections {
    let path = collections_path(config_dir);
    if path.exists()
        && let Ok(contents) = fs::read_to_string(&path)
        && let Ok(data) = serde_json::from_str(&contents)
    {
        return data;
    }
    PresetCollections::default()
}

pub fn save_collections(config_dir: &Path, data: &PresetCollections) -> Result<()> {
    let path = collections_path(config_dir);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(data)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn slugify(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn unique_id(prefix: &str, name: &str, existing: &[PresetCollection]) -> String {
    let base = slugify(name);
    if existing.iter().any(|c| c.id == base) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("{}-{}-{}", prefix, base, ts)
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Daily Drivers"), "daily-drivers");
        assert_eq!(slugify("For Work / Agents"), "for-work---agents");
    }

    #[test]
    fn test_unique_id_new() {
        let id = unique_id("coll", "Daily Drivers", &[]);
        assert_eq!(id, "daily-drivers");
    }

    #[test]
    fn test_unique_id_conflict() {
        let existing = vec![PresetCollection {
            id: "daily-drivers".into(),
            name: "Daily Drivers".into(),
            description: None,
            preset_ids: vec![],
        }];
        let id = unique_id("coll", "Daily Drivers", &existing);
        assert!(id.starts_with("coll-daily-drivers-"));
    }
}
