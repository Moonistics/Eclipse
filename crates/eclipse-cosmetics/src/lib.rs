//! Cosmetic items that Eclipse clients draw on players: hats, back items,
//! shoulder pets, name tag styles, emotes, companions, trails and auras. Only Eclipse clients
//! see them; the game protocol carries nothing about them
//! ([docs/cosmetics.md](../../../docs/cosmetics.md)).
//!
//! An item is a small model of boxes (Minecraft's box UV, as Blockbench's
//! "Modded Entity" format), one texture of at most 64×64, the bone of the
//! player model it rides on and a few simple looping animations; a name tag
//! style is a small mark and a line colour, with no model. This crate
//! reads and checks items and poses them; the game draws them.

pub mod account;
mod emote;
mod feed;
mod fetch;
pub mod follow;
mod fx;
mod item;
pub mod mark;
mod pose;
pub mod prefs;
pub mod preview;
pub mod trail;
pub mod viewer;

pub use emote::{
    BODY, Ease, Emote, Key, MAX_ALL_KEYS, MAX_KEYS, MAX_LENGTH, MAX_MOVE, MAX_ROOT_MOVE,
    MAX_ROOT_TURN, MAX_TRACKS, MAX_TURN, Playing, Sample, Target, Track,
};
pub use feed::{Feed, POLL, Seen};
pub use fetch::{BATCH, Clock, Cosmetics, Frame, Http, Local, Transport, Worn};
pub use fx::{
    Aura, Companion, From, HEAD, HEAD_CLEARANCE, MAX_AURA_RADIUS, MAX_COMPANION_ANCHOR,
    MAX_COMPANION_REACH, MAX_LAG, MAX_PARTICLE_SIZE, MAX_SPRITE, MAX_TRAIL_LIFETIME,
    MAX_TRAIL_RATE, MAX_TRAIL_SPRITES, Trail,
};
pub use item::{
    Animation, Axis, Bone, Error, FORMAT, Face, Item, MAX_MARK, MAX_PARTS, MAX_REACH, MAX_TEXTURE,
    MAX_VERTICES, Motion, Part, Slot, Tag, Texture,
};
pub use pose::{Pose, pose_matrix};

/// Cosmetics don't run by default: there is no cosmetics service, so the game asks nothing and
/// has no settings entry for them. `ECLIPSE_COSMETICS=1` turns them on, with the service
/// `ECLIPSE_COSMETICS_API` names or the items of `ECLIPSE_COSMETICS_LOCAL`. Other players' show by
/// default only to an account that owns a cosmetic (`prefs::Others::effective`: a player who owns
/// none sees a vanilla game and the game asks nothing about other players), until the player
/// chooses.
pub const ON_BY_DEFAULT: bool = false;

/// The cosmetics service: none unless `ECLIPSE_COSMETICS_API` names one.
pub const API: &str = "";

/// Whether cosmetics run: `ON_BY_DEFAULT`, or `ECLIPSE_COSMETICS=1` / `0` for one run
/// (development, the bench).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| match std::env::var("ECLIPSE_COSMETICS").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => ON_BY_DEFAULT,
    })
}

/// `ECLIPSE_COSMETICS=1` asked for this run: the launcher shows its Wardrobe even while the
/// cosmetics service has no Shop to show (development, screenshots).
pub fn forced() -> bool {
    std::env::var("ECLIPSE_COSMETICS").as_deref() == Ok("1")
}
