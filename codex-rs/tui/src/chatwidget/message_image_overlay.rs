//! Chat widget glue for message image overlays.

use super::*;
use crate::message_images::MessageImagePlacement;
use crate::message_images::layout_message_images;
use crate::pets::PetImageSupport;
use crate::pets::image_protocol::ImageProtocol;

impl ChatWidget {
    /// Images shown in the overlay: the live composer draft while it has
    /// attachments, otherwise the most recently submitted user message.
    pub(crate) fn overlay_message_images(&self) -> Vec<PathBuf> {
        let draft = self.bottom_pane.composer_local_images();
        if !draft.is_empty() {
            return draft.iter().map(|image| image.path.clone()).collect();
        }
        self.last_message_images.clone()
    }

    pub(crate) fn message_images_draw(
        &self,
        area: Rect,
        composer_bottom_y: u16,
        pet_top_y: Option<u16>,
    ) -> Option<(Vec<MessageImagePlacement>, ImageProtocol)> {
        if !self.bottom_pane.no_modal_or_popup_active() {
            return None;
        }
        let PetImageSupport::Supported(protocol) = self.message_image_support() else {
            return None;
        };
        if !matches!(
            protocol,
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile
        ) {
            return None;
        }
        let images = self.overlay_message_images();
        if images.is_empty() {
            return None;
        }
        let bottom_y = pet_top_y
            .map(|top| top.min(composer_bottom_y))
            .unwrap_or(composer_bottom_y);
        let placements = layout_message_images(area, bottom_y, &images);
        if placements.is_empty() {
            return None;
        }
        Some((placements, protocol))
    }

    pub(super) fn message_image_support(&self) -> PetImageSupport {
        #[cfg(test)]
        if let Some(support) = self.pet_image_support_override {
            return support;
        }

        #[cfg(test)]
        return PetImageSupport::Unsupported(crate::pets::PetImageUnsupportedReason::Terminal);

        #[cfg(not(test))]
        crate::pets::detect_pet_image_support()
    }

    pub(super) fn message_images_wrap_reserved_cols(&self) -> u16 {
        if self.overlay_message_images().is_empty() {
            return 0;
        }
        match self.message_image_support() {
            PetImageSupport::Supported(ImageProtocol::Kitty | ImageProtocol::KittyLocalFile) => {
                crate::message_images::overlay_reserved_columns()
            }
            PetImageSupport::Supported(ImageProtocol::Sixel) | PetImageSupport::Unsupported(_) => 0,
        }
    }

    pub(super) fn note_submitted_message_images(&mut self, images: &[LocalImageAttachment]) {
        if images.is_empty() {
            return;
        }
        self.last_message_images = images
            .iter()
            .take(crate::message_images::MAX_OVERLAY_IMAGES)
            .map(|image| image.path.clone())
            .collect();
    }
}
