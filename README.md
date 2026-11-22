# zipctl

**Лёгкий (<10 МБ ОЗУ) TUI-центр управления системой и AI-харнессами на Rust для Arch Linux / Hyprland / Kitty.**

> *English summary is available at the bottom of this document.*

---

## Обзор проекта

`zipctl` объединяет в одном быстром терминальном интерфейсе (на базе `ratatui`, `crossterm`, `sysinfo` и `tokio`) мониторинг ресурсов Arch Linux и центр запуска/настройки CLI-агентов и веб-чатов (**VibeCode Хаб**). Приложение поддерживает управление с клавиатуры и мыши (ЛКМ/ПКМ и колесо прокрутки) и оптимизировано под минимальное потребление оперативной памяти и ЦП.

### Предпросмотр интерфейса (ASCII)

```text
╭──────────────────────────╮╭─────────────────────────────────────────────────────────╮
│ zipctl  Arch / Hyprland  ││ [1] 󰍛 Система и диски  │  [2] 󰚩 VibeCode Хаб           │
╰──────────────────────────╯╰─────────────────────────────────────────────────────────╯
╭─ Процессор (ЦП) ─────────╮╭─ Память (ОЗУ / Swap) ────╮╭─ Температура и Сеть ────────╮
│ █████░░░░░ 24.3% (8 ядер)││ ██████░░░░ 6.2/16.0 ГБ   ││ Темп. ЦП: 51°C   ГП: 44°C   │
│ C0:18% C1:31% C2:22% ... ││ Swap: 0.0 / 8.0 ГБ       ││ Сеть ↓: 1.4 МБ/с  ↑: 92 КБ/с│
╰──────────────────────────╯╰──────────────────────────╯╰─────────────────────────────╯
╭─ SSD / (60 ГБ) [ext4] ───────────────────╮╭─ HDD /mnt/hdd (500 ГБ) [ext4] ──────────╮
│ ██████████████░░░░ 51.2 / 60.0 ГБ (85%)  ││ ██████░░░░░░░░░░░░ 182 / 500 ГБ (36%)   │
│ Свободно: 8.8 ГБ  [c] быстрая очистка    ││ Свободно: 318.0 ГБ [c] быстрая очистка  │
╰──────────────────────────────────────────╯╰─────────────────────────────────────────╯
╭─ Процессы (сортировка: ЦП % — [s] переключить, [k] завершить, ↑/↓ выбор) ───────────╮
│ PID      Процесс                  ЦП %       ОЗУ                                    │
│ ▶ 1420   Hyprland                 6.4%       142.3 МБ                               │
│   2815   kitty                    2.1%       64.8 МБ                                │
│   3902   zipctl                   0.2%       6.4 МБ                                 │
╰─────────────────────────────────────────────────────────────────────────────────────╯
╭─ Статус • [Tab] вкладка | [h] Handoff | [r] обновить | [q] выход ───────────────────╮
│ ● Готово. [Tab] вкладки • [h] Handoff • [c] очистка кэша • [q] выход                │
╰─────────────────────────────────────────────────────────────────────────────────────╯
```

---

## Возможности

### 1. Вкладка «Система и диски» (`[1]`)
- **Мониторинг в реальном времени**: общая загрузка ЦП и загрузка по ядрам, ОЗУ и Swap, температуры ЦП/ГП (через `sysinfo` с резервным чтением `/sys/class/thermal` и `/sys/class/drm`), текущая скорость и суммарный трафик сети.
- **Интерактивный список процессов**: сортировка по ЦП или ОЗУ (`[s]`), выбор стрелками или кликом мыши, мгновенное завершение процесса клавишей `[k]` или `Delete`.
- **Выделенные индикаторы дисков**: отдельные шкалы для `/` (60 ГБ SSD) и `/mnt/hdd` (500 ГБ HDD) с предупреждением при остатке менее 5 ГБ (`⚠ МАЛО МЕСТА (<5 ГБ)`).
- **Быстрая очистка кэша (`[c]`)**: запускает `paccache -rk1` и очистку `~/.cache` в отдельном окне Kitty (с резервным фоновым выполнением, если Kitty не установлен).

### 2. Вкладка «VibeCode Хаб» (`[2]`)
- **Харнессы**: `Antigravity`, `Codex`, `Claude Code`, `DeepSeek Harness`, `SuperAgent`, `OpenCode`, `Netlify AI`.
  - **ЛКМ / `Enter`**: проверяет наличие бинарника в `PATH`. Если установлен — запускает в новом окне Kitty; если отсутствует — автоматически запускает установку (`yay -S --noconfirm ...` или `npm i -g ...`) в окне Kitty.
  - **ПКМ / `[e]`**: открывает модальное окно **«Настройка API и провайдера»** с пресетами (`Coarena AI`, `Codex Relay`, `OpenRouter`, `Official`, `Custom`) и автоматически патчит целевой конфиг (`~/.codex/config.toml`, `~/.claude.json`, `~/.config/opencode/config.json` и др.) с сохранением существующих ключей.
- **Веб-чаты**: `Gemini`, `ChatGPT`, `Claude`, `DeepSeek` — открываются через `xdg-open`.
- **Особый режим — `Netlify AI` (смена аккаунта и передача контекста)**:
  - API-ключ не требуется. По `[e]` или ПКМ открывается селектор модели (список редактируется в интерфейсе и в `~/.config/zipctl/config.toml`) и переключатель **Google-профилей браузера** (`--profile-directory=...` для Chromium/Chrome/Brave или `-P` для Firefox).
  - **Передача контекста (`[h]` — Handoff)**: структурированная форма (`Сделано`, `В процессе`, `Следующие шаги`, `Изменённые файлы`), которая сохраняет заметку в `./.zipctl_handoff.md` и копирует её в буфер обмена Wayland через `wl-copy` для мгновенного продолжения работы в новом аккаунте Netlify.

