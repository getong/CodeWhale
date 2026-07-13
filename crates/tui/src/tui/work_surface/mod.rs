//! Ocean work-surface ownership.
//!
//! This is the replacement boundary for the transcript-top Tasks / To-do /
//! workers UI. Legacy sidebar code may feed other treatments, but Ocean state,
//! rendering, focus, scrolling, and row actions live here as one component.

mod input;
mod model;
mod render;

pub use input::{handle_key, handle_mouse};
pub use model::WorkSurfaceState;
pub use render::{height, render};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend};

    use crate::config::Config;
    use crate::tools::todo::TodoStatus;
    use crate::tui::app::{App, TaskPanelEntry, TaskPanelEntryKind, TuiOptions};

    fn app() -> App {
        let options = TuiOptions {
            model: "deepseek-v4-pro".to_string(),
            workspace: PathBuf::from("."),
            config_path: None,
            config_profile: None,
            allow_shell: false,
            use_alt_screen: true,
            use_mouse_capture: true,
            use_bracketed_paste: true,
            max_subagents: 4,
            skills_dir: PathBuf::from("."),
            memory_path: PathBuf::from("memory.md"),
            notes_path: PathBuf::from("notes.txt"),
            mcp_config_path: PathBuf::from("mcp.json"),
            use_memory: false,
            start_in_agent_mode: false,
            skip_onboarding: true,
            yolo: false,
            resume_session_id: None,
            initial_input: None,
        };
        let mut app = App::new(options, &Config::default());
        app.ui_locale = crate::localization::Locale::En;
        app
    }

    fn add_task(app: &mut App, id: &str) {
        app.task_panel.push(TaskPanelEntry {
            id: id.to_string(),
            status: "running".to_string(),
            prompt_summary: format!("task {id}"),
            duration_ms: Some(1_000),
            kind: TaskPanelEntryKind::Background,
            stale: false,
            elapsed_since_output_ms: None,
            owner_agent_id: None,
            owner_agent_name: None,
        });
    }

    #[test]
    fn projection_keeps_every_todo_reachable() {
        let mut app = app();
        add_task(&mut app, "one");
        let mut todos = app.todos.try_lock().expect("todos");
        for (text, status) in [
            ("done", TodoStatus::Completed),
            ("current", TodoStatus::InProgress),
            ("next", TodoStatus::Pending),
            ("later", TodoStatus::Pending),
        ] {
            todos.add(text.to_string(), status);
        }
        drop(todos);

        let rows = super::model::project(&mut app);
        let todo_rows = rows
            .iter()
            .filter(|row| row.id.0.starts_with("todo:"))
            .count();
        assert_eq!(todo_rows, 4);
        assert!(rows.iter().any(|row| row.label == "later"));
    }

    #[test]
    fn overflow_has_panel_owned_scroll_and_stable_selection() {
        let mut app = app();
        for id in ["one", "two", "three", "four"] {
            add_task(&mut app, id);
        }
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        assert!(app.work_surface.total_rows > app.work_surface.visible_rows);
        assert_eq!(app.work_surface.last_area.expect("area").width, 80);

        let transcript_delta = app.viewport.pending_scroll_delta;
        let outcome = super::handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 10,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(outcome.consumed);
        assert_eq!(app.viewport.pending_scroll_delta, transcript_delta);
        assert!(app.work_surface.scroll_offset > 0);
    }

    #[test]
    fn keyboard_navigation_is_panel_local_when_focused() {
        let mut app = app();
        for id in ["one", "two", "three"] {
            add_task(&mut app, id);
        }
        app.work_surface.visible_rows = 2;
        assert!(
            super::handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT)
            )
            .is_some()
        );
        let first = app.work_surface.selected.clone();
        let _ = super::handle_key(&mut app, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_ne!(app.work_surface.selected, first);
        assert!(app.work_surface.focused);
    }

    #[test]
    fn compact_surface_preserves_task_todo_and_stop_control() {
        let mut app = app();
        add_task(&mut app, "shell_compact");
        app.todos
            .try_lock()
            .expect("todos")
            .add("keep prompt readable".to_string(), TodoStatus::InProgress);
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("task shell_compact"), "{text}");
        assert!(text.contains("keep prompt"), "{text}");
        assert_eq!(app.work_surface.total_rows, 2);
        assert!(
            app.work_surface
                .hitboxes
                .iter()
                .any(|hitbox| hitbox.stop_zone_start_col.is_some())
        );
    }

    #[test]
    fn waiting_row_freezes_other_live_marks() {
        let mut app = app();
        add_task(&mut app, "run");
        add_task(&mut app, "ask");
        app.task_panel[1].status = "waiting".to_string();
        let backend = TestBackend::new(100, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains('◆'), "waiting keeps a still attention mark");
        assert!(
            !text.contains('›'),
            "other live marks freeze under attention"
        );
    }

    #[test]
    fn progress_only_workers_render_before_snapshot_refresh() {
        let mut app = app();
        for index in 1..=3 {
            let id = format!("agent_{index}");
            app.agent_label_map
                .insert(id.clone(), format!("Agent {index}"));
            app.agent_progress.insert(id, "starting".to_string());
        }
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert_eq!(app.work_surface.total_rows, 4, "section plus three workers");
        assert!(text.contains("Agent 1"), "{text}");
        assert!(text.contains("Agent 3"), "{text}");
    }

    #[test]
    fn disappearing_work_clears_owned_mouse_state() {
        let mut app = app();
        add_task(&mut app, "gone");
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        assert!(app.work_surface.last_area.is_some());
        app.work_surface.focused = true;
        app.task_panel.clear();

        assert_eq!(super::height(&mut app, 80, 8), 0);
        assert!(app.work_surface.last_area.is_none());
        assert!(app.work_surface.hitboxes.is_empty());
        assert!(!app.work_surface.focused);
    }

    #[test]
    fn compact_surface_keeps_overflow_rows_reachable() {
        let mut app = app();
        for id in ["one", "two", "three"] {
            add_task(&mut app, id);
        }
        for text in ["first", "second", "third"] {
            app.todos
                .try_lock()
                .expect("todos")
                .add(text.to_string(), TodoStatus::Pending);
        }
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| super::render(frame, frame.area(), &mut app))
            .expect("draw");
        assert_eq!(app.work_surface.total_rows, 6);
        app.work_surface.focused = true;
        let _ = super::handle_key(&mut app, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert!(app.work_surface.scroll_offset > 0);
        assert!(
            app.work_surface
                .selected
                .as_ref()
                .is_some_and(|id| id.0.starts_with("todo:"))
        );
    }
}
