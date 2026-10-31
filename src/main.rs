mod agy;
mod freebuff;
mod models;
mod opencode;
mod paths;
mod tui;

use clap::{Parser, Subcommand};
use models::TargetPlatform;

#[derive(Parser, Debug)]
#[command(
    name = "agc",
    about = "OpenCode session migrator to Freebuff or AGY (Antigravity) with Ratatui TUI",
    version = "0.1.0"
)]
struct Cli {
    #[arg(long, help = "Custom path to opencode.db")]
    opencode_db: Option<String>,

    #[arg(long, help = "Custom path to Freebuff data directory")]
    freebuff_dir: Option<String>,

    #[arg(long, help = "Custom path to AGY data directory")]
    agy_dir: Option<String>,

    #[arg(
        long,
        help = "Launch full interactive TUI mode (default when no subcommands provided)"
    )]
    tui: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List OpenCode sessions in CLI format
    List,
    /// Migrate session(s) non-interactively
    Move {
        #[arg(short, long, help = "Target platform: freebuff, agy, or both")]
        target: String,

        #[arg(short, long, help = "Specific OpenCode session ID to migrate")]
        session_id: Option<String>,

        #[arg(long, help = "Migrate all sessions")]
        all: bool,

        #[arg(
            long,
            help = "Delete source sessions from OpenCode after migration (move mode)"
        )]
        delete_source: bool,

        #[arg(long, help = "Simulate migration without modifying any databases")]
        dry_run: bool,
    },
    /// Launch the interactive Ratatui TUI
    Tui,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let opencode_db = match paths::find_opencode_db(cli.opencode_db.as_deref()) {
        Some(p) => p,
        None => {
            eprintln!("[-] Error: Could not locate opencode.db.");
            eprintln!("    Please specify with --opencode-db <path>");
            std::process::exit(1);
        }
    };

    let freebuff_dir = paths::find_freebuff_dir(cli.freebuff_dir.as_deref());
    let agy_dir = paths::find_agy_dir(cli.agy_dir.as_deref());

    // If no subcommand is specified or 'tui' is selected: launch Ratatui TUI!
    match cli.command {
        None | Some(Commands::Tui) => {
            let app_state = tui::AppState::new(opencode_db, freebuff_dir, agy_dir)
                .map_err(|e| format!("Failed to initialize TUI: {}", e))?;
            tui::run_tui(app_state)?;
        }
        Some(Commands::List) => {
            let sessions = opencode::list_sessions(&opencode_db)?;
            println!(
                "\nFound {} session(s) in {}:\n",
                sessions.len(),
                opencode_db.display()
            );
            println!(
                "{:<4} {:<34} {:<10} {:<18} Title / Directory",
                "#", "Session ID", "Messages", "Updated"
            );
            println!("{}", "-".repeat(100));

            for (idx, s) in sessions.iter().enumerate() {
                let dt_str = if s.time_updated > 0 {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(s.time_updated / 1000, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                } else {
                    "Unknown".to_string()
                };

                let mut desc = format!("{} [{}]", s.title, s.directory);
                if desc.len() > 42 {
                    desc = format!("{}...", &desc[..39]);
                }

                println!(
                    "{:<4} {:<34} {:<10} {:<18} {}",
                    idx + 1,
                    s.session_id,
                    s.message_count,
                    dt_str,
                    desc
                );
            }
            println!();
        }
        Some(Commands::Move {
            target,
            session_id,
            all,
            delete_source,
            dry_run,
        }) => {
            let target_enum = match target.to_lowercase().as_str() {
                "freebuff" => TargetPlatform::Freebuff,
                "agy" => TargetPlatform::Agy,
                "both" => TargetPlatform::Both,
                _ => {
                    eprintln!(
                        "[-] Invalid target: '{}'. Expected 'freebuff', 'agy', or 'both'.",
                        target
                    );
                    std::process::exit(1);
                }
            };

            let sessions = opencode::list_sessions(&opencode_db)?;
            let to_migrate: Vec<String> = if all {
                sessions.iter().map(|s| s.session_id.clone()).collect()
            } else if let Some(sid) = session_id {
                if !sessions.iter().any(|s| s.session_id == sid) {
                    eprintln!("[-] Session ID '{}' not found in OpenCode DB.", sid);
                    std::process::exit(1);
                }
                vec![sid]
            } else {
                eprintln!("[-] Please specify --session-id <id> or --all");
                std::process::exit(1);
            };

            println!(
                "\n[*] Migrating {} session(s) to target: {}",
                to_migrate.len(),
                target_enum
            );
            if dry_run {
                println!("    [DRY-RUN] No changes will be written.");
            }

            for sid in to_migrate {
                let session = match opencode::load_full_session(&opencode_db, &sid)? {
                    Some(s) => s,
                    None => {
                        eprintln!("[-] Failed to load session {}", sid);
                        continue;
                    }
                };

                println!(
                    "\n-> Processing: {} (\"{}\", {} messages)",
                    sid,
                    session.title,
                    session.messages.len()
                );

                if !dry_run {
                    if matches!(target_enum, TargetPlatform::Freebuff | TargetPlatform::Both) {
                        if let Some(fb_root) = &freebuff_dir {
                            match freebuff::import_session(&session, fb_root) {
                                Ok(tid) => println!("   [+] Imported to Freebuff thread: {}", tid),
                                Err(e) => eprintln!("   [-] Freebuff import error: {}", e),
                            }
                        } else {
                            eprintln!("   [-] Error: Freebuff directory not found.");
                        }
                    }

                    if matches!(target_enum, TargetPlatform::Agy | TargetPlatform::Both) {
                        if let Some(agy_root) = &agy_dir {
                            match agy::import_session(&session, agy_root) {
                                Ok(cid) => println!("   [+] Imported to AGY conversation: {}", cid),
                                Err(e) => eprintln!("   [-] AGY import error: {}", e),
                            }
                        } else {
                            eprintln!("   [-] Error: AGY directory not found.");
                        }
                    }

                    if delete_source {
                        if let Err(e) = opencode::delete_session(&opencode_db, &sid) {
                            eprintln!("   [-] Failed to delete source session: {}", e);
                        } else {
                            println!("   [+] Removed session from OpenCode source database.");
                        }
                    }
                } else {
                    println!("   [DRY-RUN] Simulated migration successfully.");
                }
            }

            println!("\n[v] Completed!");
        }
    }

    Ok(())
}
