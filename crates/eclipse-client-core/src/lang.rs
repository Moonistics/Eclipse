//! Languages (ClientLanguage / LanguageManager): `lang/<code>.json` from the
//! resource root and enabled packs layered over `en_us`, the language list
//! from `pack.mcmeta` `language` sections, and a process-wide current
//! language used by every screen.
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
    pub region: String,
    pub bidirectional: bool,
}
impl LanguageInfo {
    /// LanguageInfo.toComponent: "Name (Region)".
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.region)
    }
}
#[derive(Default)]
pub struct Language {
    pub code: String,
    /// `LanguageInfo.bidirectional`: text without a strong character reads
    /// right to left (`ClientLanguage.defaultRightToLeft`).
    bidirectional: bool,
    map: HashMap<String, String>,
}
pub const DEFAULT: &str = "en_us";

static CURRENT: RwLock<Option<Arc<Language>>> = RwLock::new(None);
/// A language that only the calling thread sees, and the keys it was asked for and did not have
/// (`scoped`).
struct Scope {
    language: Arc<Language>,
    missed: Vec<String>,
}
thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}
/// The current language's `bidirectional`, read for every drawn string.
static BIDIRECTIONAL: AtomicBool = AtomicBool::new(false);

/// A source of resource files: the prepared root plus packs, highest priority last.
pub trait ResourceSource {
    fn read(&self, path: &str) -> Option<Vec<u8>>;
    /// Whether `path` is there, without reading it where the source can tell.
    fn exists(&self, path: &str) -> bool {
        self.read(path).is_some()
    }
    /// The namespaces under `assets/` (resource packs may add their own).
    fn namespaces(&self) -> Vec<String> {
        vec!["minecraft".into()]
    }
}
/// Eclipse's own strings, compiled in so they need no prepared resources.
struct Bundled;
impl ResourceSource for Bundled {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let body = match path {
            "assets/eclipse/lang/en_us.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/en_us.json")
            }
            "assets/eclipse/lang/zh_cn.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/zh_cn.json")
            }
            "assets/eclipse/lang/ru_ru.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ru_ru.json")
            }
            "assets/eclipse/lang/pl_pl.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/pl_pl.json")
            }
            "assets/eclipse/lang/uk_ua.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/uk_ua.json")
            }
            "assets/eclipse/lang/hi_in.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/hi_in.json")
            }
            "assets/eclipse/lang/id_id.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/id_id.json")
            }
            "assets/eclipse/lang/tr_tr.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/tr_tr.json")
            }
            "assets/eclipse/lang/ko_kr.json" | "assets/eclipse/lang/ko.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ko_kr.json")
            }
            "assets/eclipse/lang/hr_hr.json" | "assets/eclipse/lang/hr.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/hr_hr.json")
            }
            "assets/eclipse/lang/pt_pt.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/pt_pt.json")
            }
            "assets/eclipse/lang/ur_pk.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ur_pk.json")
            }
            "assets/eclipse/lang/fa_ir.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fa_ir.json")
            }
            "assets/eclipse/lang/he_il.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/he_il.json")
            }
            "assets/eclipse/lang/hu_hu.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/hu_hu.json")
            }
            "assets/eclipse/lang/ro_ro.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ro_ro.json")
            }
            "assets/eclipse/lang/da_dk.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/da_dk.json")
            }
            "assets/eclipse/lang/fi_fi.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fi_fi.json")
            }
            "assets/eclipse/lang/no_no.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/no_no.json")
            }
            "assets/eclipse/lang/bg_bg.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/bg_bg.json")
            }
            "assets/eclipse/lang/sk_sk.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sk_sk.json")
            }
            "assets/eclipse/lang/lt_lt.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/lt_lt.json")
            }
            "assets/eclipse/lang/it_it.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/it_it.json")
            }
            "assets/eclipse/lang/nl_nl.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/nl_nl.json")
            }
            "assets/eclipse/lang/vi_vn.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/vi_vn.json")
            }
            "assets/eclipse/lang/th_th.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/th_th.json")
            }
            "assets/eclipse/lang/ja_jp.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ja_jp.json")
            }
            "assets/eclipse/lang/ar_sa.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ar_sa.json")
            }
            "assets/eclipse/lang/pt_br.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/pt_br.json")
            }
            "assets/eclipse/lang/cs_cz.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/cs_cz.json")
            }
            "assets/eclipse/lang/sv_se.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sv_se.json")
            }
            "assets/eclipse/lang/el_gr.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/el_gr.json")
            }
            "assets/eclipse/lang/ca_es.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ca_es.json")
            }
            "assets/eclipse/lang/et_ee.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/et_ee.json")
            }
            "assets/eclipse/lang/lv_lv.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/lv_lv.json")
            }
            "assets/eclipse/lang/sl_si.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sl_si.json")
            }
            "assets/eclipse/lang/sr_cs.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sr_cs.json")
            }
            "assets/eclipse/lang/is_is.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/is_is.json")
            }
            "assets/eclipse/lang/af_za.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/af_za.json")
            }
            "assets/eclipse/lang/ms_my.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ms_my.json")
            }
            "assets/eclipse/lang/be_by.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/be_by.json")
            }
            "assets/eclipse/lang/sr_sp.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sr_sp.json")
            }
            "assets/eclipse/lang/az_az.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/az_az.json")
            }
            "assets/eclipse/lang/uz_uz.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/uz_uz.json")
            }
            "assets/eclipse/lang/kk_kz.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/kk_kz.json")
            }
            "assets/eclipse/lang/ka_ge.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ka_ge.json")
            }
            "assets/eclipse/lang/hy_am.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/hy_am.json")
            }
            "assets/eclipse/lang/bs_ba.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/bs_ba.json")
            }
            "assets/eclipse/lang/mk_mk.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/mk_mk.json")
            }
            "assets/eclipse/lang/sq_al.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sq_al.json")
            }
            "assets/eclipse/lang/cy_gb.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/cy_gb.json")
            }
            "assets/eclipse/lang/ga_ie.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ga_ie.json")
            }
            "assets/eclipse/lang/eu_es.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/eu_es.json")
            }
            "assets/eclipse/lang/gl_es.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/gl_es.json")
            }
            "assets/eclipse/lang/fil_ph.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fil_ph.json")
            }
            "assets/eclipse/lang/ta_in.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ta_in.json")
            }
            "assets/eclipse/lang/mn_mn.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/mn_mn.json")
            }
            "assets/eclipse/lang/lo_la.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/lo_la.json")
            }
            "assets/eclipse/lang/so_so.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/so_so.json")
            }
            "assets/eclipse/lang/fy_nl.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fy_nl.json")
            }
            "assets/eclipse/lang/gd_gb.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/gd_gb.json")
            }
            "assets/eclipse/lang/lb_lu.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/lb_lu.json")
            }
            "assets/eclipse/lang/ky_kg.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ky_kg.json")
            }
            "assets/eclipse/lang/kn_in.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/kn_in.json")
            }
            "assets/eclipse/lang/mt_mt.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/mt_mt.json")
            }
            "assets/eclipse/lang/eo_uy.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/eo_uy.json")
            }
            "assets/eclipse/lang/haw_us.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/haw_us.json")
            }
            "assets/eclipse/lang/ig_ng.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ig_ng.json")
            }
            "assets/eclipse/lang/la_la.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/la_la.json")
            }
            "assets/eclipse/lang/tt_ru.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/tt_ru.json")
            }
            "assets/eclipse/lang/yi_de.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/yi_de.json")
            }
            "assets/eclipse/lang/yo_ng.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/yo_ng.json")
            }
            "assets/eclipse/lang/ba_ru.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/ba_ru.json")
            }
            "assets/eclipse/lang/br_fr.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/br_fr.json")
            }
            "assets/eclipse/lang/cv_cu.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/cv_cu.json")
            }
            "assets/eclipse/lang/fo_fo.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fo_fo.json")
            }
            "assets/eclipse/lang/fur_it.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fur_it.json")
            }
            "assets/eclipse/lang/li_li.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/li_li.json")
            }
            "assets/eclipse/lang/lmo.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/lmo.json")
            }
            "assets/eclipse/lang/oc_fr.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/oc_fr.json")
            }
            "assets/eclipse/lang/sah_sah.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/sah_sah.json")
            }
            "assets/eclipse/lang/se_no.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/se_no.json")
            }
            "assets/eclipse/lang/szl.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/szl.json")
            }
            "assets/eclipse/lang/vec_it.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/vec_it.json")
            }
            "assets/eclipse/lang/de_de.json" | "assets/eclipse/lang/de_at.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/de_de.json")
            }
            // Swiss German writes "ss" for "ß", as vanilla's de_ch does.
            "assets/eclipse/lang/de_ch.json" => {
                let de = include_str!("../../eclipse-client/assets/eclipse/lang/de_de.json");
                return Some(de.replace('ß', "ss").into_bytes());
            }
            "assets/eclipse/lang/fr_fr.json"
            | "assets/eclipse/lang/fr_ca.json"
            | "assets/eclipse/lang/fr_ch.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/fr_fr.json")
            }
            "assets/eclipse/lang/es_es.json"
            | "assets/eclipse/lang/es_ar.json"
            | "assets/eclipse/lang/es_cl.json"
            | "assets/eclipse/lang/es_ec.json"
            | "assets/eclipse/lang/es_mx.json"
            | "assets/eclipse/lang/es_uy.json"
            | "assets/eclipse/lang/es_ve.json" => {
                include_str!("../../eclipse-client/assets/eclipse/lang/es_es.json")
            }
            _ => return None,
        };
        Some(body.as_bytes().to_vec())
    }
    fn namespaces(&self) -> Vec<String> {
        vec!["eclipse".into()]
    }
}
impl ResourceSource for std::path::PathBuf {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.join(path)).ok()
    }
    fn exists(&self, path: &str) -> bool {
        self.join(path).is_file()
    }
    fn namespaces(&self) -> Vec<String> {
        std::fs::read_dir(self.join("assets"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_owned))
            .collect()
    }
}

