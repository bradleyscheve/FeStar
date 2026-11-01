#!/bin/sh
# Installs FeStar for the current user: the program goes in ~/.local/bin and
# a menu entry with its icon is added. Run it from the unpacked folder:
#   ./install.sh
set -e
here="$(cd "$(dirname "$0")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
install -Dm755 "$here/festar" "$HOME/.local/bin/festar"
install -Dm644 "$here/festar.png" "$data/icons/hicolor/256x256/apps/festar.png"
mkdir -p "$data/applications"
# Point the menu entry at the installed program, since menus don't always
# search ~/.local/bin.
sed "s|^Exec=festar|Exec=$HOME/.local/bin/festar|" "$here/festar.desktop" \
  > "$data/applications/festar.desktop"
update-desktop-database "$data/applications" 2>/dev/null || true
gtk-update-icon-cache -q "$data/icons/hicolor" 2>/dev/null || true
echo "FeStar is installed. Find it in your applications menu, or run: festar"
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) echo "Note: $HOME/.local/bin is not on your PATH, so add it to run festar by name." ;;
esac
