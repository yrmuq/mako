// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! One chibi: pictures, a skeleton, and animations.
//! A zip may omit `body` or `anima`; those files then come from bear girl.

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use tiny_skia::{IntRect, Pixmap, Point, PremultipliedColorU8, Transform};
use zip::ZipArchive;

use crate::Fault;

pub const BUILTIN_ID: &str = "bear-girl";
// The importer has always replaced dots, so existing imported IDs stay distinct.
pub const LION_ID: &str = "builtin.lion-girl";

pub fn is_builtin(id: &str) -> bool {
    matches!(id, BUILTIN_ID | LION_ID)
}
const MAX_FILE: u64 = 4_000_000;
const MAX_TOTAL: u64 = 24_000_000;
const KNEES: [(&str, &str, &str, &str); 2] = [
    ("left-leg", "left-foot", "left-thigh", "left-shin"),
    ("right-leg", "right-foot", "right-thigh", "right-shin"),
];

#[derive(Clone)]
pub struct Sprite {
    pub image: Arc<Pixmap>,
    pub x: f32,
    pub y: f32,
    ink: Arc<[InkRow]>,
}

#[derive(Clone, Copy)]
struct InkRow {
    left: u32,
    right: u32,
    y: u32,
}

impl Sprite {
    pub fn is_blank(&self) -> bool {
        self.ink.is_empty()
    }

    pub fn lower_edge(&self, transform: Transform) -> Option<f32> {
        self.ink
            .iter()
            .map(|row| {
                let x = if transform.ky < 0.0 {
                    row.left
                } else {
                    row.right
                };
                let y = row.y + u32::from(transform.sy >= 0.0);
                transform.ky * x as f32 + transform.sy * y as f32 + transform.ty
            })
            .max_by(f32::total_cmp)
    }

    pub fn closest_ink(&self, point: Point) -> Option<Point> {
        self.ink
            .iter()
            .flat_map(|row| {
                let last = row.right - 1;
                let x = point.x.floor().clamp(row.left as f32, last as f32) as u32;
                [x, row.left, last]
                    .into_iter()
                    .filter(move |&x| {
                        self.image
                            .pixel(x, row.y)
                            .is_some_and(|pixel| pixel.alpha() >= 32)
                    })
                    .map(|x| Point::from_xy(x as f32 + 0.5, row.y as f32 + 0.5))
            })
            .min_by(|a, b| {
                let distance = |p: &Point| (p.x - point.x).powi(2) + (p.y - point.y).powi(2);
                distance(a).total_cmp(&distance(b))
            })
    }
}

#[derive(Clone)]
pub struct Bone {
    pub name: String,
    pub image: String,
    pub parent: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub z: i32,
}

#[derive(Clone)]
struct Sample<T> {
    t: f32,
    value: T,
}

#[derive(Clone)]
struct Track {
    bone: String,
    samples: Vec<Sample<f32>>,
}

#[derive(Clone)]
struct ImageTrack {
    bone: String,
    samples: Vec<Sample<String>>,
}

#[derive(Clone)]
struct Anim {
    name: String,
    length: f32,
    rot: Vec<Track>,
    image: Vec<ImageTrack>,
    bob: Vec<Sample<f32>>,
}

#[derive(Clone)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub feet_x: f32,
    pub feet_y: f32,
    pub bones: Vec<Bone>,
    images: HashMap<String, Sprite>,
    anims: Vec<Anim>,
    names: HashMap<String, usize>,
}

#[derive(Clone, Copy)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub rot: f32,
}

