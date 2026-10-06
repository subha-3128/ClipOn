use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex, OnceLock},
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
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    pub state: JobState,
    pub progress: u32,
    pub stage: String,
    pub error: Option<String>,
    pub created_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    #[serde(default)]
    pub cancel_state: bool,
}

static GLOBAL_JOB_MANAGER: OnceLock<JobManager> = OnceLock::new();

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobInfo>>>,
    pids: Arc<Mutex<HashMap<String, Vec<u32>>>>,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            pids: Arc::new(Mutex::new(HashMap::new())),
            semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }

    pub fn global() -> &'static JobManager {
        GLOBAL_JOB_MANAGER.get_or_init(JobManager::new)
    }

    pub fn register_process_global(job_id: &str, pid: u32) {
        Self::global().register_process(job_id, pid);
    }

    pub fn unregister_process_global(job_id: &str, pid: u32) {
        Self::global().unregister_process(job_id, pid);
    }

    pub fn is_cancelled_global(job_id: &str) -> bool {
        Self::global().is_cancelled(job_id)
    }

    pub fn semaphore(&self) -> Arc<tokio::sync::Semaphore> {
        self.semaphore.clone()
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn create_job(&self, project_id: &str, initial_stage: &str) -> String {
        self.create_job_with_details(project_id, None, None, initial_stage)
    }

    pub fn create_job_with_details(
        &self,
        project_id: &str,
        input: Option<&str>,
        output: Option<&str>,
        initial_stage: &str,
    ) -> String {
        let job_id = format!("job_{}_{}", uuid::Uuid::new_v4(), Self::now_ms());
        let info = JobInfo {
            id: job_id.clone(),
            project_id: project_id.to_string(),
            input: input.map(|s| s.to_string()),
            output: output.map(|s| s.to_string()),
            state: JobState::Queued,
            progress: 0,
            stage: initial_stage.to_string(),
            error: None,
            created_at_ms: Self::now_ms(),
            completed_at_ms: None,
            cancel_state: false,
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

    fn prune_finished_jobs(jobs: &mut HashMap<String, JobInfo>, max_history: usize) {
        let mut finished: Vec<(String, u64)> = jobs
            .iter()
            .filter(|(_, j)| {
                matches!(
                    j.state,
                    JobState::Completed | JobState::Failed | JobState::Cancelled
                )
            })
            .map(|(id, j)| (id.clone(), j.completed_at_ms.unwrap_or(j.created_at_ms)))
            .collect();

        if finished.len() > max_history {
            finished.sort_by_key(|(_, t)| *t);
            let to_remove = finished.len() - max_history;
            for (id, _) in finished.into_iter().take(to_remove) {
                jobs.remove(&id);
            }
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
            Self::prune_finished_jobs(&mut lock, 50);
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
            Self::prune_finished_jobs(&mut lock, 50);
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
                job.cancel_state = true;
                job.stage = "Cancelled by user".to_string();
                job.completed_at_ms = Some(Self::now_ms());
                emit_info = Some(job.clone());
            } else {
                return Err(format!("Job '{job_id}' not found"));
            }
            Self::prune_finished_jobs(&mut lock, 50);
        }

        if let Ok(mut p_lock) = self.pids.lock() {
            if let Some(list) = p_lock.remove(job_id) {
                pids_to_kill = list;
            }
        }

        // Terminate all registered child processes gracefully
        for pid in pids_to_kill {
            #[cfg(unix)]
            {
                // Send SIGTERM (15) first to allow graceful cleanup
                let _ = Command::new("kill").args(["-15", &pid.to_string()]).output();
                // Spawn a quick asynchronous thread to verify exit before escalating to SIGKILL (9)
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let is_alive = Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if is_alive {
                        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                    }
                });
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
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
                return job.state == JobState::Cancelled || job.cancel_state;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_lifecycle() {
        let mgr = JobManager::new();
        let job_id = mgr.create_job_with_details("proj_1", Some("cand_1"), Some("/out/clip.mp4"), "Starting");

        let job = mgr.get_job(&job_id).expect("job exists");
        assert_eq!(job.state, JobState::Queued);
        assert_eq!(job.progress, 0);
        assert_eq!(job.input.as_deref(), Some("cand_1"));
        assert_eq!(job.output.as_deref(), Some("/out/clip.mp4"));

        mgr.update_progress(&job_id, JobState::Analyzing, 30, "Analyzing", None);
        let job = mgr.get_job(&job_id).unwrap();
        assert_eq!(job.state, JobState::Analyzing);
        assert_eq!(job.progress, 30);

        mgr.complete_job(&job_id, None);
        let job = mgr.get_job(&job_id).unwrap();
        assert_eq!(job.state, JobState::Completed);
        assert_eq!(job.progress, 100);
        assert!(job.completed_at_ms.is_some());
    }

    #[test]
    fn test_job_cancellation() {
        let mgr = JobManager::new();
        let job_id = mgr.create_job("proj_1", "Starting");

        assert!(!mgr.is_cancelled(&job_id));
        mgr.cancel_job(&job_id, None).expect("cancellation succeeds");

        assert!(mgr.is_cancelled(&job_id));
        let job = mgr.get_job(&job_id).unwrap();
        assert_eq!(job.state, JobState::Cancelled);
        assert!(job.cancel_state);
    }

    #[tokio::test]
    async fn test_bounded_queue_semaphore() {
        let mgr = JobManager::new();
        let sem = mgr.semaphore();
        assert_eq!(sem.available_permits(), 2);

        let p1 = sem.clone().try_acquire_owned().expect("slot 1 available");
        assert_eq!(sem.available_permits(), 1);

        let p2 = sem.clone().try_acquire_owned().expect("slot 2 available");
        assert_eq!(sem.available_permits(), 0);

        // Third acquire must fail when permits are exhausted
        assert!(sem.clone().try_acquire_owned().is_err());

        drop(p1);
        assert_eq!(sem.available_permits(), 1);
        let _p3 = sem.clone().try_acquire_owned().expect("slot 3 available after drop");

        drop(p2);
        drop(_p3);
        assert_eq!(sem.available_permits(), 2);
    }

    #[test]
    fn test_bounded_finished_jobs_pruning() {
        let mgr = JobManager::new();

        // Create 60 jobs and complete them
        for i in 0..60 {
            let jid = mgr.create_job("proj_prune", &format!("Stage {i}"));
            mgr.complete_job(&jid, None);
        }

        // Must be pruned to at most 50 completed jobs
        let all_jobs_count = mgr.jobs.lock().unwrap().len();
        assert_eq!(all_jobs_count, 50, "Completed jobs should be pruned to at most 50");
    }
}
