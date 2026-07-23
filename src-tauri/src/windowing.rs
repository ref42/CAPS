use dioxus::desktop::tao::dpi::PhysicalPosition;
use dioxus::desktop::tao::window::Window;
use dioxus::desktop::{DesktopContext, LogicalPosition, LogicalSize};

pub const COLLAPSED_W: f64 = 380.0;
pub const COLLAPSED_H: f64 = 56.0;
pub const EXPANDED_W: f64 = 460.0;
pub const EXPANDED_H: f64 = 490.0;
pub const MUSIC_COLLAPSED_W: f64 = EXPANDED_W;
pub const ISLAND_BLEED: f64 = 18.0;

pub fn set_island_window(
    desktop: &DesktopContext,
    expanded: bool,
    size_scale: f64,
    collapsed_width: f64,
) {
    let size_scale = size_scale.clamp(0.85, 1.35);
    let (base_width, base_height) = if expanded {
        (EXPANDED_W, EXPANDED_H)
    } else {
        (
            collapsed_width.max(COLLAPSED_W) + ISLAND_BLEED * 2.0,
            COLLAPSED_H + ISLAND_BLEED * 2.0,
        )
    };
    let width = base_width * size_scale;
    let height = base_height * size_scale;
    let old_size = desktop.inner_size();
    let old_position = desktop.outer_position().ok();
    let scale = desktop.scale_factor();
    desktop.set_inner_size(LogicalSize::new(width, height));
    desktop.set_always_on_top(true);
    if let Some(position) = old_position {
        let old_width = old_size.width as i32;
        let new_width = (width * scale).round() as i32;
        let x = position.x + (old_width - new_width) / 2;
        desktop.set_outer_position(PhysicalPosition::new(x, position.y));
    }
}

pub fn place_top_center(window: &Window, width: f64) {
    if let Some(monitor) = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
    {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let position = monitor.position().to_logical::<f64>(scale);
        let x = position.x + ((size.width - width) / 2.0).max(0.0);
        window.set_outer_position(LogicalPosition::new(x.round(), position.y + 8.0));
    }
}