impl Placed {
    pub fn point(self, local: Point) -> Point {
        let (sin, cos) = self.rot.to_radians().sin_cos();
        Point::from_xy(
            self.x + cos * local.x - sin * local.y,
            self.y + sin * local.x + cos * local.y,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BonePoint {
    pub bone: usize,
    pub local: Point,
}

impl BonePoint {
    pub fn in_pose(self, pose: &Pose) -> Point {
        pose.placed[self.bone].point(self.local)
    }
}

#[derive(Clone, Copy)]
pub enum Face {
    Neutral,
    Blink(f32),
    Shock,
}

pub struct Pose {
    pub placed: Vec<Placed>,
    pub image: Vec<String>,
    pub bob: f32,
}

struct Raw {
    heart: Option<String>,
    body: Option<String>,
    anima: Option<String>,
    images: HashMap<String, Vec<u8>>,
}

impl Pack {
    pub fn builtin() -> Self {
        builtin_pack().clone()
    }

    pub fn sprite(&self, name: &str) -> Option<&Sprite> {
        self.images.get(name)
    }

    pub fn pose(&self, anim_name: &str, time: f32) -> Pose {
        self.pose_with(anim_name, time, None)
    }

    /// Local bone degrees. `degrees` replaces the animation rotations and stops the bob.
    pub fn pose_with(&self, anim_name: &str, time: f32, degrees: Option<&[f32]>) -> Pose {
        self.pose_for(anim_name, time, degrees, Face::Neutral)
    }

    pub fn pose_for(
        &self,
        anim_name: &str,
        time: f32,
        degrees: Option<&[f32]>,
        face: Face,
    ) -> Pose {
        let (mut angle, mut image, mut bob) = self.sample_locals(anim_name, time);
        match face {
            Face::Blink(phase) => {
                if let Some(blink) = self
                    .anims
                    .iter()
                    .find(|anim| anim.name == "blink" && phase < anim.length)
                {
                    apply(blink, phase, &self.names, &mut angle, &mut image, &mut bob);
                }
            }
            Face::Shock => {
                for (bone, sprite) in [("eyes", "eyes-shock"), ("mouth", "mouth-shock")] {
                    if let Some(index) = self
                        .names
                        .get(bone)
                        .filter(|_| self.images.contains_key(sprite))
                    {
                        image[*index] = sprite.into();
                    }
                }
            }
            Face::Neutral => {}
        }
        if let Some(degrees) = degrees {
            for (slot, value) in angle.iter_mut().zip(degrees) {
                *slot = *value;
            }
            bob = 0.0;
        }
        if is_builtin(&self.id) && (degrees.is_some() || anim_name == "climb") {
            for (leg, foot, thigh, shin) in KNEES {
                let foot_index = self.names[foot];
                if angle[foot_index].abs() > 0.1 {
                    image[self.names[leg]] = thigh.into();
                    image[foot_index] = shin.into();
                }
            }
        }
        let mut placed = vec![
            Placed {
                x: 0.0,
                y: 0.0,
                rot: 0.0,
            };
            self.bones.len()
        ];
        let mut seen = vec![0_u8; self.bones.len()];
        for index in 0..self.bones.len() {
            place(self, index, &angle, &mut placed, &mut seen);
        }
        Pose { placed, image, bob }
    }

    pub fn local_degrees(&self, anim_name: &str, time: f32) -> Vec<f32> {
        self.sample_locals(anim_name, time).0
    }

    pub fn rise_degrees(&self, name: &str, progress: f32) -> Vec<f32> {
        let anim = self
            .anims
            .iter()
            .find(|anim| anim.name == name)
            .or_else(|| builtin_pack().anims.iter().find(|anim| anim.name == name))
            .expect("built-in rise animation");
        let mut angle = self.local_degrees("idle", 0.0);
        apply_rotations(anim, progress * anim.length, &self.names, &mut angle);
        angle
    }

    fn sample_locals(&self, anim_name: &str, time: f32) -> (Vec<f32>, Vec<String>, f32) {
        let mut angle = vec![0.0; self.bones.len()];
        let mut image: Vec<String> = self.bones.iter().map(|bone| bone.image.clone()).collect();
        let mut bob = 0.0;
        if let Some(anim) = self.anim_or_idle(anim_name) {
            apply(
                anim,
                local_time(time, anim.length),
                &self.names,
                &mut angle,
                &mut image,
                &mut bob,
            );
        }
        if matches!(
            anim_name,
            "climb"
                | "talk"
                | "listen"
                | "pack-snow"
                | "throw-snow"
                | "sculpt"
                | "sculpt-middle"
                | "sculpt-top"
                | "water-tree"
                | "decorate-tree"
                | "open-gift"
                | "eat-candy"
                | "carry-tree"
                | "parry-snow"
                | "hide-snow"
                | "snow-cannon"
                | "pull-sled"
                | "ride-sled"
                | "wall-recover"
                | "ramp-wave"
        ) && self.animation_length(anim_name).is_none()
        {
            if let Some(anim) = builtin_pack()
                .anims
                .iter()
                .find(|anim| anim.name == anim_name)
            {
                apply_rotations(anim, local_time(time, anim.length), &self.names, &mut angle);
            }
        }
        (angle, image, bob)
    }

    pub fn animation_length(&self, name: &str) -> Option<f32> {
        self.anims
            .iter()
            .find(|anim| anim.name == name)
            .map(|anim| anim.length)
    }

    fn anim_or_idle(&self, name: &str) -> Option<&Anim> {
        self.anims
            .iter()
            .find(|anim| anim.name == name)
            .or_else(|| self.anims.iter().find(|anim| anim.name == "idle"))
    }
}

fn apply(
    anim: &Anim,
    time: f32,
    names: &HashMap<String, usize>,
    angle: &mut [f32],
    image: &mut [String],
    bob: &mut f32,
) {
    apply_rotations(anim, time, names, angle);
    for track in &anim.image {
        let Some(&index) = names.get(&track.bone) else {
            continue;
        };
        if let Some(name) = sample_step(&track.samples, time) {
            image[index] = name.clone();
        }
    }
    if !anim.bob.is_empty() {
        *bob = sample_linear(&anim.bob, time);
    }
}

fn apply_rotations(anim: &Anim, time: f32, names: &HashMap<String, usize>, angle: &mut [f32]) {
    for track in &anim.rot {
        if let Some(&index) = names.get(&track.bone) {
            angle[index] = sample_linear(&track.samples, time);
        }
    }
}

fn place(pack: &Pack, index: usize, angle: &[f32], out: &mut [Placed], seen: &mut [u8]) {
    if seen[index] == 2 {
        return;
    }
    if seen[index] == 1 {
        return;
    }
    seen[index] = 1;
    let bone = &pack.bones[index];
    if let Some(parent) = bone.parent {
        place(pack, parent, angle, out, seen);
        let parent = &out[parent];
        let rad = parent.rot.to_radians();
        let (sin, cos) = rad.sin_cos();
        out[index] = Placed {
            x: parent.x + cos * bone.x - sin * bone.y,
            y: parent.y + sin * bone.x + cos * bone.y,
            rot: parent.rot + angle[index],
        };
    } else {
        out[index] = Placed {
            x: bone.x,
            y: bone.y,
            rot: angle[index],
        };
    }
    seen[index] = 2;
}

fn local_time(time: f32, length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }
    let wrapped = time % length;
    if wrapped < 0.0 {
        wrapped + length
    } else {
        wrapped
    }
}

fn sample_linear(samples: &[Sample<f32>], time: f32) -> f32 {
    let Some(first) = samples.first() else {
        return 0.0;
    };
    if samples.len() == 1 || time <= first.t {
        return first.value;
    }
    let Some(last) = samples.last() else {
        return 0.0;
    };
    if time >= last.t {
        return last.value;
    }
    for pair in samples.windows(2) {
        if time <= pair[1].t {
            let span = (pair[1].t - pair[0].t).max(0.000_1);
            let mix = (time - pair[0].t) / span;
            return pair[0].value + (pair[1].value - pair[0].value) * mix;
        }
    }
    last.value
}

fn sample_step<T>(samples: &[Sample<T>], time: f32) -> Option<&T> {
    let mut chosen = None;
    for sample in samples {
        if sample.t <= time {
            chosen = Some(&sample.value);
        }
    }
    chosen.or_else(|| samples.first().map(|sample| &sample.value))
}

pub fn installed() -> Vec<Pack> {
    let mut found = vec![Pack::builtin(), lion_pack().clone()];
    let Ok(entries) = fs::read_dir(chibi_dir()) else {
        return found;
    };
    let mut zips = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if is_builtin(stem) || !safe_id(stem) {
            continue;
        }
        zips.push((stem.to_string(), path));
    }
    zips.sort_by(|left, right| left.0.cmp(&right.0));
    for (id, path) in zips {
        match from_path(&path, &id) {
            Ok(pack) => found.push(pack),
            Err(error) => eprintln!("{error}"),
        }
    }
    found
}

fn chibi_dir() -> PathBuf {
    let root = if let Some(value) = env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            home_data()
        }
    } else {
        home_data()
    };
    root.join("mako").join("chibis")
}

