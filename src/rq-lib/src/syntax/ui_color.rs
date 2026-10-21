use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UiColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl UiColor {
    pub const ALL: [UiColor; 6] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.as_str() == name)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }

    pub fn names() -> String {
        Self::ALL
            .iter()
            .map(UiColor::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_accepts_every_palette_color() {
        for color in UiColor::ALL {
            assert_eq!(UiColor::from_name(color.as_str()), Some(color));
        }
    }

    #[test]
    fn from_name_rejects_names_outside_the_palette() {
        assert_eq!(UiColor::from_name("pink"), None);
    }

    #[test]
    fn from_name_is_case_sensitive() {
        assert_eq!(UiColor::from_name("Red"), None);
    }

    #[test]
    fn serializes_as_lowercase_name() {
        assert_eq!(
            serde_json::to_string(&UiColor::Purple).ok().as_deref(),
            Some("\"purple\"")
        );
    }
}
