# zipctl

**Лёгкий кроссплатформенный TUI-центр управления системой, Git-проектами, службами и AI-харнессами на Rust для Windows и Linux (Arch Linux / Hyprland).**

> *English summary is available at the bottom of this document.*

---

## Обзор проекта

`zipctl` объединяет в одном быстром терминальном интерфейсе (на базе `ratatui`, `crossterm`, `sysinfo` и `tokio`) мониторинг ресурсов системы, Git-радар, управление службами и центр запуска/настройки CLI-харнессов и веб-чатов (**VibeCode Хаб**). Приложение поддерживает управление с клавиатуры и мыши (ЛКМ/ПКМ и колесо прокрутки), выполняет сетевые и внешние операции в фоне без фризов TUI и работает на **Windows** и **Linux**.

---

## Возможности

### 1. Вкладка «Система и диски» (`[1]`)
- **Мониторинг в реальном времени**: общая загрузка ЦП и загрузка по ядрам, ОЗУ и Swap, температуры ЦП/ГП (через `sysinfo` с резервным чтением `/sys/class/thermal` и `/sys/class/drm` на Linux), текущая скорость и суммарный трафик сети.
- **Динамическое автоопределение дисков**: автоматически отображает все реальные разделы и диски через `sysinfo::Disks` (с фильтрацией виртуальных ФС Linux: `tmpfs`, `devtmpfs`, `efivarfs`, `proc`, `sysfs` и др.), реальным объёмом, занятым/свободным местом и настраиваемым порогом предупреждения `low_space_gb`.
- **Интерактивный список процессов**: сортировка по ЦП или ОЗУ (`[s]`), выбор стрелками или кликом мыши, завершение процесса клавишей `[k]` или `Delete`.
- **Асинхронная очистка кэша (`[c]`)**: очищает `~/.cache` на Linux (с подсказкой про `sudo paccache -rk1`) или `%TEMP%` на Windows в фоновой задаче без блокировки интерфейса.

### 2. Вкладка «VibeCode Хаб» (`[2]`)
- **Харнессы**: `Claude Code` (`claude`), `Codex` (`codex`), `OpenCode` (`opencode`), `DeepAI Code` (`dsh` / `deepseek-cli`), `ZCode` (`zcode`), `Antigravity` (`antigravity`), `SuperAgent` (`superagent`), `Netlify AI`.
  - **ЛКМ / `Enter`**: проверяет наличие бинарника в `PATH` (на Windows с учётом `PATHEXT`).
    - Если установлен — корректно освобождает терминал (`disable_raw_mode` + выход из `AlternateScreen`), передаёт управление CLI-харнессу и чисто возвращает обратно в TUI после завершения.
    - Если отсутствует — показывает точную команду установки (`npm install -g ...` или `yay -S ...` на Linux) и запрашивает явное подтверждение `[y/N]`.
  - **ПКМ / `[e]`**: открывает модальное окно **«Настройка API и провайдера»** с пресетами (`Coarena AI`, `Codex Relay`, `OpenRouter`, `Official`, `Custom`) и проверкой подключения (`F5` / `Ctrl+T` в фоне). Перед перезаписью любого конфига автоматически создаётся резервная копия `.bak`, а API-ключи маскируются по умолчанию (`F2` — показать/скрыть).
- **Веб-чаты**: `Gemini`, `ChatGPT`, `Claude`, `DeepSeek` — открываются через `xdg-open` (Linux) или `explorer.exe` (Windows).
- **Особый режим — `Netlify AI` (смена аккаунта и передача контекста)**:
  - API-ключ не требуется. По `[e]` или ПКМ открывается селектор модели и переключатель профилей браузера (`--profile-directory=...` для Chromium/Chrome/Edge или `-P` для Firefox).
  - **Передача контекста (`[h]` — Handoff)**: структурированная форма (`Сделано`, `В процессе`, `Следующие шаги`, `Изменённые файлы`), которая сохраняет заметку в `./.zipctl_handoff.md` и копирует её в буфер обмена (`wl-copy`/`xclip`/`xsel` на Linux или `Windows Clipboard` на Windows).

### 3. Вкладка «Git и Сервисы» (`[3]`)
- **Git Radar**: асинхронно сканирует каталог `projects_dir` из конфига, показывает ветку, число изменённых файлов, `↑ahead ↓behind` и последний коммит; `[Enter]` / `[g]` выполняет асинхронный `git add -A && git commit && git push`, `[o]` открывает папку репозитория.
- **Сервисы**: асинхронное управление списком служб из конфига — `systemd` (`systemctl` через `pkexec`/`sudo`) на Linux или службы Windows (`sc.exe`) на Windows (`[Enter]` / `[Space]` — старт/стоп, `[R]` — перезапуск).

---

## Горячие клавиши и мышь