fn home_data() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
}

pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

fn builtin_pack() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| load_builtin(BUILTIN_ID).unwrap_or_else(|error| panic!("{error}")))
}

fn lion_pack() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| load_builtin(LION_ID).unwrap_or_else(|error| panic!("{error}")))
}

fn load_builtin(id: &str) -> Result<Pack, Fault> {
    let lion = id == LION_ID;
    let (heart, body) = if lion {
        (
            include_str!("../assets/lion-girl/heart"),
            include_str!("../assets/lion-girl/body"),
        )
    } else {
        (
            include_str!("../assets/bear-girl/heart"),
            include_str!("../assets/bear-girl/body"),
        )
    };
    let mut images = HashMap::new();
    for (name, bytes) in if lion { LION_IMAGES } else { BUILTIN_IMAGES } {
        images.insert((*name).to_string(), crop(decode(bytes)?)?);
    }
    if !lion {
        images.insert("mouth-talk".into(), crate::props::talk_mouth());
    }
    images.insert(
        "it-1".into(),
        crop(decode(include_bytes!(
            "../assets/bear-girl/pocket/it-1.png"
        ))?)?,
    );
    let mut pack = assemble(
        id,
        heart,
        body,
        include_str!("../assets/bear-girl/anima"),
        images,
    )?;
    add_shared_limb(
        &mut pack,
        "hand",
        "",
        (40.0, 26.0),
        if lion {
            include_bytes!("../assets/lion-girl/creature/arm.png")
        } else {
            include_bytes!("../assets/bear-girl/creature/arm.png")
        },
    )?;
    add_shared_limb(
        &mut pack,
        "leg",
        "",
        (49.0, 47.0),
        if lion {
            include_bytes!("../assets/lion-girl/creature/leg.png")
        } else {
            include_bytes!("../assets/bear-girl/creature/leg.png")
        },
    )?;
    add_knees(&mut pack)?;
    Ok(pack)
}

fn add_knees(pack: &mut Pack) -> Result<(), Fault> {
    for (leg, foot, thigh, shin) in KNEES {
        let upper = &pack.bones[pack.names[leg]];
        let lower = &pack.bones[pack.names[foot]];
        let sprite = pack.images[leg].clone();
        let split = (upper.pivot_y + lower.y - sprite.y) as u32;
        let width = sprite.image.width();
        let height = sprite.image.height();
        // Overlap the two pieces of the same pant leg to cover the joint as it bends.
        let overlap = 20;
        let shin_top = split - overlap;
        let piece = |top, bottom| {
            let rect = IntRect::from_xywh(0, top as i32, width, bottom - top).unwrap();
            crop(sprite.image.clone_rect(rect).expect("built-in leg section"))
        };
        let mut thigh_piece = piece(0, split + overlap)?;
        thigh_piece.x += sprite.x;
        thigh_piece.y += sprite.y;
        let mut shin_piece = piece(shin_top, height)?;
        shin_piece.x += lower.pivot_x + sprite.x - upper.pivot_x - lower.x;
        shin_piece.y += lower.pivot_y - overlap as f32;
        pack.images.insert(thigh.into(), thigh_piece);
        pack.images.insert(shin.into(), shin_piece);
    }
    Ok(())
}

