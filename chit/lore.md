# Lore

## Required Behavior

- On a Wayland session the process must not open an X11 display. Falling back to `XWayland` hides a broken native backend.
- On an X11 session the process must not connect to a Wayland socket.
- A chibi zip without `body` uses bear girl's skeleton, and one without `anima` uses bear girl's animations. A pack may replace only the drawings.
- A chibi zip without a `heart` name is ignored. The menu cannot list a pack that has no name.
- Bear girl walks and sits on the upper edge of the bottom taskbar. Use the same floor for both poses.
- Bear girl uses the same hand and leg artwork in every pose.
- General scales both axes together. Dev proportions scale each axis before in-air rotation.
- A quick-list entry is independent of active clones. Minus removes one clone.
- Dragging pins the clicked point on the topmost visible body part. Gravity moves the rest of the pose around the pin, and the floor stops the body at contact.

## External Compatibility Constraints

- KWin Wayland placement uses `zwlr_layer_shell_v1`. A normal toplevel is stacked and focused like an application window, so the pet cannot use one.
- Keep the AppImage font libraries on the host so Fontconfig reads configuration files from the same distribution and version.
- The dev AppImage must not embed an update source. Update metadata belongs only on a stable release.
- The dev draft stays unpublished, so GitHub's get-release-by-tag endpoint does not return it. Update that release by listing releases and using its id.
- KWin drops the Wayland client if a layer-shell buffer does not match the size from the latest configure and the output scale. Size the buffer from that pair.
- KWin drops the Wayland client if the layer surface moves while a pointer button is held. Keep one anchored surface, move the drawing inside it, and limit pointer input to the pet.

## Compatibility Workarounds

- The tray panel uses the FemtoVG renderer because the software renderer's window buffer does not preserve alpha at rounded corners.
- KWin can announce a tray popup before its Wayland class or title arrives. Subscribe to class and caption changes as well as the initial window event.
