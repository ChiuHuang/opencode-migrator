use crate::models::OpenCodeSession;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub fn import_session(session: &OpenCodeSession, agy_root: &Path) -> Result<String, String> {
    let conv_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("opencode:{}", session.session_id).as_bytes(),
    )
    .to_string();

    let brain_dir = agy_root.join("brain").join(&conv_id);
    let logs_dir = brain_dir.join(".system_generated").join("logs");
    let chunks_t_dir = logs_dir.join("chunks").join("transcript");
    let chunks_tf_dir = logs_dir.join("chunks").join("transcript_full");

    fs::create_dir_all(&chunks_t_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&chunks_tf_dir).map_err(|e| e.to_string())?;

    let mut transcript_lines = Vec::new();
    let mut step_index = 0;
    let mut first_user_text = String::new();

    for msg in &session.messages {
        let ts_sec = if msg.time_created > 0 {
            msg.time_created as f64 / 1000.0
        } else {
            session.time_created as f64 / 1000.0
        };
        let dt = DateTime::<Utc>::from_timestamp(ts_sec as i64, 0).unwrap_or_else(Utc::now);
        let iso_ts = dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let step_obj = if msg.role == "user" {
            let user_content = msg.get_full_text();
            if first_user_text.is_empty() && !user_content.is_empty() {
                first_user_text = user_content.chars().take(200).collect();
            }
            serde_json::json!({
                "step_index": step_index,
                "source": "USER_EXPLICIT",
                "type": "USER_INPUT",
                "status": "DONE",
                "created_at": iso_ts,
                "content": format!("<USER_REQUEST>\n{}\n</USER_REQUEST>", user_content),
            })
        } else {
            let mut assistant_content = msg.get_visible_text();
            if assistant_content.is_empty() {
                assistant_content = msg.get_full_text();
            }
            serde_json::json!({
                "step_index": step_index,
                "source": "MODEL",
                "type": "PLANNER_RESPONSE",
                "status": "DONE",
                "created_at": iso_ts,
                "content": assistant_content,
            })
        };

        transcript_lines.push(serde_json::to_string(&step_obj).unwrap_or_default() + "\n");
        step_index += 1;
    }

    let transcript_data = transcript_lines.concat();
    let transcript_file = logs_dir.join("transcript.jsonl");
    let transcript_full_file = logs_dir.join("transcript_full.jsonl");
    let chunk_file = chunks_t_dir.join("00000000.jsonl");
    let chunk_full_file = chunks_tf_dir.join("00000000.jsonl");

    let _ = fs::write(transcript_file, &transcript_data);
    let _ = fs::write(transcript_full_file, &transcript_data);
    let _ = fs::write(chunk_file, &transcript_data);
    let _ = fs::write(chunk_full_file, &transcript_data);

    // Update history.jsonl
    if !first_user_text.is_empty() {
        let history_file = agy_root.join("history.jsonl");
        let display_title = first_user_text.lines().next().unwrap_or(&session.title);
        let hist_entry = serde_json::json!({
            "display": display_title.chars().take(100).collect::<String>(),
            "timestamp": session.time_created,
            "workspace": session.directory,
            "conversationId": conv_id,
        });
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(history_file)
        {
            let _ = writeln!(
                f,
                "{}",
                serde_json::to_string(&hist_entry).unwrap_or_default()
            );
        }
    }

    // Update conversation_summaries.db
    let sum_db = agy_root.join("conversation_summaries.db");
    let conn = Connection::open(&sum_db).map_err(|e| e.to_string())?;

    conn.execute(
        r#"CREATE TABLE IF NOT EXISTS conversation_summaries (
            conversation_id text primary key,
            title text,
            preview text,
            step_count integer,
            last_modified_time datetime,
            workspace_uris text,
            status text,
            source text,
            project_id text,
            agent_name text,
            parent_conversation_id text,
            nesting_depth integer,
            battle_id text,
            winning_conversation_id text,
            not_fully_idle numeric,
            killed numeric,
            last_user_input_time datetime,
            last_user_input_step_index integer,
            app_data_dir text,
            raw_summary blob,
            group_id text
        )"#,
        [],
    )
    .map_err(|e| e.to_string())?;

    let clean_uri = session
        .directory
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    let workspace_uri = format!("file:///{}", clean_uri);
    let uris_json =
        serde_json::to_string(&vec![workspace_uri]).unwrap_or_else(|_| "[]".to_string());

    let ts_mod = if session.time_updated > 0 {
        session.time_updated as f64 / 1000.0
    } else {
        session.time_created as f64 / 1000.0
    };
    let mod_iso = DateTime::<Utc>::from_timestamp(ts_mod as i64, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let project_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("project:{}", session.directory).as_bytes(),
    )
    .to_string();

    let preview = if !first_user_text.is_empty() {
        first_user_text.chars().take(150).collect::<String>()
    } else {
        session.title.clone()
    };

    conn.execute(
        r#"INSERT OR REPLACE INTO conversation_summaries (
            conversation_id, title, preview, step_count, last_modified_time,
            workspace_uris, status, source, project_id, agent_name,
            parent_conversation_id, nesting_depth, battle_id, winning_conversation_id,
            not_fully_idle, killed, last_user_input_time, last_user_input_step_index,
            app_data_dir, raw_summary, group_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'CASCADE_RUN_STATUS_IDLE', '', ?7, '', '', 0, '', '', 0, 0, ?5, -1, 'antigravity-cli', NULL, '')"#,
        params![
            conv_id,
            session.title,
            preview,
            step_index,
            mod_iso,
            uris_json,
            project_id,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(conv_id)
}
