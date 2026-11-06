//! Placed storage (BuildableMaster_Container's 15 crates, cupboards, lockers, the safe and the
//! shipping container, and BuildableMaster_Tent's tents), decompiled; the options themselves
//! are in `building::interact`:
//!
//! - **Interact** (MC_OnInteractBuildable): the actor's BP_JigMultiplayer grid
//!   (ContainersSettings[0]: Storage Crate 6x6 ... Shipping Container 10x20, buildables.json
//!   `storage`) opens beside the inventory, titled ContainerName (BeginPlay sets it to the pickup's
//!   name). The interact widget's 'Storage' stat is UpdateStorageStat's "{A} {B}" ("3 Items").
//! - **Change Container Name** (containers only): BuildableRenameObject, "RENAME OBJECT",
//!   "Rename object or leave empty to set name to 'Container'.", an EditableText (hint "OBJECT
//!   NAME HERE"), CANCEL / CONFIRM: Confirm calls OverrideBuildName with the text, or "Container"
//!   when it's empty.
//! - **Pickup**: with anything inside, YesAndNoUI (ModularYesNoUI) asks "Are you sure you want to
//!   pick up this container?"; YES = PickupContainer: DropAll (each item spawned at the survivor's
//!   transform) then the usual Svr_Spawn. Empty, it's picked up at once.
//! - The contents and the name are saved with the buildable (`PlacedBuildable::storage` / `name`;
//!   ActorPreSave/ActorLoaded keep MainJigContainers in the game).
//!
//! Ours: Enter confirms the name too and Esc cancels either dialog; names are capped at 32
//! characters; BackgroundBlur is a dark overlay. Loot_TableTop_LeftHand_Montage is
//! `building::interact`'s (`ActorDef::interact_montage`). Not ported: CheckDistanceFromActor
//! (walking away closes the grid in the game), the tents' Sleep.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::building::Buildables;
use super::interact::{Containers, GameOverlay};
use crate::menus::AppState;
use crate::menus::data::CurrentGame;
use crate::menus::widgets::{ButtonPalette, MenuFont, button, button_widget, label, label_colored};

/// Placed containers' keys: past the vehicles' storages.
pub const STORAGE_KEYS: usize = super::vehicles::STORAGE_KEYS + (1 << 30);
/// Ours: the longest name the rename box takes.
const MAX_NAME: usize = 32;
/// The rename box's hint (EditableText.HintText).
const HINT: &str = "OBJECT NAME HERE";

/// The name being typed into BuildableRenameObject.
#[derive(Resource, Default)]
pub struct RenameEntry {
    pub id: Option<usize>,
    pub text: String,
}

impl RenameEntry {
    /// One key typed: `Some(true)` = confirm, `Some(false)` = cancel.
    pub fn key(&mut self, key: &Key) -> Option<bool> {
        match key {
            Key::Enter => return Some(true),
            Key::Escape => return Some(false),
            Key::Backspace => {
                self.text.pop();
            }
            Key::Space => self.push(" "),
            Key::Character(c) => self.push(c),
            _ => {}
        }
        None
    }

    fn push(&mut self, s: &str) {
        for ch in s.chars().filter(|c| !c.is_control()) {
            if self.text.chars().count() < MAX_NAME {
                self.text.push(ch);
            }
        }
    }

    /// Confirm: OverrideBuildName(text), or "Container" when it's empty.
    pub fn name(&self) -> String {
        let t = self.text.trim();
        if t.is_empty() { "Container".into() } else { t.to_string() }
    }
}

#[derive(Component)]
struct ConfirmUi;
#[derive(Component)]
struct RenameUi;
/// ModularYesNoUI's YES (true) / NO (false).
#[derive(Component, Clone, Copy)]
pub struct YesNo(pub bool);
/// BuildableRenameObject's CONFIRM (true) / CANCEL (false).
#[derive(Component, Clone, Copy)]
struct RenameButton(bool);
#[derive(Component)]
struct NameText;

pub fn plugin(app: &mut App) {
    app.init_resource::<RenameEntry>()
        .add_systems(Update, (open_close, type_name, show_name, sync).chain().after(super::building::BuildSet).run_if(in_state(AppState::InGame)))
        .add_observer(on_yes_no)
        .add_observer(on_rename_button);
}

