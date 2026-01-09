use neptune_privacy::{api::export::Announcement, prelude::triton_vm::prelude::BFieldElement};

/// try extracting receiver identifier field from a [Announcement]
pub fn extract_receiver_identifier(announcement: &Announcement) -> Option<BFieldElement> {
    match announcement.message.get(1) {
        Some(id) => Some(*id),
        None => None,
    }
}

/// try extracting ciphertext field from a [Announcement]
pub fn extract_ciphertext(announcement: &Announcement) -> Option<Vec<BFieldElement>> {
    if announcement.message.len() <= 2 {
        return None;
    }

    Some(announcement.message[2..].to_vec())
}
