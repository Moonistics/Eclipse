//! Edit instance's launch options (`eclipse_launcher::launch`): a command before the game, a
//! wrapper, environment variables, a command after it, the Linux switches and, for an Eclipse
//! instance, its own game arguments. Save changes keeps them with the rest of the page.
use super::fields::Field;
use super::*;
use crate::ui;
use eclipse_launcher::instances::Instance;
use eclipse_launcher::launch;
use eclipse_theme::glass::space;
use instance_state::NewKind;

pub(super) struct LaunchPanel {
    /// The instance whose options the fields hold, as Edit opened it.
    instance: String,
    switches: Switches,
    /// What this computer has for the Linux switches, looked at when Edit opens.
    found: launch::Found,
    /// Why Save refused the environment variables.
    env_error: Option<String>,
    reveal: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Switches {
    gamemode: bool,
    mangohud: bool,
    dedicated_gpu: bool,
}

impl Switches {
    fn of(launch: &launch::Launch) -> Self {
        Switches {
            gamemode: launch.gamemode,
            mangohud: launch.mangohud,
            dedicated_gpu: launch.dedicated_gpu,
        }
    }
}

/// What saving the memory slider did, kept to show under the slider in the active language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MemorySaved {
    /// Saved Java heap size in MB.
    Ok(u32),
    /// Saving failed on disk.
    Error(String),
}

/// Edit instance's memory row. What it shows comes from the instance, except during a drag.
#[derive(Default)]
pub(super) struct MemorySlider {
    /// The computer's memory in MiB, read once: the slider's top.
    total: Option<u32>,
    /// The Java heap size being dragged to.
    dragging: Option<u32>,
    /// What the last change did, or why not.
    saved: Option<MemorySaved>,
    /// The instance whose value is being typed: saved to it when the box loses the keyboard, even
    /// if that's because another instance was picked.
    typing: Option<String>,
    /// The instance `saved` is about.
    shown: Option<String>,
}

impl MemorySlider {
    fn total(&mut self) -> u32 {
        *self
            .total
            .get_or_insert_with(crate::memory::total_memory_mb)
    }
}

/// A Linux switch: which one, its label, and what it does.
#[derive(Clone, Copy, Debug)]
enum Toggle {
    GameMode,
    MangoHud,
    Gpu,
}

impl Launcher {
    fn load_launch_options(&mut self, instance: &Instance, reveal: bool) {
        let l = &instance.launch;
        self.fields.text(Field::PreLaunch).set(l.pre_launch.clone());
        self.fields
            .text(Field::LaunchWrapper)
            .set(l.wrapper.clone());
        self.fields
            .text(Field::LaunchEnv)
            .set(launch::env_line(&l.env));
        self.fields.text(Field::PostExit).set(l.post_exit.clone());
        self.fields
            .text(Field::InstanceGameArgs)
            .set(instance.game_args.clone().unwrap_or_default());
        self.fields.set_field_invalid(Field::LaunchEnv, false);
        self.launch = Some(LaunchPanel {
            instance: instance.id.clone(),
            switches: Switches::of(l),
            found: launch::Found::here(),
            env_error: None,
            reveal,
        });
    }

    pub(super) fn open_at_launch_options(&mut self, instance: &Instance) {
        self.load_launch_options(instance, true);
        self.focus_page = false;
        self.fields.focus_soon = Some(Field::PreLaunch);
    }

    pub(super) fn sync_launch_options(&mut self, editing: &Instance, opening: bool) {
        let filled = self
            .launch
            .as_ref()
            .filter(|p| p.instance == editing.id)
            .is_some_and(|p| p.reveal || !opening);
        if !filled {
            self.load_launch_options(editing, false);
        }
    }

    pub(super) fn reveal_launch_options(&mut self, settled: bool) -> bool {
        let Some(panel) = &mut self.launch else {
            return false;
        };
        let reveal = panel.reveal;
        panel.reveal &= !settled;
        reveal
    }

    fn edits_game_args(&self) -> bool {
        self.instances
            .draft
            .as_ref()
            .is_some_and(|d| d.java.kind == NewKind::Eclipse)
    }

    fn typed(&self, f: Field) -> &str {
        self.fields.value(f).trim()
    }

