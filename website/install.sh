#!/bin/sh
# One-command verified installer for YouTFast on Linux (x86_64).
# Usage:
#   curl -fsSL https://youtfast.pages.dev/install.sh | sh
#   curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak
#   curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --uninstall

set -eu

REPO="bcosta19/youtfast"
APP_ID="io.github.bcosta19.YouTFast"
FLATHUB_REPO_URL="https://dl.flathub.org/repo/flathub.flatpakrepo"

METHOD="auto"
VERSION="${YOUTFAST_VERSION:-}"
UNINSTALL=0
ASSUME_YES=0

usage() {
  cat <<'EOF'
YouTFast Linux installer

Usage:
  install.sh [options]

Options:
  --method <auto|flatpak|arch|native>
      Choose installation method (default: auto).
      auto uses pacman on Arch-based distributions, otherwise Flatpak.
  --version <X.Y.Z>
      Install a specific release version instead of latest.
  --uninstall
      Remove an existing Flatpak or user-local native installation.
  -y, --yes
      Proceed without interactive confirmation prompts.
  -h, --help
      Show this help message.
EOF
}

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Required tool '$1' was not found on PATH."
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --method)
      [ "$#" -ge 2 ] || fail "--method requires a value."
      METHOD="$2"
      shift 2
      ;;
    --method=*)
      METHOD="${1#*=}"
      shift
      ;;
    --version)
      [ "$#" -ge 2 ] || fail "--version requires a value."
      VERSION="${2#v}"
      shift 2
      ;;
    --version=*)
      VERSION="${1#*=}"
      VERSION="${VERSION#v}"
      shift
      ;;
    --uninstall)
      UNINSTALL=1
      shift
      ;;
    -y|--yes)
      ASSUME_YES=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "Unknown argument: $1 (try --help)"
      ;;
  esac
done

case "$METHOD" in
  auto|flatpak|arch|native) ;;
  *) fail "Invalid --method '$METHOD' (expected auto, flatpak, arch, or native)." ;;
esac

if [ -n "$VERSION" ]; then
  printf '%s' "$VERSION" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || \
    fail "Invalid version '$VERSION' (expected MAJOR.MINOR.PATCH)."
fi

OS="$(uname -s)"
ARCH="$(uname -m)"
[ "$OS" = "Linux" ] || fail "This script supports Linux only (detected $OS). See https://youtfast.pages.dev/install.html"
[ "$ARCH" = "x86_64" ] || fail "Prebuilt Linux packages currently target x86_64 only (detected $ARCH)."

run_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    fail "Root privileges are required to run '$*'; please run as root or install sudo."
  fi
}

fetch_text() {
  url="$1"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --proto '=https' --tlsv1.2 -H "Accept: application/vnd.github+json" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- --https-only --secure-protocol=TLSv1_2 "$url"
  else
    fail "Neither curl nor wget is installed."
  fi
}

download_file() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --progress-bar --proto '=https' --tlsv1.2 -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget --https-only --secure-protocol=TLSv1_2 -O "$dest" "$url"
  else
    fail "Neither curl nor wget is installed."
  fi
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print tolower($1)}'
  else
    fail "Neither sha256sum nor shasum is installed."
  fi
}

verify_checksum() {
  checksums_file="$1"
  asset_name="$2"
  asset_path="$3"

  expected="$(awk -v target="$asset_name" '
    $1 ~ /^[0-9a-fA-F]{64}$/ {
      file = $2
      sub(/^\*/, "", file)
      if (file == target) {
        print tolower($1)
        exit
      }
    }
  ' "$checksums_file")"

  [ -n "$expected" ] || fail "Checksum entry for '$asset_name' was not found in checksums.txt."
  actual="$(sha256_file "$asset_path")"
  [ "$actual" = "$expected" ] || fail "SHA-256 verification failed for $asset_name (expected $expected, got $actual)."
  say "Verified SHA-256 checksum for $asset_name."
}

