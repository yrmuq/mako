// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

use crate::world::MAX_SNOW_WALLS;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cargo {
    Fir,
    Bag,
    Snowman(usize),
}

#[derive(Clone, Copy)]
pub enum Support {
    Sled,
    Wall(usize),
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Mounts {
    pub sled: Option<Cargo>,
    pub walls: [Option<Cargo>; MAX_SNOW_WALLS],
}

impl Mounts {
    pub fn is_empty(&self) -> bool {
        self.sled.is_none() && self.walls.iter().all(Option::is_none)
    }

    pub fn get(&self, support: Support) -> Option<Cargo> {
        match support {
            Support::Sled => self.sled,
            Support::Wall(slot) => self.walls[slot],
        }
    }

    pub fn contains(&self, cargo: Cargo) -> bool {
        self.sled == Some(cargo) || self.walls.contains(&Some(cargo))
    }

    pub fn remove(&mut self, cargo: Cargo) {
        if self.sled == Some(cargo) {
            self.sled = None;
        }
        for item in &mut self.walls {
            if *item == Some(cargo) {
                *item = None;
            }
        }
    }

    pub fn attach(&mut self, cargo: Cargo, support: Support) -> bool {
        if self.get(support).is_some_and(|loaded| loaded != cargo) {
            return false;
        }
        self.remove(cargo);
        match support {
            Support::Sled => self.sled = Some(cargo),
            Support::Wall(slot) => self.walls[slot] = Some(cargo),
        }
        true
    }

    pub fn remove_wall(&mut self, slot: usize) -> Option<Cargo> {
        let cargo = self.walls[slot].take();
        self.walls[slot..].rotate_left(1);
        cargo
    }
}
