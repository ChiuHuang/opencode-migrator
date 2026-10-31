use crate::models::{FreebuffPart, OpenCodeSession};
use rusqlite::{params, Connection, Result};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const FREEBUFF_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS freebuff_storage_metadata (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id             TEXT PRIMARY KEY,
    root_path      TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS threads (
    id                    TEXT PRIMARY KEY,
    project_id            TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    project_path          TEXT NOT NULL,
    title                 TEXT NOT NULL DEFAULT 'New thread',
    status                TEXT NOT NULL DEFAULT 'open',
    harness_id            TEXT,
    model                 TEXT,
    byok_connection       TEXT,
    reasoning_effort      TEXT,
    agent_mode            TEXT NOT NULL DEFAULT 'build',
    execution_mode        TEXT NOT NULL DEFAULT 'local',
    branch                TEXT,
    worktree_path         TEXT,
    source_branch         TEXT,
    source_ref            TEXT,
    base_ref              TEXT,
    last_seen_head        TEXT,
    turn_state            TEXT NOT NULL DEFAULT 'idle',
    queue_paused          INTEGER NOT NULL DEFAULT 0,
    auto_run              INTEGER NOT NULL DEFAULT 0,
    auto_run_scope        INTEGER NOT NULL DEFAULT 3,
    auto_run_effort       INTEGER NOT NULL DEFAULT 3,
    auto_run_prompt       TEXT,
    auto_run_started_at   INTEGER,
    auto_run_pass_count   INTEGER NOT NULL DEFAULT 0,
    auto_run_refinement_count INTEGER NOT NULL DEFAULT 0,
    auto_run_decision_count INTEGER NOT NULL DEFAULT 0,
    pending_briefs        TEXT,
    pending_briefs_diagnostic_key TEXT,
    world_snapshot        TEXT,
    workspace_epoch       INTEGER NOT NULL DEFAULT 0,
    last_prompt_at        INTEGER,
    last_turn_finished_at INTEGER,
    last_turn_outcome     TEXT,
    attention_revision    INTEGER NOT NULL DEFAULT 0,
    attention_acknowledged_revision INTEGER NOT NULL DEFAULT 0,
    attention_reason      TEXT,
    attention_at          INTEGER,
    auto_run_stopped_note TEXT,
    auto_run_stopped_at   INTEGER,
    auto_run_stopped_outcome TEXT,
    harness_state         TEXT,
    harness_state_id      TEXT,
    freebuff_instance_id  TEXT,
    freebuff_receipt_claim_id TEXT,
    sponsored             TEXT,
    sponsored_run_token   TEXT,
    sponsored_settled_at  INTEGER,
    sponsored_terminal_reports TEXT,
    sponsored_terminal_ack_at INTEGER,
    sponsored_merge_watch TEXT,
    sponsored_pending_accept TEXT,
    fork_source_thread_id TEXT,
    fork_request_id       TEXT,
    coordinator           INTEGER NOT NULL DEFAULT 0,
    coordinator_thread_id TEXT,
    resolved_at           INTEGER,
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL,
    turn_alive_at         INTEGER,
    sidebar_archived_at   INTEGER,
    sidebar_restored_at   INTEGER,
    execution_target      TEXT NOT NULL DEFAULT 'local',
    next_execution_target TEXT,
    cloud_continuation_queued INTEGER NOT NULL DEFAULT 0,
    cloud_handoff_pending INTEGER NOT NULL DEFAULT 0,
    local_handoff_pending INTEGER NOT NULL DEFAULT 0,
    cloud_handback_at     INTEGER,
    location_pinned       INTEGER NOT NULL DEFAULT 0,
    cloud_account_id      TEXT,
    cloud_api_host        TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    seq               INTEGER NOT NULL,
    thread_id         TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    request_id        TEXT,
    input_id          TEXT,
    origin            TEXT,
    steering          INTEGER NOT NULL DEFAULT 0,
    role              TEXT NOT NULL,
    parts_json        TEXT NOT NULL DEFAULT '[]',
    attachments_json  TEXT NOT NULL DEFAULT '[]',
    metrics_json      TEXT NOT NULL DEFAULT '{}',
    ts                INTEGER NOT NULL,
    PRIMARY KEY (seq, thread_id)
);
"#;

fn sanitize_filename(name: &str) -> String {
    let re = regex_lite_or_simple(name);
    if re.is_empty() {
        "project".to_string()
    } else {
        re
    }
}

fn regex_lite_or_simple(name: &str) -> String {
    name.chars()
        .map(|c| if "\\/*?:\"<>| ".contains(c) { '-' } else { c })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn ensure_project(fb_root: &Path, project_path_str: &str) -> Result<(String, PathBuf), String> {
    let state_file = fb_root.join("state.json");
    let projects_dir = fb_root.join("projects");
    fs::create_dir_all(&projects_dir).map_err(|e| e.to_string())?;

    let mut state: serde_json::Value = if state_file.is_file() {
        fs::read_to_string(&state_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !state.is_object() {
        state = serde_json::json!({});
    }

    let norm_path = PathBuf::from(project_path_str)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(project_path_str))
        .to_string_lossy()
        .to_string();

    let mut project_id = None;

    if let Some(projects_map) = state.get("projects").and_then(|p| p.as_object()) {
        for (pk, pdata) in projects_map {
            if pk == &norm_path {
                if let Some(id) = pdata.get("id").and_then(|i| i.as_str()) {
                    project_id = Some(id.to_string());
                    break;
                }
            }
        }
    }

    if project_id.is_none() && projects_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let pj = entry.path().join("project.json");
                if pj.is_file() {
                    if let Ok(content) = fs::read_to_string(&pj) {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(ppath) = parsed.get("projectPath").and_then(|p| p.as_str())
                            {
                                if ppath == norm_path {
                                    if let Some(id) =
                                        parsed.get("projectId").and_then(|i| i.as_str())
                                    {
                                        project_id = Some(id.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let project_id = project_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let proj_name = Path::new(&norm_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");

    let sanitized = sanitize_filename(proj_name);
    let folder_name = format!("{}-{}", sanitized, project_id);
    let target_dir = projects_dir.join(folder_name);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let pj_path = target_dir.join("project.json");
    if !pj_path.exists() {
        let pdata = serde_json::json!({
            "version": 1,
            "projectId": project_id,
            "projectPath": norm_path,
            "database": "desktop-v2.db"
        });
        let _ = fs::write(
            pj_path,
            serde_json::to_string_pretty(&pdata).unwrap_or_default(),
        );
    }

    // Update state.json
    if state.get("projects").is_none() {
        state["projects"] = serde_json::json!({});
    }
    state["projects"][&norm_path] = serde_json::json!({
        "id": project_id,
        "name": proj_name,
        "location": "local",
        "sources": []
    });

    if let Some(recent) = state
        .get_mut("recentProjects")
        .and_then(|r| r.as_array_mut())
    {
        let val = serde_json::Value::String(norm_path.clone());
        if !recent.contains(&val) {
            recent.insert(0, val);
        }
    } else {
        state["recentProjects"] = serde_json::json!([norm_path]);
    }

    let _ = fs::write(
        state_file,
        serde_json::to_string_pretty(&state).unwrap_or_default(),
    );

    let db_path = target_dir.join("desktop-v2.db");
    Ok((project_id, db_path))
}

pub fn import_session(session: &OpenCodeSession, fb_root: &Path) -> Result<String, String> {
    let (project_id, db_path) = ensure_project(fb_root, &session.directory)?;
    let norm_project_path = PathBuf::from(&session.directory)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&session.directory))
        .to_string_lossy()
        .to_string();

    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(FREEBUFF_SCHEMA_SQL)
        .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO freebuff_storage_metadata (key, value) VALUES ('schema_version', '2')",
        [],
    ).map_err(|e| e.to_string())?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let created_at = if session.time_created > 0 {
        session.time_created
    } else {
        now_ms
    };
    let updated_at = if session.time_updated > 0 {
        session.time_updated
    } else {
        now_ms
    };

    conn.execute(
        "INSERT OR IGNORE INTO projects (id, root_path, default_branch, created_at) VALUES (?1, ?2, 'main', ?3)",
        params![project_id, norm_project_path, created_at],
    ).map_err(|e| e.to_string())?;

    let thread_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("opencode:{}", session.session_id).as_bytes(),
    )
    .to_string();

    // Remove existing if re-importing
    let _ = conn.execute(
        "DELETE FROM messages WHERE thread_id = ?1",
        params![thread_id],
    );
    let _ = conn.execute("DELETE FROM threads WHERE id = ?1", params![thread_id]);

    conn.execute(
        r#"INSERT INTO threads (
            id, project_id, project_path, title, status, harness_id, model,
            agent_mode, execution_mode, turn_state, created_at, updated_at,
            last_prompt_at, last_turn_finished_at
        ) VALUES (?1, ?2, ?3, ?4, 'open', 'codebuff', ?5, 'build', 'local', 'idle', ?6, ?7, ?6, ?7)"#,
        params![
            thread_id,
            project_id,
            norm_project_path,
            session.title,
            session.model,
            created_at,
            updated_at,
        ],
    ).map_err(|e| e.to_string())?;

    for (seq, msg) in (1..).zip(session.messages.iter()) {
        let mut parts = Vec::new();
        let reasoning = msg.get_reasoning_text();
        if !reasoning.is_empty() {
            parts.push(FreebuffPart {
                kind: "reasoning".to_string(),
                id: Some(format!("p{}-r", seq)),
                text: reasoning,
            });
        }

        let mut visible = msg.get_visible_text();
        if visible.is_empty() && parts.is_empty() {
            visible = msg.get_full_text();
            if visible.is_empty() {
                visible = "(empty message)".to_string();
            }
        }

        if !visible.is_empty() {
            parts.push(FreebuffPart {
                kind: "text".to_string(),
                id: None,
                text: visible,
            });
        }

        let parts_json = serde_json::to_string(&parts).unwrap_or_else(|_| "[]".to_string());
        let msg_req_id = Uuid::new_v4().to_string();
        let msg_ts = if msg.time_created > 0 {
            msg.time_created
        } else {
            created_at
        };

        conn.execute(
            r#"INSERT INTO messages (
                seq, thread_id, request_id, input_id, origin, steering,
                role, parts_json, attachments_json, metrics_json, ts
            ) VALUES (?1, ?2, ?3, NULL, ?4, 0, ?4, ?5, '[]', '{}', ?6)"#,
            params![seq, thread_id, msg_req_id, msg.role, parts_json, msg_ts,],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(thread_id)
}
