//! Edit instance's maximum memory and launch options (`eclipse_launcher::launch`): a command
//! before the game, a wrapper, environment variables, a command after it, the Linux switches and,
//! for an Eclipse instance, its own game arguments. Save changes keeps them with the rest of the
//! page.
use super::fields::Field;
use crate::instance_state::NewKind;
use crate::ui::Press;
use crate::{layout, ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances::Instance;
use eclipse_launcher::{instances, launch};
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, SharedString, Stateful};

pub(super) struct LaunchDraft {
    /// The instance whose options the fields hold, as Edit opened it.
    instance: String,
    switches: Switches,
    memory: Option<u32>,
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

/// Edit instance's memory row. What it shows comes from the page's draft, except during a drag.
#[derive(Default)]
pub(super) struct MemorySlider {
    /// The computer's memory in MiB, read once: the slider's top.
    total: Option<u32>,
    /// The Java heap size being dragged to.
    dragging: Option<u32>,
    /// The box has the keyboard: what it holds is picked when it loses it.
    typing: bool,
}

impl MemorySlider {
    fn total(&mut self) -> u32 {
        *self
            .total
            .get_or_insert_with(crate::instance_memory::total_memory_mb)
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
        self.fields.set_field_invalid(Field::MemoryLimit, false);
        self.memory_slider.typing = false;
        self.launch = Some(LaunchDraft {
            instance: instance.id.clone(),
            switches: Switches::of(l),
            memory: None,
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
        if self.memory_slider.typing && !self.pick_typed_memory() {
            return false;
        }
        let memory = self
            .memory_changed()
            .then(|| self.picked_memory())
            .flatten();
        let options = self.launch_options_changed();
        if !options && memory.is_none() {
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
            if options {
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
            }
            if let (Some(mb), Some(java)) = (memory, &mut m.java) {
                java.memory_mb = Some(mb);
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

    /// The Java instance being edited.
    fn memory_instance(&self) -> Option<instances::Instance> {
        self.launch_instance().filter(|i| i.java.is_some()).cloned()
    }

    /// The value under the pointer, picked when the mouse comes up.
    fn pick_memory(&mut self, at: f32, cx: &mut Context<Self>) {
        if self.memory_instance().is_none() {
            return;
        }
        let slider = &mut self.memory_slider;
        let total = slider.total();
        slider.dragging = Some(crate::instance_memory::memory_at(at, total));
        cx.notify();
    }

    fn step_memory(&mut self, steps: i32, from: u32, cx: &mut Context<Self>) {
        let total = self.memory_slider.total();
        let by = steps * crate::instance_memory::SLIDER_STEP as i32;
        let memory =
            (from as i32 + by).clamp(crate::instance_memory::SLIDER_STEP as i32, total as i32);
        self.set_memory(memory as u32);
        cx.notify();
    }

    fn finish_memory(&mut self, cx: &mut Context<Self>) {
        if let Some(memory) = self.memory_slider.dragging.take() {
            self.set_memory(memory);
        }
        cx.notify();
    }

    fn set_memory(&mut self, memory: u32) {
        if let Some(draft) = &mut self.launch {
            draft.memory = Some(memory);
        }
    }

    fn picked_memory(&self) -> Option<u32> {
        self.launch.as_ref()?.memory
    }

    pub(super) fn memory_changed(&self) -> bool {
        let saved = self.memory_instance().and_then(|i| Self::saved_memory(&i));
        let picked = if self.memory_slider.typing {
            match crate::instance_memory::parse_memory(self.fields.value(Field::MemoryLimit)) {
                Ok(typed) => Some(typed.unwrap_or(instances::DEFAULT_MEMORY_MB)),
                Err(_) => return true,
            }
        } else {
            self.picked_memory()
        };
        picked.is_some_and(|m| Some(m) != saved)
    }

    /// The instance's saved value, as the row shows it.
    fn saved_memory(instance: &instances::Instance) -> Option<u32> {
        instance
            .java
            .as_ref()
            .map(|java| java.memory_mb.unwrap_or(instances::DEFAULT_MEMORY_MB))
    }

    /// Picks what's in the box, if it's a value. Whether it was.
    fn pick_typed_memory(&mut self) -> bool {
        self.memory_slider.typing = false;
        let typed = self.fields.value(Field::MemoryLimit).to_owned();
        let Ok(memory) = crate::instance_memory::parse_memory(&typed) else {
            self.fields.set_field_invalid(Field::MemoryLimit, true);
            return false;
        };
        self.fields.set_field_invalid(Field::MemoryLimit, false);
        self.set_memory(memory.unwrap_or(instances::DEFAULT_MEMORY_MB));
        true
    }

    /// Enter in the box.
    pub(super) fn memory_enter(&mut self) {
        if self.memory_slider.typing {
            self.pick_typed_memory();
        }
    }

    /// The box follows the picked value and the slider except while it has the keyboard, and
    /// picks what it holds when it loses it. Read every frame.
    fn sync_memory_box(&mut self, memory: u32) {
        if self.fields.has_focus(Field::MemoryLimit) {
            self.memory_slider.typing = true;
            return;
        }
        if self.memory_slider.typing {
            self.pick_typed_memory();
            return;
        }
        let shown = memory.to_string();
        if self.fields.value(Field::MemoryLimit) != shown {
            self.fields.text(Field::MemoryLimit).set(shown);
        }
    }

    /// Maximum memory as a setting row: the name and what it does on the left, the slider and
    /// the box in MB on the right. Saved with the rest of the page.
    pub(super) fn memory_setting(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let instance = self.memory_instance()?;
        let total = self.memory_slider.total();
        let saved = Self::saved_memory(&instance)?;
        let picked = self.picked_memory().unwrap_or(saved);
        let memory = self.memory_slider.dragging.unwrap_or(picked);
        self.sync_memory_box(memory);
        let typed = crate::instance_memory::parse_memory(self.fields.value(Field::MemoryLimit));
        let memory = match (self.fields.has_focus(Field::MemoryLimit), typed) {
            (true, Ok(typed)) => typed.unwrap_or(instances::DEFAULT_MEMORY_MB),
            _ => memory,
        };
        let f = crate::instance_memory::memory_fraction(memory, total).clamp(0., 1.);
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
        let note = div().child(tr(about));
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
