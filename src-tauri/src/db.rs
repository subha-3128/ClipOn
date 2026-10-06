use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::{
    Candidate, CandidateDraft, Clip, ClipCopy, InstagramPost, Project, ProjectDetail, Transcript,
};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).context("opening SQLite database")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;

        let migrations: &[(i64, &str)] = &[
            (
                1,
                "CREATE TABLE IF NOT EXISTS projects (
                    id TEXT PRIMARY KEY,
                    name TEXT,
                    source_path TEXT NOT NULL,
                    source_duration REAL,
                    status TEXT NOT NULL,
                    transcription_mode TEXT NOT NULL,
                    caption_style TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS transcripts (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    engine TEXT NOT NULL,
                    raw_json TEXT NOT NULL,
                    language TEXT,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS candidates (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    start_sec REAL NOT NULL,
                    end_sec REAL NOT NULL,
                    score REAL NOT NULL,
                    hook TEXT NOT NULL,
                    rationale TEXT NOT NULL,
                    rank INTEGER NOT NULL,
                    selected INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS clips (
                    id TEXT PRIMARY KEY,
                    candidate_id TEXT NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
                    status TEXT NOT NULL,
                    output_path TEXT,
                    face_track_json TEXT,
                    caption_ass_path TEXT,
                    render_log TEXT
                );

                CREATE TABLE IF NOT EXISTS clip_copy (
                    id TEXT PRIMARY KEY,
                    clip_id TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
                    platform TEXT NOT NULL,
                    hook_text TEXT,
                    caption_text TEXT,
                    hashtags TEXT
                );

                CREATE TABLE IF NOT EXISTS schedule_entries (
                    id TEXT PRIMARY KEY,
                    clip_id TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
                    platform TEXT NOT NULL,
                    scheduled_for TEXT,
                    status TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS instagram_posts (
                    id TEXT PRIMARY KEY,
                    candidate_id TEXT NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
                    clip_id TEXT REFERENCES clips(id) ON DELETE SET NULL,
                    status TEXT NOT NULL,
                    caption TEXT,
                    post_url TEXT,
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    published_at TEXT
                );",
            ),
            (
                2,
                "CREATE INDEX IF NOT EXISTS idx_transcripts_project_id ON transcripts(project_id);
                CREATE INDEX IF NOT EXISTS idx_candidates_project_id ON candidates(project_id);
                CREATE INDEX IF NOT EXISTS idx_clips_candidate_id ON clips(candidate_id);
                CREATE INDEX IF NOT EXISTS idx_instagram_candidate_id ON instagram_posts(candidate_id);",
            ),
            (
                3,
                "ALTER TABLE candidates ADD COLUMN layout_override TEXT;",
            ),
        ];

        for (ver, sql) in migrations {
            let already_applied: bool = conn.query_row(
                "SELECT COUNT(1) FROM schema_migrations WHERE version = ?1",
                params![ver],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )?;

            if !already_applied {
                let tx = conn.transaction()?;
                tx.execute_batch(sql)?;
                let now = Utc::now().to_rfc3339();
                tx.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![ver, now],
                )?;
                tx.commit()?;
            }
        }

        // Backward compatibility for pre-existing installations
        execute_migration_alter(&conn, "ALTER TABLE projects ADD COLUMN name TEXT")?;
        execute_migration_alter(&conn, "ALTER TABLE projects ADD COLUMN caption_style TEXT")?;
        execute_migration_alter(&conn, "ALTER TABLE candidates ADD COLUMN layout_override TEXT")?;
        Ok(())
    }

    pub fn create_project(
        &self,
        source_path: &str,
        transcription_mode: &str,
        caption_style: &str,
        source_duration: Option<f64>,
    ) -> Result<Project> {
        let now = Utc::now().to_rfc3339();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: None,
            source_path: source_path.to_string(),
            source_duration,
            status: "ingest".to_string(),
            transcription_mode: transcription_mode.to_string(),
            caption_style: Some(caption_style.to_string()),
            created_at: now.clone(),
            updated_at: now,
        };

        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT INTO projects (id, name, source_path, source_duration, status, transcription_mode, caption_style, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project.id,
                project.name,
                project.source_path,
                project.source_duration,
                project.status,
                project.transcription_mode,
                project.caption_style,
                project.created_at,
                project.updated_at
            ],
        )?;

        Ok(project)
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, source_path, source_duration, status, transcription_mode, created_at, updated_at, caption_style
             FROM projects ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([], project_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_project(&self, project_id: &str) -> Result<Project> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.query_row(
            "SELECT id, name, source_path, source_duration, status, transcription_mode, created_at, updated_at, caption_style
             FROM projects WHERE id = ?1",
            params![project_id],
            project_from_row,
        )
        .map_err(Into::into)
    }

    pub fn project_detail(&self, project_id: &str) -> Result<ProjectDetail> {
        let project = self.get_project(project_id)?;
        let transcript = self.latest_transcript(project_id)?;
        let candidates = self.list_candidates(project_id)?;
        let clips = self.list_clips_for_project(project_id)?;
        let copy = self.list_copy_for_project(project_id)?;
        let instagram_posts = self.list_instagram_posts_for_project(project_id)?;

        Ok(ProjectDetail {
            project,
            transcript,
            candidates,
            clips,
            copy,
            instagram_posts,
        })
    }

    pub fn update_project_status(
        &self,
        project_id: &str,
        status: &str,
        source_duration: Option<f64>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "UPDATE projects SET status = ?1, source_duration = COALESCE(?2, source_duration), updated_at = ?3 WHERE id = ?4",
            params![status, source_duration, now, project_id],
        )?;
        Ok(())
    }

    pub fn save_transcript(
        &self,
        project_id: &str,
        engine: &str,
        raw_json: &str,
        language: Option<&str>,
    ) -> Result<Transcript> {
        let transcript = Transcript {
            id: Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            engine: engine.to_string(),
            raw_json: raw_json.to_string(),
            language: language.map(ToOwned::to_owned),
            created_at: Utc::now().to_rfc3339(),
        };

        let mut conn = self.conn.lock().expect("database mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM transcripts WHERE project_id = ?1",
            params![project_id],
        )?;
        tx.execute(
            "INSERT INTO transcripts (id, project_id, engine, raw_json, language, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                transcript.id,
                transcript.project_id,
                transcript.engine,
                transcript.raw_json,
                transcript.language,
                transcript.created_at
            ],
        )?;
        tx.commit()?;
        Ok(transcript)
    }

    pub fn latest_transcript(&self, project_id: &str) -> Result<Option<Transcript>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.query_row(
            "SELECT id, project_id, engine, raw_json, language, created_at
             FROM transcripts WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![project_id],
            |row| {
                Ok(Transcript {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    engine: row.get(2)?,
                    raw_json: row.get(3)?,
                    language: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn replace_candidates(
        &self,
        project_id: &str,
        drafts: &[CandidateDraft],
    ) -> Result<Vec<Candidate>> {
        let mut conn = self.conn.lock().expect("database mutex poisoned");
        let tx = conn.transaction()?;

        // Delete only unrendered/uncommitted candidate drafts.
        // Candidates that have completed rendered clips or instagram posts are strictly preserved.
        tx.execute(
            "DELETE FROM candidates 
             WHERE project_id = ?1 
               AND id NOT IN (
                   SELECT candidate_id FROM clips WHERE status = 'done' OR output_path IS NOT NULL
               )
               AND id NOT IN (
                   SELECT candidate_id FROM instagram_posts
               )",
            params![project_id],
        )?;

        // Fetch remaining preserved candidates to determine rank and avoid duplicates
        let mut preserved = Vec::new();
        {
            let mut preserved_stmt = tx.prepare(
                "SELECT id, project_id, start_sec, end_sec, score, hook, rationale, rank, selected, layout_override
                 FROM candidates WHERE project_id = ?1 ORDER BY rank ASC",
            )?;
            let preserved_rows = preserved_stmt.query_map(params![project_id], candidate_from_row)?;
            for row in preserved_rows {
                preserved.push(row?);
            }
        }

        let max_rank = preserved.iter().map(|c| c.rank).max().unwrap_or(0);
        let selected_cutoff = drafts.len().min(6).max(3).min(drafts.len());

        let mut next_rank = max_rank;
        for (index, draft) in drafts.iter().enumerate() {
            // Check if draft closely matches an already preserved candidate
            let is_duplicate = preserved.iter().any(|p| {
                (p.start_sec - draft.start).abs() < 1.5 && (p.end_sec - draft.end).abs() < 1.5
            });
            if is_duplicate {
                continue;
            }

            next_rank += 1;
            let candidate = Candidate {
                id: Uuid::new_v4().to_string(),
                project_id: project_id.to_string(),
                start_sec: draft.start,
                end_sec: draft.end,
                score: draft.score,
                hook: draft.hook.clone(),
                rationale: draft.rationale.clone(),
                rank: next_rank,
                selected: index < selected_cutoff,
                layout_override: None,
            };

            tx.execute(
                "INSERT INTO candidates (id, project_id, start_sec, end_sec, score, hook, rationale, rank, selected, layout_override)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &candidate.id,
                    &candidate.project_id,
                    candidate.start_sec,
                    candidate.end_sec,
                    candidate.score,
                    &candidate.hook,
                    &candidate.rationale,
                    candidate.rank,
                    if candidate.selected { 1 } else { 0 },
                    &candidate.layout_override
                ],
            )?;

            tx.execute(
                "INSERT INTO clips (id, candidate_id, status) VALUES (?1, ?2, 'pending')",
                params![Uuid::new_v4().to_string(), &candidate.id],
            )?;
        }

        tx.commit()?;
        drop(conn);
        self.list_candidates(project_id)
    }

    pub fn update_candidate_timing(
        &self,
        candidate_id: &str,
        start_sec: f64,
        end_sec: f64,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "UPDATE candidates SET start_sec = ?1, end_sec = ?2 WHERE id = ?3",
            params![start_sec, end_sec, candidate_id],
        )?;
        Ok(())
    }

    pub fn update_candidate_layout_override(
        &self,
        candidate_id: &str,
        layout_override: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "UPDATE candidates SET layout_override = ?1 WHERE id = ?2",
            params![layout_override, candidate_id],
        )?;
        Ok(())
    }

    pub fn list_candidates(&self, project_id: &str) -> Result<Vec<Candidate>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, start_sec, end_sec, score, hook, rationale, rank, selected, layout_override
             FROM candidates WHERE project_id = ?1 ORDER BY rank ASC",
        )?;
        let rows = stmt.query_map(params![project_id], candidate_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_candidate_with_project(&self, candidate_id: &str) -> Result<(Candidate, Project)> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.query_row(
            "SELECT
                candidates.id, candidates.project_id, candidates.start_sec, candidates.end_sec,
                candidates.score, candidates.hook, candidates.rationale, candidates.rank, candidates.selected,
                candidates.layout_override,
                projects.id, projects.name, projects.source_path, projects.source_duration, projects.status,
                projects.transcription_mode, projects.created_at, projects.updated_at, projects.caption_style
             FROM candidates
             INNER JOIN projects ON projects.id = candidates.project_id
             WHERE candidates.id = ?1",
            params![candidate_id],
            |row| {
                let selected: i64 = row.get(8)?;
                let candidate = Candidate {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    start_sec: row.get(2)?,
                    end_sec: row.get(3)?,
                    score: row.get(4)?,
                    hook: row.get(5)?,
                    rationale: row.get(6)?,
                    rank: row.get(7)?,
                    selected: selected == 1,
                    layout_override: row.get(9).ok(),
                };
                let project = Project {
                    id: row.get(10)?,
                    name: row.get(11)?,
                    source_path: row.get(12)?,
                    source_duration: row.get(13)?,
                    status: row.get(14)?,
                    transcription_mode: row.get(15)?,
                    created_at: row.get(16)?,
                    updated_at: row.get(17)?,
                    caption_style: row.get(18)?,
                };
                Ok((candidate, project))
            },
        )
        .map_err(Into::into)
    }

    pub fn update_clip_for_candidate(
        &self,
        candidate_id: &str,
        status: &str,
        output_path: Option<&str>,
        caption_ass_path: Option<&str>,
        render_log: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "UPDATE clips
             SET status = ?1,
                 output_path = COALESCE(?2, output_path),
                 caption_ass_path = COALESCE(?3, caption_ass_path),
                 render_log = COALESCE(?4, render_log)
             WHERE candidate_id = ?5",
            params![
                status,
                output_path,
                caption_ass_path,
                render_log,
                candidate_id
            ],
        )?;
        Ok(())
    }

    pub fn set_selected_clip_count(
        &self,
        project_id: &str,
        count: usize,
    ) -> Result<Vec<Candidate>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "UPDATE candidates SET selected = CASE WHEN rank <= ?1 THEN 1 ELSE 0 END WHERE project_id = ?2",
            params![count as i64, project_id],
        )?;
        drop(conn);
        self.list_candidates(project_id)
    }

    pub fn list_clips_for_project(&self, project_id: &str) -> Result<Vec<Clip>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT clips.id, clips.candidate_id, clips.status, clips.output_path, clips.face_track_json, clips.caption_ass_path, clips.render_log
             FROM clips
             INNER JOIN candidates ON candidates.id = clips.candidate_id
             WHERE candidates.project_id = ?1
             ORDER BY candidates.rank ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(Clip {
                id: row.get(0)?,
                candidate_id: row.get(1)?,
                status: row.get(2)?,
                output_path: row.get(3)?,
                face_track_json: row.get(4)?,
                caption_ass_path: row.get(5)?,
                render_log: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn list_copy_for_project(&self, project_id: &str) -> Result<Vec<ClipCopy>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT clip_copy.id, clip_copy.clip_id, clip_copy.platform, clip_copy.hook_text, clip_copy.caption_text, clip_copy.hashtags
             FROM clip_copy
             INNER JOIN clips ON clips.id = clip_copy.clip_id
             INNER JOIN candidates ON candidates.id = clips.candidate_id
             WHERE candidates.project_id = ?1",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(ClipCopy {
                id: row.get(0)?,
                clip_id: row.get(1)?,
                platform: row.get(2)?,
                hook_text: row.get(3)?,
                caption_text: row.get(4)?,
                hashtags: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
        Ok(())
    }

    pub fn rename_project(&self, project_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now, project_id],
        )?;
        Ok(())
    }

    pub fn list_instagram_posts_for_project(&self, project_id: &str) -> Result<Vec<InstagramPost>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT p.id, p.candidate_id, p.clip_id, p.status, p.caption, p.post_url, p.error_message, p.created_at, p.published_at 
             FROM instagram_posts p 
             INNER JOIN candidates c ON c.id = p.candidate_id 
             WHERE c.project_id = ?1 
             ORDER BY p.created_at DESC"
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(InstagramPost {
                id: row.get(0)?,
                candidate_id: row.get(1)?,
                clip_id: row.get(2)?,
                status: row.get(3)?,
                caption: row.get(4)?,
                post_url: row.get(5)?,
                error_message: row.get(6)?,
                created_at: row.get(7)?,
                published_at: row.get(8)?,
            })
        })?;
        let mut posts = Vec::new();
        for r in rows {
            posts.push(r?);
        }
        Ok(posts)
    }

    pub fn upsert_instagram_post(
        &self,
        candidate_id: &str,
        clip_id: Option<&str>,
        status: &str,
        caption: Option<&str>,
        post_url: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<InstagramPost> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let now = Utc::now().to_rfc3339();
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM instagram_posts WHERE candidate_id = ?1",
                params![candidate_id],
                |row| row.get(0),
            )
            .optional()?;

        let published_at = if status == "published" {
            Some(now.clone())
        } else {
            None
        };

        let post = if let Some(id) = existing {
            conn.execute(
                "UPDATE instagram_posts SET clip_id = COALESCE(?1, clip_id), status = ?2, caption = COALESCE(?3, caption), post_url = COALESCE(?4, post_url), error_message = ?5, published_at = COALESCE(?6, published_at) WHERE id = ?7",
                params![clip_id, status, caption, post_url, error_message, published_at, id],
            )?;
            InstagramPost {
                id,
                candidate_id: candidate_id.to_string(),
                clip_id: clip_id.map(ToOwned::to_owned),
                status: status.to_string(),
                caption: caption.map(ToOwned::to_owned),
                post_url: post_url.map(ToOwned::to_owned),
                error_message: error_message.map(ToOwned::to_owned),
                created_at: now,
                published_at,
            }
        } else {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO instagram_posts (id, candidate_id, clip_id, status, caption, post_url, error_message, created_at, published_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![id, candidate_id, clip_id, status, caption, post_url, error_message, now, published_at],
            )?;
            InstagramPost {
                id,
                candidate_id: candidate_id.to_string(),
                clip_id: clip_id.map(ToOwned::to_owned),
                status: status.to_string(),
                caption: caption.map(ToOwned::to_owned),
                post_url: post_url.map(ToOwned::to_owned),
                error_message: error_message.map(ToOwned::to_owned),
                created_at: now,
                published_at,
            }
        };
        Ok(post)
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute_batch(
            "
            DELETE FROM instagram_posts;
            DELETE FROM schedule_entries;
            DELETE FROM clip_copy;
            DELETE FROM clips;
            DELETE FROM candidates;
            DELETE FROM transcripts;
            DELETE FROM projects;
            VACUUM;
            ",
        )?;
        Ok(())
    }
}

