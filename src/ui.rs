use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;

use crate::app::{
    ActiveTab, ApiConfigModal, App, ConfirmInstallModal, HandoffModal, Modal, NetlifyConfigModal,
    OpsSection, VibeSection,
};
use zipctl::config_patcher::zipctl_config_path;
use zipctl::harnesses::HarnessKind;
use zipctl::platform;
use zipctl::services::ServiceState;
use zipctl::sys::{format_bytes, format_gb, format_rate, DiskStatus};

const BG_ACCENT: Color = Color::Rgb(30, 30, 46);
const FG_MAIN: Color = Color::Rgb(205, 214, 244);
const FG_MUTED: Color = Color::Rgb(147, 153, 178);
const C_MAUVE: Color = Color::Rgb(203, 166, 247);
const C_BLUE: Color = Color::Rgb(137, 180, 250);
const C_GREEN: Color = Color::Rgb(166, 227, 161);
const C_YELLOW: Color = Color::Rgb(249, 226, 175);
const C_RED: Color = Color::Rgb(243, 139, 168);
const C_TEAL: Color = Color::Rgb(148, 226, 213);

pub fn render(frame: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Заголовок и вкладки
            Constraint::Min(10),   // Основная область
            Constraint::Length(3), // Статус-бар и подсказки
        ])
        .split(frame.area());

    render_header(frame, app, root[0]);

    match app.active_tab {
        ActiveTab::System => render_system_tab(frame, app, root[1]),
        ActiveTab::VibeCode => render_vibecode_tab(frame, app, root[1]),
        ActiveTab::GitServices => render_git_services_tab(frame, app, root[1]),
    }

    render_footer(frame, app, root[2]);

    match &app.modal {
        Modal::None => {}
        Modal::ApiConfig(m) => render_api_modal(frame, m),
        Modal::NetlifyConfig(m) => render_netlify_modal(frame, app, m),
        Modal::Handoff(m) => render_handoff_modal(frame, app, m),
        Modal::ConfirmInstall(m) => render_confirm_install_modal(frame, m),
    }
}

fn render_header(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(36)])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " zipctl ",
            Style::default()
                .fg(BG_ACCENT)
                .bg(C_MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", platform::os_badge()), Style::default().fg(FG_MUTED)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_MAUVE)),
    );
    frame.render_widget(title, chunks[0]);

    let titles = vec![
        Line::from(" [1] Система и диски "),
        Line::from(" [2] VibeCode Хаб "),
        Line::from(" [3] Git и Сервисы "),
    ];
    let selected = match app.active_tab {
        ActiveTab::System => 0,
        ActiveTab::VibeCode => 1,
        ActiveTab::GitServices => 2,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(C_BLUE)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .style(Style::default().fg(FG_MUTED))
        .divider(" │ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(C_BLUE))
                .title(" Вкладки (Tab / 1-3 / ЛКМ) "),
        );

    app.areas.tabs_area = chunks[1];
    frame.render_widget(tabs, chunks[1]);
}

fn render_system_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let disk_count = app.sys.disks_status.len();
    let disks_height = match disk_count {
        0..=3 => 6,
        4..=6 if area.height >= 26 => 11,
        _ => (disk_count as u16 + 3).clamp(5, 9),
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),            // ЦП, ОЗУ, Температуры, Сеть
            Constraint::Length(disks_height), // Динамические диски + очистка кэша
            Constraint::Min(8),               // Интерактивный список процессов
        ])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(34),
            Constraint::Percentage(32),
        ])
        .split(rows[0]);

    render_cpu_box(frame, app, top_cols[0]);
    render_mem_box(frame, app, top_cols[1]);
    render_temp_net_box(frame, app, top_cols[2]);

    render_disks_section(frame, app, rows[1]);
    render_process_table(frame, app, rows[2]);
}

