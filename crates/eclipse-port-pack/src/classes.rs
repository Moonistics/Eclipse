//! What a mod's Java code does, read from its class files without running
//! or decompiling them: each class's superclass, interfaces, declared
//! methods and the classes and annotations its constant pool names. The
//! mod's own classes are followed up to the first game class they extend,
//! and that class says what the code is: a block entity, a menu, an entity,
//! a block or item with behaviour of its own, a worldgen feature...
//!
//! Since 26.1 the game ships unobfuscated, and NeoForge has run on Mojang's
//! names since 1.20.5, so game classes and methods are read by name. Fabric
//! jars before 26.1 name them in intermediary (`net/minecraft/class_2248`)
//! and Forge's methods are SRG (`m_60503_`): a class extending such a game
//! class is counted as code of its own, whatever it overrides.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One class file, as much as this module reads.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub superclass: String,
    pub interfaces: Vec<String>,
    /// Declared methods, by name (constructors and lambdas left out).
    pub methods: Vec<String>,
    /// Every class the constant pool names (`net/minecraft/...`).
    pub refs: BTreeSet<String>,
    /// Annotation and descriptor strings (`Lorg/spongepowered/asm/mixin/Mixin;`).
    pub mixin: bool,
    /// The foods its static initializer builds into its fields, by field name.
    pub foods: BTreeMap<String, Food>,
}

/// A `FoodProperties` as its builder is given it (`nutrition`, `saturationModifier`,
/// `alwaysEdible`).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Food {
    pub nutrition: i32,
    pub saturation_modifier: f32,
    pub always_edible: bool,
}

/// Reads a class file's header, constant pool and declared methods.
pub fn parse(bytes: &[u8]) -> Option<Class> {
    let mut r = Reader { bytes, at: 0 };
    if r.u4()? != 0xCAFE_BABE {
        return None;
    }
    r.skip(4)?; // minor, major
    let count = r.u2()? as usize;
    let mut utf8: HashMap<usize, String> = HashMap::new();
    let mut class_of: HashMap<usize, usize> = HashMap::new();
    let mut numbers: HashMap<usize, (u8, u32)> = HashMap::new();
    let mut pairs: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut i = 1;
    while i < count {
        let tag = r.u1()?;
        match tag {
            1 => {
                let len = r.u2()? as usize;
                let s = r.take(len)?;
                utf8.insert(i, String::from_utf8_lossy(s).into_owned());
            }
            7 => {
                class_of.insert(i, r.u2()? as usize);
            }
            8 | 16 | 19 | 20 => r.skip(2)?,
            3 | 4 => {
                numbers.insert(i, (tag, r.u4()?));
            }
            9..=12 => {
                pairs.insert(i, (r.u2()? as usize, r.u2()? as usize));
            }
            17 | 18 => r.skip(4)?,
            15 => r.skip(3)?,
            5 | 6 => {
                r.skip(8)?;
                i += 1;
            }
            _ => return None,
        }
        i += 1;
    }
    let class_name = |idx: usize| -> String {
        class_of
            .get(&idx)
            .and_then(|u| utf8.get(u))
            .cloned()
            .unwrap_or_default()
    };
    r.skip(2)?; // access flags
    let name = class_name(r.u2()? as usize);
    let superclass = class_name(r.u2()? as usize);
    let n = r.u2()? as usize;
    let mut interfaces = Vec::with_capacity(n);
    for _ in 0..n {
        interfaces.push(class_name(r.u2()? as usize));
    }
    // Fields, then methods: flags, name, descriptor, attributes.
    let mut methods = Vec::new();
    let mut clinit = None;
    for member in 0..2 {
        let n = r.u2()?;
        for _ in 0..n {
            r.skip(2)?;
            let name = r.u2()? as usize;
            r.skip(2)?;
            let attributes = r.u2()?;
            for _ in 0..attributes {
                let kind = r.u2()? as usize;
                let len = r.u4()? as usize;
                let body = r.take(len)?;
                if member == 1
                    && utf8.get(&name).is_some_and(|m| m == "<clinit>")
                    && utf8.get(&kind).is_some_and(|k| k == "Code")
                    && let Some(n) = body.get(4..8)
                {
                    let n = u32::from_be_bytes([n[0], n[1], n[2], n[3]]) as usize;
                    clinit = body.get(8..8 + n);
                }
            }
            if member == 1
                && let Some(m) = utf8.get(&name)
                && !m.starts_with('<')
                && !m.starts_with("lambda$")
            {
                methods.push(m.clone());
            }
        }
    }
    let refs = class_of
        .values()
        .filter_map(|u| utf8.get(u))
        .filter(|s| !s.starts_with('['))
        .cloned()
        .collect();
    let mixin = utf8
        .values()
        .any(|s| s == "Lorg/spongepowered/asm/mixin/Mixin;");
    let member = |idx: usize| -> Option<(String, &str, &str)> {
        let (class, pair) = pairs.get(&idx)?;
        let (name, desc) = pairs.get(pair)?;
        Some((class_name(*class), utf8.get(name)?, utf8.get(desc)?))
    };
    let foods = clinit
        .map(|code| static_foods(code, &numbers, &member))
        .unwrap_or_default();
    Some(Class {
        name,
        superclass,
        interfaces,
        methods,
        refs,
        mixin,
        foods,
    })
}