fn add_shared_limb(
    pack: &mut Pack,
    limb: &str,
    suffix: &str,
    pivot: (f32, f32),
    bytes: &[u8],
) -> Result<(), Fault> {
    let sprite = crop(decode(bytes)?)?;
    for side in ["left", "right"] {
        let name = format!("{side}-{limb}");
        let bone = &pack.bones[pack.names[&name]];
        pack.images.insert(
            format!("{name}{suffix}"),
            Sprite {
                x: bone.pivot_x + sprite.x - pivot.0,
                y: bone.pivot_y + sprite.y - pivot.1,
                ..sprite.clone()
            },
        );
    }
    Ok(())
}

const BUILTIN_IMAGES: &[(&str, &[u8])] = &[
    (
        "head",
        include_bytes!("../assets/bear-girl/creature/head.png"),
    ),
    (
        "eyes-open",
        include_bytes!("../assets/bear-girl/creature/eyes-open.png"),
    ),
    (
        "eyes-closed",
        include_bytes!("../assets/bear-girl/creature/eyes-closed.png"),
    ),
    (
        "mouth",
        include_bytes!("../assets/bear-girl/creature/mouth.png"),
    ),
    (
        "eyes-shock",
        include_bytes!("../assets/bear-girl/creature/eyes-shock.png"),
    ),
    (
        "mouth-shock",
        include_bytes!("../assets/bear-girl/creature/mouth-shock.png"),
    ),
    (
        "torso",
        include_bytes!("../assets/bear-girl/creature/torso.png"),
    ),
];

macro_rules! lion_images {
    ($($name:literal),+ $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/lion-girl/creature/", $name, ".png")))),+]
    };
}

const LION_IMAGES: &[(&str, &[u8])] = lion_images!(
    "head",
    "eyes-open",
    "eyes-closed",
    "eyes-shock",
    "mouth",
    "mouth-shock",
    "mouth-talk",
    "torso",
    "tail",
);

fn from_path(path: &Path, id: &str) -> Result<Pack, Fault> {
    let file = File::open(path).map_err(|error| {
        Fault::new(
            "MAKO-PACK-0001",
            format!("The chibi archive could not be read: {error}"),
        )
    })?;
    let meta = file.metadata().map_err(|error| {
        Fault::new(
            "MAKO-PACK-0001",
            format!("The chibi archive could not be read: {error}"),
        )
    })?;
    if meta.len() > MAX_TOTAL {
        return Err(Fault::new(
            "MAKO-PACK-0001",
            "The chibi archive could not be read: it is too large",
        ));
    }
    from_reader(id, file)
}

#[cfg(test)]
impl Pack {
    fn from_bytes(id: &str, bytes: &[u8]) -> Result<Self, Fault> {
        from_reader(id, Cursor::new(bytes))
    }
}

fn from_reader<R: Read + Seek>(id: &str, reader: R) -> Result<Pack, Fault> {
    let raw = read_zip(reader)?;
    let Some(heart) = raw.heart else {
        return Err(Fault::new(
            "MAKO-PACK-0002",
            "The chibi archive has no name in heart",
        ));
    };
    let body = raw
        .body
        .unwrap_or_else(|| include_str!("../assets/bear-girl/body").to_string());
    let anima = raw
        .anima
        .unwrap_or_else(|| include_str!("../assets/bear-girl/anima").to_string());
    let mut images = HashMap::new();
    for (name, bytes) in raw.images {
        images.insert(
            name.clone(),
            crop(decode(&bytes).map_err(|error| {
                Fault::new(
                    "MAKO-PACK-0003",
                    format!("A chibi picture is not a PNG: {name}: {error}"),
                )
            })?)?,
        );
    }
    assemble(id, &heart, &body, &anima, images)
}

fn read_zip<R: Read + Seek>(reader: R) -> Result<Raw, Fault> {
    let mut archive = ZipArchive::new(reader).map_err(|error| {
        Fault::new(
            "MAKO-PACK-0001",
            format!("The chibi archive could not be read: {error}"),
        )
    })?;
    let mut raw = Raw {
        heart: None,
        body: None,
        anima: None,
        images: HashMap::new(),
    };
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            Fault::new(
                "MAKO-PACK-0001",
                format!("The chibi archive could not be read: {error}"),
            )
        })?;
        let name = file.name().replace('\\', "/");
        if name.ends_with('/') || name.split('/').any(|part| part == "..") {
            continue;
        }
        let mut buf = Vec::new();
        file.by_ref()
            .take(MAX_FILE + 1)
            .read_to_end(&mut buf)
            .map_err(|error| {
                Fault::new(
                    "MAKO-PACK-0001",
                    format!("The chibi archive could not be read: {error}"),
                )
            })?;
        if buf.len() as u64 > MAX_FILE {
            return Err(Fault::new(
                "MAKO-PACK-0001",
                "The chibi archive could not be read: a file is too large",
            ));
        }
        total += buf.len() as u64;
        if total > MAX_TOTAL {
            return Err(Fault::new(
                "MAKO-PACK-0001",
                "The chibi archive could not be read: it is too large",
            ));
        }
        match name.as_str() {
            "heart" => raw.heart = Some(utf8_text(&buf)?),
            "body" => raw.body = Some(utf8_text(&buf)?),
            "anima" => raw.anima = Some(utf8_text(&buf)?),
            other => {
                if let Some((folder, stem)) = picture_name(other) {
                    if folder == "creature" || !raw.images.contains_key(&stem) {
                        raw.images.insert(stem, buf);
                    }
                }
            }
        }
    }
    Ok(raw)
}