fn render_disks_section(frame: &mut Frame, app: &App, area: Rect) {
    let disks = &app.sys.disks_status;
    let low_gb = app.sys.low_space_gb();

    if disks.is_empty() {
        let empty = Paragraph::new(format!(
            "Физические диски и разделы не обнаружены. {}",
            platform::cache_cleanup_hint()
        ))
        .style(Style::default().fg(FG_MUTED))
        .block(
            Block::default()
                .title(" Диски ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(FG_MUTED)),
        );
        frame.render_widget(empty, area);
        return;
    }

    if disks.len() <= 3 {
        let constraints: Vec<Constraint> = (0..disks.len())
            .map(|_| Constraint::Ratio(1, disks.len() as u32))
            .collect();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);
        for (disk, rect) in disks.iter().zip(cols.iter()) {
            render_disk_box(frame, disk, low_gb, *rect);
        }
        return;
    }

    if disks.len() <= 6 && area.height >= 10 {
        let split_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let first_row_count = disks.len().div_ceil(2);
        let (row1, row2) = disks.split_at(first_row_count);

        let c1: Vec<Constraint> = (0..row1.len())
            .map(|_| Constraint::Ratio(1, row1.len() as u32))
            .collect();
        let cols1 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(c1)
            .split(split_rows[0]);
        for (disk, rect) in row1.iter().zip(cols1.iter()) {
            render_disk_box(frame, disk, low_gb, *rect);
        }

        if !row2.is_empty() {
            let c2: Vec<Constraint> = (0..row2.len())
                .map(|_| Constraint::Ratio(1, row2.len() as u32))
                .collect();
            let cols2 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(c2)
                .split(split_rows[1]);
            for (disk, rect) in row2.iter().zip(cols2.iter()) {
                render_disk_box(frame, disk, low_gb, *rect);
            }
        }
        return;
    }

    // Compact table view when many partitions are mounted
    let header = Row::new(vec!["Точка монтирования", "Тип / ФС", "Занято / Всего", "Свободно", "Статус"])
        .style(Style::default().fg(C_MAUVE).add_modifier(Modifier::BOLD));

    let table_rows = disks.iter().map(|d| {
        let kind_fs = if d.disk_kind.is_empty() {
            d.fs_type.clone()
        } else {
            format!("{} {}", d.disk_kind, d.fs_type)
        };
        let status = if d.low_space {
            format!("⚠ МАЛО (<{low_gb} ГБ)")
        } else {
            "OK".to_string()
        };
        let style = if d.low_space {
            Style::default().fg(C_RED)
        } else {
            Style::default().fg(FG_MAIN)
        };
        Row::new(vec![
            d.mount_point.clone(),
            kind_fs,
            format!(
                "{} / {} ({:.0}%)",
                format_gb(d.used_bytes),
                format_gb(d.total_bytes),
                d.ratio() * 100.0
            ),
            format_gb(d.available_bytes),
            status,
        ])
        .style(style)
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Min(18),
            Constraint::Length(14),
            Constraint::Length(26),
            Constraint::Length(12),
            Constraint::Length(18),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(
                " Диски ({}) • {} ",
                disks.len(),
                platform::cache_cleanup_hint()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_GREEN)),
    );
    frame.render_widget(table, area);
}

fn render_cpu_box(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Процессор (ЦП) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_BLUE));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sub = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let ratio = (app.sys.cpu_usage as f64 / 100.0).clamp(0.0, 1.0);
    let color = if app.sys.cpu_usage > 85.0 {
        C_RED
    } else if app.sys.cpu_usage > 60.0 {
        C_YELLOW
    } else {
        C_GREEN
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(BG_ACCENT))
        .ratio(ratio)
        .label(format!(
            "Общая загрузка: {:.1}% ({} ядер)",
            app.sys.cpu_usage,
            app.sys.per_core_usage.len()
        ));
    frame.render_widget(gauge, sub[0]);

    let cores_summary: String = app
        .sys
        .per_core_usage
        .iter()
        .take(8)
        .enumerate()
        .map(|(i, u)| format!("C{i}:{u:>2.0}%"))
        .collect::<Vec<_>>()
        .join(" ");

    let cores_p = Paragraph::new(cores_summary)
        .style(Style::default().fg(FG_MUTED))
        .wrap(Wrap { trim: true });
    frame.render_widget(cores_p, sub[1]);
}

