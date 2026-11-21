#![allow(dead_code)]

pub struct ThemeDef {
    pub id: &'static str,
    pub name: &'static str,
    pub engine: &'static str,
}

pub const THEMES: &[ThemeDef] = &[
    ThemeDef { id: "rose-pine", name: "Rosé Pine", engine: "Rosé Pine" },
    ThemeDef { id: "rose-pine-moon", name: "Rosé Pine Moon", engine: "Rosé Pine" },
    ThemeDef { id: "rose-pine-dawn", name: "Rosé Pine Dawn", engine: "Rosé Pine" },
    ThemeDef { id: "ctp-mocha", name: "Catppuccin Mocha", engine: "Catppuccin" },
    ThemeDef { id: "ctp-macchiato", name: "Catppuccin Macchiato", engine: "Catppuccin" },
    ThemeDef { id: "tokyo-storm", name: "Tokyo Night Storm", engine: "Tokyo Night" },
    ThemeDef { id: "tokyo-night", name: "Tokyo Night", engine: "Tokyo Night" },
    ThemeDef { id: "nord", name: "Nord Arctic", engine: "Elite" },
    ThemeDef { id: "dracula", name: "Dracula Vampire", engine: "Elite" },
    ThemeDef { id: "gruvbox-dark", name: "Gruvbox Dark", engine: "Elite" },
    ThemeDef { id: "one-dark", name: "One Dark Pro", engine: "Elite" },
    ThemeDef { id: "sakura", name: "Sakura Blossom", engine: "Gacha" },
    ThemeDef { id: "astral", name: "Astral Express", engine: "Gacha" },
    ThemeDef { id: "hollow", name: "Hollow Zero", engine: "Gacha" },
    ThemeDef { id: "synthwave", name: "Synthwave Neon", engine: "Gacha" },
];

pub const THEME_IDS: &[&str] = &[
    "rose-pine",
    "rose-pine-moon",
    "rose-pine-dawn",
    "ctp-mocha",
    "ctp-macchiato",
    "tokyo-storm",
    "tokyo-night",
    "nord",
    "dracula",
    "gruvbox-dark",
    "one-dark",
    "sakura",
    "astral",
    "hollow",
    "synthwave",
];