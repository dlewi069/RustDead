use crate::engine::GameState;
use crate::survival::TraumaType;
use crate::ui::menu_state::{FACIAL_DETAILS, FACIAL_HAIR, HAIR_COLORS};
use std::io::{self, Write};

pub struct AuthenticMenuRenderer;

impl AuthenticMenuRenderer {
    pub fn render_main_menu(selected_idx: usize) -> io::Result<()> {
        let mut stdout = io::stdout();
        print!("\x1B[2J\x1B[H");

        println!("╔══════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                                  SURROUNDEAD : MAIN MENU                                     ║");
        println!("║                                 Surviving the Apocalypse                                     ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");

        let items = [
            "CONTINUE",
            "NEW GAME",
            "LOAD GAME",
            "SETTINGS",
            "VEHICLE PLAYGROUND",
            "QUIT TO DESKTOP",
        ];

        println!("║                                                                                              ║");
        for (idx, &item) in items.iter().enumerate() {
            let pointer = if idx == selected_idx { "▶ [ " } else { "    " };
            let close = if idx == selected_idx { " ]" } else { "  " };
            println!("║                        {:<16} {:<24}                        ║", pointer, format!("{}{}", item, close));
        }
        println!("║                                                                                              ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");
        println!("║  CONTROLS: [Up/Down or W/S] Select Option  |  [Enter/Space] Confirm  |  [Esc] Back           ║");
        println!("║  Build: v0.7.1g (Authentic Rust Port)                                                        ║");
        println!("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        stdout.flush()?;
        Ok(())
    }

    pub fn render_pause_menu(selected_idx: usize) -> io::Result<()> {
        let mut stdout = io::stdout();
        print!("\x1B[2J\x1B[H");

        println!("╔══════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                                        PAUSE SCREEN                                          ║");
        println!("║                                   Preparation - v0.7.1g                                      ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");

        let items = [
            "RESUME",
            "QUICK SAVE (F5)",
            "SAVE GAME",
            "DIFFICULTY SETTINGS",
            "SETTINGS",
            "QUIT TO MAIN MENU",
        ];

        println!("║                                                                                              ║");
        for (idx, &item) in items.iter().enumerate() {
            let pointer = if idx == selected_idx { "▶ [ " } else { "    " };
            let close = if idx == selected_idx { " ]" } else { "  " };
            println!("║                        {:<16} {:<24}                        ║", pointer, format!("{}{}", item, close));
        }
        println!("║                                                                                              ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");
        println!("║  [Esc] Resume Game  |  [Up/Down] Navigate  |  [Enter] Confirm                                 ║");
        println!("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        stdout.flush()?;
        Ok(())
    }

    pub fn render_character_creator(detail_idx: usize, hair_idx: usize, color_idx: usize, active_field: usize) -> io::Result<()> {
        let mut stdout = io::stdout();
        print!("\x1B[2J\x1B[H");

        println!("╔══════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                                 CHARACTER CREATOR MENU                                       ║");
        println!("║                                  Survivor Identity                                           ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");

        let fields = [
            format!("FACIAL DETAIL: < {} >", FACIAL_DETAILS[detail_idx % FACIAL_DETAILS.len()]),
            format!("FACIAL HAIR:   < {} >", FACIAL_HAIR[hair_idx % FACIAL_HAIR.len()]),
            format!("HAIR COLOR:    < {} >", HAIR_COLORS[color_idx % HAIR_COLORS.len()]),
            "START ADVENTURE [CONFIRM]".to_string(),
        ];

        for (idx, field) in fields.iter().enumerate() {
            let marker = if idx == active_field { "▶ [*] " } else { "  [ ] " };
            println!("║     {:<85} ║", format!("{}{}", marker, field));
        }

        println!("║                                                                                              ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");
        println!("║  [W/S] Select Row  |  [A/D] Change Variant  |  [Enter] Confirm  |  [Esc] Cancel               ║");
        println!("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        stdout.flush()?;
        Ok(())
    }

    pub fn render_medical_screen(state: &GameState) -> io::Result<()> {
        let mut stdout = io::stdout();
        print!("\x1B[2J\x1B[H");

        println!("╔══════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!("║  FIELD EMERGENCY TRAUMA CLINIC & PHYSIOLOGICAL DIAGNOSTICS [PRESS H / ESC TO EXIT]            ║");
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");

        let bleed_str = if state.survival.bleeding { "LACERATION BLEEDING (-1.0 HP/s) [ACTIVE]" } else { "NOMINAL" };
        let fracture_str = if state.medical.has_trauma(TraumaType::Fracture) { "COMPOUND BONE FRACTURE (LIMPING) -45% SPEED" } else { "NOMINAL" };
        let infection_str = if state.survival.infection > 0.0 { format!("BLOOD SEPSIS / INFECTION ({:.0}%)", state.survival.infection) } else { "0% (STERILE)".to_string() };

        println!("║ ANATOMICAL DIAGRAM & PATHOLOGIES:                                                            ║");
        println!("║   • Torso Status:     {:<71} ║", bleed_str);
        println!("║   • Limbs Status:     {:<71} ║", fracture_str);
        println!("║   • Toxicity/Sepsis:  {:<71} ║", infection_str);
        println!("║   • Radiation Dose:   {:<71.1} ║", state.survival.radiation);
        println!("║                                                                                              ║");
        println!("║ FIELD PHARMACEUTICALS (HOTKEYS 1-8):                                                         ║");
        println!("║   [1] Sterile Bandage                (Stops Bleed, +20 HP)                                   ║");
        println!("║   [2] Military Trauma Kit            (Heals Heavy Wounds, +65 HP)                            ║");
        println!("║   [3] Medical Fracture Splint        (Sets Broken Bones, Restores Sprint)                    ║");
        println!("║   [4] Broad-Spectrum Antibiotics     (Halts Infection -50%)                                  ║");
        println!("║   [5] Anti-Rad Iodide Pills          (Purges Radiation -50 rads)                             ║");
        println!("║   [6] Purified Bottled Water         (Relieves Thirst +40%)                                  ║");
        println!("║   [7] Canned Rations                 (Restores Hunger +45%)                                  ║");
        println!("║   [8] Painkiller Bottle              (Relieves Acute Pain Shock)                             ║");
        println!("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        stdout.flush()?;
        Ok(())
    }
}