fn render_mem_box(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Память (ОЗУ / Swap) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_TEAL));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sub = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let mem_ratio = if app.sys.mem_total == 0 {
        0.0
    } else {
        (app.sys.mem_used as f64 / app.sys.mem_total as f64).clamp(0.0, 1.0)
    };
    let color = if mem_ratio > 0.85 {
        C_RED
    } else if mem_ratio > 0.65 {
        C_YELLOW
    } else {
        C_TEAL
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(BG_ACCENT))
        .ratio(mem_ratio)
        .label(format!(
            "{} / {} ({:.0}%)",
            format_bytes(app.sys.mem_used),
            format_bytes(app.sys.mem_total),
            mem_ratio * 100.0
        ));
    frame.render_widget(gauge, sub[0]);

    let swap_line = if app.sys.swap_total > 0 {
        format!(
            "Swap: {} / {}",
            format_bytes(app.sys.swap_used),
            format_bytes(app.sys.swap_total)
        )
    } else {
        "Swap: не активен".to_string()
    };
    frame.render_widget(
        Paragraph::new(swap_line).style(Style::default().fg(FG_MUTED)),
        sub[1],
    );
}

fn render_temp_net_box(frame: &mut Frame, app: &App, area: Rect) {
    let cpu_t = app
        .sys
        .cpu_temp
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "н/д".to_string());
    let gpu_t = app
        .sys
        .gpu_temp
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "н/д".to_string());

    let cpu_color = match app.sys.cpu_temp {
        Some(t) if t >= 85.0 => C_RED,
        Some(t) if t >= 70.0 => C_YELLOW,
        _ => C_GREEN,
    };
    let gpu_color = match app.sys.gpu_temp {
        Some(t) if t >= 85.0 => C_RED,
        Some(t) if t >= 70.0 => C_YELLOW,
        _ => C_GREEN,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Темп. ЦП: ", Style::default().fg(FG_MUTED)),
            Span::styled(cpu_t, Style::default().fg(cpu_color).add_modifier(Modifier::BOLD)),
            Span::styled("   ГП: ", Style::default().fg(FG_MUTED)),
            Span::styled(gpu_t, Style::default().fg(gpu_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Сеть ↓:   ", Style::default().fg(FG_MUTED)),
            Span::styled(format_rate(app.sys.rx_rate), Style::default().fg(C_GREEN)),
            Span::styled(
                format!(" (всего {})", format_bytes(app.sys.rx_total)),
                Style::default().fg(FG_MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("Сеть ↑:   ", Style::default().fg(FG_MUTED)),
            Span::styled(format_rate(app.sys.tx_rate), Style::default().fg(C_BLUE)),
            Span::styled(
                format!(" (всего {})", format_bytes(app.sys.tx_total)),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ];

    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" Температура и Сеть ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_MAUVE)),
    );
    frame.render_widget(widget, area);
}

fn render_disk_box(frame: &mut Frame, disk: &DiskStatus, low_space_gb: u64, area: Rect) {
    let border_color = if disk.low_space { C_RED } else { C_GREEN };

    let kind_badge = if disk.disk_kind.is_empty() {
        disk.fs_type.clone()
    } else {
        format!("{} • {}", disk.disk_kind, disk.fs_type)
    };

    let title = if disk.low_space {
        format!(" {} [{}] — ⚠ МАЛО МЕСТА (<{low_space_gb} ГБ) ", disk.mount_point, kind_badge)
    } else if disk.name.is_empty() {
        format!(" {} [{}] ", disk.mount_point, kind_badge)
    } else {
        format!(" {} ({}) [{}] ", disk.mount_point, disk.name, kind_badge)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sub = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let ratio = disk.ratio();
    let gauge_color = if disk.low_space || ratio > 0.90 {
        C_RED
    } else if ratio > 0.75 {
        C_YELLOW
    } else {
        C_GREEN
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color).bg(BG_ACCENT))
        .ratio(ratio)
        .label(format!(
            "Занято {} / {} ({:.0}%)",
            format_gb(disk.used_bytes),
            format_gb(disk.total_bytes),
            ratio * 100.0
        ));
    frame.render_widget(gauge, sub[0]);

    let hint = if disk.low_space {
        Line::from(vec![
            Span::styled(
                format!("Свободно: {}! ", format_gb(disk.available_bytes)),
                Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(platform::cache_cleanup_hint(), Style::default().fg(C_YELLOW)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("Свободно: {}  ", format_gb(disk.available_bytes)),
                Style::default().fg(FG_MAIN),
            ),
            Span::styled(platform::cache_cleanup_hint(), Style::default().fg(FG_MUTED)),
        ])
    };
    frame.render_widget(Paragraph::new(hint), sub[1]);
}

fn render_process_table(frame: &mut Frame, app: &mut App, area: Rect) {
    app.areas.process_table_area = area;

    let header = Row::new(vec!["PID", "Процесс", "ЦП %", "ОЗУ"])
        .style(
            Style::default()
                .fg(C_MAUVE)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(0);

    let rows = app.sys.processes.iter().map(|p| {
        let cpu_style = if p.cpu_usage > 50.0 {
            Style::default().fg(C_RED)
        } else if p.cpu_usage > 15.0 {
            Style::default().fg(C_YELLOW)
        } else {
            Style::default().fg(FG_MAIN)
        };
        Row::new(vec![
            p.pid.to_string(),
            p.name.clone(),
            format!("{:.1}%", p.cpu_usage),
            format_bytes(p.memory_bytes),
        ])
        .style(cpu_style)
    });

    let title = format!(
        " Процессы (сортировка: {} — [s] переключить, [k] завершить, ↑/↓ выбор) ",
        app.sys.sort_mode.label()
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Min(24),
            Constraint::Length(10),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_BLUE)),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::Rgb(49, 50, 68))
            .fg(C_YELLOW)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut app.process_state);
}

fn render_vibecode_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);

    app.areas.harnesses_area = cols[0];
    let harnesses_focused = app.vibe_section == VibeSection::Harnesses;

    let harness_items: Vec<ListItem> = app
        .harnesses
        .iter()
        .map(|h| {
            let status_span = if h.kind == HarnessKind::NetlifyAi {
                let model = app.config.netlify_ai.current_model();
                let prof = app
                    .config
                    .netlify_ai
                    .current_profile()
                    .map(|p| p.profile_dir.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| platform::url_opener_label());
                Span::styled(
                    format!(" [Профиль: {prof} | {model}]"),
                    Style::default().fg(C_TEAL),
                )
            } else if h.installed {
                Span::styled(" [Установлен]", Style::default().fg(C_GREEN))
            } else {
                Span::styled(
                    " [Не установлен — Enter запросит установку]",
                    Style::default().fg(C_YELLOW),
                )
            };

            let cfg_display = h.config_path_display();
            let provider_info = if h.kind == HarnessKind::NetlifyAi {
                "Без API-ключа • [e]/ПКМ: модель и Google-профиль • [h]: Handoff".to_string()
            } else if let Some(saved) = app.config.harnesses.get(h.id) {
                format!(
                    "Провайдер: {} • Модель: {} • {cfg_display}",
                    saved.provider, saved.model
                )
            } else {
                format!("{} • {cfg_display}", h.description)
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!(" {}", h.name),
                        Style::default().fg(FG_MAIN).add_modifier(Modifier::BOLD),
                    ),
                    status_span,
                ]),
                Line::from(Span::styled(
                    format!("   {provider_info}"),
                    Style::default().fg(FG_MUTED),
                )),
            ])
        })
        .collect();

    let harness_list = List::new(harness_items)
        .block(
            Block::default()
                .title(" Харнессы (ЛКМ/Enter: запуск/установка • ПКМ/[e]: конфиг) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if harnesses_focused {
                    C_MAUVE
                } else {
                    FG_MUTED
                })),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(49, 50, 68))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    frame.render_stateful_widget(harness_list, cols[0], &mut app.harness_state);

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(cols[1]);

    app.areas.webchats_area = right_rows[0];
    let web_focused = app.vibe_section == VibeSection::WebChats;
    let opener = platform::url_opener_label();

    let chat_items: Vec<ListItem> = app
        .web_chats
        .iter()
        .map(|c| {
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!(" {}", c.name),
                        Style::default().fg(C_BLUE).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("  ↗ {opener}"), Style::default().fg(FG_MUTED)),
                ]),
                Line::from(Span::styled(
                    format!("   {}", c.description),
                    Style::default().fg(FG_MUTED),
                )),
            ])
        })
        .collect();

    let chat_list = List::new(chat_items)
        .block(
            Block::default()
                .title(" Веб-чаты (ЛКМ/Enter) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if web_focused {
                    C_BLUE
                } else {
                    FG_MUTED
                })),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(49, 50, 68))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    frame.render_stateful_widget(chat_list, right_rows[0], &mut app.webchat_state);

    let active_model = app.config.netlify_ai.current_model();
    let active_profile = app
        .config
        .netlify_ai
        .current_profile()
        .map(|p| p.name.as_str())
        .unwrap_or("Не задан");

    let handoff_card = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Модель Netlify AI: ", Style::default().fg(FG_MUTED)),
            Span::styled(
                active_model,
                Style::default().fg(C_TEAL).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Google-профиль:   ", Style::default().fg(FG_MUTED)),
            Span::styled(active_profile, Style::default().fg(C_YELLOW)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Нажмите [h] чтобы быстро записать заметку",
            Style::default().fg(FG_MAIN),
        )),
        Line::from(Span::styled(
            format!("в ./.zipctl_handoff.md и буфер ({})", platform::clipboard_label()),
            Style::default().fg(C_GREEN),
        )),
        Line::from(Span::styled(
            "для бесшовной смены аккаунта без потери контекста.",
            Style::default().fg(FG_MUTED),
        )),
    ])
    .wrap(Wrap { trim: true })
    .block(
        Block::default()
            .title(" Передача контекста (Handoff) [h] ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_TEAL)),
    );

    frame.render_widget(handoff_card, right_rows[1]);
}