/// DeprecatedTranslationsInfo.applyToMap: drop removed keys, then move each
/// renamed key's text to its new key (a missing old key removes the new one).
pub fn apply_deprecated(map: &mut HashMap<String, String>, json: &[u8]) {
    let Ok(info) = serde_json::from_slice::<serde_json::Value>(json) else {
        return;
    };
    for key in info["removed"].as_array().into_iter().flatten() {
        if let Some(k) = key.as_str() {
            map.remove(k);
        }
    }
    for (from, to) in info["renamed"].as_object().into_iter().flatten() {
        let Some(to) = to.as_str() else { continue };
        match map.remove(from) {
            Some(value) => {
                map.insert(to.to_owned(), value);
            }
            None => {
                map.remove(to);
            }
        }
    }
}

/// `Language.loadFromJson` for one `lang/<code>.json`: its entries go into `map` in file order.
/// A file that is plain JSON reads as before; any other goes through Gson's lenient reader
/// (`#`, `//` and `/* */` comments, unquoted or single-quoted names and values, raw control
/// characters in strings, `=` or `=>` and `;`), which vanilla reads lang files with (expected
/// from `Language.loadFromJson`, Gson). A file that is not JSON even so adds nothing; a value
/// that is not a string, number or boolean (`GsonHelper.convertToString`) ends the file there.
fn read_lang_file(bytes: &[u8], map: &mut HashMap<String, String>) {
    use eclipse_world::datafix::json::{Json, parse_lenient};
    if let Ok(entries) = serde_json::from_slice::<HashMap<String, String>>(bytes) {
        map.extend(entries);
        return;
    }
    let text = String::from_utf8_lossy(bytes);
    // Gson's JsonReader skips a leading byte order mark.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let Some(Json::Obj(entries)) = parse_lenient(&utf16) else {
        return;
    };
    for (key, value) in entries {
        let value = match value {
            Json::Str(s) => String::from_utf16_lossy(&s),
            Json::Num(n) => n,
            Json::Bool(b) => b.to_string(),
            Json::Null | Json::Arr(_) | Json::Obj(_) => break,
        };
        map.insert(String::from_utf16_lossy(&key), value);
    }
}

