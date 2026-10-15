#!/usr/bin/env bash
# Install md (the md-tui Markdown reader).
#
#   curl -fsSL https://md-tui.over9000.cc/install.sh | bash
#
# Env overrides:
#   MD_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   MD_VERSION      release tag such as v0.1.0 (default: latest)
#
# Supports macOS (arm64, x86_64), Linux (x86_64, aarch64), and WSL. If no
# prebuilt binary fits, it falls back to `cargo install` from the repo.
set -euo pipefail

REPO="over9000-apps/md-tui"
BIN="md"
INSTALL_DIR="${MD_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${MD_VERSION:-latest}"

if [ -t 1 ]; then B=$'\033[1m'; R=$'\033[31m'; G=$'\033[32m'; Y=$'\033[33m'; N=$'\033[0m'; else B=; R=; G=; Y=; N=; fi
info() { printf '%s\n' "${B}==>${N} $*"; }
warn() { printf '%s\n' "${Y}warning:${N} $*" >&2; }
die()  { printf '%s\n' "${R}error:${N} $*" >&2; exit 1; }

have() { command -v "$1" >/dev/null 2>&1; }

fetch() { # fetch URL DEST
  if have curl; then curl -fsSL --retry 3 -o "$2" "$1"
  elif have wget; then wget -qO "$2" "$1"
  else die "need curl or wget"; fi
}

detect_target() {
  local os arch
  case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=macos ;;
    MINGW*|MSYS*|CYGWIN*) die "native Windows is not supported; run this inside WSL" ;;
    *) die "unsupported OS: $(uname -s)" ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) die "unsupported architecture: $(uname -m)" ;;
  esac
  # Rosetta: an x86_64 shell on Apple Silicon should still get the native build.
  if [ "$os" = macos ] && [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
    arch=aarch64
  fi
  if [ "$os" = linux ] && grep -qi microsoft /proc/version 2>/dev/null; then
    info "WSL detected; installing the Linux build" >&2
  fi
  printf '%s-%s' "$os" "$arch"
}

sha256() {
  if have sha256sum; then sha256sum "$1" | awk '{print $1}'
  elif have shasum; then shasum -a 256 "$1" | awk '{print $1}'
  else echo ""; fi
}

install_prebuilt() { # target tmpdir -> 0 on success
  local target="$1" tmp="$2" asset base
  asset="md-${target}"
  if [ "$VERSION" = latest ]; then base="https://github.com/${REPO}/releases/latest/download"
  else base="https://github.com/${REPO}/releases/download/${VERSION}"; fi

  info "Downloading ${asset} (${VERSION})"
  fetch "${base}/${asset}" "${tmp}/${BIN}" 2>/dev/null || return 1

  if fetch "${base}/SHA256SUMS" "${tmp}/SHA256SUMS" 2>/dev/null; then
    local want got
    want="$(awk -v a="$asset" '$2==a || $2=="*"a {print $1}' "${tmp}/SHA256SUMS")"
    got="$(sha256 "${tmp}/${BIN}")"
    if [ -n "$want" ] && [ -n "$got" ]; then
      [ "$want" = "$got" ] || die "checksum mismatch for ${asset}"
      info "Checksum OK"
    fi
  else
    warn "no SHA256SUMS in the release; skipping checksum check"
  fi

  chmod +x "${tmp}/${BIN}"
  mkdir -p "$INSTALL_DIR"
  mv -f "${tmp}/${BIN}" "${INSTALL_DIR}/${BIN}"
  if [ "$(uname -s)" = Darwin ]; then
    xattr -d com.apple.quarantine "${INSTALL_DIR}/${BIN}" 2>/dev/null || true
  fi
}

install_cargo() {
  have cargo || return 1
  info "No prebuilt binary; building from source with cargo (this takes a minute)"
  local root
  root="$(dirname "$INSTALL_DIR")"
  cargo install --locked --git "https://github.com/${REPO}" --root "$root" md
  [ -x "${INSTALL_DIR}/${BIN}" ] || { mkdir -p "$INSTALL_DIR"; cp "${root}/bin/${BIN}" "${INSTALL_DIR}/${BIN}"; }
}

path_hint() {
  case ":${PATH}:" in *":${INSTALL_DIR}:"*) return ;; esac
  local rc shell_name
  shell_name="$(basename "${SHELL:-bash}")"
  case "$shell_name" in
    zsh)  rc="~/.zshrc" ;;
    fish) rc="~/.config/fish/config.fish" ;;
    *)    rc="~/.bashrc" ;;
  esac
  echo
  warn "${INSTALL_DIR} is not on your PATH."
  if [ "$shell_name" = fish ]; then
    echo "  Add it with:  fish_add_path ${INSTALL_DIR}"
  else
    echo "  Add it with:  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ${rc}"
    echo "  Then restart your shell or run:  export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

main() {
  local target tmp
  target="$(detect_target)"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  if ! install_prebuilt "$target" "$tmp"; then
    warn "no prebuilt binary for ${target} (${VERSION})"
    install_cargo || die "install Rust (https://rustup.rs) and re-run, or build from https://github.com/${REPO}"
  fi

  echo
  printf '%s\n' "${G}Installed${N} ${INSTALL_DIR}/${BIN} ($("${INSTALL_DIR}/${BIN}" --version 2>/dev/null || echo "$BIN"))"
  path_hint
  echo
  echo "  Try:  md README.md"
}

main "$@"
