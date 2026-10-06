use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct AnalysisCache {
    base_dir: PathBuf,
}

impl AnalysisCache {
    pub fn new(data_dir: &Path) -> Self {
        let base_dir = data_dir.join("analysis");
        let _ = fs::create_dir_all(&base_dir);
        Self { base_dir }
    }

    pub fn global() -> Self {
        let base = dirs::data_dir()
            .map(|d| d.join("clipon"))
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(&base)
    }

    pub fn compute_source_key(source_path: &str, start_sec: f64, duration_sec: f64) -> String {
        let p = Path::new(source_path);
        let mtime = p.metadata().and_then(|m| m.modified()).ok();
        let size = p.metadata().map(|m| m.len()).unwrap_or(0);
        let mtime_sec = mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("source")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>();

        let raw =
            format!("{source_path}:{size}:{mtime_sec}:{start_sec:.2}:{duration_sec:.2}:clipon_v2");
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        raw.hash(&mut hasher);
        let hash_val = hasher.finish();

        format!("{stem}_{:016x}", hash_val)
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str, analysis_name: &str) -> Option<T> {
        let path = self
            .base_dir
            .join(key)
            .join(format!("{analysis_name}.json"));
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str(&content) {
                    return Some(val);
                }
            }
        }
        None
    }

    pub fn put<T: Serialize>(&self, key: &str, analysis_name: &str, data: &T) -> Result<()> {
        let dir = self.base_dir.join(key);
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{analysis_name}.json"));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn invalidate(&self, key: &str) -> Result<()> {
        let dir = self.base_dir.join(key);
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        if self.base_dir.exists() {
            for entry in fs::read_dir(&self.base_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
        Ok(())
    }
}
