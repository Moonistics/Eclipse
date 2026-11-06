//! The launcher's languages: English (the strings in the code) and the ones `languages!` lists. A
//! table per language (`launcher/lang/<code>.json`) maps each English string to its translation; a
//! string the table lacks shows in English.
//!
//! The language follows the system's unless the player picks one in Settings
//! (`Settings::language`). Translations are Eclipse's own, written for this launcher.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

macro_rules! languages {
    ($($language:ident $code:literal $name:literal,)*) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Language {
            #[default]
            #[serde(rename = "en")]
            English,
            $(#[serde(rename = $code)] $language,)*
        }

        impl Language {
            pub const ALL: [Language; 1 + [$($code),*].len()] = [Self::English, $(Self::$language),*];

            /// Its code in `settings.json` and on the command line.
            pub fn code(self) -> &'static str {
                match self {
                    Self::English => "en",
                    $(Self::$language => $code,)*
                }
            }

            /// Its name in itself: what a player looks for in a list of languages.
            pub fn name(self) -> &'static str {
                match self {
                    Self::English => "English",
                    $(Self::$language => $name,)*
                }
            }

            /// The table, as shipped: English text to this language's.
            fn source(self) -> &'static str {
                match self {
                    Self::English => "{}",
                    $(Self::$language => include_str!(concat!("../lang/", $code, ".json")),)*
                }
            }
        }
    };
}

languages! {
    French "fr" "Français",
    Russian "ru" "Русский",
    German "de" "Deutsch",
    Chinese "zh" "简体中文",
    Spanish "es" "Español",
    Ukrainian "uk" "Українська",
    Polish "pl" "Polski",
    PortugueseBrazil "pt-br" "Português (Brasil)",
    Japanese "ja" "日本語",
    Turkish "tr" "Türkçe",
    Korean "ko" "한국어",
    Croatian "hr" "Hrvatski",
    Arabic "ar" "العربية",
    Hindi "hi" "हिन्दी",
    Indonesian "id" "Bahasa Indonesia",
    Portuguese "pt" "Português (Portugal)",
    Hebrew "he" "עברית",
    Hungarian "hu" "Magyar",
    Romanian "ro" "Română",
    Danish "da" "Dansk",
    Finnish "fi" "Suomi",
    Norwegian "no" "Norsk",
    Bulgarian "bg" "Български",
    Slovak "sk" "Slovenčina",
    Lithuanian "lt" "Lietuvių",
    Italian "it" "Italiano",
    Dutch "nl" "Nederlands",
    Vietnamese "vi" "Tiếng Việt",
    Thai "th" "ไทย",
    Urdu "ur" "اردو",
    Czech "cs" "Čeština",
    Swedish "sv" "Svenska",
    Greek "el" "Ελληνικά",
    Persian "fa" "فارسی",
    Catalan "ca" "Català",
    Estonian "et" "Eesti",
    Latvian "lv" "Latviešu",
    Slovenian "sl" "Slovenščina",
    Serbian "sr" "Српски",
    Icelandic "is" "Íslenska",
    Afrikaans "af" "Afrikaans",
    Malay "ms" "Bahasa Melayu",
    Belarusian "be" "Беларуская",
    Azerbaijani "az" "Azərbaycanca",
    Uzbek "uz" "Oʻzbekcha",
    Kazakh "kk" "Қазақша",
    Georgian "ka" "ქართული",
    Armenian "hy" "Հայերեն",
    Bosnian "bs" "Bosanski",
    Macedonian "mk" "Македонски",
    Albanian "sq" "Shqip",
    Welsh "cy" "Cymraeg",
    Irish "ga" "Gaeilge",
    Basque "eu" "Euskara",
    Galician "gl" "Galego",
    Filipino "fil" "Filipino",
    Tamil "ta" "தமிழ்",
    Mongolian "mn" "Монгол",
    Lao "lo" "ລາວ",
    Somali "so" "Soomaali",
    Frisian "fy" "Frysk",
    ScottishGaelic "gd" "Gàidhlig",
    Luxembourgish "lb" "Lëtzebuergesch",
    Kyrgyz "ky" "Кыргызча",
    Kannada "kn" "ಕನ್ನಡ",
    Maltese "mt" "Malti",
    Esperanto "eo" "Esperanto",
    Hawaiian "haw" "ʻŌlelo Hawaiʻi",
    Igbo "ig" "Igbo",
    Latin "la" "Latina",
    Tatar "tt" "Татарча",
    Yiddish "yi" "ייִדיש",
    Yoruba "yo" "Yorùbá",
    Zulu "zu" "isiZulu",
    Bashkir "ba" "Башҡортса",
    Breton "br" "Brezhoneg",
    Chuvash "cv" "Чӑвашла",
    Faroese "fo" "Føroyskt",
    Friulian "fur" "Furlan",
    Limburgish "li" "Limburgs",
    Lombard "lmo" "Lombard",
    Occitan "oc" "Occitan",
    Sakha "sah" "Саха тыла",
    NorthernSami "se" "Davvisámegiella",
    Silesian "szl" "Ślōnskŏ",
    Venetian "vec" "Vèneto",
}