fn execute_migration_alter(conn: &Connection, sql: &str) -> Result<()> {
    if let Err(e) = conn.execute(sql, []) {
        let err_msg = e.to_string().to_lowercase();
        if err_msg.contains("duplicate column") {
            Ok(())
        } else {
            Err(e).with_context(|| format!("failed migration statement: {sql}"))
        }
    } else {
        Ok(())
    }
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        source_path: row.get(2)?,
        source_duration: row.get(3)?,
        status: row.get(4)?,
        transcription_mode: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        caption_style: row.get(8)?,
    })
}

fn candidate_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Candidate> {
    let selected: i64 = row.get(8)?;
    let layout_override: Option<String> = row.get(9).ok();
    Ok(Candidate {
        id: row.get(0)?,
        project_id: row.get(1)?,
        start_sec: row.get(2)?,
        end_sec: row.get(3)?,
        score: row.get(4)?,
        hook: row.get(5)?,
        rationale: row.get(6)?,
        rank: row.get(7)?,
        selected: selected == 1,
        layout_override,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> Database {
        let temp_file = std::env::temp_dir().join(format!("clipon_test_db_{}.sqlite", Uuid::new_v4()));
        Database::open(&temp_file).expect("open temp db")
    }

    #[test]
    fn test_fresh_db_migrations() {
        let db = temp_db();
        let conn = db.conn.lock().unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(1) FROM schema_migrations", [], |row| row.get(0))
            .expect("query migrations");
        assert!(count >= 2, "must have at least 2 migrations applied");
    }

    #[test]
    fn test_migrations_idempotence() {
        let db = temp_db();
        // Running migrate() repeatedly should succeed without error
        assert!(db.migrate().is_ok());
        assert!(db.migrate().is_ok());
    }

    #[test]
    fn test_foreign_key_cascade_deletion() {
        let db = temp_db();
        let proj = db
            .create_project("/dummy/path.mp4", "local", "modern-box", Some(60.0))
            .expect("create project");

        let drafts = vec![CandidateDraft {
            start: 0.0,
            end: 15.0,
            score: 9.0,
            hook: "Test hook".into(),
            rationale: "Rationale".into(),
        }];

        let candidates = db.replace_candidates(&proj.id, &drafts).expect("replace candidates");
        assert_eq!(candidates.len(), 1);

        // Delete project
        db.delete_project(&proj.id).expect("delete project");

        // Candidates must be automatically deleted via foreign key cascade
        let remaining = db.list_candidates(&proj.id).expect("list candidates");
        assert!(remaining.is_empty(), "cascading delete should clean up candidates");
    }

    #[test]
    fn test_foreign_key_constraint_enforced() {
        let db = temp_db();
        let conn = db.conn.lock().unwrap();

        // Inserting a candidate pointing to a non-existent project_id must fail
        let res = conn.execute(
            "INSERT INTO candidates (id, project_id, start_sec, end_sec, score, hook, rationale, rank, selected)
             VALUES (?1, ?2, 0.0, 10.0, 8.0, 'Hook', 'Rat', 1, 0)",
            params!["cand_orphan", "non_existent_project_id"],
        );
        assert!(res.is_err(), "foreign key constraint must reject orphan candidate");
    }

    #[test]
    fn test_candidate_layout_override_persistence() {
        let db = temp_db();
        let proj = db
            .create_project("/dummy/path.mp4", "local", "modern-box", Some(60.0))
            .expect("create project");

        let drafts = vec![CandidateDraft {
            start: 0.0,
            end: 15.0,
            score: 9.0,
            hook: "Test hook".into(),
            rationale: "Rationale".into(),
        }];

        let candidates = db.replace_candidates(&proj.id, &drafts).expect("replace candidates");
        let cand_id = &candidates[0].id;
        assert_eq!(candidates[0].layout_override, None);

        db.update_candidate_layout_override(cand_id, Some("split_two"))
            .expect("update override");
        let (updated, _) = db.get_candidate_with_project(cand_id).expect("get candidate");
        assert_eq!(updated.layout_override.as_deref(), Some("split_two"));

        db.update_candidate_layout_override(cand_id, None)
            .expect("clear override");
        let (cleared, _) = db.get_candidate_with_project(cand_id).expect("get candidate");
        assert_eq!(cleared.layout_override, None);
    }

    #[test]
    fn test_replace_candidates_preserves_rendered_clips_and_posts() {
        let db = temp_db();
        let proj = db
            .create_project("/dummy/path.mp4", "local", "modern-box", Some(120.0))
            .expect("create project");

        let drafts_v1 = vec![
            CandidateDraft {
                start: 0.0,
                end: 15.0,
                score: 9.0,
                hook: "Rendered Clip Hook".into(),
                rationale: "Rationale 1".into(),
            },
            CandidateDraft {
                start: 20.0,
                end: 35.0,
                score: 8.0,
                hook: "Pending Clip Hook".into(),
                rationale: "Rationale 2".into(),
            },
        ];

        let cands_v1 = db.replace_candidates(&proj.id, &drafts_v1).expect("replace 1");
        assert_eq!(cands_v1.len(), 2);
        let cand1_id = cands_v1[0].id.clone();
        let cand2_id = cands_v1[1].id.clone();

        // Mark candidate 1 as rendered (done)
        db.update_clip_for_candidate(&cand1_id, "done", Some("/clips/clip1.mp4"), None, None)
            .expect("mark done");

        // Rerun candidate generation with new drafts
        let drafts_v2 = vec![
            CandidateDraft {
                start: 40.0,
                end: 55.0,
                score: 8.5,
                hook: "New Discovery Hook".into(),
                rationale: "Rationale 3".into(),
            },
        ];

        let cands_v2 = db.replace_candidates(&proj.id, &drafts_v2).expect("replace 2");
        
        // Candidate 1 must STILL EXIST because it has a finished rendered clip!
        assert!(cands_v2.iter().any(|c| c.id == cand1_id));

        // Candidate 2 (pending unrendered) was replaced
        assert!(!cands_v2.iter().any(|c| c.id == cand2_id));

        // Candidate 3 is newly discovered
        assert!(cands_v2.iter().any(|c| c.hook == "New Discovery Hook"));

        // Clips table still has candidate 1's finished clip
        let clips = db.list_clips_for_project(&proj.id).expect("list clips");
        let clip1 = clips.iter().find(|c| c.candidate_id == cand1_id).unwrap();
        assert_eq!(clip1.status, "done");
        assert_eq!(clip1.output_path.as_deref(), Some("/clips/clip1.mp4"));
    }

    #[test]
    fn test_sqlite_wal_and_pragmas() {
        let db = temp_db();
        let conn = db.conn.lock().unwrap();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("query journal_mode");
        assert_eq!(journal_mode.to_lowercase(), "wal");

        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("query foreign_keys");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn test_execute_migration_alter_accepts_duplicate_column_and_rejects_other_errors() {
        let db = temp_db();
        let conn = db.conn.lock().unwrap();

        // 1. Adding an already existing column must succeed (duplicate column is accepted as harmless)
        let dup_res = execute_migration_alter(&conn, "ALTER TABLE projects ADD COLUMN name TEXT");
        assert!(dup_res.is_ok(), "duplicate column error must be accepted as harmless");

        // 2. Syntax errors or invalid table references must return an Err, not be swallowed
        let err_res = execute_migration_alter(&conn, "ALTER TABLE non_existent_table ADD COLUMN test_col TEXT");
        assert!(err_res.is_err(), "non-duplicate errors must be propagated");
    }

    #[test]
    fn test_save_transcript_transactional_integrity() {
        let db = temp_db();
        let proj = db
            .create_project("/dummy/path.mp4", "local", "modern-box", Some(60.0))
            .expect("create project");

        let t1 = db
            .save_transcript(&proj.id, "deepgram", "{\"words\":[{\"text\":\"first\"}]}", Some("en"))
            .expect("save transcript 1");
        assert_eq!(t1.engine, "deepgram");

        let latest1 = db.latest_transcript(&proj.id).expect("latest transcript").unwrap();
        assert_eq!(latest1.engine, "deepgram");
        assert!(latest1.raw_json.contains("first"));

        let t2 = db
            .save_transcript(&proj.id, "whisper", "{\"words\":[{\"text\":\"second\"}]}", Some("en"))
            .expect("save transcript 2");
        assert_eq!(t2.engine, "whisper");

        let latest2 = db.latest_transcript(&proj.id).expect("latest transcript").unwrap();
        assert_eq!(latest2.engine, "whisper");
        assert!(latest2.raw_json.contains("second"));
    }
}