fn utf8_text(bytes: &[u8]) -> Result<String, Fault> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        Fault::new(
            "MAKO-PACK-0001",
            "The chibi archive could not be read: a text file is not UTF-8",
        )
    })
}

fn picture_name(path: &str) -> Option<(String, String)> {
    let mut parts = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".");
    let folder = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() || (folder != "creature" && folder != "pocket") {
        return None;
    }
    let stem = file.strip_suffix(".png")?;
    if stem.is_empty() || stem.contains(' ') {
        return None;
    }
    Some((folder.to_string(), stem.to_string()))
}

fn assemble(
    id: &str,
    heart: &str,
    body: &str,
    anima: &str,
    images: HashMap<String, Sprite>,
) -> Result<Pack, Fault> {
    let name = parse_heart(heart)?;
    let parsed = parse_body(body)?;
    let anims = parse_anima(anima)?;
    let mut names = HashMap::new();
    for (index, bone) in parsed.bones.iter().enumerate() {
        names.insert(bone.name.clone(), index);
    }
    Ok(Pack {
        id: id.to_string(),
        name,
        canvas_w: parsed.canvas_w,
        canvas_h: parsed.canvas_h,
        feet_x: parsed.feet_x,
        feet_y: parsed.feet_y,
        bones: parsed.bones,
        images,
        anims,
        names,
    })
}

fn parse_heart(text: &str) -> Result<String, Fault> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "name" {
            let name = value.trim();
            if !name.is_empty() {
                return Ok(name.to_string());
            }
        }
    }
    Err(Fault::new(
        "MAKO-PACK-0002",
        "The chibi archive has no name in heart",
    ))
}

struct ParsedBody {
    canvas_w: f32,
    canvas_h: f32,
    feet_x: f32,
    feet_y: f32,
    bones: Vec<Bone>,
}

struct Draft {
    name: String,
    image: String,
    parent: String,
    x: f32,
    y: f32,
    pivot_x: f32,
    pivot_y: f32,
    z: i32,
}

fn parse_body(text: &str) -> Result<ParsedBody, Fault> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut canvas = None;
    let mut feet = None;
    let mut drafts = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let token: Vec<&str> = line.split_whitespace().collect();
        match token.first().copied() {
            Some("canvas") if token.len() == 3 => {
                let width = number_at(&token, 1, number, "MAKO-PACK-0004")?;
                let height = number_at(&token, 2, number, "MAKO-PACK-0004")?;
                if width <= 0.0 || height <= 0.0 || canvas.is_some() {
                    return Err(body_error(number, "canvas"));
                }
                canvas = Some((width, height));
            }
            Some("feet") if token.len() == 3 => {
                if feet.is_some() {
                    return Err(body_error(number, "feet"));
                }
                feet = Some((
                    number_at(&token, 1, number, "MAKO-PACK-0004")?,
                    number_at(&token, 2, number, "MAKO-PACK-0004")?,
                ));
            }
            Some("bone") if token.len() == 9 => {
                drafts.push(Draft {
                    name: token[1].to_string(),
                    image: token[2].to_string(),
                    parent: token[3].to_string(),
                    x: number_at(&token, 4, number, "MAKO-PACK-0004")?,
                    y: number_at(&token, 5, number, "MAKO-PACK-0004")?,
                    pivot_x: number_at(&token, 6, number, "MAKO-PACK-0004")?,
                    pivot_y: number_at(&token, 7, number, "MAKO-PACK-0004")?,
                    z: token[8]
                        .parse::<i32>()
                        .map_err(|_| body_error(number, "z"))?,
                });
            }
            _ => return Err(body_error(number, line)),
        }
    }
    let Some((canvas_w, canvas_h)) = canvas else {
        return Err(Fault::new(
            "MAKO-PACK-0004",
            "The chibi skeleton could not be read: canvas is missing",
        ));
    };
    if drafts.is_empty() {
        return Err(Fault::new(
            "MAKO-PACK-0004",
            "The chibi skeleton could not be read: it has no bones",
        ));
    }
    let (feet_x, feet_y) = feet.unwrap_or((canvas_w * 0.5, canvas_h - 1.0));
    let bones = link_bones(drafts)?;
    Ok(ParsedBody {
        canvas_w,
        canvas_h,
        feet_x,
        feet_y,
        bones,
    })
}

