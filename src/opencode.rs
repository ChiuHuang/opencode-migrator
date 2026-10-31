use crate::models::{OpenCodeMessage, OpenCodePart, OpenCodeSession};
use rusqlite::{Connection, OpenFlags, Result};
use std::path::Path;

pub fn list_sessions(db_path: &Path) -> Result<Vec<OpenCodeSession>> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.title, s.directory, s.time_created, s.time_updated, s.model,
                (SELECT count(m.id) FROM message m WHERE m.session_id = s.id) as msg_count
         FROM session s
         ORDER BY s.time_updated DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let directory: Option<String> = row.get(2)?;
        let time_created: Option<i64> = row.get(3)?;
        let time_updated: Option<i64> = row.get(4)?;
        let model: Option<String> = row.get(5)?;
        let msg_count: usize = row.get(6)?;

        Ok(OpenCodeSession {
            session_id: id,
            title: title.unwrap_or_else(|| "Untitled Session".to_string()),
            directory: directory.unwrap_or_default(),
            time_created: time_created.unwrap_or(0),
            time_updated: time_updated.unwrap_or(0),
            model: model.unwrap_or_else(|| "default".to_string()),
            message_count: msg_count,
            messages: Vec::new(),
        })
    })?;

    let mut sessions = Vec::new();
    for r in rows {
        sessions.push(r?);
    }
    Ok(sessions)
}

pub fn load_full_session(db_path: &Path, session_id: &str) -> Result<Option<OpenCodeSession>> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut s_stmt = conn.prepare(
        "SELECT id, title, directory, time_created, time_updated, model
         FROM session WHERE id = ?1",
    )?;

    let mut s_iter = s_stmt.query_map([session_id], |row| {
        let id: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let directory: Option<String> = row.get(2)?;
        let time_created: Option<i64> = row.get(3)?;
        let time_updated: Option<i64> = row.get(4)?;
        let model: Option<String> = row.get(5)?;

        Ok(OpenCodeSession {
            session_id: id,
            title: title.unwrap_or_else(|| "Untitled Session".to_string()),
            directory: directory.unwrap_or_default(),
            time_created: time_created.unwrap_or(0),
            time_updated: time_updated.unwrap_or(0),
            model: model.unwrap_or_else(|| "default".to_string()),
            message_count: 0,
            messages: Vec::new(),
        })
    })?;

    let mut session = match s_iter.next() {
        Some(s) => s?,
        None => return Ok(None),
    };

    let mut m_stmt = conn.prepare(
        "SELECT id, time_created, data FROM message WHERE session_id = ?1 ORDER BY time_created ASC",
    )?;
    let m_rows = m_stmt.query_map([session_id], |row| {
        let id: String = row.get(0)?;
        let time_created: Option<i64> = row.get(1)?;
        let data_raw: Option<String> = row.get(2)?;
        Ok((id, time_created, data_raw))
    })?;

    let mut messages = Vec::new();
    for m in m_rows {
        let (m_id, m_time, m_data_raw) = m?;
        let raw_json: serde_json::Value = m_data_raw
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let role = raw_json
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("user")
            .to_string();

        let mut msg = OpenCodeMessage {
            message_id: m_id.clone(),
            session_id: session_id.to_string(),
            role,
            time_created: m_time.unwrap_or(session.time_created),
            raw_json,
            parts: Vec::new(),
        };

        let mut p_stmt =
            conn.prepare("SELECT id, data FROM part WHERE message_id = ?1 ORDER BY id ASC")?;
        let p_rows = p_stmt.query_map([&m_id], |row| {
            let id: String = row.get(0)?;
            let p_raw: Option<String> = row.get(1)?;
            Ok((id, p_raw))
        })?;

        for p in p_rows {
            let (p_id, p_raw) = p?;
            let p_json: serde_json::Value = p_raw
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            let p_type = p_json
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("text")
                .to_string();

            let text = p_json
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            msg.parts.push(OpenCodePart {
                part_id: p_id,
                message_id: m_id.clone(),
                part_type: p_type,
                text,
                raw_json: p_json,
            });
        }
        messages.push(msg);
    }

    session.message_count = messages.len();
    session.messages = messages;
    Ok(Some(session))
}

pub fn delete_session(db_path: &Path, session_id: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;
    conn.execute("DELETE FROM part WHERE session_id = ?1", [session_id])?;
    conn.execute("DELETE FROM message WHERE session_id = ?1", [session_id])?;
    conn.execute("DELETE FROM session WHERE id = ?1", [session_id])?;
    Ok(())
}
