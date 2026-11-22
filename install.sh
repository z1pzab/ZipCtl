#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Установка zipctl ==="

if [[ "${1:-}" == "--cargo" ]]; then
    echo "-> Сборка и установка через cargo install --path ."
    cd "${SCRIPT_DIR}"
    cargo install --path . --force
    echo "-> Готово! Запустите: zipctl"
    exit 0
fi

if command -v makepkg >/dev/null 2>&1 && [[ -f "${SCRIPT_DIR}/packaging/PKGBUILD" ]]; then
    echo "-> Обнаружен Arch Linux (makepkg). Сборка и установка пакета..."
    cd "${SCRIPT_DIR}/packaging"
    makepkg -si --needed
    echo ""
    echo "-> Готово! Запустите zipctl в Kitty или любом терминале."
elif command -v cargo >/dev/null 2>&1; then
    echo "-> makepkg не найден, используем cargo install --path ."
    cd "${SCRIPT_DIR}"
    cargo install --path . --force
    echo ""
    echo "-> Готово! Убедитесь, что ~/.cargo/bin добавлен в PATH, и запустите: zipctl"
else
    echo "Ошибка: не найдены ни makepkg, ни cargo. Установите rust/cargo или base-devel." >&2
    exit 1
fi