/// Spawn / despawn the two dialogs with the overlay.
fn open_close(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    font: Res<MenuFont>,
    mut entry: ResMut<RenameEntry>,
    confirms: Query<Entity, With<ConfirmUi>>,
    renames: Query<Entity, With<RenameUi>>,
) {
    let confirm = matches!(*overlay, GameOverlay::ConfirmPickup(_));
    if !confirm {
        confirms.iter().for_each(|e| commands.entity(e).despawn());
    } else if confirms.is_empty() {
        spawn_confirm(&mut commands, &font);
    }
    let rename = match *overlay {
        GameOverlay::Rename(id) => Some(id),
        _ => None,
    };
    if rename != entry.id {
        renames.iter().for_each(|e| commands.entity(e).despawn());
        *entry = RenameEntry { id: rename, text: String::new() };
        if rename.is_some() {
            spawn_rename(&mut commands, &font);
        }
    }
}

/// A full-screen layer over the HUD; BackgroundBlur is approximated with a dark overlay.
fn backdrop(dim: f32) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, dim)),
        GlobalZIndex(40),
        DespawnOnExit(AppState::InGame),
    )
}

/// ModularYesNoUI: the question centred over (alignment (0.5, 1.5)), YES / NO (125 x 60) side
/// by side under the centre (alignments (1.25, 0) and (-0.25, 0)), UMG's default 24 pt text.
fn spawn_confirm(commands: &mut Commands, font: &MenuFont) {
    let root = commands.spawn((backdrop(0.5), ConfirmUi)).id();
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: Val::Percent(50.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        ChildOf(root),
        children![(label(font, "Are you sure you want to pick up this container?", 24.0), Node { margin: UiRect::top(Val::Px(-1.5 * 44.0)), ..default() })],
    ));
    for (yes, text, dx) in [(true, "YES", -1.25 * 125.0), (false, "NO", 0.25 * 125.0)] {
        let node = Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(dx)),
            width: Val::Px(125.0),
            height: Val::Px(60.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        };
        commands.spawn((button(node, ButtonPalette::MENU, YesNo(yes)), ChildOf(root), children![label(font, text, 24.0)]));
    }
}

/// BuildableRenameObject: a 600 x 140 VerticalBox centred: the title (26), the line under it
/// (14), the text box (black 50 %, 5 px corners, 10 px padding; 14 pt white), CANCEL / CONFIRM.
fn spawn_rename(commands: &mut Commands, font: &MenuFont) {
    let root = commands.spawn((backdrop(0.35), RenameUi)).id();
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(600.0),
                height: Val::Px(140.0),
                margin: UiRect::new(Val::Px(-300.0), Val::ZERO, Val::Px(-70.0), Val::ZERO),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((label(font, "RENAME OBJECT", 26.0), Node { margin: UiRect::left(Val::Px(5.0)), ..default() }, ChildOf(column)));
    commands.spawn((label(font, "Rename object or leave empty to set name to 'Container'.", 14.0), Node { margin: UiRect::left(Val::Px(5.0)), ..default() }, ChildOf(column)));
    commands.spawn((
        Node { padding: UiRect::all(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ChildOf(column),
        children![(label_colored(font, HINT, 14.0, Color::srgba(1.0, 1.0, 1.0, 0.5)), NameText)],
    ));
    commands.spawn((
        Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() },
        ChildOf(column),
        children![
            button_widget(font, "CANCEL", 12.0, Node::default(), RenameButton(false)),
            button_widget(font, "CONFIRM", 12.0, Node::default(), RenameButton(true)),
        ],
    ));
}

/// Typing into the rename box.
fn type_name(mut keys: MessageReader<KeyboardInput>, mut entry: ResMut<RenameEntry>, mut overlay: ResMut<GameOverlay>, mut placed: ResMut<Buildables>) {
    let Some(id) = entry.id else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if let Some(ok) = entry.key(&ev.logical_key) {
            finish_rename(&entry, ok, id, &mut overlay, &mut placed);
            break;
        }
    }
}

fn finish_rename(entry: &RenameEntry, ok: bool, id: usize, overlay: &mut GameOverlay, placed: &mut Buildables) {
    if ok && let Some(l) = placed.list.iter_mut().find(|l| l.id == id) {
        l.name = Some(entry.name());
        info!("storage: renamed {} to {:?}", l.item, entry.name());
    }
    *overlay = GameOverlay::None;
}

