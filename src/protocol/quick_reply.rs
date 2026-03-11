use serde::{Deserialize, Serialize};

/// A clickable button attached to a bot message.
///
/// When clicked, the button's `action` text is injected as a user message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuickReplyButton {
    /// Display text on the button.
    pub label: String,
    /// The text to inject as user input when clicked.
    pub action: String,
    /// Visual style for rendering.
    #[serde(default)]
    pub style: ButtonStyle,
}

/// Visual style for quick reply buttons.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ButtonStyle {
    #[default]
    Primary,
    Secondary,
    Subtle,
}
