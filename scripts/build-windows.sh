#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="x86_64-pc-windows-gnu"
bundle="$root/target/mreader-windows-x64"
runtime="$bundle/runtime"

cd "$root"

if [[ "${MSYSTEM:-}" != "MINGW64" ]]; then
    echo "Run this script from the MSYS2 MINGW64 shell." >&2
    exit 1
fi
if ! command -v ldd >/dev/null; then
    echo "The MSYS2 ldd command is required to collect runtime DLLs." >&2
    exit 1
fi

export PKG_CONFIG_PATH="/mingw64/lib/pkgconfig:/mingw64/share/pkgconfig"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"

cargo build --release --target "$target"
rm -rf "$bundle"
mkdir -p "$runtime/share/glib-2.0" "$runtime/share/icons" "$runtime/lib"
cp "target/$target/release/mreader.exe" "$bundle/"
cp assets/windows/mreader.cmd "$bundle/"
if [[ -x /mingw64/bin/gdk-pixbuf-query-loaders.exe ]]; then
    cp /mingw64/bin/gdk-pixbuf-query-loaders.exe "$bundle/"
fi

for resource in \
    /mingw64/share/glib-2.0/schemas \
    /mingw64/share/icons/Adwaita \
    /mingw64/share/icons/hicolor \
    /mingw64/share/poppler \
    /mingw64/etc/fonts \
    /mingw64/lib/gtk-4.0 \
    /mingw64/lib/gdk-pixbuf-2.0
do
    if [[ -d "$resource" ]]; then
        cp -a "$resource" "$runtime/${resource#/mingw64/}"
    fi
done

declare -A copied=()
pending=("$bundle/mreader.exe")
if [[ -f "$bundle/gdk-pixbuf-query-loaders.exe" ]]; then
    pending+=("$bundle/gdk-pixbuf-query-loaders.exe")
fi
while IFS= read -r -d '' module; do
    pending+=("$module")
done < <(find "$runtime/lib" -type f -iname '*.dll' -print0)

while ((${#pending[@]})); do
    current="${pending[0]}"
    pending=("${pending[@]:1}")
    dependencies="$(ldd "$current")" || {
        echo "Could not inspect runtime dependencies of $current" >&2
        exit 1
    }
    if grep -q "not found" <<< "$dependencies"; then
        echo "Unresolved runtime dependency for $current:" >&2
        grep "not found" <<< "$dependencies" >&2
        exit 1
    fi
    while IFS= read -r dependency; do
        [[ -f "$dependency" ]] || continue
        name="$(basename "$dependency")"
        [[ -n "${copied[$name]:-}" ]] && continue
        copied["$name"]=1
        cp "$dependency" "$bundle/$name"
        pending+=("$dependency")
    done < <(sed -nE 's/.*=>[[:space:]]+(\/mingw64\/[^[:space:]]+\.dll).*/\1/p' <<< "$dependencies")
done

archive="$root/target/mreader-windows-x64.zip"
rm -f "$archive"
(
    cd "$root/target"
    zip -qr "$archive" "mreader-windows-x64"
)
printf 'Created %s\n' "$archive"