impl Language {
    /// Parses a language code (e.g. `fr`, `fr-CH`, `pt-BR`, or `pt_BR.UTF-8`)
    /// if the launcher supports it.
    ///
    /// Underscores are normalized to hyphens to support locale formats commonly
    /// returned by operating systems. Brazilian Portuguese (`pt-BR`) is matched
    /// explicitly, while other Portuguese variants (such as `pt-PT`) are not
    /// mapped to it.
    pub fn parse(code: &str) -> Option<Self> {
        let lower = code.trim().to_ascii_lowercase();
        let clean = lower.split(['.', '@']).next().unwrap_or("");
        let normalized = clean.replace('_', "-");

        if normalized == "pt-br" {
            return Some(Self::PortugueseBrazil);
        }

        let base = match normalized.split('-').next().unwrap_or("") {
            // Systems name Norwegian by its written form: Bokmål (`nb-NO`, what macOS and Windows
            // report) or Nynorsk (`nn`). The launcher's one table is Bokmål.
            "nb" | "nn" => "no",
            base => base,
        };

        Self::ALL
            .into_iter()
            .find(|language| language.code() == base)
    }

    /// The first of the system's preferred languages the launcher has, else English.
    pub fn pick<'a>(preferred: impl IntoIterator<Item = &'a str>) -> Self {
        preferred
            .into_iter()
            .find_map(Self::parse)
            .unwrap_or_default()
    }

    /// The system's language, as far as the launcher has it.
    pub fn system() -> Self {
        let locales: Vec<String> = sys_locale::get_locales().collect();
        Self::pick(locales.iter().map(String::as_str))
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Shows the launcher in `language` from now on.
pub fn set(language: Language) {
    let i = Language::ALL
        .iter()
        .position(|l| *l == language)
        .unwrap_or(0);
    CURRENT.store(i as u8, Ordering::Relaxed);
}

pub fn current() -> Language {
    Language::ALL[CURRENT.load(Ordering::Relaxed) as usize % Language::ALL.len()]
}

type Table = HashMap<String, &'static str>;

fn table(language: Language) -> &'static Table {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    let tables = TABLES.get_or_init(|| {
        Language::ALL
            .iter()
            .map(|l| {
                serde_json::from_str::<HashMap<String, String>>(l.source())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(en, said)| (en, &*Box::leak(said.into_boxed_str())))
                    .collect()
            })
            .collect()
    });
    let i = Language::ALL
        .iter()
        .position(|l| *l == language)
        .unwrap_or(0);
    &tables[i]
}

/// `en` in `language`, or `en` itself where the table lacks it.
pub fn tr_in(language: Language, en: &str) -> &str {
    table(language).get(en).copied().unwrap_or(en)
}

/// `en` in the launcher's language.
pub fn tr(en: &str) -> &str {
    tr_in(current(), en)
}

/// `en` in the launcher's language with its `{name}` holes filled.
pub fn trf(en: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(en).to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
#[path = "i18n/tests.rs"]
mod tests;