    fn launch_instance(&self) -> Option<&Instance> {
        let id = &self.launch.as_ref()?.instance;
        let editing = self.instances.draft.as_ref()?.editing.as_ref()?;
        (id == editing)
            .then(|| self.instances.list.iter().find(|i| &i.id == id))
            .flatten()
    }

    pub(super) fn launch_options_changed(&self) -> bool {
        let (Some(panel), Some(i)) = (&self.launch, self.launch_instance()) else {
            return false;
        };
        let l = &i.launch;
        self.typed(Field::PreLaunch) != l.pre_launch.trim()
            || self.typed(Field::LaunchWrapper) != l.wrapper.trim()
            || self.typed(Field::LaunchEnv) != launch::env_line(&l.env)
            || self.typed(Field::PostExit) != l.post_exit.trim()
            || panel.switches != Switches::of(l)
            || (self.edits_game_args()
                && self.typed(Field::InstanceGameArgs) != i.game_args.as_deref().unwrap_or(""))
    }

    pub(super) fn save_launch_options(&mut self) -> bool {
        if !self.launch_options_changed() {
            return true;
        }
        let Some(panel) = &self.launch else {
            return true;
        };
        let (id, switches) = (panel.instance.clone(), panel.switches);
        let env = match launch::parse_env(self.fields.value(Field::LaunchEnv)) {
            Ok(env) => env,
            Err(e) => {
                self.fields.set_field_invalid(Field::LaunchEnv, true);
                if let Some(panel) = &mut self.launch {
                    panel.env_error = Some(e);
                }
                return false;
            }
        };
        let pre_launch = self.typed(Field::PreLaunch).to_string();
        let wrapper = self.typed(Field::LaunchWrapper).to_string();
        let post_exit = self.typed(Field::PostExit).to_string();
        let args = self.edits_game_args().then(|| {
            Some(self.typed(Field::InstanceGameArgs).to_string()).filter(|a| !a.is_empty())
        });
        let saved = instances::edit(&id, |m| {
            m.launch = launch::Launch {
                pre_launch,
                wrapper,
                env,
                post_exit,
                gamemode: switches.gamemode,
                mangohud: switches.mangohud,
                dedicated_gpu: switches.dedicated_gpu,
            };
            if let Some(args) = args {
                m.game_args = args;
            }
        });
        match saved {
            Ok(_) => {
                self.reload_instances();
                true
            }
            Err(e) => {
                if let Some(d) = &mut self.instances.draft {
                    d.error = Some(e);
                }
                false
            }
        }
    }

    fn flip(&mut self, toggle: Toggle) {
        let Some(panel) = &mut self.launch else {
            return;
        };
        let on = match toggle {
            Toggle::GameMode => &mut panel.switches.gamemode,
            Toggle::MangoHud => &mut panel.switches.mangohud,
            Toggle::Gpu => &mut panel.switches.dedicated_gpu,
        };
        *on = !*on;
    }

    /// The Java instance being edited, or the selected Java instance on Home.
    fn memory_instance(&self) -> Option<instances::Instance> {
        match self
            .instances
            .draft
            .as_ref()
            .and_then(|d| d.editing.as_ref())
        {
            Some(id) => self.instances.list.iter().find(|i| &i.id == id).cloned(),
            None => self.selected_instance().cloned(),
        }
        .filter(|i| i.java.is_some())
    }

    /// The value under the pointer. Saving waits until the mouse comes up.
    fn pick_memory(&mut self, at: f32, cx: &mut Context<Self>) {
        if self.memory_instance().is_none() {
            return;
        }
        let slider = &mut self.memory_slider;
        let total = slider.total();
        slider.saved = None;
        slider.dragging = Some(crate::memory::memory_at(at, total));
        cx.notify();
    }

    fn step_memory(&mut self, steps: i32, from: u32, cx: &mut Context<Self>) {
        let Some(id) = self.memory_instance().map(|i| i.id) else {
            return;
        };
        let total = self.memory_slider.total();
        let by = steps * crate::memory::SLIDER_STEP as i32;
        let memory = (from as i32 + by).clamp(crate::memory::SLIDER_STEP as i32, total as i32);
        self.save_memory(&id, memory as u32);
        cx.notify();
    }

    /// A release ends the drag and saves it to the instance, once.
    fn finish_memory(&mut self, cx: &mut Context<Self>) {
        let Some(memory) = self.memory_slider.dragging.take() else {
            return;
        };
        let Some(id) = self.memory_instance().map(|i| i.id) else {
            return;
        };
        self.save_memory(&id, memory);
        cx.notify();
    }

