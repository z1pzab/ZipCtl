use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;

use crate::app::{
    ActiveTab, ApiConfigModal, App, HandoffModal, Modal, NetlifyConfigModal, VibeSection,
};
use crate::harnesses::HarnessKind;
use crate::sys::{format_bytes, format_gb, format_rate, DiskStatus};

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
    }

    render_footer(frame, app, root[2]);

    match &app.modal {
        Modal::None => {}
        Modal::ApiConfig(m) => render_api_modal(frame, m),
        Modal::NetlifyConfig(m) => render_netlify_modal(frame, app, m),
        Modal::Handoff(m) => render_handoff_modal(frame, app, m),
    }
}

fn render_header(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(30)])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " zipctl ",
            Style::default()
                .fg(BG_ACCENT)
                .bg(C_MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Arch / Hyprland", Style::default().fg(FG_MUTED)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_MAUVE)),
    );
    frame.render_widget(title, chunks[0]);

    let titles = vec![
        Line::from(" [1] 󰍛 Система и диски "),
        Line::from(" [2] 󰚩 VibeCode Хаб "),
    ];
    let selected = match app.active_tab {
        ActiveTab::System => 0,
        ActiveTab::VibeCode => 1,
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
                .title(" Вкладки (Tab / ЛКМ) "),
        );

    app.areas.tabs_area = chunks[1];
    frame.render_widget(tabs, chunks[1]);
}

fn render_system_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // ЦП, ОЗУ, Температуры, Сеть
            Constraint::Length(6), // Диски (/ и /mnt/hdd) + очистка кэша
            Constraint::Min(8),    // Интерактивный список процессов
        ])
        .split(area);

    // Верхний ряд: 3 карточки
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

    // Средний ряд: два выделенных диска
    let disk_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_disk_box(frame, &app.sys.root_disk, disk_cols[0]);
    render_disk_box(frame, &app.sys.hdd_disk, disk_cols[1]);

    // Нижний ряд: процессы
    render_process_table(frame, app, rows[2]);
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

fn render_disk_box(frame: &mut Frame, disk: &DiskStatus, area: Rect) {
    let border_color = if disk.low_space {
        C_RED
    } else if !disk.mounted {
        FG_MUTED
    } else {
        C_GREEN
    };

    let title = if disk.low_space {
        format!(" {} — ⚠ МАЛО МЕСТА (<5 ГБ) ", disk.label)
    } else {
        format!(" {} [{}] ", disk.label, disk.fs_type)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !disk.mounted {
        let p = Paragraph::new(format!(
            "Раздел {} не примонтирован. Нажмите [c] для очистки кэша.",
            disk.mount_point
        ))
        .style(Style::default().fg(FG_MUTED));
        frame.render_widget(p, inner);
        return;
    }

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
            Span::styled(
                "Нажмите [c] — очистить paccache и ~/.cache",
                Style::default().fg(C_YELLOW),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("Свободно: {}  ", format_gb(disk.available_bytes)),
                Style::default().fg(FG_MAIN),
            ),
            Span::styled("[c] быстрая очистка кэша", Style::default().fg(FG_MUTED)),
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

    // Левая колонка: Харнессы
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
                    .unwrap_or("xdg-open");
                Span::styled(
                    format!(" [Профиль: {prof} | {model}]"),
                    Style::default().fg(C_TEAL),
                )
            } else if h.installed {
                Span::styled(" [Установлен]", Style::default().fg(C_GREEN))
            } else {
                Span::styled(" [Не установлен — Enter установит]", Style::default().fg(C_YELLOW))
            };

            let provider_info = if h.kind == HarnessKind::NetlifyAi {
                "Без API-ключа • [e]/ПКМ: модель и Google-профиль • [h]: Handoff".to_string()
            } else if let Some(saved) = app.config.harnesses.get(h.id) {
                format!(
                    "Провайдер: {} • Модель: {} • {}",
                    saved.provider, saved.model, h.config_path_display
                )
            } else {
                format!("{} • {}", h.description, h.config_path_display)
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

    // Правая колонка: Веб-чаты + Карточка быстрого Handoff
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(cols[1]);

    app.areas.webchats_area = right_rows[0];
    let web_focused = app.vibe_section == VibeSection::WebChats;

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
                    Span::styled("  ↗ xdg-open", Style::default().fg(FG_MUTED)),
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

    // Карточка Netlify AI & Handoff
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
            "в ./.zipctl_handoff.md и буфер (wl-copy)",
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

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status_color = if app.status_is_error { C_RED } else { C_GREEN };
    let line = Line::from(vec![
        Span::styled(
            if app.status_is_error {
                " ✖ "
            } else {
                " ● "
            },
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
    let area = centered_rect(72, 62, frame.area());
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

    let masked_key = if m.show_key {
        m.api_key.clone()
    } else {
        "•".repeat(m.api_key.chars().count())
    };
    render_input_field(
        frame,
        rows[3],
        "API-ключ (F2 — показать/скрыть)",
        &masked_key,
        m.active_field == 3,
    );

    let hint = Paragraph::new(
        "Tab / ↑↓ — переключение полей • ←/→ — пресет провайдера • Enter / Ctrl+S — сохранить в конфиг • Esc — отмена",
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(FG_MUTED))
    .wrap(Wrap { trim: true });
    frame.render_widget(hint, rows[4]);
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
    render_input_field(
        frame,
        rows[0],
        "Активная модель (←/→ выбор, список в ~/.config/zipctl/config.toml)",
        &model_text,
        m.active_field == 0,
    );

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
        " Передача контекста (./.zipctl_handoff.md + wl-copy) • Модель: {} ",
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

    let hint = Paragraph::new(
        "Tab / ↑↓ — переключение полей • Ctrl+S или Enter на последнем поле — сохранить и скопировать в Wayland-буфер • Esc — отмена",
    )
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
