// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

function isMakoPopup(window) {
    return window.resourceClass === "mako-popup" || window.caption === "mako-popup";
}

function clamp(value, low, high) {
    return Math.max(low, Math.min(value, Math.max(low, high)));
}

function position(window, pointer) {
    const output = workspace.screenAt(pointer);
    const area = workspace.clientArea(KWin.MaximizeArea, output, workspace.currentDesktop);
    const rectangle = window.frameGeometry;
    const padding = 4;
    let x = pointer.x - rectangle.width / 2;
    let y = pointer.y < area.y + area.height / 2
        ? pointer.y + padding : pointer.y - rectangle.height - padding;
    x = clamp(x, area.x + padding, area.x + area.width - rectangle.width - padding);
    y = clamp(y, area.y + padding, area.y + area.height - rectangle.height - padding);
    if (rectangle.x !== x || rectangle.y !== y) {
        rectangle.x = x;
        rectangle.y = y;
        window.frameGeometry = rectangle;
    }
}

workspace.windowAdded.connect(function (window) {
    if (!isMakoPopup(window)) {
        return;
    }
    const pointer = workspace.cursorPos;
    window.skipTaskbar = true;
    window.skipPager = true;
    window.keepAbove = true;
    position(window, pointer);
    window.frameGeometryChanged.connect(function () { position(window, pointer); });
    workspace.activeWindow = window;
});