    fn save_memory(&mut self, id: &str, memory: u32) {
        let saved = instances::edit(id, |m| {
            if let Some(java) = &mut m.java {
                java.memory_mb = Some(memory);
            }
        });
        self.memory_slider.saved = Some(match saved {
            Ok(_) => MemorySaved::Ok(memory),
            Err(e) => MemorySaved::Error(e),
        });
        self.reload_instances();
    }

    /// The instance's saved value, as the row shows it.
    fn saved_memory(instance: &instances::Instance) -> Option<u32> {
        instance
            .java
            .as_ref()
            .map(|java| java.memory_mb.unwrap_or(instances::DEFAULT_MEMORY_MB))
    }

    /// Saves what's in the box to `id`, if it's a value and a new one.
    fn save_typed_memory(&mut self, id: &str) {
        let typed = self.fields.value(Field::MemoryLimit).to_owned();
        let Ok(memory) = crate::memory::parse_memory(&typed) else {
            self.memory_slider.saved = None;
            self.fields.set_field_invalid(Field::MemoryLimit, true);
            return;
        };
        let memory = memory.unwrap_or(instances::DEFAULT_MEMORY_MB);
        self.fields.set_field_invalid(Field::MemoryLimit, false);
        let same = instances::list()
            .iter()
            .find(|i| i.id == id)
            .is_some_and(|i| Self::saved_memory(i) == Some(memory));
        if !same {
            self.save_memory(id, memory);
        }
    }

    /// Enter in the box.
    pub(super) fn memory_enter(&mut self) {
        if let Some(id) = self.memory_slider.typing.clone() {
            self.save_typed_memory(&id);
        }
    }

    /// The box follows the saved value and the slider except while it has the keyboard, and saves
    /// when it loses it. Read every frame.
    fn sync_memory_box(&mut self, instance: &str, memory: u32) {
        if self.memory_slider.shown.as_deref() != Some(instance) {
            self.memory_slider.saved = None;
            self.memory_slider.shown = Some(instance.to_owned());
        }
        if self.fields.has_focus(Field::MemoryLimit) {
            self.memory_slider.typing = Some(instance.to_owned());
            return;
        }
        if let Some(id) = self.memory_slider.typing.take() {
            self.save_typed_memory(&id);
            return;
        }
        let shown = memory.to_string();
        if self.fields.value(Field::MemoryLimit) != shown {
            self.fields.text(Field::MemoryLimit).set(shown);
        }
    }

    /// Maximum memory as a setting row: the name and what it does on the left, the slider and
    /// the box in MB on the right. Saved to the instance as it is set.
    pub(super) fn memory_setting(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let instance = self.memory_instance()?;
        let total = self.memory_slider.total();
        let saved = Self::saved_memory(&instance)?;
        let memory = self.memory_slider.dragging.unwrap_or(saved);
        self.sync_memory_box(&instance.id, memory);
        let typed = crate::memory::parse_memory(self.fields.value(Field::MemoryLimit));
        let memory = match (self.fields.has_focus(Field::MemoryLimit), typed) {
            (true, Ok(typed)) => typed.unwrap_or(instances::DEFAULT_MEMORY_MB),
            _ => memory,
        };
        let f = crate::memory::memory_fraction(memory, total).clamp(0., 1.);
        let about = "Too little and the game can crash.";
        let this = cx.entity().downgrade();
        let track = ui::slider(
            "instance-memory",
            tr("Maximum memory"),
            f,
            f64::from(memory),
            (0., f64::from(total)),
            ui::Track::Fill,
            move |to, _, cx| {
                let _ = this.update(cx, |this, cx| match to {
                    ui::SliderMove::To(at) => this.pick_memory(at, cx),
                    ui::SliderMove::Step(n) => this.step_memory(n, memory, cx),
                    ui::SliderMove::Done => this.finish_memory(cx),
                });
            },
        );
        let unit = ui::card_note("MB");
        let input = self.field(
            Field::MemoryLimit,
            "4096",
            layout::FIELD_XS,
            ui::ButtonSize::Md,
            cx,
        );
        let result = self.memory_slider.saved.as_ref().map(|s| match s {
            MemorySaved::Ok(mb) => div().child(trf(
                "Saved: the next Play uses at most {mb} MB.",
                &[("mb", &mb.to_string())],
            )),
            MemorySaved::Error(e) => ui::error_note(e.clone()),
        });
        let note = div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(tr(about))
            .children(result);
        let beside = layout::content_width(self.window_width) >= layout::INSTANCE_COMPACT_BELOW;
        Some(ui::setting_row_fit(
            beside,
            tr("Maximum memory"),
            Some(note.into_any_element()),
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .child(track)
                .child(input)
                .child(unit)
                .into_any_element(),
        ))
    }