fn link_bones(drafts: Vec<Draft>) -> Result<Vec<Bone>, Fault> {
    let mut index = HashMap::new();
    for (i, draft) in drafts.iter().enumerate() {
        if index.insert(draft.name.clone(), i).is_some() {
            return Err(Fault::new(
                "MAKO-PACK-0004",
                format!(
                    "The chibi skeleton could not be read: {} is repeated",
                    draft.name
                ),
            ));
        }
    }
    let mut bones = Vec::new();
    for draft in &drafts {
        let parent = if draft.parent == "-" {
            None
        } else {
            let Some(&parent) = index.get(&draft.parent) else {
                return Err(Fault::new(
                    "MAKO-PACK-0004",
                    format!(
                        "The chibi skeleton could not be read: {} has no parent {}",
                        draft.name, draft.parent
                    ),
                ));
            };
            Some(parent)
        };
        bones.push(Bone {
            name: draft.name.clone(),
            image: if draft.image == "-" {
                String::new()
            } else {
                draft.image.clone()
            },
            parent,
            x: draft.x,
            y: draft.y,
            pivot_x: draft.pivot_x,
            pivot_y: draft.pivot_y,
            z: draft.z,
        });
    }
    for start in 0..bones.len() {
        let mut seen = vec![false; bones.len()];
        let mut cursor = Some(start);
        while let Some(current) = cursor {
            if seen[current] {
                return Err(Fault::new(
                    "MAKO-PACK-0004",
                    "The chibi skeleton could not be read: the bones loop",
                ));
            }
            seen[current] = true;
            cursor = bones[current].parent;
        }
    }
    Ok(bones)
}

fn parse_anima(text: &str) -> Result<Vec<Anim>, Fault> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut anims = Vec::new();
    let mut current: Option<Anim> = None;
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let token: Vec<&str> = line.split_whitespace().collect();
        match token.first().copied() {
            Some("anim") if token.len() == 3 => {
                if let Some(anim) = current.take() {
                    anims.push(anim);
                }
                current = Some(Anim {
                    name: token[1].to_string(),
                    length: number_at(&token, 2, number, "MAKO-PACK-0005")?,
                    rot: Vec::new(),
                    image: Vec::new(),
                    bob: Vec::new(),
                });
            }
            Some("rot") if token.len() >= 4 && token.len().is_multiple_of(2) => {
                let anim = current.as_mut().ok_or_else(|| anima_error(number, "rot"))?;
                anim.rot.push(Track {
                    bone: token[1].to_string(),
                    samples: pairs_f32(&token, number)?,
                });
            }
            Some("image") if token.len() >= 4 && token.len().is_multiple_of(2) => {
                let anim = current
                    .as_mut()
                    .ok_or_else(|| anima_error(number, "image"))?;
                let mut samples = Vec::new();
                let mut index = 2;
                while index < token.len() {
                    samples.push(Sample {
                        t: number_token(token[index], number, "MAKO-PACK-0005")?,
                        value: token[index + 1].to_string(),
                    });
                    index += 2;
                }
                anim.image.push(ImageTrack {
                    bone: token[1].to_string(),
                    samples,
                });
            }
            Some("bob") if token.len() >= 3 && !token.len().is_multiple_of(2) => {
                let anim = current.as_mut().ok_or_else(|| anima_error(number, "bob"))?;
                anim.bob = pairs_f32_bob(&token, number)?;
            }
            _ => return Err(anima_error(number, line)),
        }
    }
    if let Some(anim) = current {
        anims.push(anim);
    }
    if anims.is_empty() {
        return Err(Fault::new(
            "MAKO-PACK-0005",
            "The chibi animations could not be read: the file is empty",
        ));
    }
    Ok(anims)
}

fn pairs_f32(token: &[&str], number: usize) -> Result<Vec<Sample<f32>>, Fault> {
    let mut samples = Vec::new();
    let mut index = 2;
    while index < token.len() {
        samples.push(Sample {
            t: number_token(token[index], number, "MAKO-PACK-0005")?,
            value: number_token(token[index + 1], number, "MAKO-PACK-0005")?,
        });
        index += 2;
    }
    Ok(samples)
}

fn pairs_f32_bob(token: &[&str], number: usize) -> Result<Vec<Sample<f32>>, Fault> {
    let mut samples = Vec::new();
    let mut index = 1;
    while index < token.len() {
        samples.push(Sample {
            t: number_token(token[index], number, "MAKO-PACK-0005")?,
            value: number_token(token[index + 1], number, "MAKO-PACK-0005")?,
        });
        index += 2;
    }
    Ok(samples)
}

fn number_at(token: &[&str], index: usize, line: usize, code: &'static str) -> Result<f32, Fault> {
    number_token(token[index], line, code)
}

fn number_token(token: &str, line: usize, code: &'static str) -> Result<f32, Fault> {
    let value = token
        .parse::<f32>()
        .map_err(|_| Fault::new(code, format!("line {}: {token}", line + 1)))?;
    if !value.is_finite() {
        return Err(Fault::new(code, format!("line {}: {token}", line + 1)));
    }
    Ok(value)
}

fn body_error(line: usize, text: &str) -> Fault {
    Fault::new(
        "MAKO-PACK-0004",
        format!(
            "The chibi skeleton could not be read: line {}: {text}",
            line + 1
        ),
    )
}

fn anima_error(line: usize, text: &str) -> Fault {
    Fault::new(
        "MAKO-PACK-0005",
        format!(
            "The chibi animations could not be read: line {}: {text}",
            line + 1
        ),
    )
}

