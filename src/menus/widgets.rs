//! Shared building blocks matching the game's UMG styling: Montserrat-Bold with a 2px outline,
//! black buttons at 25% / 50% / 75% alpha for normal / hovered / pressed (`ButtonWidget` and every
//! menu `Button` share that `WidgetStyle`), and full-screen blur backdrops.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::{HoverMap, Hovered};
use bevy::prelude::*;
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui_widgets::Button;

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuFont>()
            .add_systems(Update, (button_appearance, scroll_hovered));
    }
}

#[derive(Resource, Clone)]
pub struct MenuFont(pub Handle<Font>);

// FromWorld rather than a Startup system: the initial OnEnter(MenuScreen::Main) runs first.
impl FromWorld for MenuFont {
    fn from_world(world: &mut World) -> Self {
        Self(world.resource::<AssetServer>().load("fonts/Montserrat-Bold.ttf"))
    }
}

/// UMG font sizes are points at 96 DPI.
pub fn pt(size: f32) -> FontSize {
    FontSize::Px(size * 96.0 / 72.0)
}

pub const TEXT: Color = Color::WHITE;
pub const TEXT_DIM: Color = Color::srgb(0.85, 0.85, 0.85);

/// Text as the menus draw it: white Montserrat-Bold, 2px black outline (approximated by a shadow).
pub fn label(font: &MenuFont, text: impl Into<String>, size: f32) -> impl Bundle {
    label_colored(font, text, size, TEXT)
}

pub fn label_colored(font: &MenuFont, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont { font: font.0.clone().into(), font_size: pt(size), ..default() },
        TextColor(color),
        TextShadow { offset: Vec2::splat(2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) },
    )
}

/// Background tints for a button's states.
#[derive(Component, Clone, Copy)]
pub struct ButtonPalette {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
    /// Used while the button carries [`Chosen`] (selected category / difficulty / save slot).
    pub chosen: Color,
}

impl ButtonPalette {
    /// The `WidgetStyle` every menu button in the game uses.
    pub const MENU: Self = Self {
        normal: Color::srgba(0.0, 0.0, 0.0, 0.25),
        hovered: Color::srgba(0.0, 0.0, 0.0, 0.5),
        pressed: Color::srgba(0.0, 0.0, 0.0, 0.75),
        chosen: Color::srgba(0.0, 0.0, 0.0, 0.75),
    };
    /// `CategoryButton`: white at 20%, with a white bar on the selected one.
    pub const CATEGORY: Self = Self {
        normal: Color::srgba(1.0, 1.0, 1.0, 0.1),
        hovered: Color::srgba(1.0, 1.0, 1.0, 0.2),
        pressed: Color::srgba(1.0, 1.0, 1.0, 0.3),
        chosen: Color::srgba(1.0, 1.0, 1.0, 0.2),
    };
    /// Text-only buttons (icon buttons, big "Start"/"Back" captions).
    pub const BARE: Self = Self {
        normal: Color::NONE,
        hovered: Color::srgba(0.0, 0.0, 0.0, 0.25),
        pressed: Color::srgba(0.0, 0.0, 0.0, 0.5),
        chosen: Color::srgba(0.0, 0.0, 0.0, 0.5),
    };
}

/// Marks the selected entry of a radio-style group.
#[derive(Component)]
pub struct Chosen;

/// A clickable box with the menu style. `node` sets its size/placement; `extra` is the action
/// component(s) the click observer looks for.
pub fn button(node: Node, palette: ButtonPalette, extra: impl Bundle) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Node { border_radius: BorderRadius::all(Val::Px(4.0)), ..node },
        BackgroundColor(palette.normal),
        palette,
        extra,
    )
}

/// `ButtonWidget`: a button sized to its text (2px text padding in the asset, plus the default
/// button margin). `node` positions it.
pub fn button_widget(font: &MenuFont, text: &str, size: f32, node: Node, extra: impl Bundle) -> impl Bundle {
    (
        button(
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..node
            },
            ButtonPalette::MENU,
            extra,
        ),
        children![label(font, text, size)],
    )
}

/// The `[Esc] Back` button every overlay has at its bottom-right corner
/// (alignment (1.1, 1.35) against the bottom-right anchor).
pub fn back_button(font: &MenuFont, extra: impl Bundle) -> impl Bundle {
    let node = Node { position_type: PositionType::Absolute, right: Val::Px(24.0), bottom: Val::Px(20.0), ..default() };
    (button_widget(font, "[Esc] Back", 16.0, node, extra), GlobalZIndex(5))
}

/// A full-screen layer that lives as long as `state`.
pub fn screen<S: States>(state: S) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        DespawnOnExit(state),
    )
}

/// Stand-in for UMG's `BackgroundBlur` (strength 10-15): Bevy UI has no backdrop blur yet.
pub fn backdrop(alpha: f32) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.02, 0.02, 0.02, alpha)),
    )
}

/// Place a node the way a UMG `CanvasPanelSlot` does: `anchor` (0/0.5/1 per axis), pixel
/// `offset` from the anchor, `size`, and `align` (pivot as a fraction of size).
pub fn anchored(anchor: Vec2, offset: Vec2, size: Vec2, align: Vec2) -> Node {
    let start = offset - align * size;
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(size.x),
        height: Val::Px(size.y),
        ..default()
    };
    match anchor.x {
        a if a <= 0.0 => node.left = Val::Px(start.x),
        a if a >= 1.0 => node.right = Val::Px(-(start.x + size.x)),
        a => {
            node.left = Val::Percent(a * 100.0);
            node.margin.left = Val::Px(start.x);
        }
    }
    match anchor.y {
        a if a <= 0.0 => node.top = Val::Px(start.y),
        a if a >= 1.0 => node.bottom = Val::Px(-(start.y + size.y)),
        a => {
            node.top = Val::Percent(a * 100.0);
            node.margin.top = Val::Px(start.y);
        }
    }
    node
}

fn button_appearance(
    mut buttons: Query<
        (&Hovered, Has<Pressed>, Has<Chosen>, Has<InteractionDisabled>, &ButtonPalette, &mut BackgroundColor),
        With<Button>,
    >,
) {
    for (hovered, pressed, chosen, disabled, palette, mut bg) in &mut buttons {
        let color = match (pressed, hovered.get(), chosen) {
            _ if disabled => palette.normal,
            (true, _, _) => palette.pressed,
            (_, _, true) => palette.chosen,
            (_, true, _) => palette.hovered,
            _ => palette.normal,
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Mouse wheel scrolls whichever scrollable node is under the cursor.
fn scroll_hovered(
    mut wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    parents: Query<&ChildOf>,
    mut scrollables: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    for ev in wheel.read() {
        let dy = -ev.y * if ev.unit == MouseScrollUnit::Line { 40.0 } else { 1.0 };
        let mut targets = Vec::new();
        for pointer in hover_map.values() {
            for &hit in pointer.keys() {
                // Walk up from the hovered node to the nearest vertical scroll container.
                let mut e = hit;
                loop {
                    if scrollables.get(e).is_ok_and(|(_, node, _)| node.overflow.y == OverflowAxis::Scroll) {
                        if !targets.contains(&e) {
                            targets.push(e);
                        }
                        break;
                    }
                    match parents.get(e) {
                        Ok(p) => e = p.parent(),
                        Err(_) => break,
                    }
                }
            }
        }
        for e in targets {
            let Ok((mut pos, _, computed)) = scrollables.get_mut(e) else { continue };
            let max = (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor();
            pos.0.y = (pos.0.y + dy).clamp(0.0, max.max(0.0));
        }
    }
}
