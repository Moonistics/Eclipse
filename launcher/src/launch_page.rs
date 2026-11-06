//! Launch… on Home's Instance row: what runs around the selected instance's game
//! (`eclipse_launcher::launch`): a command before it, a wrapper, and environment variables.
use super::fields::Field;
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::launch;
use eclipse_theme::glass::{fill, ink, radius, space, step};
use gpui::{
    canvas, relative, Bounds, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point,
};

pub(super) struct LaunchPanel {
    /// The instance it is for: picking another closes it.
    instance: String,
    /// What Save did: Ok with the summary of what runs (None when nothing runs), or Err with reason.
    saved: Option<Result<Option<String>, String>>,
    /// What this computer has for the Linux switches, looked at when the panel opens.
    found: launch::Found,
}

/// What saving the memory slider did, kept to show under the slider in the active language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MemorySaved {
    /// Saved Java heap size in MB.
    Ok(u32),
    /// Saving failed on disk.
    Error(String),
}

/// The memory row on Home. What it shows comes from the selected instance, except during a drag.
#[derive(Default)]
pub(super) struct MemorySlider {
    /// The computer's memory in MiB, read once: the slider's top.
    total: Option<u32>,
    area: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
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
        *self.total.get_or_insert_with(launch::total_memory_mb)
    }
}

const FIELD_WIDTH: f32 = 520.;

/// A Linux switch: which one, its label, and what it does.
#[derive(Clone, Copy, Debug)]
enum Toggle {
    GameMode,
    MangoHud,
    Gpu,
}

impl Launcher {
    pub(super) fn toggle_launch(&mut self) {
        if self.launch.take().is_some() {
            return;
        }
        let Some(i) = self.selected_instance().cloned() else {
            return;
        };
        self.fields
            .text(Field::PreLaunch)
            .set(i.launch.pre_launch.clone());
        self.fields
            .text(Field::LaunchWrapper)
            .set(i.launch.wrapper.clone());
        self.fields
            .text(Field::LaunchEnv)
            .set(launch::env_line(&i.launch.env));
        self.fields
            .text(Field::PostExit)
            .set(i.launch.post_exit.clone());
        self.launch = Some(LaunchPanel {
            instance: i.id,
            saved: None,
            found: launch::Found::here(),
        });
    }

    pub(super) fn save_launch(&mut self) {
        let Some(id) = self.launch.as_ref().map(|p| p.instance.clone()) else {
            return;
        };
        let pre = self.fields.value(Field::PreLaunch).trim().to_string();
        let wrapper = self.fields.value(Field::LaunchWrapper).trim().to_string();
        let post_exit = self.fields.value(Field::PostExit).trim().to_string();
        let saved = launch::parse_env(self.fields.value(Field::LaunchEnv)).and_then(|env| {
            instances::edit(&id, |m| {
                m.launch = launch::Launch {
                    pre_launch: pre,
                    wrapper,
                    env,
                    post_exit,
                    ..m.launch.clone()
                }
            })
        });
        let saved = saved.map(|i| {
            self.reload_instances();
            i.launch.summary()
        });
        if let Some(p) = &mut self.launch {
            p.saved = Some(saved);
        }
    }

