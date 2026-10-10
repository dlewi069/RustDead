use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentSlot {
    Optic,
    Muzzle,
    Magazine,
    Tactical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub slot: AttachmentSlot,
    pub zoom_multiplier: f32,       // 1.0 = none, 4.0 = 4x
    pub recoil_v_multiplier: f32,   // 0.8 = 20% recoil reduction
    pub recoil_h_multiplier: f32,
    pub spread_multiplier: f32,     // 0.85 = 15% spread reduction
    pub noise_multiplier: f32,      // 0.25 = 75% quiet
    pub mag_capacity_bonus: u32,    // +15, +30
    pub has_flashlight: bool,
    pub has_laser: bool,
}

impl Attachment {
    pub fn new(id: impl Into<String>, name: impl Into<String>, slot: AttachmentSlot) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            slot,
            zoom_multiplier: 1.0,
            recoil_v_multiplier: 1.0,
            recoil_h_multiplier: 1.0,
            spread_multiplier: 1.0,
            noise_multiplier: 1.0,
            mag_capacity_bonus: 0,
            has_flashlight: false,
            has_laser: false,
        }
    }
}

pub fn get_stock_attachment(id: &str) -> Option<Attachment> {
    match id {
        "optic_red_dot" => {
            let mut a = Attachment::new("optic_red_dot", "Kobra Red Dot Sight", AttachmentSlot::Optic);
            a.spread_multiplier = 0.85;
            a.zoom_multiplier = 1.2;
            Some(a)
        }
        "optic_holo" => {
            let mut a = Attachment::new("optic_holo", "EOTech Holographic Sight", AttachmentSlot::Optic);
            a.spread_multiplier = 0.80;
            a.recoil_v_multiplier = 0.95;
            a.zoom_multiplier = 1.3;
            Some(a)
        }
        "optic_acog_4x" => {
            let mut a = Attachment::new("optic_acog_4x", "ACOG 4x Combat Scope", AttachmentSlot::Optic);
            a.spread_multiplier = 0.65;
            a.zoom_multiplier = 4.0;
            Some(a)
        }
        "optic_sniper_8x" => {
            let mut a = Attachment::new("optic_sniper_8x", "Leupold 8x Sniper Scope", AttachmentSlot::Optic);
            a.spread_multiplier = 0.40;
            a.zoom_multiplier = 8.0;
            Some(a)
        }
        "muzzle_suppressor" => {
            let mut a = Attachment::new("muzzle_suppressor", "Osprey Tactical Suppressor", AttachmentSlot::Muzzle);
            a.noise_multiplier = 0.20; // 80% sound reduction!
            a.recoil_v_multiplier = 0.90;
            Some(a)
        }
        "muzzle_compensator" => {
            let mut a = Attachment::new("muzzle_compensator", "Competition Muzzle Compensator", AttachmentSlot::Muzzle);
            a.recoil_v_multiplier = 0.65; // 35% recoil reduction
            a.recoil_h_multiplier = 0.70;
            Some(a)
        }
        "mag_extended_30" => {
            let mut a = Attachment::new("mag_extended_30", "PMAG 40-Round Extended Magazine", AttachmentSlot::Magazine);
            a.mag_capacity_bonus = 10;
            Some(a)
        }
        "mag_drum_60" => {
            let mut a = Attachment::new("mag_drum_60", "D-60 60-Round Drum Magazine", AttachmentSlot::Magazine);
            a.mag_capacity_bonus = 30;
            Some(a)
        }
        "tactical_flashlight" => {
            let mut a = Attachment::new("tactical_flashlight", "SureFire Weapon Flashlight", AttachmentSlot::Tactical);
            a.has_flashlight = true;
            Some(a)
        }
        "tactical_laser" => {
            let mut a = Attachment::new("tactical_laser", "PEQ-15 Green Laser Aiming Module", AttachmentSlot::Tactical);
            a.has_laser = true;
            a.spread_multiplier = 0.75;
            Some(a)
        }
        _ => None,
    }
}
