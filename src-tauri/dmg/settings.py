# dmgbuild settings for the macOS installer. dmgbuild writes the Finder layout directly
# (no Finder scripting), so the background and icon positions come out the same locally and in CI.
# Run from the repository root:
#   uvx --from dmgbuild==1.6.5 dmgbuild -s src-tauri/dmg/settings.py -D app=path/to/Wanderdex.app Wanderdex out.dmg
import os.path

app = defines["app"]  # noqa: F821 (provided by dmgbuild)

format = "UDZO"
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = os.path.join(app, "Contents/Resources/icon.icns")
hide_extension = [os.path.basename(app)]

background = "src-tauri/dmg/background.png"  # background@2x.png is picked up for Retina
window_rect = ((200, 120), (660, 400))
default_view = "icon-view"
show_status_bar = show_tab_view = show_toolbar = show_pathbar = show_sidebar = False
icon_size = 128
text_size = 13
icon_locations = {os.path.basename(app): (170, 170), "Applications": (490, 170)}