fn render_git_services_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Левая колонка: Git Radar
    app.areas.git_area = cols[0];
    let git_focused = app.ops_section == OpsSection::Git;
    let git_title = format!(
        " Git Radar ({}) — [Enter/g] push • [o] папка ",
        app.config.projects_dir
    );
    let git_block = Block::default()
        .title(git_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if git_focused { C_BLUE } else { FG_MUTED }));

    if app.repos_loading && app.repos.is_empty() {
        let p = Paragraph::new("Сканирование Git-репозиториев в фоне...")
            .style(Style::default().fg(C_YELLOW))
            .block(git_block);
        frame.render_widget(p, cols[0]);
    } else if let Some(ref err) = app.repos_error {
        let p = Paragraph::new(format!(
            "{err}\n\nИзмените параметр projects_dir в:\n{}",
            zipctl_config_path().display()
        ))
        .style(Style::default().fg(C_YELLOW))
        .wrap(Wrap { trim: true })
        .block(git_block);
        frame.render_widget(p, cols[0]);
    } else if app.repos.is_empty() {
        let p = Paragraph::new(format!(
            "В каталоге «{}» не найдено Git-репозиториев.",
            app.config.projects_dir
        ))
        .style(Style::default().fg(FG_MUTED))
        .wrap(Wrap { trim: true })
        .block(git_block);
        frame.render_widget(p, cols[0]);
    } else {
        let repo_items: Vec<ListItem> = app
            .repos
            .iter()
            .map(|r| {
                let state_span = if r.is_dirty() {
                    Span::styled(
                        format!(" [изм: {}]", r.changed_count),
                        Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(" [чисто]", Style::default().fg(C_GREEN))
                };
                let sync = if r.ahead > 0 || r.behind > 0 {
                    format!(" ↑{} ↓{}", r.ahead, r.behind)
                } else {
                    String::new()
                };
                let commit_line = if r.last_commit.is_empty() {
                    "—".to_string()
                } else {
                    r.last_commit.clone()
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" {} ", r.name),
                            Style::default().fg(FG_MAIN).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("({})", r.branch), Style::default().fg(C_MAUVE)),
                        Span::styled(sync, Style::default().fg(C_TEAL)),
                        state_span,
                    ]),
                    Line::from(Span::styled(
                        format!("   {commit_line}"),
                        Style::default().fg(FG_MUTED),
                    )),
                ])
            })
            .collect();

        let list = List::new(repo_items)
            .block(git_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(49, 50, 68))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶");
        frame.render_stateful_widget(list, cols[0], &mut app.repo_state);
    }

    // Правая колонка: Сервисы (systemd на Linux / sc.exe на Windows)
    app.areas.services_area = cols[1];
    let svc_focused = app.ops_section == OpsSection::Services;
    let svc_title = format!(
        " Сервисы ({}) — [Enter/Space] вкл/выкл • [R] рестарт ",
        platform::services_manager_label()
    );
    let svc_block = Block::default()
        .title(svc_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if svc_focused { C_TEAL } else { FG_MUTED }));

    if app.services_loading && app.services.is_empty() {
        let p = Paragraph::new("Опрос состояния служб в фоне...")
            .style(Style::default().fg(C_YELLOW))
            .block(svc_block);
        frame.render_widget(p, cols[1]);
    } else if app.services.is_empty() {
        let p = Paragraph::new(format!(
            "Список служб пуст. Настройте services в:\n{}",
            zipctl_config_path().display()
        ))
        .style(Style::default().fg(FG_MUTED))
        .wrap(Wrap { trim: true })
        .block(svc_block);
        frame.render_widget(p, cols[1]);
    } else {
        let svc_items: Vec<ListItem> = app
            .services
            .iter()
            .map(|s| {
                let state_color = match s.state {
                    ServiceState::Active => C_GREEN,
                    ServiceState::Activating => C_YELLOW,
                    ServiceState::Failed => C_RED,
                    ServiceState::Inactive | ServiceState::NotFound | ServiceState::Unknown => {
                        FG_MUTED
                    }
                };
                let auto_tag = if s.enabled { "auto" } else { "manual" };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" {} ", s.name),
                            Style::default().fg(FG_MAIN).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("[{}]", s.state.label()),
                            Style::default().fg(state_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!(" ({auto_tag})"), Style::default().fg(FG_MUTED)),
                    ]),
                    Line::from(Span::styled(
                        format!("   {}", s.description),
                        Style::default().fg(FG_MUTED),
                    )),
                ])
            })
            .collect();

        let list = List::new(svc_items)
            .block(svc_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(49, 50, 68))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶");
        frame.render_stateful_widget(list, cols[1], &mut app.service_state);
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status_color = if app.status_is_error { C_RED } else { C_GREEN };
    let line = Line::from(vec![
        Span::styled(
            if app.status_is_error { " ✖ " } else { " ● " },
            Style::default().fg(status_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.status_msg, Style::default().fg(FG_MAIN)),
    ]);

    let footer = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(FG_MUTED))
            .title(" Статус • [Tab] вкладка | [h] Handoff | [r] обновить | [q] выход "),
    );
    frame.render_widget(footer, area);
}

