//! What the browser shows beyond a search hit: a project's page (description, gallery, links,
//! authors) and the search's filters (categories, loader, game version, environment, sort), with
//! the lists of values each source offers for them. Data only: `mods_page` draws it.
use super::{Kind, GAME};

/// How results are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl Sort {
    pub const ALL: [Sort; 5] = [
        Sort::Relevance,
        Sort::Downloads,
        Sort::Follows,
        Sort::Newest,
        Sort::Updated,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Sort::Relevance => "Relevance",
            Sort::Downloads => "Downloads",
            Sort::Follows => "Follows",
            Sort::Newest => "Newest",
            Sort::Updated => "Updated",
        }
    }
    /// Modrinth's `index`.
    pub fn modrinth(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::Follows => "follows",
            Sort::Newest => "newest",
            Sort::Updated => "updated",
        }
    }
}

/// Where a project runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Environment {
    Client,
    Server,
}

impl Environment {
    pub const ALL: [Environment; 2] = [Environment::Client, Environment::Server];
    pub fn label(self) -> &'static str {
        match self {
            Environment::Client => "Client",
            Environment::Server => "Server",
        }
    }
}

/// A search's filters. Empty: the browser's defaults (what runs in Eclipse).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Filters {
    /// Every one of these categories (`decoration`, `worldgen`...).
    pub categories: Vec<String>,
    /// Every one of these tags: Eclipse's registry only, a project's own words (`zoom`, `pvp`...).
    pub tags: Vec<String>,
    /// One loader (`neoforge`, `fabric`...), in place of the kind's defaults.
    pub loader: Option<String>,
    /// One Minecraft version, in place of the one Eclipse plays.
    pub game_version: Option<String>,
    pub environment: Option<Environment>,
    /// None: by relevance with search text, else by downloads.
    pub sort: Option<Sort>,
}

impl Filters {
    /// The sort a search uses.
    pub fn sort(&self, text: &str) -> Sort {
        self.sort.unwrap_or(if text.is_empty() {
            Sort::Downloads
        } else {
            Sort::Relevance
        })
    }
    /// A game version other than the one Eclipse plays was asked for.
    pub fn other_version(&self) -> bool {
        self.game_version.as_deref().is_some_and(|v| v != GAME)
    }
}

/// One value a filter can take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    /// The group a category is in on its source (`categories`, `features`, `resolutions`).
    pub header: String,
}

/// The values each filter can take for a kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tags {
    pub categories: Vec<Tag>,
    /// Eclipse's registry only: the tags its projects of the kind carry, the most used first.
    pub tags: Vec<String>,
    pub loaders: Vec<String>,
    /// Release versions, newest first.
    pub game_versions: Vec<String>,
}

/// A category as the registry's website named it (`categoryLabel` in its `mods-logic.ts`):
/// `game-mechanics` reads "Game mechanics", `gui` "GUI". English: the launcher translates the
/// names its tables have.
pub fn category_label(slug: &str) -> String {
    let words = match slug {
        "gui" => return "GUI".into(),
        "pbr" => return "PBR".into(),
        "worldgen" => return "World generation".into(),
        "semi-realistic" => return "Semi-realistic".into(),
        "vanilla-like" => return "Vanilla-like".into(),
        "core-shaders" => return "Core shaders".into(),
        _ => slug.replace('-', " "),
    };
    crate::i18n::capitalised(&words)
}

/// A gallery image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub url: String,
    pub title: String,
    pub description: String,
    pub featured: bool,
}

/// A link a project gives: its source, issues, wiki, Discord, donations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

/// A project's GitHub repository and how many stars it has: a registry project whose source is
/// on GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHub {
    /// `https://github.com/<owner>/<repo>`.
    pub url: String,
    pub stars: u64,
}

/// A project's page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    /// Markdown (Modrinth's body; HTML blocks in it are left as text the renderer skips).
    pub body: String,
    pub gallery: Vec<Image>,
    pub links: Vec<Link>,
    pub authors: Vec<String>,
    /// Modrinth's followers; a registry project has none (it shows [`Details::github`] instead).
    pub followers: u64,
    /// Where to star a registry project, and its stars there.
    pub github: Option<GitHub>,
    pub published: String,
    pub updated: String,
    pub categories: Vec<String>,
    /// `required`, `optional`, `unsupported` or `unknown`.
    pub client_side: String,
    pub server_side: String,
}

/// The loaders a kind lists by default, when no loader is picked: Java mod loaders for mods.
pub(crate) fn default_loaders(kind: Kind) -> Vec<&'static str> {
    match kind {
        Kind::Mod => super::java::LOADERS.to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_read_as_the_site_names_them() {
        assert_eq!(category_label("game-mechanics"), "Game mechanics");
        assert_eq!(category_label("kitchen-sink"), "Kitchen sink");
        assert_eq!(category_label("worldgen"), "World generation");
        assert_eq!(category_label("vanilla-like"), "Vanilla-like");
        assert_eq!(category_label("gui"), "GUI");
        assert_eq!(category_label("16x"), "16x");
        assert_eq!(category_label(""), "");
    }

    #[test]
    fn sorts_default_to_relevance_with_text() {
        let f = Filters::default();
        assert_eq!(f.sort(""), Sort::Downloads);
        assert_eq!(f.sort("fences"), Sort::Relevance);
        let f = Filters {
            sort: Some(Sort::Updated),
            game_version: Some("1.21.1".into()),
            ..Filters::default()
        };
        assert_eq!(f.sort("x"), Sort::Updated);
        assert!(f.other_version());
    }
}
