// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{
    festive::Tree,
    mounts::{Cargo, Mounts},
    pack,
};

pub const SNOWMAN_SECONDS: f32 = 600.0;
pub const MAX_SNOWMEN: usize = 2;
const SAVE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct WinterObjects {
    year: Option<i32>,
    pub piles: Vec<Pile>,
    pub snowman: Option<Snowman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    second_snowman: Option<Snowman>,
    #[serde(default)]
    pub walls: Vec<SnowWall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sled: Option<Sled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub santa_bag: Option<f32>,
    #[serde(default, rename = "santa_bag_on_sled", skip_serializing)]
    legacy_bag_on_sled: bool,
    #[serde(default, skip_serializing_if = "Mounts::is_empty")]
    pub mounts: Mounts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub santa_gifts: Vec<VisitorGift>,
}

impl WinterObjects {
    pub fn snowman(&self, slot: usize) -> Option<Snowman> {
        [self.snowman, self.second_snowman][slot]
    }

    pub fn snowman_mut(&mut self, slot: usize) -> &mut Option<Snowman> {
        match slot {
            0 => &mut self.snowman,
            1 => &mut self.second_snowman,
            _ => unreachable!("snowman slot {slot}"),
        }
    }

    pub fn snowmen(&self) -> impl Iterator<Item = (usize, Snowman)> + Clone {
        [self.snowman, self.second_snowman]
            .into_iter()
            .enumerate()
            .filter_map(|(slot, man)| man.map(|man| (slot, man)))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct VisitorGift {
    pub position: f32,
    pub box_color: u8,
    pub ribbon_color: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SnowWall {
    pub position: f32,
    pub seconds: f32,
    #[serde(default)]
    pub slope_right: bool,
}

pub const MAX_SNOW_WALLS: usize = 2;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Sled {
    pub position: f32,
    pub rides: u8,
}

impl SnowWall {
    pub fn finished(self) -> bool {
        self.seconds >= SNOWMAN_SECONDS
    }
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
    #[serde(default)]
    tree: Option<Tree>,
}

impl Save {
    fn new(chibi: &str) -> Self {
        Self {
            version: 1,
            chibis: vec![chibi.to_owned()],
            winter: WinterObjects::default(),
            tree: None,
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
        for slot in 0..MAX_SNOWMEN {
            let man = saved.winter.snowman_mut(slot);
            *man = man.filter(|man| {
                (0.0..=1.0).contains(&man.position)
                    && (0.0..=SNOWMAN_SECONDS).contains(&man.seconds)
            });
        }
        saved.winter.walls.retain(|wall| {
            (0.0..=1.0).contains(&wall.position) && (0.0..=SNOWMAN_SECONDS).contains(&wall.seconds)
        });
        saved.winter.walls.truncate(MAX_SNOW_WALLS);
        saved.winter.sled = saved
            .winter
            .sled
            .filter(|sled| (0.0..=1.0).contains(&sled.position) && sled.rides < 2);
        saved.winter.santa_bag = saved.winter.santa_bag.filter(|x| (0.0..=1.0).contains(x));
        if saved.winter.legacy_bag_on_sled && saved.winter.mounts.sled.is_none() {
            saved.winter.mounts.sled = Some(Cargo::Bag);
        }
        let mut mounts = std::mem::take(&mut saved.winter.mounts);
        let valid = |cargo| match cargo {
            Cargo::Bag => saved.winter.santa_bag.is_some(),
            Cargo::Fir => saved.tree.is_some_and(Tree::valid),
            Cargo::Snowman(slot) => slot < MAX_SNOWMEN && saved.winter.snowman(slot).is_some(),
        };
        mounts.sled = mounts
            .sled
            .filter(|&cargo| saved.winter.sled.is_some() && valid(cargo));
        let mut loaded = vec![mounts.sled];
        for (slot, cargo) in mounts.walls.iter_mut().enumerate() {
            *cargo = cargo.filter(|&cargo| {
                saved
                    .winter
                    .walls
                    .get(slot)
                    .is_some_and(|wall| wall.finished())
                    && valid(cargo)
                    && !loaded.contains(&Some(cargo))
            });
            loaded.push(*cargo);
        }
        saved.winter.mounts = mounts;
        saved.winter.santa_gifts.retain(|gift| {
            (0.0..=1.0).contains(&gift.position) && gift.box_color < 4 && gift.ribbon_color < 4
        });
        saved.tree = saved.tree.filter(|tree| tree.valid());
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
    preview: Option<Save>,
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
            preview: None,
            save_at: None,
        }
    }

    #[cfg(test)]
    pub fn test(default_chibi: &str) -> Self {
        Self {
            _lock: None,
            path: None,
            save: Save::new(default_chibi),
            preview: None,
            save_at: None,
        }
    }

    pub fn chibis(&self) -> &[String] {
        &self.preview.as_ref().unwrap_or(&self.save).chibis
    }

    pub fn start_preview(&mut self) {
        self.flush();
        self.preview = Some(Save {
            version: self.save.version,
            chibis: self.save.chibis.clone(),
            winter: WinterObjects::default(),
            tree: None,
        });
    }

    pub fn stop_preview(&mut self) {
        self.preview = None;
    }

    fn active_mut(&mut self) -> &mut Save {
        self.preview.as_mut().unwrap_or(&mut self.save)
    }

    pub fn add(&mut self, id: String) {
        self.active_mut().chibis.push(id);
        self.save_now();
    }

    pub fn remove(&mut self, id: &str) {
        let save = self.active_mut();
        if let Some(index) = save.chibis.iter().rposition(|saved| saved == id) {
            save.chibis.remove(index);
            self.save_now();
        }
    }

    pub fn uninstall(&mut self, id: &str) {
        self.save.chibis.retain(|entry| entry != id);
        if let Some(preview) = &mut self.preview {
            preview.chibis.retain(|entry| entry != id);
        }
        self.save_at = self.path.as_ref().map(|_| Instant::now());
        self.flush();
    }

    pub fn update_season(&mut self, year: Option<i32>) -> bool {
        if self.winter().year == year {
            return false;
        }
        *self.winter_mut() = WinterObjects {
            year,
            ..WinterObjects::default()
        };
        self.changed();
        true
    }

    pub fn winter(&self) -> &WinterObjects {
        &self.preview.as_ref().unwrap_or(&self.save).winter
    }

    pub fn winter_mut(&mut self) -> &mut WinterObjects {
        &mut self.active_mut().winter
    }

    pub fn tree(&self) -> Option<Tree> {
        self.preview.as_ref().unwrap_or(&self.save).tree
    }

    pub fn tree_mut(&mut self) -> &mut Option<Tree> {
        &mut self.active_mut().tree
    }

    pub fn changed(&mut self) {
        if self.path.is_some() && self.preview.is_none() {
            self.save_at
                .get_or_insert_with(|| Instant::now() + SAVE_INTERVAL);
        }
    }

    pub fn save_now(&mut self) {
        if self.path.is_some() && self.preview.is_none() {
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
    fn legacy_bag_on_sled_is_restored_as_the_sled_load() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scene.json");
        fs::write(
            &path,
            r#"{
            "version": 1,
            "chibis": ["bear-girl"],
            "winter": {
                "year": 2042,
                "piles": [],
                "sled": {"position": 0.4, "rides": 1},
                "santa_bag": 0.38,
                "santa_bag_on_sled": true
            }
        }"#,
        )
        .unwrap();
        let save = Save::read(&path).unwrap();
        assert_eq!(save.winter.mounts.sled, Some(Cargo::Bag));
        save.write(&path).unwrap();
        assert_eq!(
            Save::read(&path).unwrap().winter.mounts.sled,
            Some(Cargo::Bag)
        );
    }

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
            *world.winter_mut().snowman_mut(1) = Some(Snowman {
                position: 0.61,
                seconds: 356.0,
            });
            world.winter_mut().walls.push(SnowWall {
                slope_right: true,
                position: 0.2,
                seconds: 347.0,
            });
            world.winter_mut().sled = Some(Sled {
                position: 0.56,
                rides: 1,
            });
            *world.tree_mut() = Some(Tree::new(0.50, 2027, true));
        }
        {
            let mut world = World::open(path.clone(), "bear-girl");
            assert_eq!(world.chibis(), ["bear-girl", "bear-girl"]);
            assert!(!world.update_season(Some(2027)));
            assert_eq!(world.winter().piles[0].servings, 2);
            assert_eq!(world.winter().piles[0].position, 0.35);
            let saved = world.winter().snowman.unwrap();
            assert_eq!((saved.position, saved.seconds), (0.78, 124.0));
            let saved = world.winter().snowman(1).unwrap();
            assert_eq!((saved.position, saved.seconds), (0.61, 356.0));
            assert_eq!(world.winter().walls[0].seconds, 347.0);
            let sled = world.winter().sled.unwrap();
            assert_eq!((sled.position, sled.rides), (0.56, 1));
            assert_eq!(world.tree().unwrap().position, 0.50);
            assert!(world.update_season(Some(2028)));
            world.remove("bear-girl");
            world.remove("bear-girl");
        }
        let world = World::open(path, "bear-girl");
        assert!(world.chibis().is_empty());
        assert!(world.winter().piles.is_empty());
        assert!(world.winter().snowman.is_none());
        assert!(world.winter().snowman(1).is_none());
        assert!(world.winter().walls.is_empty());
        assert!(world.winter().sled.is_none());
        assert!(world.tree().is_some());
    }
}
#[test]
fn preview_never_writes_test_objects_or_rosters_to_the_scene_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.json");
    {
        let mut world = World::open(path.clone(), "bear-girl");
        world.update_season(Some(2027));
        world.winter_mut().snowman = Some(Snowman {
            position: 0.4,
            seconds: 89.0,
        });
        *world.tree_mut() = Some(Tree::new(0.65, 2027, true));
        world.save_now();
        world.start_preview();
        world.update_season(Some(2099));
        world.add(pack::LION_ID.to_owned());
        assert!(world.winter().snowman.is_none());
        assert!(world.tree().is_none());
        *world.tree_mut() = Some(Tree::new(0.3, 2099, false));
        world.winter_mut().snowman = Some(Snowman {
            position: 0.6,
            seconds: 600.0,
        });
        world.save_now();
        world.stop_preview();
        assert_eq!(world.chibis(), ["bear-girl"]);
        assert_eq!(world.winter().snowman.unwrap().seconds, 89.0);
        world.start_preview();
        world.remove("bear-girl");
        assert!(world.chibis().is_empty());
    }
    let world = World::open(path, "bear-girl");
    assert_eq!(world.chibis(), ["bear-girl"]);
    assert_eq!(world.winter().snowman.unwrap().position, 0.4);
    assert_eq!(world.tree().unwrap().position, 0.65);
}