/// The `FoodProperties` a static initializer builds and stores in the class's fields: the
/// builder's numbers before each `putstatic` of one, or the numbers given to a static helper
/// taking them (`food(int nutrition, float saturation[, boolean alwaysEdible])`). Game names
/// only (26.1 on, NeoForge).
fn static_foods<'a>(
    code: &[u8],
    numbers: &HashMap<usize, (u8, u32)>,
    member: &dyn Fn(usize) -> Option<(String, &'a str, &'a str)>,
) -> BTreeMap<String, Food> {
    let mut out = BTreeMap::new();
    let (mut ints, mut float) = ([0, 0], 0.0);
    let mut food: Option<Food> = None;
    let mut at = 0;
    let index = |at: usize| -> Option<usize> {
        let b = code.get(at + 1..at + 3)?;
        Some(u16::from_be_bytes([b[0], b[1]]) as usize)
    };
    while let Some(&op) = code.get(at) {
        let mut int = None;
        match op {
            0x02..=0x08 => int = Some(op as i32 - 3),
            0x0b..=0x0d => float = (op - 0x0b) as f32,
            0x10 => int = code.get(at + 1).map(|&b| b as i8 as i32),
            0x11 => int = index(at).map(|v| v as u16 as i16 as i32),
            0x12 | 0x13 => {
                let i = if op == 0x12 {
                    code.get(at + 1).map(|&b| b as usize)
                } else {
                    index(at)
                };
                match i.and_then(|i| numbers.get(&i)) {
                    Some((3, v)) => int = Some(*v as i32),
                    Some((_, v)) => float = f32::from_bits(*v),
                    None => {}
                }
            }
            0xb6..=0xb9 => {
                let (class, name, desc) = index(at).and_then(member).unwrap_or_default();
                if class == "net/minecraft/world/food/FoodProperties$Builder" {
                    let f = food.get_or_insert_with(Food::default);
                    match name {
                        "nutrition" => f.nutrition = ints[1],
                        "saturationModifier" => f.saturation_modifier = float,
                        "alwaysEdible" => f.always_edible = true,
                        _ => {}
                    }
                } else if op == 0xb8 && desc.contains(")Lnet/minecraft/world/food/FoodProperties") {
                    let always = name.to_ascii_lowercase().contains("always");
                    food = match desc.split(')').next() {
                        Some("(IF") => Some(Food {
                            nutrition: ints[1],
                            saturation_modifier: float,
                            always_edible: always,
                        }),
                        Some("(IFZ") => Some(Food {
                            nutrition: ints[0],
                            saturation_modifier: float,
                            always_edible: ints[1] != 0,
                        }),
                        _ => food,
                    };
                }
            }
            0xb3 => {
                if let Some((_, name, desc)) = index(at).and_then(member)
                    && desc == "Lnet/minecraft/world/food/FoodProperties;"
                    && let Some(f) = food.take()
                {
                    out.insert(name.to_owned(), f);
                }
            }
            _ => {}
        }
        if let Some(i) = int {
            ints = [ints[1], i];
        }
        let Some(next) = op_len(code, at).and_then(|len| at.checked_add(len)) else {
            break;
        };
        at = next;
    }
    out
}

