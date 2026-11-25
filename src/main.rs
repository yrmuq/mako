// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

fn main() {
    if let Err(error) = mako::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
