# Lore

## Required Behavior

- On a Wayland session the process must not open an X11 display. Falling back to `XWayland` hides a broken native backend.
- On an X11 session the process must not connect to a Wayland socket.

## External Compatibility Constraints

- KWin Wayland placement uses `zwlr_layer_shell_v1`. A normal toplevel is stacked and focused like an application window, so the pet cannot use one.
- The dev AppImage must not embed an update source. Update metadata belongs only on a stable release.
- The dev draft stays unpublished, so GitHub's get-release-by-tag endpoint does not return it. Update that release by listing releases and using its id.
