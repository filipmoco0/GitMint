import os

application = defines["app"]
background = defines["background"]
icon = defines["volume_icon"]

format = "UDZO"
filesystem = "HFS+"
files = [application]
symlinks = {"Applications": "/Applications"}

icon_size = 128
text_size = 12
default_view = "icon-view"
background = background
icon = icon
window_rect = ((10, 60), (760, 500))
show_toolbar = False
show_status_bar = False
show_sidebar = False
icon_locations = {
    os.path.basename(application): (190, 300),
    "Applications": (570, 300),
}
