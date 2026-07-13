use ratatui::{
    Frame,
    layout::Rect,
    prelude::Widget,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::tui::app::{App, SidebarHoverRow, SidebarHoverSection};
use crate::tui::ui_text::truncate_line_to_width;

use super::model::{WorkHitbox, WorkRow, WorkTone, project};

/// Responsive work-surface height. The component owns a bounded window; long
/// work lists scroll instead of consuming the transcript.
pub fn height(app: &mut App, _width: u16, terminal_height: u16) -> u16 {
    let rows = project(app);
    if rows.is_empty() {
        app.work_surface.focused = false;
        app.work_surface.selected = None;
        app.work_surface.hovered = None;
        app.work_surface.last_area = None;
        app.work_surface.hitboxes.clear();
        app.work_surface.latest_rows.clear();
        app.work_surface.visible_rows = 0;
        app.work_surface.total_rows = 0;
        app.work_surface.scroll_offset = 0;
        return 0;
    }
    match terminal_height {
        0..=12 => 3,
        13..=16 => 5,
        17..=23 => 6,
        _ => 8,
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if area.width == 0 || area.height == 0 {
        app.work_surface.last_area = None;
        return;
    }

    if let Some(previous) = app.work_surface.last_area {
        app.sidebar_hover
            .sections
            .retain(|section| section.content_area != previous);
    }

    let mut rows = project(app);
    if area.height <= 3 {
        // Compact fallback spends its two content rows on the first actionable
        // Task and To-do/worker objects instead of section chrome.
        let mut compact = Vec::new();
        for prefix in ["task:", "todo:", "worker:"] {
            if let Some(row) = rows.iter().find(|row| row.id.0.starts_with(prefix)) {
                compact.push(row.clone());
            }
        }
        for row in rows.iter().filter(|row| row.selectable) {
            if !compact.iter().any(|candidate| candidate.id == row.id) {
                compact.push(row.clone());
            }
        }
        rows = compact;
    }
    let body_height = usize::from(area.height.saturating_sub(1));
    let overflow = rows.len() > body_height;
    let inset = u16::from(area.width >= 60);
    let rail_width = u16::from(overflow);
    let content_area = Rect {
        x: area.x.saturating_add(inset),
        y: area.y,
        width: area
            .width
            .saturating_sub(inset.saturating_mul(2))
            .saturating_sub(rail_width),
        height: area.height.saturating_sub(1),
    };

    app.work_surface.visible_rows = body_height;
    app.work_surface.total_rows = rows.len();
    app.work_surface.clamp_selection(&rows);
    let max_offset = rows.len().saturating_sub(body_height.max(1));
    app.work_surface.scroll_offset = app.work_surface.scroll_offset.min(max_offset);

    Block::default()
        .style(Style::default().bg(app.ui_theme.surface_bg))
        .render(area, frame.buffer_mut());

    let start = app.work_surface.scroll_offset;
    let visible = rows
        .iter()
        .skip(start)
        .take(body_height)
        .collect::<Vec<_>>();
    let mut lines = Vec::with_capacity(visible.len());
    let mut hover_rows = Vec::new();
    let mut hitboxes = Vec::new();
    for (visible_index, row) in visible.iter().enumerate() {
        let row_y = content_area.y.saturating_add(visible_index as u16);
        let selected =
            app.work_surface.focused && app.work_surface.selected.as_ref() == Some(&row.id);
        let hovered = app.work_surface.hovered.as_ref() == Some(&row.id);
        let style = row_style(app, row, selected || hovered);
        let controls = controls_text(app, row, content_area.width);
        let controls_width = UnicodeWidthStr::width(controls.as_str());
        let compact_owner = if area.height <= 3 {
            row.id
                .0
                .split_once(':')
                .map(|(kind, _)| match kind {
                    "task" => format!(
                        "{} · ",
                        app.tr(crate::localization::MessageId::SidebarTasksLabel)
                    ),
                    "todo" => format!(
                        "{} · ",
                        app.tr(crate::localization::MessageId::SidebarTodoLabel)
                    ),
                    "worker" => format!(
                        "{} · ",
                        app.tr(crate::localization::MessageId::FleetRosterWorkers)
                    ),
                    _ => String::new(),
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let prefix = if row.tone == WorkTone::Heading {
            format!("{} ", row.mark)
        } else {
            format!("{compact_owner}{} ", row.mark)
        };
        let label_width = usize::from(content_area.width)
            .saturating_sub(UnicodeWidthStr::width(prefix.as_str()) + controls_width)
            .max(1);
        let label = truncate_line_to_width(&row.label, label_width);
        let gap = usize::from(content_area.width).saturating_sub(
            UnicodeWidthStr::width(prefix.as_str())
                + UnicodeWidthStr::width(label.as_str())
                + controls_width,
        );
        let display = format!("{prefix}{label}{}{controls}", " ".repeat(gap));
        lines.push(Line::from(Span::styled(display.clone(), style)));
        let stop_width = if row.stop_action.is_some() {
            UnicodeWidthStr::width(if content_area.width < 60 {
                " stop"
            } else {
                " [stop]"
            })
        } else {
            0
        };
        let row_right = content_area.x.saturating_add(content_area.width);
        let stop_start = row
            .stop_action
            .as_ref()
            .map(|_| row_right.saturating_sub(stop_width as u16));
        hitboxes.push(WorkHitbox {
            id: row.id.clone(),
            row_y,
            stop_zone_start_col: stop_start,
            stop_zone_end_col: stop_start.map(|start| start.saturating_add(stop_width as u16)),
        });

        if row.selectable {
            hover_rows.push(SidebarHoverRow {
                row_y,
                display_text: display,
                full_text: row.label.clone(),
                detail: Some(row.detail.clone()),
                is_truncated: label != row.label,
                click_action: row.primary_action.clone(),
                stop_action: row.stop_action.clone(),
                stop_zone_start_col: stop_start,
                stop_zone_end_col: stop_start.map(|start| start.saturating_add(stop_width as u16)),
            });
        }
    }

    Paragraph::new(lines).render(content_area, frame.buffer_mut());
    render_rule(frame, area, app);
    if overflow {
        render_scrollbar(
            frame,
            area,
            app.work_surface.scroll_offset,
            body_height,
            rows.len(),
            app,
        );
    }

    app.work_surface.last_area = Some(area);
    app.work_surface.hitboxes = hitboxes;
    app.sidebar_hover.sections.push(SidebarHoverSection {
        content_area,
        lines: visible.iter().map(|row| row.label.clone()).collect(),
        rows: hover_rows,
    });
}

fn controls_text(app: &App, row: &WorkRow, width: u16) -> String {
    let open = app.tr(crate::localization::MessageId::SidebarOpenControl);
    let stop = app.tr(crate::localization::MessageId::SidebarStopControl);
    match (
        row.primary_action.is_some(),
        row.stop_action.is_some(),
        width < 60,
    ) {
        (true, true, true) => format!(" {open} {stop}"),
        (true, false, true) => format!(" {open}"),
        (true, true, false) => format!(" [{open}] [{stop}]"),
        (true, false, false) => format!(" [{open}]"),
        _ => String::new(),
    }
}

fn row_style(app: &App, row: &WorkRow, highlighted: bool) -> Style {
    let fg = match row.tone {
        WorkTone::Heading => app.ui_theme.accent_primary,
        WorkTone::Live => app.ui_theme.status_working,
        WorkTone::Attention => app.ui_theme.error_fg,
        WorkTone::Success => app.ui_theme.success,
        WorkTone::Muted => app.ui_theme.text_muted,
        WorkTone::Worker => app.ui_theme.accent_secondary,
    };
    let mut style = Style::default().fg(fg).bg(app.ui_theme.surface_bg);
    if row.tone == WorkTone::Heading {
        style = style.add_modifier(Modifier::BOLD);
    }
    if highlighted && row.selectable {
        style = style
            .bg(app.ui_theme.selection_bg)
            .add_modifier(Modifier::BOLD);
    }
    style
}

fn render_rule(frame: &mut Frame, area: Rect, app: &App) {
    let y = area.bottom().saturating_sub(1);
    for x in area.left()..area.right() {
        frame.buffer_mut()[(x, y)]
            .set_symbol("─")
            .set_fg(app.ui_theme.border)
            .set_bg(app.ui_theme.surface_bg);
    }
}

fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    offset: usize,
    visible: usize,
    total: usize,
    app: &App,
) {
    let rail_height = area.height.saturating_sub(1);
    if rail_height == 0 || total == 0 {
        return;
    }
    let thumb_height = ((usize::from(rail_height) * visible) / total)
        .max(1)
        .min(usize::from(rail_height));
    let max_offset = total.saturating_sub(visible).max(1);
    let max_start = usize::from(rail_height).saturating_sub(thumb_height);
    let thumb_start = offset.saturating_mul(max_start) / max_offset;
    let x = area.right().saturating_sub(1);
    for row in 0..usize::from(rail_height) {
        let in_thumb = row >= thumb_start && row < thumb_start.saturating_add(thumb_height);
        frame.buffer_mut()[(x, area.y.saturating_add(row as u16))]
            .set_symbol(if in_thumb { "█" } else { "│" })
            .set_fg(if in_thumb {
                app.ui_theme.text_hint
            } else {
                app.ui_theme.border
            })
            .set_bg(app.ui_theme.surface_bg);
    }
}