/// The length of the instruction at `at`, operands included. A switch's operands start at the
/// next multiple of four.
fn op_len(code: &[u8], at: usize) -> Option<usize> {
    let word = |at: usize| -> Option<usize> {
        let b = code.get(at..at + 4)?;
        Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let pad = (4 - (at + 1) % 4) % 4;
    Some(match *code.get(at)? {
        0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
        0x11
        | 0x13
        | 0x14
        | 0x84
        | 0x99..=0xa8
        | 0xb2..=0xb8
        | 0xbb
        | 0xbd
        | 0xc0
        | 0xc1
        | 0xc6
        | 0xc7 => 3,
        0xc5 => 4,
        0xb9 | 0xba | 0xc8 | 0xc9 => 5,
        0xc4 => {
            if *code.get(at + 1)? == 0x84 {
                6
            } else {
                4
            }
        }
        0xaa => {
            let (low, high) = (word(at + 1 + pad + 4)?, word(at + 1 + pad + 8)?);
            let cases = high.wrapping_sub(low).wrapping_add(1);
            cases.checked_mul(4)?.checked_add(13 + pad)?
        }
        0xab => word(at + 1 + pad + 4)?
            .checked_mul(8)?
            .checked_add(9 + pad)?,
        _ => 1,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(drop)
    }
    fn u1(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u2(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// What a class does, as the port sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// A block with behaviour of its own (use, ticks, collisions...).
    BlockBehaviour,
    /// A block with a shape of its own: it ports with its template's.
    BlockShape,
    /// An item with behaviour of its own (use, on hit...).
    ItemBehaviour,
    BlockEntity,
    Menu,
    Entity,
    Screen,
    Renderer,
    /// A worldgen feature, placement or structure type written in Java.
    Worldgen,
    /// A recipe type (its recipes don't craft) or ingredient type.
    Recipe,
    Effect,
    Enchantment,
    Particle,
    Command,
    Network,
    /// Reacts to game events (NeoForge's bus, Fabric's callbacks).
    Events,
    /// A Mixin: a patch to the game's own code.
    Mixin,
    /// Extends a game class under obfuscated names (Fabric before 26.1,
    /// Forge's methods): what it does can't be read.
    Unread,
}

impl Feature {
    /// How the coverage line names `n` of them.
    pub fn describe(self, n: usize) -> String {
        let (one, many) = match self {
            Feature::BlockBehaviour => (
                "block with behaviour of its own (use, ticks, collisions)",
                "blocks with behaviour of their own (use, ticks, collisions)",
            ),
            Feature::BlockShape => (
                "block with a shape of its own: it ports with its template's",
                "blocks with shapes of their own: they port with their templates'",
            ),
            Feature::ItemBehaviour => (
                "item with behaviour of its own (use, on hit)",
                "items with behaviour of their own (use, on hit)",
            ),
            Feature::BlockEntity => (
                "block entity (machine, container or animated block)",
                "block entities (machines, containers, animated blocks)",
            ),
            Feature::Menu => ("menu (a container screen)", "menus (container screens)"),
            Feature::Entity => ("mob or entity", "mobs or entities"),
            Feature::Screen => ("screen of its own", "screens of its own"),
            Feature::Renderer => ("renderer or model in code", "renderers or models in code"),
            Feature::Worldgen => (
                "worldgen feature or structure type in code",
                "worldgen features or structure types in code",
            ),
            Feature::Recipe => (
                "recipe type of its own (its recipes don't craft)",
                "recipe types of its own (their recipes don't craft)",
            ),
            Feature::Effect => ("status effect", "status effects"),
            Feature::Enchantment => ("enchantment effect in code", "enchantment effects in code"),
            Feature::Particle => ("particle type", "particle types"),
            Feature::Command => ("command", "commands"),
            Feature::Network => ("network message", "network messages"),
            Feature::Events => ("game event it reacts to", "game events it reacts to"),
            Feature::Mixin => (
                "Mixin (a patch to the game's code)",
                "Mixins (patches to the game's code)",
            ),
            Feature::Unread => (
                "class extending the game under obfuscated names",
                "classes extending the game under obfuscated names",
            ),
        };
        format!("{n} {}", if n == 1 { one } else { many })
    }
}

/// Block methods that only say how it looks or is placed: the template's
/// do the same for the states a blockstate file lists.
const BLOCK_LOOKS: [&str; 18] = [
    "codec",
    "createBlockStateDefinition",
    "getStateForPlacement",
    "rotate",
    "mirror",
    "getRenderShape",
    "skipRendering",
    "getShadeBrightness",
    "propagatesSkylightDown",
    "getCloneItemStack",
    "isPathfindable",
    "getFluidState",
    "updateShape",
    "canBeReplaced",
    "getLightBlock",
    "useShapeForLightOcclusion",
    "getDescriptionId",
    "animateTick",
];
/// Block methods that only change its outline or collision.
const BLOCK_SHAPES: [&str; 6] = [
    "getShape",
    "getCollisionShape",
    "getVisualShape",
    "getOcclusionShape",
    "getInteractionShape",
    "getBlockSupportShape",
];
/// Item methods that only say how it looks or is named.
const ITEM_LOOKS: [&str; 6] = [
    "appendHoverText",
    "getName",
    "isFoil",
    "getDescriptionId",
    "getBarColor",
    "getBarWidth",
];

/// The game's own, loader and library classes: the first of these a mod's
/// class chain reaches is what it extends.
fn external(name: &str, own: &BTreeMap<String, Class>) -> bool {
    !own.contains_key(name)
}

fn obfuscated_method(m: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    m.strip_prefix("method_").is_some_and(digits)
        || m.strip_prefix("m_")
            .and_then(|s| s.strip_suffix('_'))
            .is_some_and(digits)
}

/// What a game class (Mojang's names) is, by its package and name.
fn kind_of_game_class(name: &str) -> Option<Feature> {
    let rest = name.strip_prefix("net/minecraft/")?;
    if rest.starts_with("class_") {
        return Some(Feature::Unread);
    }
    let (package, simple) = rest.rsplit_once('/').unwrap_or(("", rest));
    Some(match package {
        "world/level/block/entity" => Feature::BlockEntity,
        p if p.starts_with("world/level/block") => Feature::BlockBehaviour,
        "world/inventory" if simple.ends_with("Menu") => Feature::Menu,
        p if p.starts_with("world/entity") => Feature::Entity,
        p if p.starts_with("world/item/crafting") => Feature::Recipe,
        p if p.starts_with("world/item/enchantment") => Feature::Enchantment,
        p if p.starts_with("world/item") => Feature::ItemBehaviour,
        p if p.starts_with("world/level/levelgen") => Feature::Worldgen,
        "world/effect" => Feature::Effect,
        p if p.starts_with("client/gui/screens") || p.starts_with("client/gui/components") => {
            Feature::Screen
        }
        p if p.starts_with("client/renderer") || p.starts_with("client/model") => Feature::Renderer,
        p if p.starts_with("client/particle") || p.starts_with("core/particles") => {
            Feature::Particle
        }
        _ => return None,
    })
}

/// Events a content mod uses to register what it has: not behaviour.
fn registration_event(name: &str) -> bool {
    [
        "BuildCreativeModeTabContentsEvent",
        "RegisterEvent",
        "Register",
        "Lifecycle",
        "ModConfigEvent",
        "AddPackFindersEvent",
        "ItemGroupEvents",
        "ClientLifecycleEvents",
        "ServerLifecycleEvents",
    ]
    .iter()
    .any(|k| name.rsplit('/').next().unwrap_or(name).contains(k))
        || name.contains("/lifecycle/")
        || name.contains("/registries/")
        || name.contains("/registry/")
}

/// The code census: how many of each feature, and examples.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Code {
    pub classes: usize,
    pub features: BTreeMap<Feature, usize>,
    /// The game events it subscribes to, by simple name.
    pub events: BTreeSet<String>,
    /// Blocks and items with behaviour, and the methods they override.
    pub examples: BTreeMap<String, Vec<String>>,
}

impl Code {
    /// Whether it does anything beyond registering blocks and items.
    pub fn behaviour(&self) -> bool {
        self.features
            .keys()
            .any(|f| *f != Feature::BlockShape && *f != Feature::Particle)
    }
}

/// Reads every class of a jar. `classes` are the class files' bytes.
pub fn census<'a>(classes: impl IntoIterator<Item = &'a [u8]>) -> Code {
    let own: BTreeMap<String, Class> = classes
        .into_iter()
        .filter_map(parse)
        .map(|c| (c.name.clone(), c))
        .collect();
    let mut code = Code {
        classes: own.len(),
        ..Code::default()
    };
    let mut count = |f: Feature| *code.features.entry(f).or_default() += 1;
    for c in own.values() {
        if c.mixin {
            count(Feature::Mixin);
            continue;
        }
        // The first class outside the jar the chain reaches, and the
        // methods declared along the way.
        let mut methods: BTreeSet<&str> = c.methods.iter().map(String::as_str).collect();
        let mut at = c;
        let mut seen = 0;
        while !external(&at.superclass, &own) && seen < 64 {
            at = &own[&at.superclass];
            methods.extend(at.methods.iter().map(String::as_str));
            seen += 1;
        }
        let base = at.superclass.as_str();
        let kind = kind_of_game_class(base).or_else(|| {
            c.interfaces.iter().find_map(|i| match i.as_str() {
                "net/minecraft/network/protocol/common/custom/CustomPacketPayload" => {
                    Some(Feature::Network)
                }
                "net/minecraft/world/MenuProvider" => Some(Feature::Menu),
                _ => None,
            })
        });
        let Some(kind) = kind else {
            continue;
        };
        let feature = match kind {
            Feature::BlockBehaviour => {
                let behaviour: Vec<&str> = methods
                    .iter()
                    .copied()
                    .filter(|m| !BLOCK_LOOKS.contains(m) && !BLOCK_SHAPES.contains(m))
                    .collect();
                if !behaviour.is_empty() {
                    code.examples.insert(
                        c.name.rsplit('/').next().unwrap_or("").to_owned(),
                        behaviour.iter().map(|m| m.to_string()).collect(),
                    );
                    if behaviour.iter().any(|m| obfuscated_method(m)) {
                        Some(Feature::Unread)
                    } else {
                        Some(Feature::BlockBehaviour)
                    }
                } else if methods.iter().any(|m| BLOCK_SHAPES.contains(m)) {
                    Some(Feature::BlockShape)
                } else {
                    None
                }
            }
            Feature::ItemBehaviour => {
                let behaviour: Vec<&str> = methods
                    .iter()
                    .copied()
                    .filter(|m| !ITEM_LOOKS.contains(m))
                    .collect();
                if behaviour.is_empty() {
                    None
                } else {
                    code.examples.insert(
                        c.name.rsplit('/').next().unwrap_or("").to_owned(),
                        behaviour.iter().map(|m| m.to_string()).collect(),
                    );
                    Some(Feature::ItemBehaviour)
                }
            }
            // A class extending an obfuscated game class without declaring
            // anything adds nothing to read.
            Feature::Unread if methods.is_empty() => None,
            k => Some(k),
        };
        if let Some(f) = feature {
            count(f);
        }
    }
    for c in own.values() {
        for r in &c.refs {
            let event = (r.starts_with("net/neoforged/neoforge/event/")
                || r.starts_with("net/minecraftforge/event/")
                || r.starts_with("net/fabricmc/fabric/api/event/")
                || r.starts_with("net/fabricmc/fabric/api/entity/event/"))
                && !registration_event(r);
            if event {
                let simple = r.rsplit('/').next().unwrap_or(r);
                code.events
                    .insert(simple.split('$').next().unwrap_or(simple).to_owned());
            }
            if r.starts_with("com/mojang/brigadier/")
                && !code.features.contains_key(&Feature::Command)
            {
                code.features.insert(Feature::Command, 1);
            }
        }
    }
    if !code.events.is_empty() {
        code.features.insert(Feature::Events, code.events.len());
    }
    code
}

#[cfg(test)]
pub(crate) mod tests_support {
    /// A class file with `name` extending `superclass`, declaring `methods`
    /// and naming `refs` in its constant pool.
    pub fn class_with(
        name: &str,
        superclass: &str,
        methods: &[&str],
        refs: &[&str],
        mixin: bool,
    ) -> Vec<u8> {
        let mut pool: Vec<Vec<u8>> = Vec::new();
        let utf = |pool: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let mut e = vec![1];
            e.extend((s.len() as u16).to_be_bytes());
            e.extend(s.as_bytes());
            pool.push(e);
            pool.len() as u16
        };
        let class = |pool: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let u = utf(pool, s);
            let mut e = vec![7];
            e.extend(u.to_be_bytes());
            pool.push(e);
            pool.len() as u16
        };
        let this = class(&mut pool, name);
        let sup = class(&mut pool, superclass);
        for r in refs {
            class(&mut pool, r);
        }
        // A long takes two slots.
        pool.push(vec![5, 0, 0, 0, 0, 0, 0, 0, 1]);
        pool.push(Vec::new());
        if mixin {
            utf(&mut pool, "Lorg/spongepowered/asm/mixin/Mixin;");
        }
        let desc = utf(&mut pool, "()V");
        let names: Vec<u16> = methods.iter().map(|m| utf(&mut pool, m)).collect();
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        out.extend(((pool.len() + 1) as u16).to_be_bytes());
        for e in &pool {
            out.extend(e);
        }
        out.extend([0, 0x21]);
        out.extend(this.to_be_bytes());
        out.extend(sup.to_be_bytes());
        out.extend([0, 0]); // interfaces
        out.extend([0, 0]); // fields
        out.extend((names.len() as u16).to_be_bytes());
        for n in names {
            out.extend([0, 1]);
            out.extend(n.to_be_bytes());
            out.extend(desc.to_be_bytes());
            // One attribute, to be skipped.
            out.extend([0, 1, 0, 1, 0, 0, 0, 2, 0, 0]);
        }
        out.extend([0, 0]); // class attributes
        out
    }

    /// A class extending `superclass` and declaring nothing.
    pub fn class_file(name: &str, superclass: &str) -> Vec<u8> {
        class_with(name, superclass, &[], &[], false)
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::class_with as class_file;
    use super::*;

    #[test]
    fn reads_the_header_methods_and_pool() {
        let bytes = class_file(
            "a/Lamp",
            "net/minecraft/world/level/block/Block",
            &["<init>", "useWithoutItem", "lambda$x$0"],
            &["net/minecraft/world/item/Item"],
            false,
        );
        let c = parse(&bytes).unwrap();
        assert_eq!(c.name, "a/Lamp");
        assert_eq!(c.superclass, "net/minecraft/world/level/block/Block");
        assert_eq!(c.methods, ["useWithoutItem"]);
        assert!(c.refs.contains("net/minecraft/world/item/Item"));
        assert!(!c.mixin);
        assert!(parse(&bytes[..bytes.len() - 3]).is_none());
        assert!(parse(b"not a class").is_none());
    }

    #[test]
    fn follows_own_classes_to_the_game() {
        let block = "net/minecraft/world/level/block/Block";
        let files = [
            // Looks only: a template covers it.
            class_file("m/Plain", block, &["getStateForPlacement"], &[], false),
            class_file("m/Round", block, &["getShape"], &[], false),
            // Behaviour two levels down.
            class_file("m/Base", block, &["randomTick"], &[], false),
            class_file("m/Lamp", "m/Base", &[], &[], false),
            class_file(
                "m/Tile",
                "net/minecraft/world/level/block/entity/BlockEntity",
                &[],
                &[],
                false,
            ),
            class_file("m/Mix", "java/lang/Object", &[], &[], true),
            class_file(
                "m/Old",
                "net/minecraft/class_2248",
                &["method_9534"],
                &[],
                false,
            ),
            class_file(
                "m/Events",
                "java/lang/Object",
                &[],
                &[
                    "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent$RightClickBlock",
                    "net/neoforged/neoforge/event/BuildCreativeModeTabContentsEvent",
                    "net/neoforged/neoforge/registries/DeferredRegister",
                ],
                false,
            ),
            class_file(
                "m/Gem",
                "net/minecraft/world/item/Item",
                &["appendHoverText"],
                &[],
                false,
            ),
        ];
        let code = census(files.iter().map(Vec::as_slice));
        assert_eq!(code.classes, 9);
        let n = |f| code.features.get(&f).copied().unwrap_or(0);
        assert_eq!(n(Feature::BlockBehaviour), 2, "{code:?}");
        assert_eq!(n(Feature::BlockShape), 1);
        assert_eq!(n(Feature::BlockEntity), 1);
        assert_eq!(n(Feature::Mixin), 1);
        assert_eq!(n(Feature::Unread), 1);
        assert_eq!(n(Feature::ItemBehaviour), 0);
        assert_eq!(
            code.events.iter().collect::<Vec<_>>(),
            ["PlayerInteractEvent"]
        );
        assert!(code.behaviour());
    }

    #[test]
    fn reads_foods_from_the_static_initializer() {
        let builder = "net/minecraft/world/food/FoodProperties$Builder";
        let food = "Lnet/minecraft/world/food/FoodProperties;";
        let mut pool: Vec<Vec<u8>> = Vec::new();
        let mut add = |e: Vec<u8>| {
            pool.push(e);
            pool.len() as u16
        };
        let utf = |s: &str| [vec![1], (s.len() as u16).to_be_bytes().to_vec(), s.into()].concat();
        let two = |tag: u8, a: u16, b: u16| {
            [vec![tag], a.to_be_bytes().into(), b.to_be_bytes().into()].concat()
        };
        let this = add(utf("m/Foods"));
        let this = add([vec![7], this.to_be_bytes().to_vec()].concat());
        let object = add(utf("java/lang/Object"));
        let object = add([vec![7], object.to_be_bytes().to_vec()].concat());
        let class = add(utf(builder));
        let class = add([vec![7], class.to_be_bytes().to_vec()].concat());
        let mut method = |name: &str| {
            let (n, d) = (add(utf(name)), add(utf("()V")));
            let pair = add(two(12, n, d));
            add(two(10, class, pair))
        };
        let (nutrition, saturation, always) = (
            method("nutrition"),
            method("saturationModifier"),
            method("alwaysEdible"),
        );
        let desc = add(utf(food));
        let mut field = |name: &str| {
            let n = add(utf(name));
            let pair = add(two(12, n, desc));
            add(two(9, this, pair))
        };
        let (cabbage, stew, berry) = (field("CABBAGE"), field("STEW"), field("BERRY"));
        let (n, d) = (add(utf("food")), add(utf(&format!("(IF){food}"))));
        let pair = add(two(12, n, d));
        let helper = add(two(10, this, pair));
        let low = add([vec![4], 0.4f32.to_bits().to_be_bytes().to_vec()].concat());
        let high = add([vec![4], 0.6f32.to_bits().to_be_bytes().to_vec()].concat());
        let (clinit, void, code_name) = (add(utf("<clinit>")), add(utf("()V")), add(utf("Code")));
        let call = |op: u8, i: u16| [vec![op], i.to_be_bytes().to_vec()].concat();
        let mut code = [
            vec![0x05],
            call(0xb6, nutrition),
            vec![0x12, low as u8],
            call(0xb6, saturation),
            call(0xb3, cabbage),
            vec![0x10, 10],
            call(0xb6, nutrition),
            call(0x13, high),
            call(0xb6, saturation),
            call(0xb6, always),
        ]
        .concat();
        code.push(0xaa);
        while code.len() % 4 != 0 {
            code.push(0);
        }
        code.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        code.extend(call(0xb3, stew));
        code.extend([0x06, 0x12, low as u8]);
        code.extend(call(0xb8, helper));
        code.extend(call(0xb3, berry));
        code.push(0xb1);
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        out.extend(((pool.len() + 1) as u16).to_be_bytes());
        pool.iter().for_each(|e| out.extend(e));
        out.extend([0, 0x21]);
        out.extend(this.to_be_bytes());
        out.extend(object.to_be_bytes());
        out.extend([0, 0, 0, 0, 0, 1, 0, 8]);
        out.extend(clinit.to_be_bytes());
        out.extend(void.to_be_bytes());
        out.extend([0, 1]);
        out.extend(code_name.to_be_bytes());
        out.extend((12 + code.len() as u32).to_be_bytes());
        out.extend([0, 2, 0, 0]);
        out.extend((code.len() as u32).to_be_bytes());
        out.extend(&code);
        out.extend([0, 0, 0, 0, 0, 0]);
        let c = parse(&out).unwrap();
        assert_eq!(
            c.foods.get("CABBAGE"),
            Some(&Food {
                nutrition: 2,
                saturation_modifier: 0.4,
                always_edible: false
            })
        );
        assert_eq!(
            c.foods.get("STEW"),
            Some(&Food {
                nutrition: 10,
                saturation_modifier: 0.6,
                always_edible: true
            })
        );
        assert_eq!(c.foods["BERRY"].nutrition, 3);
        assert!(c.methods.is_empty());
    }

    #[test]
    fn registration_only_is_not_behaviour() {
        let files = [
            class_file(
                "m/Mod",
                "java/lang/Object",
                &["register"],
                &[
                    "net/neoforged/neoforge/registries/DeferredRegister",
                    "net/minecraft/world/level/block/Block",
                ],
                false,
            ),
            class_file(
                "m/Pillar",
                "net/minecraft/world/level/block/RotatedPillarBlock",
                &[],
                &[],
                false,
            ),
        ];
        let code = census(files.iter().map(Vec::as_slice));
        assert!(!code.behaviour(), "{code:?}");
    }
}
