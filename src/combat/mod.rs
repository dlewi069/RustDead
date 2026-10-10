pub mod attachments;
pub mod ballistics;
pub mod hit;
pub mod weapon;

pub use attachments::{get_stock_attachment, Attachment, AttachmentSlot};
pub use ballistics::{BallisticShot, HitZone};
pub use hit::TargetHitbox;
pub use weapon::{create_weapon_preset, Caliber, FireMode, WeaponStats};
