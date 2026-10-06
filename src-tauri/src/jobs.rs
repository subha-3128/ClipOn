use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Analyzing,
    Processing,
    Encoding,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: String,
    pub project_id: String,
    pub state: JobState,
    pub progress: u32,
    pub stage: String,
    pub error: Option<String>,
    pub created_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

#[derive(Clone, Default)]
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobInfo>>>,
    pids: Arc<Mutex<HashMap<String, Vec<u32>>>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            pids: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn create_job(&self, project_id: &str, initial_stage: &str) -> String {
        let job_id = format!("job_{}_{}", uuid::Uuid::new_v4(), Self::now_ms());
        let info = JobInfo {
            id: job_id.clone(),
            project_id: project_id.to_string(),
            state: JobState::Queued,
            progress: 0,
            stage: initial_stage.to_string(),
            error: None,
            created_at_ms: Self::now_ms(),
            completed_at_ms: None,
        };

        if let Ok(mut lock) = self.jobs.lock() {
            lock.insert(job_id.clone(), info);
        }
        job_id
    }

    pub fn update_progress(
        &self,
        job_id: &str,
        state: JobState,
        progress: u32,
        stage: &str,
        app: Option<&tauri::AppHandle>,
    ) {
        let mut emit_info = None;
        if let Ok(mut lock) = self.jobs.lock() {
            if let Some(job) = lock.get_mut(job_id) {
                if job.state != JobState::Cancelled {
                    job.state = state;
                    job.progress = progress.min(100);
                    job.stage = stage.to_string();
                    emit_info = Some(job.clone());
                }
            }
        }

        if let (Some(info), Some(handle)) = (emit_info, app) {
            let _ = handle.emit("job-progress", info);
        }
    }

    pub fn complete_job(&self, job_id: &str, app: Option<&tauri::AppHandle>) {
        let mut emit_info = None;
        if let Ok(mut lock) = self.jobs.lock() {
            if let Some(job) = lock.get_mut(job_id) {
                if job.state != JobState::Cancelled {
                    job.state = JobState::Completed;
                    job.progress = 100;
                    job.stage = "Completed".to_string();
                    job.completed_at_ms = Some(Self::now_ms());
                    emit_info = Some(job.clone());
                }
            }
        }

        if let (Some(info), Some(handle)) = (emit_info, app) {
            let _ = handle.emit("job-progress", info);
        }
        self.cleanup_pids(job_id);
    }

    pub fn fail_job(&self, job_id: &str, err_msg: &str, app: Option<&tauri::AppHandle>) {
        let mut emit_info = None;
        if let Ok(mut lock) = self.jobs.lock() {
            if let Some(job) = lock.get_mut(job_id) {
                if job.state != JobState::Cancelled {
                    job.state = JobState::Failed;
                    job.stage = "Failed".to_string();
                    job.error = Some(err_msg.to_string());
                    job.completed_at_ms = Some(Self::now_ms());
                    emit_info = Some(job.clone());
                }
            }
        }

        if let (Some(info), Some(handle)) = (emit_info, app) {
            let _ = handle.emit("job-progress", info);
        }
        self.cleanup_pids(job_id);
    }

    pub fn register_process(&self, job_id: &str, pid: u32) {
        if let Ok(mut lock) = self.pids.lock() {
            lock.entry(job_id.to_string()).or_default().push(pid);
        }
    }

    pub fn unregister_process(&self, job_id: &str, pid: u32) {
        if let Ok(mut lock) = self.pids.lock() {
            if let Some(list) = lock.get_mut(job_id) {
                list.retain(|p| *p != pid);
            }
        }
    }

    fn cleanup_pids(&self, job_id: &str) {
        if let Ok(mut lock) = self.pids.lock() {
            lock.remove(job_id);
        }
    }

    pub fn cancel_job(&self, job_id: &str, app: Option<&tauri::AppHandle>) -> Result<(), String> {
        let mut emit_info = None;
        let mut pids_to_kill = Vec::new();

        if let Ok(mut lock) = self.jobs.lock() {
            if let Some(job) = lock.get_mut(job_id) {
                job.state = JobState::Cancelled;
                job.stage = "Cancelled by user".to_string();
                job.completed_at_ms = Some(Self::now_ms());
                emit_info = Some(job.clone());
            } else {
                return Err(format!("Job '{job_id}' not found"));
            }
        }

        if let Ok(mut p_lock) = self.pids.lock() {
            if let Some(list) = p_lock.remove(job_id) {
                pids_to_kill = list;
            }
        }

        // Terminate all registered child processes
        for pid in pids_to_kill {
            #[cfg(unix)]
            {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
            }
        }

        if let (Some(info), Some(handle)) = (emit_info, app) {
            let _ = handle.emit("job-progress", info);
        }

        Ok(())
    }

    pub fn is_cancelled(&self, job_id: &str) -> bool {
        if let Ok(lock) = self.jobs.lock() {
            if let Some(job) = lock.get(job_id) {
                return job.state == JobState::Cancelled;
            }
        }
        false
    }

    pub fn get_job(&self, job_id: &str) -> Option<JobInfo> {
        self.jobs.lock().ok()?.get(job_id).cloned()
    }

    pub fn list_active_jobs(&self) -> Vec<JobInfo> {
        self.jobs
            .lock()
            .map(|m| {
                m.values()
                    .filter(|j| {
                        matches!(
                            j.state,
                            JobState::Queued
                                | JobState::Analyzing
                                | JobState::Processing
                                | JobState::Encoding
                        )
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}