fn decode(bytes: &[u8]) -> Result<Pixmap, Fault> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|_| Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|_| Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"))?;
    if info.bit_depth != png::BitDepth::Eight || info.color_type != png::ColorType::Rgba {
        return Err(Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"));
    }
    let mut pixmap = Pixmap::new(info.width, info.height)
        .ok_or_else(|| Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"))?;
    let data = &buf[..info.buffer_size()];
    let (src_chunks, _) = data.as_chunks::<4>();
    for (src, dst) in src_chunks.iter().zip(pixmap.pixels_mut()) {
        *dst = PremultipliedColorU8::from_rgba(src[0], src[1], src[2], src[3])
            .unwrap_or(PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
    }
    Ok(pixmap)
}

pub(crate) fn crop(pixmap: Pixmap) -> Result<Sprite, Fault> {
    let width = pixmap.width();
    let height = pixmap.height();
    let pixels = pixmap.pixels();
    let mut ink = Vec::new();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for (y, row) in pixels.chunks_exact(width as usize).enumerate() {
        let Some(left) = row.iter().position(|pixel| pixel.alpha() != 0) else {
            continue;
        };
        let right = row.iter().rposition(|pixel| pixel.alpha() != 0).unwrap();
        let (left, right, y) = (left as u32, right as u32, y as u32);
        min_x = min_x.min(left);
        min_y = min_y.min(y);
        max_x = max_x.max(right);
        max_y = max_y.max(y);
        ink.push(InkRow {
            left,
            right: right + 1,
            y,
        });
    }
    if ink.is_empty() {
        let image = Pixmap::new(1, 1)
            .ok_or_else(|| Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"))?;
        return Ok(Sprite {
            image: Arc::new(image),
            x: 0.0,
            y: 0.0,
            ink: Arc::from([]),
        });
    }
    let piece_w = max_x - min_x + 1;
    let piece_h = max_y - min_y + 1;
    let mut image = Pixmap::new(piece_w, piece_h)
        .ok_or_else(|| Fault::new("MAKO-PACK-0003", "A chibi picture is not a PNG"))?;
    for y in 0..piece_h {
        for x in 0..piece_w {
            image.pixels_mut()[(y * piece_w + x) as usize] =
                pixels[((min_y + y) * width + min_x + x) as usize];
        }
    }
    for row in &mut ink {
        row.left -= min_x;
        row.right -= min_x;
        row.y -= min_y;
    }
    Ok(Sprite {
        image: Arc::new(image),
        x: min_x as f32,
        y: min_y as f32,
        ink: ink.into(),
    })
}

pub fn import(path: &Path) -> Result<Pack, Fault> {
    use std::io::Write;

    let basename = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("chibi");
    let normalized: String = basename
        .chars()
        .take(100)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let base = if normalized.is_empty() || is_builtin(&normalized) {
        "imported-chibi"
    } else {
        &normalized
    };
    let file = File::open(path).map_err(import_read)?;
    let mut bytes = Vec::new();
    file.take(MAX_TOTAL + 1)
        .read_to_end(&mut bytes)
        .map_err(import_read)?;
    if bytes.len() as u64 > MAX_TOTAL {
        return Err(Fault::new(
            "MAKO-IMPORT-0002",
            "The ZIP exceeds the chibi archive size limit",
        ));
    }
    let mut pack = from_reader(base, Cursor::new(&bytes))?;
    let folder = chibi_dir();
    fs::create_dir_all(&folder).map_err(import_write)?;
    let temporary = tempfile::NamedTempFile::new_in(&folder).map_err(import_write)?;
    temporary
        .as_file()
        .write_all(&bytes)
        .map_err(import_write)?;
    for number in 1.. {
        let id = if number == 1 {
            base.to_owned()
        } else {
            format!("{base}-{number}")
        };
        let destination = folder.join(format!("{id}.zip"));
        match fs::hard_link(temporary.path(), destination) {
            Ok(()) => {
                pack.id = id;
                return Ok(pack);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(import_write(error)),
        }
    }
    unreachable!()
}

pub fn uninstall(id: &str) -> Result<(), Fault> {
    if is_builtin(id) || !safe_id(id) {
        return Err(Fault::new(
            "MAKO-IMPORT-0004",
            "Only installed custom chibis can be removed",
        ));
    }
    let path = chibi_dir().join(format!("{id}.zip"));
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Fault::new(
            "MAKO-IMPORT-0005",
            format!("Could not remove the installed ZIP: {error}"),
        )),
    }
}

fn import_read(error: std::io::Error) -> Fault {
    Fault::new(
        "MAKO-IMPORT-0001",
        format!("Could not read the selected ZIP: {error}"),
    )
}

fn import_write(error: std::io::Error) -> Fault {
    Fault::new(
        "MAKO-IMPORT-0003",
        format!("Could not install the ZIP: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&[
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ])
                .unwrap();
        }
        out
    }

    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default();
            for (name, bytes) in files {
                writer.start_file((*name).to_string(), options).unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn missing_skeleton_uses_bear_girl_and_keeps_the_name() {
        let bytes = zip(&[("heart", b"name = kit\n"), ("creature/mark.png", &png())]);
        let pack = Pack::from_bytes("kit", &bytes).unwrap();
        assert_eq!(pack.name, "kit");
        assert_eq!(pack.bones.len(), Pack::builtin().bones.len());
        assert!(pack.bones.iter().any(|bone| bone.name == "left-hand"));
        let early = pack.pose("walk", 0.0);
        let late = pack.pose("walk", 0.35);
        let leg = pack.names["left-leg"];
        assert!((early.placed[leg].rot - late.placed[leg].rot).abs() > 10.0);
    }

    #[test]
    fn missing_animations_use_bear_girl() {
        let body = b"canvas 10 10\nfeet 5 9\nbone torso torso - 5 5 5 5 1\n";
        let bytes = zip(&[("heart", b"name = kit\n"), ("body", body)]);
        let pack = Pack::from_bytes("kit", &bytes).unwrap();
        assert_eq!(pack.bones.len(), 1);
        assert!(pack.anims.iter().any(|anim| anim.name == "walk"));
    }

    #[test]
    fn missing_heart_is_rejected() {
        let Err(error) = Pack::from_bytes("x", &zip(&[])) else {
            panic!("expected a missing heart");
        };
        assert_eq!(error.code, "MAKO-PACK-0002");
    }

    #[test]
    fn broken_skeleton_is_rejected() {
        let bytes = zip(&[("heart", b"name = kit\n"), ("body", b"nope\n")]);
        let Err(error) = Pack::from_bytes("kit", &bytes) else {
            panic!("expected a broken skeleton");
        };
        assert_eq!(error.code, "MAKO-PACK-0004");
    }

    #[test]
    fn shock_swaps_both_face_layers_while_ragdoll_rotations_are_preserved() {
        let pack = Pack::builtin();
        let angles = vec![12.0; pack.bones.len()];
        let shock = pack.pose_for("fall", 0.08, Some(&angles), Face::Shock);
        assert_eq!(shock.image[pack.names["eyes"]], "eyes-shock");
        assert_eq!(shock.image[pack.names["mouth"]], "mouth-shock");
        assert_eq!(shock.placed[pack.names["torso"]].rot, 12.0);
    }

    #[test]
    fn blink_swaps_the_open_eyes() {
        let pack = Pack::builtin();
        let open = pack.pose("idle", 0.0);
        let shut = pack.pose_for("idle", 0.08, None, Face::Blink(0.08));
        let eyes = pack.names["eyes"];
        assert_eq!(open.image[eyes], "eyes-open");
        assert_eq!(shut.image[eyes], "eyes-closed");
    }

    #[test]
    fn bear_girl_joints_overlap_the_torso() {
        let pack = Pack::builtin();
        let torso = &pack.bones[pack.names["torso"]];
        let is_opaque = |sprite: &Sprite, x: f32, y: f32| {
            let (x, y) = (x - sprite.x, y - sprite.y);
            x >= 0.0
                && y >= 0.0
                && sprite
                    .image
                    .pixel(x as u32, y as u32)
                    .is_some_and(|pixel| pixel.alpha() > 200)
        };
        let body = pack.sprite(&torso.image).unwrap();
        for name in ["head", "left-hand", "right-hand", "left-leg", "right-leg"] {
            let limb = &pack.bones[pack.names[name]];
            let sprite = pack.sprite(&limb.image).unwrap();
            assert!(
                is_opaque(body, torso.pivot_x + limb.x, torso.pivot_y + limb.y),
                "{name}"
            );
            assert!(is_opaque(sprite, limb.pivot_x, limb.pivot_y), "{name}");
            if name.ends_with("leg") {
                assert!(limb.z < torso.z, "{name}");
            }
        }
        assert!(pack.bones[pack.names["right-hand"]].z < pack.bones[pack.names["right-leg"]].z);
    }

    #[test]
    fn bear_girl_reuses_the_same_limb_art_on_both_sides() {
        let pack = Pack::builtin();
        for (limb, suffix) in [("hand", ""), ("leg", "")] {
            let left_bone = &pack.bones[pack.names[&format!("left-{limb}")]];
            let right_bone = &pack.bones[pack.names[&format!("right-{limb}")]];
            let left = pack.sprite(&format!("left-{limb}{suffix}")).unwrap();
            let right = pack.sprite(&format!("right-{limb}{suffix}")).unwrap();
            assert!(Arc::ptr_eq(&left.image, &right.image));
            assert_eq!(left.x - left_bone.pivot_x, right.x - right_bone.pivot_x);
            assert_eq!(left.y - left_bone.pivot_y, right.y - right_bone.pivot_y);
            assert_eq!(left_bone.y, right_bone.y);
        }
    }
}
#[test]
fn both_builtins_are_protected_and_lion_renders_all_core_poses() {
    for id in [BUILTIN_ID, LION_ID, "../outside"] {
        assert_eq!(uninstall(id).unwrap_err().code, "MAKO-IMPORT-0004");
    }
    let lion = lion_pack();
    assert_eq!(lion.id, LION_ID);
    assert!(lion.sprite("tail").is_some());
    assert!(crate::draw::portrait(lion).is_some());
    for motion in [
        "idle",
        "walk",
        "climb",
        "talk",
        "pack-snow",
        "sculpt",
        "rise-back",
    ] {
        assert!(lion.animation_length(motion).is_some());
        assert_eq!(
            lion.pose_for(motion, 0.2, None, Face::Shock).image.len(),
            lion.bones.len()
        );
    }
}