    /// Flips a Linux switch, saved at once.
    fn flip(&mut self, toggle: Toggle) {
        let Some(panel) = &mut self.launch else {
            return;
        };
        let saved = instances::edit(&panel.instance, |m| {
            let on = match toggle {
                Toggle::GameMode => &mut m.launch.gamemode,
                Toggle::MangoHud => &mut m.launch.mangohud,
                Toggle::Gpu => &mut m.launch.dedicated_gpu,
            };
            *on = !*on;
        });
        panel.saved = Some(saved.map(|i| i.launch.summary()));
        self.reload_instances();
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
    fn pick_memory(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(_) = self.memory_instance() else {
            return;
        };
        let slider = &mut self.memory_slider;
        let Some(area) = slider.area.get() else {
            return;
        };
        let width: f32 = area.size.width.into();
        if width <= 0. {
            return;
        }
        let across: f32 = (at.x - area.origin.x).into();
        let total = slider.total();
        let picked = launch::memory_at((across / width).clamp(0., 1.), total);
        slider.saved = None;
        slider.dragging = Some(picked);
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
        let Ok(memory) = launch::parse_memory(&typed) else {
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

    /// The maximum Java heap size for the selected Java instance, as a card on Home.
    pub(super) fn memory_row(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let row = self.memory_setting(cx)?;
        Some(ui::card(radius::PANEL).p(px(ui::LIST_INSET)).child(row))
    }

    /// Maximum memory as a setting row: the name and what it does on the left, the slider and
    /// the box in MB on the right. Saved to the instance as it is set.
    pub(super) fn memory_setting(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        const TRACK: f32 = 208.;
        let instance = self.memory_instance()?;
        let total = self.memory_slider.total();
        let area = self.memory_slider.area.clone();
        let saved = Self::saved_memory(&instance)?;
        let memory = self.memory_slider.dragging.unwrap_or(saved);
        self.sync_memory_box(&instance.id, memory);
        let typed = launch::parse_memory(self.fields.value(Field::MemoryLimit));
        let memory = match (self.fields.has_focus(Field::MemoryLimit), typed) {
            (true, Ok(typed)) => typed.unwrap_or(instances::DEFAULT_MEMORY_MB),
            _ => memory,
        };
        let f = launch::memory_fraction(memory, total).clamp(0., 1.);
        let about = "Too little and the game can crash.";
        let track = div()
            .id("instance-memory")
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .w(px(TRACK))
            .h(px(24.))
            .cursor_pointer()
            .child(
                canvas(move |bounds, _, _| area.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(white(fill::ACTIVE))
                    .child(
                        div()
                            .h_full()
                            .w(relative(f))
                            .rounded_full()
                            .bg(ui::color(ui::look().accent_soft)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(f * (TRACK - 16.)))
                    .size(px(16.))
                    .rounded_full()
                    .bg(crate::ui::paper()),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.pick_memory(ev.position, cx)),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if ev.pressed_button == Some(MouseButton::Left)
                    && this.memory_slider.dragging.is_some()
                {
                    this.pick_memory(ev.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.finish_memory(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.finish_memory(cx)),
            );
        let unit = ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child("MB");
        let input = self.field(Field::MemoryLimit, "4096", 96., ui::ButtonSize::Md, cx);
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
        Some(ui::setting_row(
            tr("Maximum memory"),
            Some(note.into_any_element()),
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .child(track)
                    .child(input)
                    .child(unit)
                    .into_any_element(),
            ),
        ))
    }

    /// The Linux switches this computer can use: GameMode and MangoHud when installed, the
    /// dedicated GPU with two cards.
    fn linux_switches(&self, cx: &mut Context<Self>) -> Option<Div> {
        let found = self.launch.as_ref()?.found;
        let now = self.selected_instance()?.launch.clone();
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
                    .children(about.map(|about| {
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(tr(about))
                    })),
            );
        }
        found.any().then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .child(ui::label(tr("On this computer")))
                .child(list)
        })
    }

    /// The Launch card on Home, while open for the selected instance.
    pub(super) fn launch_panel(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let selected = self.selected_instance().map(|i| i.id.clone());
        if self.launch.as_ref().map(|p| &p.instance) != selected.as_ref() {
            self.launch = None;
        }
        let saved = self.launch.as_ref()?.saved.clone();
        let row = |this: &mut Self,
                   cx: &mut Context<Self>,
                   label: &'static str,
                   field: Field,
                   placeholder: &'static str,
                   about: Option<&'static str>| {
            div()
                .flex()
                .flex_col()
                .gap(px(space::SM))
                .child(ui::label(tr(label)))
                .child(this.field(field, tr(placeholder), FIELD_WIDTH, ui::ButtonSize::Md, cx))
                .children(about.map(|about| {
                    ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(tr(about))
                }))
        };
        let pre = row(
            self,
            cx,
            "Command before the game",
            Field::PreLaunch,
            "For example ./backup.sh",
            Some("The game starts only if this command succeeds."),
        );
        let wrapper = row(
            self,
            cx,
            "Run the game through",
            Field::LaunchWrapper,
            "For example nice -n 5",
            None,
        );
        let env = row(
            self,
            cx,
            "Environment variables",
            Field::LaunchEnv,
            "For example NAME=value OTHER=\"two words\"",
            None,
        );
        let post = row(
            self,
            cx,
            "Command after the game",
            Field::PostExit,
            "For example ./backup.sh",
            None,
        );
        let save = ui::button(
            "launch-save",
            None,
            tr("Save"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.save_launch();
            cx.notify();
        }));
        let result = saved.map(|s| match s {
            Ok(Some(what)) => ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                .child(trf("Saved: the next Play {what}.", &[("what", &what)])),
            Ok(None) => ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                .child(tr("Saved: nothing runs around the game.")),
            Err(e) => ui::error_note(e),
        });
        let close = ui::icon_button(
            "launch-close",
            "cancel-01",
            tr("Close"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            false,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.launch = None;
            cx.notify();
        }));
        Some(
            ui::card(radius::PANEL)
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .p(px(ui::CARD_PADDING))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(ui::card_title(tr("When it starts")))
                        .child(close),
                )
                .child(pre)
                .child(wrapper)
                .child(env)
                .child(post)
                .children(self.linux_switches(cx))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(space::MD))
                        .child(save)
                        .children(result),
                ),
        )
    }
}