    /// The Linux switches this computer can use: GameMode and MangoHud when installed, the
    /// dedicated GPU with two cards.
    fn linux_switches(&self, cx: &mut Context<Self>) -> Option<Div> {
        let panel = self.launch.as_ref()?;
        let (found, now) = (panel.found, panel.switches);
        let mut list = div().flex().flex_col().gap(px(space::MD));
        for (toggle, shown, on, label, about) in [
            (
                Toggle::GameMode,
                found.gamemode,
                now.gamemode,
                "GameMode",
                Some("Tunes your computer for games while you play."),
            ),
            (
                Toggle::MangoHud,
                found.mangohud,
                now.mangohud,
                "MangoHud",
                Some("Shows an FPS and performance overlay."),
            ),
            (
                Toggle::Gpu,
                found.two_gpus,
                now.dedicated_gpu,
                "Dedicated graphics card",
                None,
            ),
        ] {
            if !shown {
                continue;
            }
            list = list.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::MD))
                    .child(
                        ui::toggle_label(
                            SharedString::from(format!("launch-switch-{toggle:?}")),
                            tr(label),
                            on,
                        )
                        .on_press(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.flip(toggle);
                                cx.notify();
                            },
                        )),
                    )
                    .children(about.map(|about| ui::card_note(tr(about)))),
            );
        }
        found.any().then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(space::SM))
                .child(ui::label(tr("On this computer")))
                .child(list)
        })
    }

    fn launch_field(
        &mut self,
        f: Field,
        label: &'static str,
        placeholder: &'static str,
        note: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Div {
        let field = self
            .field(f, tr(placeholder), layout::FIELD_XL, ui::ButtonSize::Md, cx)
            .w_full()
            .max_w(px(layout::FIELD_XL));
        ui::labelled_field(tr(label), field).children(note)
    }

    fn game_args_note(&self) -> Option<AnyElement> {
        let default = self.settings.game_args.trim();
        if default.is_empty() {
            return None;
        }
        let args = [("args", default)];
        let said = if self.typed(Field::InstanceGameArgs).is_empty() {
            trf("The default applies: {args}", &args)
        } else {
            trf("Used instead of the default: {args}", &args)
        };
        Some(ui::card_note(said).into_any_element())
    }

    pub(super) fn launch_card(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let before = Some(
            ui::card_note(tr("The game starts only if this command succeeds.")).into_any_element(),
        );
        let pre = self.launch_field(
            Field::PreLaunch,
            "Command before the game",
            "For example ./backup.sh",
            before,
            cx,
        );
        let wrapper = self.launch_field(
            Field::LaunchWrapper,
            "Run the game through",
            "For example nice -n 5",
            None,
            cx,
        );
        let refused = self
            .launch
            .as_ref()
            .and_then(|p| p.env_error.clone())
            .filter(|_| {
                self.fields
                    .invalid
                    .is_some_and(|(f, _)| f == Field::LaunchEnv)
            })
            .map(|e| ui::error_note(e).into_any_element());
        let env = self.launch_field(
            Field::LaunchEnv,
            "Environment variables",
            "For example NAME=value OTHER=\"two words\"",
            refused,
            cx,
        );
        let post = self.launch_field(
            Field::PostExit,
            "Command after the game",
            "For example ./backup.sh",
            None,
            cx,
        );
        let args = self.edits_game_args().then(|| {
            let note = self.game_args_note();
            self.launch_field(
                Field::InstanceGameArgs,
                "Eclipse game arguments",
                "For example --server-thread",
                note,
                cx,
            )
        });
        let body = div()
            .flex()
            .flex_col()
            .gap(px(space::LG))
            .child(pre)
            .child(wrapper)
            .child(env)
            .child(post)
            .children(args)
            .children(self.linux_switches(cx));
        ui::section_card("instance-launch", tr("Launch options"), None, body)
    }
}