fn render_api_modal(frame: &mut Frame, m: &ApiConfigModal) {
    let area = centered_rect(76, 68, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(
            " Настройка API и провайдера — {} ({}) ",
            m.harness_name, m.config_path_display
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(C_MAUVE))
        .style(Style::default().bg(BG_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(2),
        ])
        .split(inner);

    let provider_text = format!("◀  {}  ▶", m.provider.label());
    render_input_field(
        frame,
        rows[0],
        "Провайдер (←/→ выбор пресета)",
        &provider_text,
        m.active_field == 0,
    );
    render_input_field(frame, rows[1], "Base URL", &m.base_url, m.active_field == 1);
    render_input_field(frame, rows[2], "Модель", &m.model, m.active_field == 2);

    let key_label = match &m.existing_key_hint {
        Some(masked) => format!("API-ключ (F2 — показать/скрыть; в файле: {masked})"),
        None => "API-ключ (F2 — показать/скрыть)".to_string(),
    };
    let masked_key = if m.show_key {
        m.api_key.clone()
    } else {
        "•".repeat(m.api_key.chars().count())
    };
    render_input_field(frame, rows[3], &key_label, &masked_key, m.active_field == 3);

    let test_line = if m.testing {
        Line::from(Span::styled(
            " ⏳ Проверка подключения к API...",
            Style::default().fg(C_YELLOW),
        ))
    } else if let Some(ref res) = m.test_result {
        let color = if res.ok { C_GREEN } else { C_RED };
        let icon = if res.ok { "✔" } else { "✖" };
        Line::from(Span::styled(
            format!(" {icon} {}", res.message),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(Span::styled(
            " Нажмите F5 или Ctrl+T для проверки подключения (создаётся .bak перед сохранением).",
            Style::default().fg(FG_MUTED),
        ))
    };
    frame.render_widget(Paragraph::new(test_line), rows[4]);

    let hint = Paragraph::new(
        "Tab / ↑↓ — поля • ←/→ — пресет • F5 / Ctrl+T — тест API • Enter / Ctrl+S — сохранить (.bak создаётся автоматически) • Esc — отмена",
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(FG_MUTED))
    .wrap(Wrap { trim: true });
    frame.render_widget(hint, rows[5]);
}

fn render_confirm_install_modal(frame: &mut Frame, m: &ConfirmInstallModal) {
    let area = centered_rect(68, 44, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Подтверждение установки — {} ", m.harness_name))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(C_YELLOW))
        .style(Style::default().bg(BG_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Бинарник ", Style::default().fg(FG_MAIN)),
            Span::styled(
                &m.missing_binary,
                Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" не найден в PATH.", Style::default().fg(FG_MAIN)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Будет выполнена интерактивная команда установки:",
            Style::default().fg(FG_MUTED),
        )),
        Line::from(Span::styled(
            format!("  $ {}", m.command_display),
            Style::default().fg(C_TEAL).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "TUI временно передаст управление терминалу и вернётся после завершения.",
            Style::default().fg(FG_MUTED),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [y] Выполнить установку ",
                Style::default()
                    .fg(BG_ACCENT)
                    .bg(C_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                " [N / Esc] Отмена ",
                Style::default()
                    .fg(BG_ACCENT)
                    .bg(C_RED)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

fn render_netlify_modal(frame: &mut Frame, app: &App, m: &NetlifyConfigModal) {
    let area = centered_rect(72, 58, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Netlify AI — Выбор модели и Google-профиля (без API-ключа) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(C_TEAL))
        .style(Style::default().bg(BG_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(inner);

    let model_text = format!(
        "◀  {} ({}/{})  ▶",
        app.config.netlify_ai.current_model(),
        app.config.netlify_ai.selected_model + 1,
        app.config.netlify_ai.models.len()
    );
    let model_label = format!(
        "Активная модель (←/→ выбор, список в {})",
        zipctl_config_path().display()
    );
    render_input_field(frame, rows[0], &model_label, &model_text, m.active_field == 0);

    let profile_text = if let Some(p) = app.config.netlify_ai.current_profile() {
        format!(
            "◀  {} [{} --profile-directory={}]  ▶",
            p.name,
            p.browser,
            if p.profile_dir.is_empty() {
                "default"
            } else {
                &p.profile_dir
            }
        )
    } else {
        "Нет профилей".to_string()
    };
    render_input_field(
        frame,
        rows[1],
        "Профиль браузера / Google-аккаунт (←/→)",
        &profile_text,
        m.active_field == 1,
    );

    render_input_field(
        frame,
        rows[2],
        "Добавить новую модель в список (введите имя и нажмите Enter)",
        &m.new_model_input,
        m.active_field == 2,
    );

    let hint = Paragraph::new(
        "←/→ — выбрать модель или профиль • Enter / Ctrl+O — открыть в выбранном профиле • Ctrl+S / Esc — сохранить и закрыть",
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(FG_MUTED))
    .wrap(Wrap { trim: true });
    frame.render_widget(hint, rows[3]);
}

fn render_handoff_modal(frame: &mut Frame, app: &App, m: &HandoffModal) {
    let area = centered_rect(76, 68, frame.area());
    frame.render_widget(Clear, area);

    let title = format!(
        " Передача контекста (./.zipctl_handoff.md + {}) • Модель: {} ",
        platform::clipboard_label(),
        app.config.netlify_ai.current_model()
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(C_GREEN))
        .style(Style::default().bg(BG_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(inner);

    render_input_field(
        frame,
        rows[0],
        "Сделано (Done)",
        &m.note.done,
        m.active_field == 0,
    );
    render_input_field(
        frame,
        rows[1],
        "В процессе (In Progress)",
        &m.note.in_progress,
        m.active_field == 1,
    );
    render_input_field(
        frame,
        rows[2],
        "Следующие шаги (Next Steps)",
        &m.note.next_steps,
        m.active_field == 2,
    );
    render_input_field(
        frame,
        rows[3],
        "Изменённые файлы (Modified Files)",
        &m.note.modified_files,
        m.active_field == 3,
    );

    let hint = Paragraph::new(format!(
        "Tab / ↑↓ — переключение полей • Ctrl+S или Enter на последнем поле — сохранить и скопировать ({}) • Esc — отмена",
        platform::clipboard_label()
    ))
    .alignment(Alignment::Center)
    .style(Style::default().fg(FG_MUTED))
    .wrap(Wrap { trim: true });
    frame.render_widget(hint, rows[4]);
}

fn render_input_field(frame: &mut Frame, area: Rect, label: &str, value: &str, focused: bool) {
    let border_color = if focused { C_YELLOW } else { FG_MUTED };
    let display = if focused {
        format!("{value}▌")
    } else {
        value.to_string()
    };

    let p = Paragraph::new(display)
        .style(Style::default().fg(if focused { C_YELLOW } else { FG_MAIN }))
        .block(
            Block::default()
                .title(format!(" {label} "))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color)),
        );
    frame.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
