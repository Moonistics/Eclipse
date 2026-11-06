use super::*;

#[derive(Default)]
pub(super) struct NativeBuild {
    pub(super) step: Option<String>,
    pub(super) started: Option<(Instant, u64)>,
    pub(super) done: Option<String>,
    pub(super) error: Option<String>,
    pub(super) confirm: Option<Result<Vec<native_mods::Bundle>, String>>,
    pub(super) for_instance: Option<String>,
    pub(super) update: Option<String>,
    pub(super) on_base: bool,
    pub(super) builds: std::cell::RefCell<HashMap<String, Option<native_status::Build>>>,
    pub(super) mismatch: Option<String>,
}
