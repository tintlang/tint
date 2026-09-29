#!/usr/bin/env bash
set -euo pipefail

repo="${TINT_REPO:-tintlang/tint}"
install_dir="${TINT_INSTALL_DIR:-${HOME}/.local/bin}"

case "$(uname -s):$(uname -m)" in
  Darwin:x86_64) target="x86_64-apple-darwin" ;;
  Darwin:arm64|Darwin:aarch64) target="aarch64-apple-darwin" ;;
  Linux:x86_64|Linux:amd64) target="x86_64-unknown-linux-gnu" ;;
  *)
    echo "Unsupported platform: $(uname -s) $(uname -m)" >&2
    echo "Download a supported asset manually from https://github.com/${repo}/releases" >&2
    exit 1
    ;;
esac

command -v curl >/dev/null 2>&1 || {
  echo "curl is required" >&2
  exit 1
}

tag="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" \
  | awk -F'"' '/"tag_name"[[:space:]]*:/ { print $4; exit }')"
if [[ -z "${tag}" ]]; then
  echo "Could not determine the latest Tint release" >&2
  exit 1
fi

archive="tint-${tag}-${target}.tar.gz"
base_url="https://github.com/${repo}/releases/download/${tag}"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

curl -fsSL "${base_url}/${archive}" -o "${tmp_dir}/${archive}"
curl -fsSL "${base_url}/${archive}.sha256" -o "${tmp_dir}/${archive}.sha256"
expected="$(awk '{ print $1; exit }' "${tmp_dir}/${archive}.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${tmp_dir}/${archive}" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "${tmp_dir}/${archive}" | awk '{ print $1 }')"
else
  echo "Neither sha256sum nor shasum is available for checksum verification" >&2
  exit 1
fi
if [[ "${actual}" != "${expected}" ]]; then
  echo "Checksum verification failed" >&2
  exit 1
fi

mkdir -p "${install_dir}"
tar -xzf "${tmp_dir}/${archive}" -C "${tmp_dir}"
install "${tmp_dir}/tint" "${install_dir}/tint"

echo "Installed Tint ${tag} to ${install_dir}/tint"
case ":${PATH}:" in
  *:"${install_dir}":*) ;;
  *) echo "Add ${install_dir} to PATH if it is not already there:"; echo "  export PATH=\"${install_dir}:\$PATH\"" ;;
esac
