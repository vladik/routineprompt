use gpui::{
    App, Context, Entity, FocusHandle, KeyDownEvent, MouseButton, Render, SharedString, Window,
    div, prelude::*, rgb,
};

pub struct TextInput {
    pub value: String,
    placeholder: SharedString,
    focus_handle: FocusHandle,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn with_value(cx: &mut Context<Self>, placeholder: impl Into<SharedString>, value: String) -> Self {
        Self {
            value,
            placeholder: placeholder.into(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let is_empty = self.value.is_empty();

        div()
            .id("text-input")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if let Some(chars) = &event.keystroke.key_char {
                    this.value.push_str(chars);
                    cx.notify();
                } else if event.keystroke.key == "backspace" {
                    this.value.pop();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .w_full()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if focused { rgb(0x4f46e5) } else { rgb(0xd1d5db) })
            .bg(rgb(0xffffff))
            .text_base()
            .text_color(if is_empty && !focused {
                rgb(0x9ca3af)
            } else {
                rgb(0x111827)
            })
            .child(if is_empty && !focused {
                SharedString::from(self.placeholder.clone())
            } else {
                SharedString::from(format!("{}{}", self.value, if focused { "▎" } else { "" }))
            })
    }
}

pub fn text_input(cx: &mut App, placeholder: impl Into<SharedString>) -> Entity<TextInput> {
    let placeholder = placeholder.into();
    cx.new(|cx| TextInput::new(cx, placeholder))
}
