#!/usr/bin/env bash
# Compila e instala o UnderCoverNc para o usuário atual (~/.local), no Arch ou no Ubuntu/Debian.
# Uso: ./install.sh              instala (pergunta antes de instalar dependências)
#      ./install.sh -y           instala sem perguntar
#      ./install.sh --uninstall  remove
set -euo pipefail
cd "$(dirname "$0")"
PREFIX="${PREFIX:-$HOME/.local}"
YES=0
[[ "${1:-}" == "-y" ]] && YES=1

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$PREFIX/bin/undercovernc" "$PREFIX/share/applications/undercovernc.desktop" "$PREFIX/share/icons/hicolor/scalable/apps/undercovernc.svg"
  echo "UnderCoverNc removido. (Suas configurações ficam em ~/.config/undercovernc.)"
  exit 0
fi

ask() {
  [[ $YES == 1 ]] && return 0
  read -r -p "$1 [S/n] " r
  [[ -z "$r" || "$r" =~ ^[sSyY] ]]
}
SUDO=""
[[ $EUID -ne 0 ]] && SUDO="sudo"

# Pacote com binário pronto (release .tar.gz): só copia, sem compilar.
PREBUILT=0
[[ -x ./undercovernc && ! -f Cargo.toml ]] && PREBUILT=1

# Bibliotecas que o app usa (janela, OpenGL, teclado, seletor de arquivos e impressão).
if command -v pacman >/dev/null; then
  PKGS=(libxkbcommon wayland libglvnd xdg-desktop-portal cups)
  [[ $PREBUILT == 0 ]] && PKGS+=(base-devel)
  MISSING=()
  for p in "${PKGS[@]}"; do pacman -Qq "$p" &>/dev/null || pacman -Qqg "$p" &>/dev/null || MISSING+=("$p"); done
  if (( ${#MISSING[@]} )) && ask "Instalar dependências do sistema (${MISSING[*]})?"; then
    $SUDO pacman -S --needed --noconfirm "${MISSING[@]}"
  fi
elif command -v apt-get >/dev/null; then
  PKGS=(libxkbcommon0 libwayland-client0 libgl1 libegl1 xdg-desktop-portal cups-client)
  [[ $PREBUILT == 0 ]] && PKGS+=(build-essential curl ca-certificates pkg-config)
  MISSING=()
  for p in "${PKGS[@]}"; do dpkg -s "$p" &>/dev/null || MISSING+=("$p"); done
  if (( ${#MISSING[@]} )) && ask "Instalar dependências do sistema (${MISSING[*]})?"; then
    $SUDO apt-get update
    $SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y "${MISSING[@]}"
  fi
else
  echo "Distribuição não reconhecida: instale Rust, libxkbcommon, wayland, OpenGL e CUPS manualmente."
fi

if [[ $PREBUILT == 1 ]]; then
  BIN=./undercovernc
else
# O projeto precisa de Rust 1.88+ (edição 2024). O rustc do apt do Ubuntu é antigo, então usa o rustup.
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
rust_ok() {
  command -v cargo >/dev/null || return 1
  local v; v=$(rustc --version | awk '{print $2}')
  [[ "$(printf '%s\n1.88.0\n' "$v" | sort -V | head -1)" == "1.88.0" ]]
}
if ! rust_ok; then
  if command -v rustup >/dev/null; then
    rustup update stable
  elif command -v pacman >/dev/null && ! command -v cargo >/dev/null && ask "Instalar Rust (pacman -S rust)?"; then
    $SUDO pacman -S --needed --noconfirm rust
  elif ask "Instalar Rust (rustup, em ~/.cargo)?"; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    source "$HOME/.cargo/env"
  fi
  rust_ok || { echo "Preciso do Rust 1.88 ou mais novo (rustc --version)."; exit 1; }
fi

cargo build --release --locked
BIN=target/release/undercovernc
fi

install -Dm755 "$BIN" "$PREFIX/bin/undercovernc"
# Caminho completo no atalho: alguns menus (wofi/rofi no Hyprland, por exemplo) não têm
# ~/.local/bin no PATH e escondem atalhos cujo programa não acham.
install -dm755 "$PREFIX/share/applications"
sed "s|^Exec=undercovernc|Exec=$PREFIX/bin/undercovernc|" packaging/undercovernc.desktop > "$PREFIX/share/applications/undercovernc.desktop"
install -Dm644 packaging/undercovernc.svg "$PREFIX/share/icons/hicolor/scalable/apps/undercovernc.svg"
update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
gtk-update-icon-cache -q "$PREFIX/share/icons/hicolor" 2>/dev/null || true
echo "Instalado em $PREFIX/bin/undercovernc — procure \"UnderCoverNc\" no menu de aplicativos."
command -v lp >/dev/null || echo "Aviso: CUPS (lp) não encontrado — dá pra exportar PDF, mas não imprimir direto."
case ":$PATH:" in *":$PREFIX/bin:"*) ;; *) echo "Aviso: $PREFIX/bin não está no PATH." ;; esac
