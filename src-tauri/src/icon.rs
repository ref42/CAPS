use dioxus::desktop::icon_from_memory;
use dioxus::desktop::tao::window::Icon;

pub fn app_icon() -> Option<Icon> {
    icon_from_memory::<Icon>(include_bytes!("../../assets/caps.png")).ok()
}
