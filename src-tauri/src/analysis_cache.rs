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

    pub fn compute_source_key_with_params(
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        analyzer_version: &str,
        model_version: &str,
        analysis_params: &str,
    ) -> String {
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

        let raw = format!(
            "{source_path}:{size}:{mtime_sec}:{start_sec:.3}:{duration_sec:.3}:{analyzer_version}:{model_version}:{analysis_params}:clipon_v3"
        );
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        raw.hash(&mut hasher);
        let hash_val = hasher.finish();

        format!("{stem}_{:016x}", hash_val)
    }

    pub fn compute_source_key(source_path: &str, start_sec: f64, duration_sec: f64) -> String {
        Self::compute_source_key_with_params(
            source_path,
            start_sec,
            duration_sec,
            "v2",
            "default",
            "",
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_stability() {
        let k1 = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v1",
            "model_a",
            "param=1",
        );
        let k2 = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v1",
            "model_a",
            "param=1",
        );
        assert_eq!(k1, k2, "identical inputs must yield identical cache keys");
    }

    #[test]
    fn test_cache_key_invalidation_on_change() {
        let base = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v1",
            "model_a",
            "param=1",
        );

        let diff_analyzer = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v2",
            "model_a",
            "param=1",
        );
        assert_ne!(base, diff_analyzer, "changing analyzer version must invalidate cache key");

        let diff_model = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v1",
            "model_b",
            "param=1",
        );
        assert_ne!(base, diff_model, "changing model version must invalidate cache key");

        let diff_params = AnalysisCache::compute_source_key_with_params(
            "/path/to/video.mp4",
            10.0,
            20.0,
            "v1",
            "model_a",
            "param=2",
        );
        assert_ne!(base, diff_params, "changing parameters must invalidate cache key");
    }

    #[test]
    fn test_cache_put_get_invalidate() {
        let temp_dir = std::env::temp_dir().join(format!("clipon_cache_test_{}", uuid::Uuid::new_v4()));
        let cache = AnalysisCache::new(&temp_dir);
        let key = "test_key_123";

        #[derive(Serialize, serde::Deserialize, PartialEq, Debug)]
        struct SampleData {
            score: f64,
            label: String,
        }

        let sample = SampleData {
            score: 0.95,
            label: "test".into(),
        };

        assert!(cache.get::<SampleData>(key, "analysis").is_none());
        cache.put(key, "analysis", &sample).expect("put succeeds");

        let retrieved = cache.get::<SampleData>(key, "analysis").expect("get succeeds");
        assert_eq!(retrieved, sample);

        cache.invalidate(key).expect("invalidate succeeds");
        assert!(cache.get::<SampleData>(key, "analysis").is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