impl Language {
    /// ClientLanguage.loadFrom: en_us first, then the selected language, each
    /// from every namespace and every source in priority order (later sources
    /// override).
    pub fn load(sources: &[&dyn ResourceSource], code: &str) -> Self {
        let mut map = HashMap::new();
        let mut codes = vec![DEFAULT];
        if code != DEFAULT {
            codes.push(code);
        }
        // Eclipse's own strings first, so a pack or the prepared root can still
        // override them. Vanilla has no key for "Controller vibration".
        let bundled = Bundled;
        let mut all: Vec<&dyn ResourceSource> = vec![&bundled];
        all.extend_from_slice(sources);
        let sources = all.as_slice();
        let namespaces: std::collections::BTreeSet<String> =
            sources.iter().flat_map(|s| s.namespaces()).collect();
        for c in codes {
            for namespace in &namespaces {
                for source in sources {
                    let Some(bytes) = source.read(&format!("assets/{namespace}/lang/{c}.json"))
                    else {
                        continue;
                    };
                    read_lang_file(&bytes, &mut map);
                }
            }
        }
        // DeprecatedTranslationsInfo.loadFromDefaultResource().applyToMap: the
        // game's own deprecated.json, not a resource pack's.
        if let Some(bytes) = sources
            .iter()
            .find_map(|s| s.read("assets/minecraft/lang/deprecated.json"))
        {
            apply_deprecated(&mut map, &bytes);
        }
        let bidirectional = available(sources)
            .iter()
            .any(|info| info.code == code && info.bidirectional);
        Self {
            code: code.to_owned(),
            bidirectional,
            map,
        }
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }
    /// The resolved map, for `eclipse_protocol::text::plain`: a server's
    /// components carry keys, and the network layer resolves them with this.
    pub fn map(&self) -> &HashMap<String, String> {
        &self.map
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Languages declared by `pack.mcmeta` files (the vanilla asset root first).
pub fn available(sources: &[&dyn ResourceSource]) -> Vec<LanguageInfo> {
    let mut found: BTreeMap<String, LanguageInfo> = BTreeMap::new();
    found.insert(
        DEFAULT.into(),
        LanguageInfo {
            code: DEFAULT.into(),
            name: "English".into(),
            region: "US".into(),
            bidirectional: false,
        },
    );
    for source in sources {
        for path in ["assets/pack.mcmeta", "pack.mcmeta"] {
            let Some(bytes) = source.read(path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            for (code, info) in value["language"].as_object().into_iter().flatten() {
                // Only languages whose file exists somewhere are selectable.
                let file = format!("assets/minecraft/lang/{code}.json");
                if code != DEFAULT && !sources.iter().any(|s| s.exists(&file)) {
                    continue;
                }
                found.insert(
                    code.clone(),
                    LanguageInfo {
                        code: code.clone(),
                        name: info["name"].as_str().unwrap_or(code).to_owned(),
                        region: info["region"].as_str().unwrap_or("").to_owned(),
                        bidirectional: info["bidirectional"].as_bool().unwrap_or(false),
                    },
                );
            }
        }
    }
    found.into_values().collect()
}

pub fn set(language: Language) {
    BIDIRECTIONAL.store(language.bidirectional, Ordering::Relaxed);
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(language));
}
pub fn current() -> Option<Arc<Language>> {
    if let Some(language) = SCOPE.with(|s| s.borrow().as_ref().map(|s| s.language.clone())) {
        return Some(language);
    }
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone()
}
/// Runs `f` with `language` as the current language of this thread only, and returns what it
/// returned with the keys `get` and `get_or` looked up and `language` did not have, in the order
/// asked and without repeats. Nothing is set process-wide, so a test can draw a screen in another
/// language while the tests beside it read theirs. Scopes nest: the inner one wins, and the outer
/// one's misses do not include the inner one's. For tests: `bidirectional` stays the process-wide
/// language's.
pub fn scoped<R>(language: Language, f: impl FnOnce() -> R) -> (R, Vec<String>) {
    /// Puts the outer scope back, also when `f` panics.
    struct Restore(Option<Scope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPE.with(|s| *s.borrow_mut() = self.0.take());
        }
    }
    let outer = SCOPE.with(|s| {
        s.borrow_mut().replace(Scope {
            language: Arc::new(language),
            missed: Vec::new(),
        })
    });
    let _restore = Restore(outer);
    let result = f();
    let missed = SCOPE.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map(|s| std::mem::take(&mut s.missed))
    });
    (result, missed.unwrap_or_default())
}
/// `key` was asked for in a `scoped` language that has no text for it.
fn missed(key: &str) {
    SCOPE.with(|s| {
        if let Some(scope) = s.borrow_mut().as_mut()
            && !scope.missed.iter().any(|m| m == key)
        {
            scope.missed.push(key.to_owned());
        }
    });
}
/// `ClientLanguage.defaultRightToLeft` of the current language.
pub fn bidirectional() -> bool {
    BIDIRECTIONAL.load(Ordering::Relaxed)
}
pub fn code() -> String {
    current().map_or_else(|| DEFAULT.to_owned(), |l| l.code.clone())
}
/// Language.getOrDefault: the translation, or the key itself.
pub fn get(key: &str) -> String {
    current()
        .and_then(|l| l.get(key).map(str::to_owned))
        .unwrap_or_else(|| {
            missed(key);
            key.to_owned()
        })
}
/// The translation, or `fallback` when the key is missing.
pub fn get_or(key: &str, fallback: &str) -> String {
    current()
        .and_then(|l| l.get(key).map(str::to_owned))
        .unwrap_or_else(|| {
            missed(key);
            fallback.to_owned()
        })
}
pub fn has(key: &str) -> bool {
    current().is_some_and(|l| l.get(key).is_some())
}
/// I18n.get with plain string arguments.
pub fn tr(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    crate::text::Text::tr(key, args).plain()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mem(HashMap<String, Vec<u8>>);
    impl ResourceSource for Mem {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
    }
    #[test]
    fn bundled_eclipse_strings_follow_the_language() {
        // No prepared resources at all: the namespace is compiled in.
        let empty = Mem(HashMap::new());
        let en = Language::load(&[&empty], "en_us");
        assert_eq!(en.get("eclipse.options.controller"), Some("Controller"));
        let ru = Language::load(&[&empty], "ru_ru");
        assert_eq!(ru.get("eclipse.options.controller"), Some("Геймпад"));
        // Every bundled string carries both languages: a key added to one file
        // and forgotten in the other shows as a key name on screen.
        for key in en.map().keys() {
            assert!(ru.get(key).is_some(), "{key} has no ru_ru value");
        }
        assert_eq!(en.get("eclipse.pages.mods"), Some("Mods"));
        assert_eq!(ru.get("eclipse.pages.mods"), Some("Моды"));
        let zh = Language::load(&[&empty], "zh_cn");
        assert_eq!(zh.get("eclipse.options.controller"), Some("手柄"));
        assert_eq!(zh.get("eclipse.pages.mods"), Some("模组"));
        assert_eq!(zh.get("Preset"), Some("预设"));
        assert_eq!(zh.get("%s chunks"), Some("%s 个区块"));
        // Not every bundle is complete: the en_us layer is in every map, so a
        // missing key falls back to English (`eclipse.mods.not_built` is not
        // in ru_ru either).
        assert_eq!(zh.get("LOD Distance"), Some("LOD 距离"));
        assert_eq!(ru.get("eclipse.mods.not_built"), Some(" (Not built)"));
        let pl = Language::load(&[&empty], "pl_pl");
        assert_eq!(pl.get("eclipse.options.controller"), Some("Kontroler"));
        assert_eq!(pl.get("eclipse.options.vibration"), Some("Wibracje"));
        let uk = Language::load(&[&empty], "uk_ua");
        assert_eq!(
            uk.get("eclipse.options.on_screen_keyboard"),
            Some("Екранна клавіатура")
        );
        let tr = Language::load(&[&empty], "tr_tr");
        assert_eq!(tr.get("eclipse.options.controller"), Some("Kontrolcü"));
        let es = Language::load(&[&empty], "es_es");
        assert_eq!(es.get("eclipse.options.controller"), Some("Mando"));
        // German and French, and their regional variants on the same file
        // (Swiss German without "ß").
        let de = Language::load(&[&empty], "de_de");
        assert_eq!(de.get("eclipse.osk.close"), Some("Schließen"));
        let fr = Language::load(&[&empty], "fr_fr");
        assert_eq!(fr.get("eclipse.osk.close"), Some("Fermer"));
        for (code, base) in [("de_at", &de), ("fr_ca", &fr), ("fr_ch", &fr)] {
            let variant = Language::load(&[&empty], code);
            assert_eq!(variant.map(), base.map(), "{code}");
        }
        let de_ch = Language::load(&[&empty], "de_ch");
        assert_eq!(de_ch.get("eclipse.osk.close"), Some("Schliessen"));
        assert!(de_ch.map().values().all(|v| !v.contains('ß')));
        assert_eq!(de_ch.len(), de.len());
        let es_ve = Language::load(&[&empty], "es_ve");
        assert_eq!(
            es_ve.get("eclipse.options.on_screen_keyboard"),
            Some("Teclado en pantalla")
        );
    }

    #[test]
    fn a_scoped_language_is_the_calling_threads_and_reports_what_it_lacks() {
        let empty = Mem(HashMap::new());
        let outside = code();
        let ((german, english_only, fallback, elsewhere), missed) =
            scoped(Language::load(&[&empty], "de_de"), || {
                (
                    get("eclipse.osk.close"),
                    get("eclipse.no_such_key"),
                    get_or("eclipse.no_fallback", "English"),
                    // Another thread keeps the process-wide language.
                    std::thread::spawn(code).join().unwrap(),
                )
            });
        assert_eq!(german, "Schließen");
        assert_eq!(english_only, "eclipse.no_such_key");
        assert_eq!(fallback, "English");
        assert_eq!(elsewhere, outside);
        assert_eq!(missed, ["eclipse.no_such_key", "eclipse.no_fallback"]);
        // The scope ends with its closure.
        assert_eq!(code(), outside);
    }

    #[test]
    fn scopes_nest_and_end_when_their_closure_panics() {
        let empty = Mem(HashMap::new());
        let outside = code();
        let (inner, missed) = scoped(Language::load(&[&empty], "fr_fr"), || {
            let (inner, inner_missed) = scoped(Language::load(&[&empty], "pl_pl"), || {
                (code(), get("eclipse.inner_miss"))
            });
            assert_eq!(inner_missed, ["eclipse.inner_miss"]);
            assert_eq!(code(), "fr_fr");
            inner.0
        });
        assert_eq!(inner, "pl_pl");
        assert!(missed.is_empty());
        let panicked = std::panic::catch_unwind(|| {
            scoped(Language::load(&[&Mem(HashMap::new())], "de_de"), || {
                panic!("a test failed inside a scope")
            })
        });
        assert!(panicked.is_err());
        assert_eq!(code(), outside);
    }

    /// `crates/eclipse-client/assets/eclipse/lang`, where Eclipse's own strings live.
    fn bundled_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../eclipse-client/assets/eclipse/lang")
    }
    /// The keys of a flat bundled file in file order, duplicates included (a map
    /// would keep only the last of two equal keys). One `"key": "value"` per line.
    fn file_keys(body: &str) -> Vec<String> {
        body.lines()
            .filter(|line| line.starts_with("  \""))
            .map(|line| {
                serde_json::Deserializer::from_str(line.trim_start())
                    .into_iter::<String>()
                    .next()
                    .and_then(Result::ok)
                    .unwrap_or_else(|| panic!("no key on line {line}"))
            })
            .collect()
    }
    /// The fields the code fills in (`%s`, `%d`, `%1$s`, `%.2f`, `%%`), sorted. A
    /// percent sign that starts none of them ("100% is") is plain text.
    fn placeholders(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find('%') {
            rest = &rest[at + 1..];
            let spec = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '$' || c == '.'))
                .unwrap_or(rest.len());
            let after = rest[spec..].chars().next();
            if after.is_some_and(|c| matches!(c, 's' | 'd' | 'f' | '%')) {
                let end = spec + 1;
                found.push(format!("%{}", &rest[..end]));
                rest = &rest[end..];
            }
        }
        found.sort();
        found
    }

    #[test]
    fn bundled_files_are_served_and_have_no_stale_or_repeated_keys() {
        let dir = bundled_dir();
        let en_body = std::fs::read_to_string(dir.join("en_us.json")).unwrap();
        let en: HashMap<String, String> = serde_json::from_str(&en_body).unwrap();
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = entry.file_name().into_string().unwrap();
            let Some(code) = name.strip_suffix(".json") else {
                continue;
            };
            seen += 1;
            // A file that Bundled::read does not name is never loaded.
            let path = format!("assets/eclipse/lang/{name}");
            assert!(
                Bundled.read(&path).is_some(),
                "{name} is not served by Bundled"
            );
            let body = std::fs::read_to_string(entry.path()).unwrap();
            let map: HashMap<String, String> = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("{name} is not a flat string map: {e}"));
            let keys = file_keys(&body);
            assert_eq!(
                keys.len(),
                map.len(),
                "{name} repeats a key or has a multi-line entry"
            );
            for (key, value) in &map {
                // Stale: the code no longer asks for it, or it was renamed in en_us only.
                let english = en
                    .get(key)
                    .unwrap_or_else(|| panic!("{code}: {key} is not in en_us.json"));
                // A translation keeps the fields the code fills in.
                assert_eq!(
                    placeholders(value),
                    placeholders(english),
                    "{code}: {key} changes the placeholders of the English text"
                );
                assert!(!value.trim().is_empty(), "{code}: {key} is empty");
            }
        }
        assert!(
            seen >= 7,
            "found {seen} bundled lang files in {}",
            dir.display()
        );
    }

    #[test]
    fn every_eclipse_key_the_code_names_is_in_en_us() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    rust_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let en: HashMap<String, String> = serde_json::from_str(
            &std::fs::read_to_string(bundled_dir().join("en_us.json")).unwrap(),
        )
        .unwrap();
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        for krate in [
            "eclipse-client",
            "eclipse-client-render",
            "eclipse-client-core",
        ] {
            rust_files(&crates.join(krate).join("src"), &mut files);
        }
        assert!(files.len() > 100, "found only {} sources", files.len());
        let mut found = 0;
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for (at, _) in text.match_indices("\"eclipse.") {
                let rest = &text[at + 1..];
                let Some(end) = rest.find('"') else { continue };
                let key = &rest[..end];
                // A key is `eclipse.<group>.<name>`: lower-case letters, digits and `_`
                // between the dots. A key built at run time has a brace; a file name
                // has one dot; the server addresses that look like keys are listed.
                const NOT_KEYS: &[&str] = &["eclipse.bunchy7s.dev"];
                let key_like = key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.".contains(&b));
                if key.matches('.').count() < 2 || !key_like || NOT_KEYS.contains(&key) {
                    continue;
                }
                found += 1;
                assert!(
                    en.contains_key(key),
                    "{} names {key}, which is not in en_us.json",
                    file.display()
                );
            }
        }
        assert!(found > 50, "found only {found} key names");
    }

    #[test]
    fn layers_selected_language_over_english() {
        let base = Mem(HashMap::from([
            ("assets/minecraft/lang/en_us.json".into(), br#"{"a":"Apple","b":"Banana"}"#.to_vec()),
            ("assets/minecraft/lang/fr_fr.json".into(), br#"{"a":"Pomme"}"#.to_vec()),
            ("assets/pack.mcmeta".into(), r#"{"language":{"fr_fr":{"name":"Français","region":"France"},"de_de":{"name":"Deutsch","region":"Deutschland"}}}"#.as_bytes().to_vec()),
        ]));
        let pack = Mem(HashMap::from([(
            "assets/minecraft/lang/fr_fr.json".into(),
            br#"{"b":"Banane"}"#.to_vec(),
        )]));
        let sources: [&dyn ResourceSource; 2] = [&base, &pack];
        let fr = Language::load(&sources, "fr_fr");
        assert_eq!(fr.get("a"), Some("Pomme"));
        assert_eq!(fr.get("b"), Some("Banane"));
        let langs = available(&sources);
        // de_de has no file, so only English and French are offered.
        assert_eq!(
            langs.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(),
            ["en_us", "fr_fr"]
        );
        assert_eq!(langs[1].label(), "Français (France)");
    }

    #[test]
    fn languages_are_listed_without_reading_their_files() {
        struct NoLangReads;
        impl ResourceSource for NoLangReads {
            fn read(&self, path: &str) -> Option<Vec<u8>> {
                assert!(!path.contains("/lang/"), "read {path}");
                (path == "assets/pack.mcmeta").then(|| {
                    br#"{"language":{"fr_fr":{"name":"French"},"de_de":{"name":"German"}}}"#
                        .to_vec()
                })
            }
            fn exists(&self, path: &str) -> bool {
                path == "assets/minecraft/lang/fr_fr.json"
            }
        }
        let langs = available(&[&NoLangReads]);
        assert_eq!(
            langs.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(),
            ["en_us", "fr_fr"]
        );
    }

    fn read(body: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        read_lang_file(body.as_bytes(), &mut map);
        map
    }
    fn entries(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_plain_json_lang_file_reads_as_before() {
        let body = "{\"a\": \"Apple \\\"quoted\\\" \\u00e9\", \"b\": \"\", \"a\": \"Again\"}";
        let strict: HashMap<String, String> = serde_json::from_str(body).unwrap();
        assert_eq!(read(body), strict);
        assert_eq!(read(body), entries(&[("a", "Again"), ("b", "")]));
    }

    #[test]
    fn lang_files_read_with_comments() {
        let want = entries(&[("a", "Apple"), ("b", "Banana # not a comment // either")]);
        for body in [
            "# header\n{\n# a comment\n\"a\": \"Apple\",\n\"b\": \"Banana # not a comment // either\"\n}\n",
            "// header\n{\"a\": \"Apple\", // after\n\"b\": \"Banana # not a comment // either\"}",
            "/* header\n over lines */{\"a\": /* inline */ \"Apple\", \"b\": \"Banana # not a comment // either\"}",
        ] {
            assert_eq!(read(body), want, "{body}");
        }
    }

    #[test]
    fn lang_files_read_unquoted_and_single_quoted_names_and_values() {
        assert_eq!(
            read("{a: 'Apple', 'b': \"Banana\", c: Cherry}"),
            entries(&[("a", "Apple"), ("b", "Banana"), ("c", "Cherry")])
        );
        // A single-quoted string may hold a double quote, and the reverse.
        assert_eq!(
            read("{'a': 'say \"hi\"', \"b\": \"it's\"}"),
            entries(&[("a", "say \"hi\""), ("b", "it's")])
        );
    }

    #[test]
    fn lang_files_read_raw_control_characters_in_strings() {
        assert_eq!(
            read("{\"a\": \"tab\there\", \"b\": \"two\nlines\"}"),
            entries(&[("a", "tab\there"), ("b", "two\nlines")])
        );
    }

    #[test]
    fn lang_files_read_gson_separators_and_a_byte_order_mark() {
        assert_eq!(
            read("\u{feff}{\"a\" = \"Apple\"; \"b\" => \"Banana\"}"),
            entries(&[("a", "Apple"), ("b", "Banana")])
        );
        // A trailing comma before `}` is not JSON even to Gson's lenient reader.
        assert_eq!(read("{# c\n\"a\": \"Apple\",}"), HashMap::new());
    }

    #[test]
    fn numbers_and_booleans_read_as_their_text() {
        // GsonHelper.convertToString: a primitive reads as its text.
        assert_eq!(
            read("{\"a\": 1.50, \"b\": true, \"c\": -2e3}"),
            entries(&[("a", "1.50"), ("b", "true"), ("c", "-2e3")])
        );
    }

    #[test]
    fn a_value_that_is_no_text_ends_the_file_there() {
        assert_eq!(
            read("# c\n{\"a\": \"Apple\", \"b\": [\"x\"], \"c\": \"Cherry\"}"),
            entries(&[("a", "Apple")])
        );
        assert_eq!(read("{\"a\": null, \"b\": \"Banana\"}"), HashMap::new());
    }

    #[test]
    fn a_lang_file_that_is_not_json_adds_nothing() {
        for body in [
            "# c\n{\"a\": \"Apple\"",
            "[\"a\"]",
            "\"a\"",
            "",
            "{\"a\": \"Apple\"} {",
        ] {
            assert_eq!(read(body), HashMap::new(), "{body:?}");
        }
    }

    #[test]
    fn a_pack_lang_file_with_comments_layers_over_english() {
        let base = Mem(HashMap::from([(
            "assets/minecraft/lang/en_us.json".into(),
            br#"{"a":"Apple","b":"Banana"}"#.to_vec(),
        )]));
        let pack = Mem(HashMap::from([(
            "assets/minecraft/lang/fr_fr.json".into(),
            b"{\n# Fruits\n\"a\": \"Pomme\"\n}\n".to_vec(),
        )]));
        let sources: [&dyn ResourceSource; 2] = [&base, &pack];
        let fr = Language::load(&sources, "fr_fr");
        assert_eq!(fr.get("a"), Some("Pomme"));
        assert_eq!(fr.get("b"), Some("Banana"));
    }

    /// Every lang file of a downloaded pack, e.g. BACAP Language Pack:
    /// `ECLIPSE_LANG_PACK_DIR=<pack>/assets/minecraft/lang cargo test -p eclipse-client-core
    /// lang::tests::every_lang_file_of_a_pack_reads -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a downloaded pack in ECLIPSE_LANG_PACK_DIR"]
    fn every_lang_file_of_a_pack_reads() {
        let Ok(dir) = std::env::var("ECLIPSE_LANG_PACK_DIR") else {
            return;
        };
        let mut failed = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            let bytes = std::fs::read(&path).unwrap();
            let strict = serde_json::from_slice::<HashMap<String, String>>(&bytes).is_ok();
            let mut map = HashMap::new();
            read_lang_file(&bytes, &mut map);
            // An independent count: the distinct `"name":` that start a line.
            let text = String::from_utf8_lossy(&bytes);
            let names: std::collections::HashSet<&str> = text
                .lines()
                .filter_map(|line| {
                    let rest = line.trim_start().strip_prefix('"')?;
                    let mut escaped = false;
                    let end = rest.char_indices().find_map(|(i, c)| {
                        let close = c == '"' && !escaped;
                        escaped = c == '\\' && !escaped;
                        close.then_some(i)
                    })?;
                    rest[end + 1..]
                        .trim_start()
                        .starts_with(':')
                        .then(|| &rest[..end])
                })
                .collect();
            println!(
                "{}: strict {strict}, {} keys, {} names",
                path.display(),
                map.len(),
                names.len()
            );
            if map.is_empty() || map.len() != names.len() {
                failed.push(path);
            }
        }
        assert!(failed.is_empty(), "{failed:?}");
    }
}