---

## Горячие клавиши и мышь

| Клавиша / Мышь | Область | Действие |
|---|---|---|
| `Tab` / `1` / `2` / ЛКМ по вкладке | Глобально | Переключение вкладок (`Система` / `VibeCode`) |
| `h` | Глобально | Открыть панель передачи контекста (**Handoff**) |
| `r` | Глобально | Принудительно обновить метрики и статусы бинарников |
| `q` / `Ctrl+C` | Глобально | Выход из `zipctl` |
| `↑` / `↓` / Колесо мыши | Система | Навигация по списку процессов |
| `s` | Система | Переключить сортировку процессов (`ЦП %` / `ОЗУ`) |
| `k` / `Delete` | Система | Завершить выбранный процесс |
| `c` | Система | Быстрая очистка кэша (`paccache -rk1` и `~/.cache`) |
| `←` / `→` | VibeCode | Переключение между «Харнессы» и «Веб-чаты» |
| `Enter` / ЛКМ | VibeCode | Запустить / установить харнесс или открыть веб-чат |
| `e` / ПКМ | VibeCode | Настройка API-ключа/провайдера (или модели/профиля для Netlify AI) |
| `F2` | Модал API | Показать / скрыть введённый API-ключ |
| `Ctrl+S` / `Enter` | Модальные окна | Сохранить и применить изменения |
| `Esc` | Модальные окна | Закрыть модальное окно |

---

## Установка на Arch Linux

### Быстрая установка в одну команду (через `makepkg -si`)

```bash
git clone https://github.com/z1pzab/ZipCtl.git && cd ZipCtl && bash install.sh
```

### Ручная сборка через `makepkg`

```bash
git clone https://github.com/z1pzab/ZipCtl.git
cd ZipCtl/packaging
makepkg -si
```

### Установка через Cargo

```bash
git clone https://github.com/z1pzab/ZipCtl.git
cd ZipCtl
cargo install --path .
```

### Рекомендуемые пакеты Arch Linux

```bash
sudo pacman -S kitty wl-clipboard pacman-contrib xdg-utils
```

---

## Конфигурация

При первом запуске `zipctl` автоматически создаёт файл `~/.config/zipctl/config.toml`. Вы можете редактировать список моделей и профилей браузера для **Netlify AI**, а также порог предупреждения о нехватке места на диске:

```toml
low_space_gb = 5

[netlify_ai]
url = "https://app.netlify.com"
selected_model = 0
models = [
    "claude-sonnet-4-5",
    "gemini-2.5-pro",
    "gpt-5",
    "deepseek-r1",
]
selected_profile = 0

[[netlify_ai.profiles]]
name = "Основной Google (Default)"
browser = "chromium"
profile_dir = "Default"

[[netlify_ai.profiles]]
name = "Запасной аккаунт #1 (Profile 1)"
browser = "chromium"
profile_dir = "Profile 1"
```

### Автоматически патчимые конфиги харнессов

| Харнесс | Путь к конфигу | Формат |
|---|---|---|
| Codex | `~/.codex/config.toml` | TOML |
| Claude Code | `~/.claude.json` | JSON |
| OpenCode | `~/.config/opencode/config.json` | JSON |
| Antigravity | `~/.config/antigravity/config.json` | JSON |
| DeepSeek Harness | `~/.config/deepseek-harness/config.toml` | TOML |
| SuperAgent | `~/.config/superagent/config.json` | JSON |
| Netlify AI | `~/.config/zipctl/config.toml` | TOML |

---

## English Overview

**`zipctl`** is an ultra-lightweight (<10 MB RAM) Rust TUI built with `ratatui`, `crossterm`, `sysinfo`, and `tokio` for Arch Linux, Hyprland, and Kitty:

- **System & Disks (`[1]`)**: Live CPU/per-core usage, RAM/Swap, CPU/GPU temperatures, network I/O rates, interactive process table (`k` to kill, `s` to sort), dedicated `/` (60 GB SSD) and `/mnt/hdd` (500 GB HDD) gauges with `<5 GB` low-space warnings, and `[c]` quick cache cleanup (`paccache -rk1` + `~/.cache`).
- **VibeCode Hub (`[2]`)**: Manage and launch CLI harnesses (`Antigravity`, `Codex`, `Claude Code`, `DeepSeek Harness`, `SuperAgent`, `OpenCode`, `Netlify AI`) and web chats (`Gemini`, `ChatGPT`, `Claude`, `DeepSeek` via `xdg-open`). Left-click/`Enter` auto-installs missing binaries via `yay`/`npm` in Kitty or launches installed ones; right-click/`e` opens the API & Provider preset patcher (`Coarena AI`, `Codex Relay`, `OpenRouter`, `Official`, `Custom`).
- **Netlify AI & Handoff (`[h]`)**: Switch models and Google browser profiles (`--profile-directory`) without API keys, and save/copy structured handoff notes to `./.zipctl_handoff.md` + Wayland clipboard (`wl-copy`).

### Quick Install (Arch Linux)

```bash
git clone https://github.com/z1pzab/ZipCtl.git && cd ZipCtl && bash install.sh
```
