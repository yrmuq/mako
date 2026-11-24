// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

const GAP = 8;
const tracked = new Map();
let lastAnchor = null;

function isPopup(window) {
    const classes = [window.resourceClass, window.resourceName].map(String);
    return !window.dialog && (classes.includes("mako-popup")
        || (classes.includes("mako") && String(window.captionNormal || window.caption) === "mako-popup"));
}

function contains(rect, point) {
    return point.x >= rect.x && point.x < rect.x + rect.width
        && point.y >= rect.y && point.y < rect.y + rect.height;
}

function edgeOf(panel) {
    const rect = panel.frameGeometry;
    const screen = panel.output.geometry;
    if (rect.width >= rect.height) {
        return rect.y + rect.height / 2 < screen.y + screen.height / 2 ? "top" : "bottom";
    }
    return rect.x + rect.width / 2 < screen.x + screen.width / 2 ? "left" : "right";
}

function anchorFor(cursor, output) {
    const panels = workspace.windowList().filter((window) =>
        window.dock && String(window.resourceClass) === "plasmashell" && window.output
        && window.frameGeometry.width > 0 && window.frameGeometry.height > 0
    );
    const clicked = panels.find((panel) => contains(panel.frameGeometry, cursor));
    if (clicked) {
        lastAnchor = { panel: clicked, point: cursor };
        return lastAnchor;
    }
    if (lastAnchor && panels.includes(lastAnchor.panel) && lastAnchor.panel.output === output
            && contains(lastAnchor.panel.frameGeometry, lastAnchor.point)) {
        return lastAnchor;
    }
    const panel = panels.find((candidate) => candidate.output === output);
    if (!panel) { return null; }
    const rect = panel.frameGeometry;
    return { panel, point: { x: rect.x + rect.width - GAP, y: rect.y + rect.height - GAP } };
}

function clamp(value, low, high) {
    return Math.max(low, Math.min(value, Math.max(low, high)));
}

function place(entry) {
    const { window, anchor } = entry;
    if (entry.moving || window.deleted || window.hidden || window.minimized) { return; }
    const { x: oldX, y: oldY, width, height } = window.frameGeometry;
    if (width <= 0 || height <= 0) { return; }
    const output = anchor ? anchor.panel.output : entry.output || window.output;
    if (!output) { return; }
    const area = workspace.clientArea(KWin.MaximizeArea, output, workspace.currentDesktop);
    let x = entry.cursor.x - width / 2;
    let y = entry.cursor.y - height - GAP;
    if (anchor) {
        const rect = anchor.panel.frameGeometry;
        const edge = edgeOf(anchor.panel);
        if (edge === "top" || edge === "bottom") {
            x = anchor.point.x - width / 2;
            y = edge === "top" ? rect.y + rect.height + GAP : rect.y - height - GAP;
        } else {
            x = edge === "left" ? rect.x + rect.width + GAP : rect.x - width - GAP;
            y = anchor.point.y - height / 2;
        }
    }
    const ratio = Number(output.devicePixelRatio) || 1;
    x = Math.round(clamp(x, area.x + GAP, area.x + area.width - width - GAP) * ratio) / ratio;
    y = Math.round(clamp(y, area.y + GAP, area.y + area.height - height - GAP) * ratio) / ratio;
    if (Math.abs(x - oldX) < 0.5 && Math.abs(y - oldY) < 0.5) { return; }
    entry.moving = true;
    try {
        window.frameGeometry = { x, y, width, height };
    } finally {
        entry.moving = false;
    }
}

function watch(window) {
    const initial = workspace.cursorPos;
    const cursor = { x: initial.x, y: initial.y };
    const identify = () => {
        if (!isPopup(window) || tracked.has(window)) { return; }
        const output = workspace.screenAt(cursor) || window.output;
        const entry = { window, cursor, output, anchor: anchorFor(cursor, output), moving: false };
        tracked.set(window, entry);
        const shown = () => {
            if (window.hidden || window.deleted || window.minimized) { return; }
            window.skipTaskbar = true;
            window.skipSwitcher = true;
            window.skipPager = true;
            window.noBorder = true;
            window.keepAbove = true;
            place(entry);
            if (workspace.activeWindow !== window) { workspace.activeWindow = window; }
        };
        const visibilityChanged = window.windowShown || window.hiddenChanged;
        if (visibilityChanged) { visibilityChanged.connect(shown); }
        window.frameGeometryChanged.connect(() => place(entry));
        window.closed.connect(() => tracked.delete(window));
        shown();
    };
    identify();
    if (window.captionChanged) { window.captionChanged.connect(identify); }
    if (window.captionNormalChanged) { window.captionNormalChanged.connect(identify); }
    if (window.windowClassChanged) { window.windowClassChanged.connect(identify); }
}

let lastFullscreenOutputs;

function reportFullscreen() {
    const active = new Map();
    for (const window of workspace.windowList()) {
        if (!window.fullScreen || window.minimized || window.hidden || !window.output) { continue; }
        const output = window.output;
        const desktop = workspace.currentDesktopForScreen
            ? workspace.currentDesktopForScreen(output) : workspace.currentDesktop;
        if (window.desktops && window.desktops.length && !Array.from(window.desktops).includes(desktop)) { continue; }
        if (window.activities && window.activities.length && !Array.from(window.activities).includes(workspace.currentActivity)) { continue; }
        if (window.hiddenByShowDesktop) { continue; }
        const name = String(output.name || "");
        const { x, y } = output.geometry;
        active.set(name || x + "," + y, { name, x, y });
    }
    const activeJson = JSON.stringify(Array.from(active.keys()).sort().map((key) => active.get(key)));
    if (activeJson !== lastFullscreenOutputs) {
        lastFullscreenOutputs = activeJson;
        callDBus(makoReadyBus, "/mako/Placer", "io.github.yrmuq.mako.Placer", "Fullscreen", activeJson);
    }
}

function watchWindow(window) {
    watch(window);
    window.fullScreenChanged.connect(reportFullscreen);
    window.outputChanged.connect(reportFullscreen);
    window.minimizedChanged.connect(reportFullscreen);
    window.desktopsChanged.connect(reportFullscreen);
    window.activitiesChanged.connect(reportFullscreen);
    window.hiddenChanged.connect(reportFullscreen);
    if (window.hiddenByShowDesktopChanged) { window.hiddenByShowDesktopChanged.connect(reportFullscreen); }
}

workspace.windowAdded.connect((window) => {
    watchWindow(window);
    reportFullscreen();
});
workspace.windowRemoved.connect(reportFullscreen);
workspace.currentDesktopChanged.connect(reportFullscreen);
if (workspace.currentActivityChanged) { workspace.currentActivityChanged.connect(reportFullscreen); }
workspace.windowList().forEach(watchWindow);
reportFullscreen();
callDBus(makoReadyBus, "/mako/Placer", "io.github.yrmuq.mako.Placer", "Ready");
