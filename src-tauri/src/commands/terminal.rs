use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct TerminalSession {
    pub session_id: String,
    pub command: String,
    pub cwd: String,
    pub status: String,
    pub exit_code: Option<i32>,
}

pub struct SessionHandle {
    pub command: String,
    pub cwd: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub child: Arc<tokio::sync::Mutex<Child>>,
    pub kill_tx: Option<mpsc::Sender<()>>,
}

impl SessionHandle {
    fn snapshot(&self) -> TerminalSession {
        TerminalSession {
            session_id: String::new(), // filled by caller
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            status: self.status.clone(),
            exit_code: self.exit_code,
        }
    }
}

#[tauri::command]
pub fn terminal_run(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
    cwd: String,
) -> Result<String, String> {
    let command = command.trim().to_string();
    if command.is_empty() {
        return Err("empty command".to_string());
    }

    let cwd = if cwd.trim().is_empty() {
        dirs::home_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string())
    } else {
        cwd
    };
    let cwd_path = PathBuf::from(&cwd);
    if !cwd_path.is_dir() {
        return Err(format!("cwd is not a directory: {cwd}"));
    }

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(&command);
        c
    };
    cmd.current_dir(&cwd_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let session_id = format!("{:x}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0));

    let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);
    let child_arc = Arc::new(tokio::sync::Mutex::new(child));

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(
            session_id.clone(),
            SessionHandle {
                command: command.clone(),
                cwd: cwd.clone(),
                status: "running".to_string(),
                exit_code: None,
                child: child_arc.clone(),
                kill_tx: Some(kill_tx),
            },
        );
    }

    let app2 = app.clone();
    let sid = session_id.clone();
    tauri::async_runtime::spawn(async move {
        // Reader tasks
        if let Some(out) = stdout {
            let app = app2.clone();
            let sid = sid.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = app.emit(
                        "terminal://output",
                        serde_json::json!({
                            "session_id": sid,
                            "stream": "stdout",
                            "line": line,
                        }),
                    );
                }
            });
        }
        if let Some(err) = stderr {
            let app = app2.clone();
            let sid = sid.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = app.emit(
                        "terminal://output",
                        serde_json::json!({
                            "session_id": sid,
                            "stream": "stderr",
                            "line": line,
                        }),
                    );
                }
            });
        }

        // Wait for exit or kill signal
        let exit_code = tokio::select! {
            status = wait_for_exit(&child_arc) => status,
            _ = kill_rx.recv() => {
                kill_child(&child_arc).await;
                wait_for_exit(&child_arc).await
            }
        };

        let _ = app2.emit(
            "terminal://exit",
            serde_json::json!({ "session_id": sid, "code": exit_code }),
        );
    });

    Ok(session_id)
}

async fn wait_for_exit(child: &Arc<tokio::sync::Mutex<Child>>) -> Option<i32> {
    let mut c = child.lock().await;
    c.wait().await.ok().and_then(|s| s.code())
}

async fn kill_child(child: &Arc<tokio::sync::Mutex<Child>>) {
    #[cfg(target_os = "windows")]
    {
        // Kill the whole process tree on Windows.
        if let Some(pid) = child.lock().await.id() {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = child.lock().await.start_kill();
    }
}

#[tauri::command]
pub fn terminal_kill(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    let Some(session) = sessions.get_mut(&session_id) else {
        return Err(format!("no session: {session_id}"));
    };
    if session.status == "running" {
        if let Some(tx) = session.kill_tx.take() {
            let _ = tx.try_send(());
        }
        session.status = "killed".to_string();
    }
    Ok(())
}

#[tauri::command]
pub fn terminal_list(state: State<'_, AppState>) -> Result<Vec<TerminalSession>, String> {
    let sessions = state.sessions.lock().unwrap();
    let mut out: Vec<TerminalSession> = sessions
        .iter()
        .map(|(id, s)| {
            let mut snap = s.snapshot();
            snap.session_id = id.clone();
            snap
        })
        .collect();
    out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    Ok(out)
}