/// The typed text, or the hint while it's empty.
fn show_name(entry: Res<RenameEntry>, mut texts: Query<(&mut Text, &mut TextColor), With<NameText>>) {
    if !entry.is_changed() {
        return;
    }
    for (mut t, mut c) in &mut texts {
        if entry.text.is_empty() {
            t.0 = HINT.into();
            c.0 = Color::srgba(1.0, 1.0, 1.0, 0.5);
        } else {
            t.0 = entry.text.clone();
            c.0 = Color::WHITE;
        }
    }
}

fn on_yes_no(ev: On<Activate>, buttons: Query<&YesNo>, mut overlay: ResMut<GameOverlay>, mut placed: ResMut<Buildables>) {
    let Ok(YesNo(yes)) = buttons.get(ev.entity) else { return };
    if let GameOverlay::ConfirmPickup(id) = *overlay
        && *yes
    {
        placed.confirmed = Some(id);
    }
    *overlay = GameOverlay::None;
}

fn on_rename_button(ev: On<Activate>, buttons: Query<&RenameButton>, entry: Res<RenameEntry>, mut overlay: ResMut<GameOverlay>, mut placed: ResMut<Buildables>) {
    let Ok(RenameButton(ok)) = buttons.get(ev.entity) else { return };
    let Some(id) = entry.id else { return };
    finish_rename(&entry, *ok, id, &mut overlay, &mut placed);
}

/// Keep the save's buildables in step with their names and contents.
fn sync(placed: Res<Buildables>, containers: Res<Containers>, mut game: ResMut<CurrentGame>) {
    let Some(state) = game.bypass_change_detection().state.as_mut() else { return };
    if state.buildables.len() != placed.list.len() {
        return;
    }
    let mut changed = false;
    for (b, l) in state.buildables.iter_mut().zip(&placed.list) {
        let grid = containers.searched.get(&l.storage_key()).filter(|g| !g.items.is_empty());
        if b.storage.as_ref() != grid {
            b.storage = grid.cloned();
            changed = true;
        }
        let output = containers.searched.get(&(super::utility_benches::OUTPUT_KEYS + l.id)).filter(|g| !g.items.is_empty());
        if b.output.as_ref() != output {
            b.output = output.cloned();
            changed = true;
        }
        if b.name != l.name {
            b.name = l.name.clone();
            changed = true;
        }
    }
    if changed {
        game.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_box_types_and_confirms() {
        let mut e = RenameEntry::default();
        assert_eq!(e.name(), "Container");
        for k in [Key::Character("A".into()), Key::Character("m".into()), Key::Space, Key::Character("o".into())] {
            assert_eq!(e.key(&k), None);
        }
        assert_eq!(e.text, "Am o");
        e.key(&Key::Backspace);
        assert_eq!(e.name(), "Am");
        assert_eq!(e.key(&Key::Enter), Some(true));
        assert_eq!(e.key(&Key::Escape), Some(false));
        // Spaces only = empty.
        let mut blank = RenameEntry { id: None, text: "   ".into() };
        assert_eq!(blank.name(), "Container");
        blank.push(&"x".repeat(40));
        assert_eq!(blank.text.chars().count(), MAX_NAME);
    }

    #[test]
    fn storage_data() {
        let db = super::super::building::BuildableDb::load();
        let grid = |a: &str| db.actors[a].storage;
        assert_eq!(grid("Buildable_StorageCrate_C"), Some([6, 6]));
        assert_eq!(grid("Buildable_MoneySafe_C"), Some([4, 4]));
        assert_eq!(grid("Buildable_ShippingContainer_C"), Some([10, 20]));
        assert_eq!(grid("Buildable_Tent1_C"), Some([8, 7]));
        // Benches and the beacon aren't storage.
        assert_eq!(grid("Buildable_CraftingWorkbench_C"), None);
        assert_eq!(grid("Buildable_HordeBeacon_C"), None);
        // Every storage fits the loot column.
        assert!(db.actors.values().filter_map(|a| a.storage).all(|[c, r]| c >= 1 && c <= super::super::interact::LOOT_COLUMNS && r >= 1));
        assert_eq!(super::super::building::storage_stat(1), "Storage: 1 Item");
        assert_eq!(super::super::building::storage_stat(3), "Storage: 3 Items");
        assert!(STORAGE_KEYS > super::super::vehicles::STORAGE_KEYS + (1 << 29));
    }
}