| Клавиша / Мышь | Область | Действие |
|---|---|---|
| `Tab` / `1` / `2` / `3` / ЛКМ по вкладке | Глобально | Переключение вкладок (`Система` / `VibeCode` / `Git и Сервисы`) |
| `h` | Глобально | Открыть панель передачи контекста (**Handoff**) |
| `r` | Глобально | Фоновое обновление метрик, харнессов, Git и служб |
| `q` / `Ctrl+C` | Глобально | Выход из `zipctl` |
| `↑` / `↓` / Колесо мыши | Система | Навигация по списку процессов |
| `s` | Система | Переключить сортировку процессов (`ЦП %` / `ОЗУ`) |
| `k` / `Delete` | Система | Завершить выбранный процесс |
| `c` | Система | Фоновая очистка пользовательского кэша (`~/.cache` / `%TEMP%`) |
| `←` / `→` | VibeCode | Переключение между «Харнессы» и «Веб-чаты» |
| `Enter` / ЛКМ | VibeCode | Запустить харнесс, подтвердить установку `[y/N]` или открыть веб-чат |
| `e` / ПКМ | VibeCode | Настройка API-ключа/провайдера (или модели/профиля для Netlify AI) |
| `F2` | Модал API | Показать / скрыть введённый API-ключ |
| `F5` / `Ctrl+T` | Модал API | Проверить подключение к API-эндпоинту в фоне |
| `←` / `→` | Git и Сервисы | Переключение между «Git Radar» и «Сервисы» |
| `Enter` / `g` | Git Radar | Быстрый `git add -A`, `commit` и `push` |
| `o` | Git Radar | Открыть каталог репозитория в файловом менеджере |
| `Enter` / `Space` / `R` | Сервисы | Запустить/остановить (`Enter`/`Space`) или перезапустить (`R`) службу |
| `Ctrl+S` / `Enter` | Модальные окна | Сохранить и применить изменения |
| `Esc` | Модальные окна | Закрыть модальное окно |

---

## Сборка и установка

### Windows и Linux через Cargo

```bash
git clone https://github.com/z1pzab/ZipCtl.git
cd ZipCtl
cargo build --release
# или установить в PATH:
cargo install --path .
```

### Arch Linux (через `makepkg`)

```bash
git clone https://github.com/z1pzab/ZipCtl.git && cd ZipCtl && bash install.sh
```

Рекомендуемые пакеты на Arch Linux:

```bash
sudo pacman -S wl-clipboard pacman-contrib xdg-utils
```

---

## Конфигурация

При первом запуске `zipctl` автоматически создаёт стандартный конфиг-файл:
- **Linux**: `~/.config/zipctl/config.toml`
- **Windows**: `%APPDATA%\zipctl\config.toml`

Пример структуры:

```toml
low_space_gb = 5
projects_dir = "~/Projects"
services = ["docker", "NetworkManager"]

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
```

### Автоматически патчимые конфиги харнессов (с резервной копией `.bak`)

| Харнесс | Путь (Linux) | Путь (Windows) | Формат |
|---|---|---|---|
| Codex | `~/.codex/config.toml` | `%USERPROFILE%\.codex\config.toml` | TOML |
| Claude Code | `~/.claude.json` | `%USERPROFILE%\.claude.json` | JSON |
| OpenCode | `~/.config/opencode/config.json` | `%APPDATA%\opencode\config.json` | JSON |
| DeepAI Code | `~/.config/deepseek-harness/config.toml` | `%APPDATA%\deepseek-harness\config.toml` | TOML |
| ZCode | `~/.config/zcode/config.json` | `%APPDATA%\zcode\config.json` | JSON |
| Antigravity | `~/.config/antigravity/config.json` | `%APPDATA%\antigravity\config.json` | JSON |
| SuperAgent | `~/.config/superagent/config.json` | `%APPDATA%\superagent\config.json` | JSON |

---

## English Overview

**`zipctl`** is a lightweight cross-platform Rust TUI (`ratatui`, `crossterm`, `sysinfo`, `tokio`) for **Windows** and **Linux**:

- **System & Disks (`[1]`)**: Real-time CPU/per-core usage, RAM/Swap, temperatures, network I/O, interactive process list, dynamic physical disk discovery via `sysinfo::Disks` (filtering Linux virtual filesystems such as `tmpfs`, `devtmpfs`, `efivarfs`, `proc`, `sysfs`), and async user cache cleanup.
- **VibeCode Hub (`[2]`)**: Manage and launch CLI harnesses (`claude`, `codex`, `opencode`, `dsh`, `zcode`, `antigravity`, `superagent`, `Netlify AI`) with clean terminal suspend/resume, explicit `[y/N]` install confirmation, masked API key editing with automatic `.bak` backups, and async API endpoint testing.
- **Git & Services (`[3]`)**: Async Git repository radar (quick commit & push) and cross-platform service manager (`systemctl` on Linux, `sc.exe` on Windows).