is_arch_linux() {
  command -v pacman >/dev/null 2>&1 || return 1
  if [ -r /etc/os-release ] && grep -qi '^ID=steamos' /etc/os-release; then
    return 1
  fi
  [ -f /etc/arch-release ] || { [ -r /etc/os-release ] && grep -qiE '^ID(_LIKE)?=.*arch' /etc/os-release; }
}

confirm_prompt() {
  prompt="$1"
  if [ "$ASSUME_YES" -eq 1 ]; then
    return 0
  fi
  if [ -t 0 ]; then
    printf '%s [Y/n] ' "$prompt"
    IFS= read -r reply || return 1
  elif [ -r /dev/tty ]; then
    printf '%s [Y/n] ' "$prompt" >/dev/tty
    IFS= read -r reply </dev/tty || return 1
  else
    return 1
  fi
  case "$reply" in
    ''|[Yy]|[Yy][Ee][Ss]|[Ss]|[Ss][Ii][Mm]) return 0 ;;
    *) return 1 ;;
  esac
}

ensure_flatpak() {
  if command -v flatpak >/dev/null 2>&1; then
    return 0
  fi
  if command -v apt-get >/dev/null 2>&1; then
    confirm_prompt "Flatpak is not installed. Install flatpak via apt now?" || \
      fail "Install flatpak first (sudo apt update && sudo apt install flatpak) or pass --method native."
    run_root apt-get update
    run_root apt-get install -y flatpak
  elif command -v dnf >/dev/null 2>&1; then
    confirm_prompt "Flatpak is not installed. Install flatpak via dnf now?" || \
      fail "Install flatpak first (sudo dnf install flatpak) or pass --method native."
    run_root dnf install -y flatpak
  elif command -v zypper >/dev/null 2>&1; then
    confirm_prompt "Flatpak is not installed. Install flatpak via zypper now?" || \
      fail "Install flatpak first (sudo zypper install flatpak) or pass --method native."
    run_root zypper install -y flatpak
  else
    fail "flatpak is not installed. Install Flatpak using your distribution's package manager or pass --method native."
  fi
}

do_uninstall() {
  removed=0
  if command -v flatpak >/dev/null 2>&1 && flatpak info --user "$APP_ID" >/dev/null 2>&1; then
    say "Removing Flatpak user installation ($APP_ID)..."
    flatpak uninstall --user -y "$APP_ID"
    removed=1
  fi

  local_bin="${HOME}/.local/bin/youtfast"
  local_desktop="${HOME}/.local/share/applications/${APP_ID}.desktop"
  local_icon="${HOME}/.local/share/icons/hicolor/scalable/apps/youtfast.svg"
  if [ -e "$local_bin" ] || [ -e "$local_desktop" ] || [ -e "$local_icon" ]; then
    say "Removing user-local native installation..."
    rm -f "$local_bin" "$local_desktop" "$local_icon"
    removed=1
  fi

  if command -v pacman >/dev/null 2>&1 && pacman -Qi youtfast >/dev/null 2>&1; then
    if [ "$ASSUME_YES" -eq 1 ] || confirm_prompt "Remove Arch package 'youtfast' via pacman?"; then
      if [ "$ASSUME_YES" -eq 1 ]; then
        run_root pacman -R --noconfirm youtfast
      else
        run_root pacman -R youtfast
      fi
      removed=1
    else
      say "To remove the Arch package manually, run: sudo pacman -R youtfast"
    fi
  fi

  if [ "$removed" -eq 0 ]; then
    say "No YouTFast installation was removed."
  else
    say "YouTFast uninstall complete."
  fi
}

if [ "$UNINSTALL" -eq 1 ]; then
  do_uninstall
  exit 0
fi

if [ "$METHOD" = "auto" ]; then
  if is_arch_linux; then
    METHOD="arch"
  else
    METHOD="flatpak"
  fi
fi

