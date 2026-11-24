// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::pack;

pub const SNOWMAN_SECONDS: f32 = 600.0;
const SAVE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct WinterObjects {
    year: Option<i32>,
    pub piles: Vec<Pile>,
    pub snowman: Option<Snowman>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Pile {
    pub position: f32,
    pub servings: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Snowman {
    pub position: f32,
    pub seconds: f32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Save {
    version: u32,
    chibis: Vec<String>,
    winter: WinterObjects,
}

impl Save {
    fn new(chibi: &str) -> Self {
        Self {
            version: 1,
            chibis: vec![chibi.to_owned()],
            winter: WinterObjects::default(),
        }
    }

    fn read(path: &Path) -> io::Result<Self> {
        let mut saved: Self = serde_json::from_reader(fs::File::open(path)?)?;
        if saved.version != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown scene version",
            ));
        }
        saved.chibis.retain(|id| pack::safe_id(id));
        saved.winter.piles.retain(|pile| {
            (0.0..=1.0).contains(&pile.position) && (1..=5).contains(&pile.servings)
        });
        saved.winter.snowman = saved.winter.snowman.filter(|man| {
            (0.0..=1.0).contains(&man.position) && (0.0..=SNOWMAN_SECONDS).contains(&man.seconds)
        });
        Ok(saved)
    }

    fn write(&self, path: &Path) -> io::Result<()> {
        let directory = path.parent().expect("scene directory");
        fs::create_dir_all(directory)?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer(&mut temporary, self)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        fs::File::open(directory)?.sync_all()?;
        Ok(())
    }
}

pub struct World {
    _lock: Option<fs::File>,
    path: Option<PathBuf>,
    save: Save,
    save_at: Option<Instant>,
}

impl World {
    pub fn load(default_chibi: &str) -> Self {
        Self::open(scene_path(), default_chibi)
    }

    fn open(path: PathBuf, default_chibi: &str) -> Self {
        let lock = match scene_lock(&path) {
            Ok(lock) => Some(lock),
            Err(error) => {
                eprintln!("MAKO-WORLD-0003: Could not acquire the scene save lock: {error}");
                None
            }
        };
        let (save, path) = match Save::read(&path) {
            Ok(save) => (save, Some(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (Save::new(default_chibi), Some(path))
            }
            Err(error) => {
                eprintln!("MAKO-WORLD-0001: Could not read the saved scene: {error}");
                (Save::new(default_chibi), None)
            }
        };
        Self {
            path: path.filter(|_| lock.is_some()),
            _lock: lock,
            save,
            save_at: None,
        }
    }

    #[cfg(test)]
    pub fn test(default_chibi: &str) -> Self {
        Self {
            _lock: None,
            path: None,
            save: Save::new(default_chibi),
            save_at: None,
        }
    }

    pub fn chibis(&self) -> &[String] {
        &self.save.chibis
    }

    pub fn add(&mut self, id: String) {
        self.save.chibis.push(id);
        self.save_now();
    }

    pub fn remove(&mut self, id: &str) {
        if let Some(index) = self.save.chibis.iter().rposition(|saved| saved == id) {
            self.save.chibis.remove(index);
            self.save_now();
        }
    }

    pub fn update_season(&mut self, year: Option<i32>) -> bool {
        if self.save.winter.year == year {
            return false;
        }
        self.save.winter = WinterObjects {
            year,
            ..WinterObjects::default()
        };
        self.changed();
        true
    }

    pub fn winter(&self) -> &WinterObjects {
        &self.save.winter
    }

    pub fn winter_mut(&mut self) -> &mut WinterObjects {
        &mut self.save.winter
    }

    pub fn changed(&mut self) {
        if self.path.is_some() {
            self.save_at
                .get_or_insert_with(|| Instant::now() + SAVE_INTERVAL);
        }
    }

    pub fn save_now(&mut self) {
        if self.path.is_some() {
            self.save_at = Some(Instant::now());
            self.flush();
        }
    }

    pub fn checkpoint(&mut self) {
        if self
            .save_at
            .is_some_and(|scheduled| Instant::now() >= scheduled)
        {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if let Some(path) = self.path.as_ref().filter(|_| self.save_at.is_some()) {
            match self.save.write(path) {
                Ok(()) => self.save_at = None,
                Err(error) => {
                    eprintln!("MAKO-WORLD-0002: Could not save the scene: {error}");
                    self.save_at = Some(Instant::now() + Duration::from_secs(10));
                }
            }
        }
    }
}

fn scene_lock(path: &Path) -> io::Result<fs::File> {
    let directory = path.parent().expect("scene directory");
    fs::create_dir_all(directory)?;
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("lock"))?;
    // Keep the separate lock open across atomic replacements of the scene file.
    let status = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if status == 0 {
        Ok(lock)
    } else {
        Err(io::Error::last_os_error())
    }
}

impl Drop for World {
    fn drop(&mut self) {
        self.flush();
    }
}

fn scene_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map_or_else(|| PathBuf::from("."), PathBuf::from)
                .join(".local")
                .join("state")
        });
    state.join("mako").join("scene.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_survives_shutdown_and_melts_even_if_a_season_was_missed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scene.json");
        {
            let mut world = World::open(path.clone(), "bear-girl");
            world.add("bear-girl".into());
            world.update_season(Some(2027));
            world.winter_mut().piles.push(Pile {
                position: 0.35,
                servings: 2,
            });
            world.winter_mut().snowman = Some(Snowman {
                position: 0.78,
                seconds: 124.0,
            });
        }
        {
            let mut world = World::open(path.clone(), "bear-girl");
            assert_eq!(world.chibis(), ["bear-girl", "bear-girl"]);
            assert!(!world.update_season(Some(2027)));
            assert_eq!(world.winter().piles[0].servings, 2);
            assert_eq!(world.winter().piles[0].position, 0.35);
            let saved = world.winter().snowman.unwrap();
            assert_eq!((saved.position, saved.seconds), (0.78, 124.0));
            assert!(world.update_season(Some(2028)));
            world.remove("bear-girl");
            world.remove("bear-girl");
        }
        let world = World::open(path, "bear-girl");
        assert!(world.chibis().is_empty());
        assert!(world.winter().piles.is_empty());
        assert!(world.winter().snowman.is_none());
    }
}