if [ -z "$VERSION" ]; then
  say "Checking latest YouTFast release..."
  release_json="$(fetch_text "https://api.github.com/repos/${REPO}/releases/latest")"
  tag="$(printf '%s' "$release_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  VERSION="${tag#v}"
  printf '%s' "$VERSION" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || \
    fail "Could not resolve a valid release version from GitHub Releases."
fi

BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/youtfast-install.XXXXXX")"
trap 'rm -rf "$WORKDIR"' EXIT INT TERM

say "Installing YouTFast ${VERSION} using method '${METHOD}'..."
download_file "${BASE_URL}/checksums.txt" "${WORKDIR}/checksums.txt"

case "$METHOD" in
  arch)
    need_cmd pacman
    ASSET="youtfast-${VERSION}-1-x86_64.pkg.tar.zst"
    download_file "${BASE_URL}/${ASSET}" "${WORKDIR}/${ASSET}"
    verify_checksum "${WORKDIR}/checksums.txt" "$ASSET" "${WORKDIR}/${ASSET}"
    if [ "$ASSUME_YES" -eq 1 ]; then
      run_root pacman -U --noconfirm "${WORKDIR}/${ASSET}"
    else
      run_root pacman -U "${WORKDIR}/${ASSET}"
    fi
    say "Done. Run 'youtfast' or launch YouTFast from your application menu."
    ;;

  flatpak)
    ensure_flatpak
    ASSET="youtfast-v${VERSION}-x86_64.flatpak"
    download_file "${BASE_URL}/${ASSET}" "${WORKDIR}/${ASSET}"
    verify_checksum "${WORKDIR}/checksums.txt" "$ASSET" "${WORKDIR}/${ASSET}"
    # The app itself installs from the verified GitHub bundle; flathub provides only the shared Freedesktop runtime.
    flatpak remote-add --user --if-not-exists flathub "$FLATHUB_REPO_URL"
    flatpak install --user -y "${WORKDIR}/${ASSET}"
    say "Done. Run 'flatpak run ${APP_ID}' or launch YouTFast from your application menu."
    ;;

  native)
    need_cmd tar
    ASSET="youtfast-v${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
    download_file "${BASE_URL}/${ASSET}" "${WORKDIR}/${ASSET}"
    verify_checksum "${WORKDIR}/checksums.txt" "$ASSET" "${WORKDIR}/${ASSET}"
    tar -xzf "${WORKDIR}/${ASSET}" -C "$WORKDIR"
    bundle_dir="${WORKDIR}/youtfast-v${VERSION}-x86_64-unknown-linux-gnu"
    [ -f "${bundle_dir}/youtfast" ] || fail "Extracted archive is missing youtfast binary."

    install -d "${HOME}/.local/bin" \
      "${HOME}/.local/share/applications" \
      "${HOME}/.local/share/icons/hicolor/scalable/apps"
    install -m 755 "${bundle_dir}/youtfast" "${HOME}/.local/bin/youtfast"
    install -m 644 "${bundle_dir}/packaging/icons/youtfast.svg" \
      "${HOME}/.local/share/icons/hicolor/scalable/apps/youtfast.svg"
    sed "s|^Exec=youtfast|Exec=${HOME}/.local/bin/youtfast|" \
      "${bundle_dir}/packaging/applications/youtfast.desktop" \
      > "${HOME}/.local/share/applications/${APP_ID}.desktop"

    if command -v update-desktop-database >/dev/null 2>&1; then
      update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
    fi

    say "Installed binary to ${HOME}/.local/bin/youtfast"
    case ":${PATH}:" in
      *":${HOME}/.local/bin:"*) ;;
      *) say "Note: add ${HOME}/.local/bin to your PATH to invoke 'youtfast' from a terminal." ;;
    esac
    say "Runtime requirement: system libmpv 0.42+ (with Lua), yt-dlp, and Deno must be installed."
    say "If video playback fails on older distros, use: curl -fsSL https://youtfast.pages.dev/install.sh | sh -s -- --method flatpak"
    ;;
esac
