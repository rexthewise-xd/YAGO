use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum IconName {
    Home,
    Mods,
    Profiles,
    Skins,
    Settings,
    Add,
    Launch,
    Deploy,
    Kill,
    Folder,
    Shield,
    Minimize,
    Maximize,
    Close,
    Search,
    Trash,
    Edit,
    Pause,
    Refresh,
    ChevronDown,
    ChevronRight,
    Check,
    Warning,
    Download,
    Image,
    Copy,
    Save,
    Power,
    Grid,
    List,
    Info,
    File,
    Tag,
    Drag,
}

impl IconName {
    pub fn path(self) -> &'static str {
        match self {
            Self::Home => "M3 11.5 12 4l9 7.5M5.5 10v9h13v-9M10 19v-5h4v5",
            Self::Mods => "M4 4h16v16H4zM12 4v16M8 9h1m6 0h1M8 15h1m6 0h1",
            Self::Profiles => "M12 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM5 20a7 7 0 0 1 14 0M18 6h3m-1.5-1.5v3",
            Self::Skins => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM8 10h.01M16 10h.01M8.5 15a4.5 4.5 0 0 0 7 0",
            Self::Settings => "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7ZM19 12l2-1-1-3-2 .3-1.5-1.2L16 4h-3l-.5 2.1L11 5.3 9.5 7 8 6.5 7 9l2 1.5L8.5 12 7 13.5 9 16l2-.5L12.5 17 13 19h3l.5-2 1.5-1.2L20 16l1-3-2-1Z",
            Self::Add => "M12 5v14M5 12h14",
            Self::Launch => "M8 5v14l11-7L8 5Z",
            Self::Deploy => "M12 3v13m0 0-5-5m5 5 5-5M4 20h16",
            Self::Kill => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM6 6l12 12",
            Self::Folder => "M3 6h7l2 2h9v11H3V6Z",
            Self::Shield => "M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Zm0 5v6m0 3h.01",
            Self::Minimize => "M5 12h14",
            Self::Maximize => "M5 5h14v14H5z",
            Self::Close => "M6 6l12 12M18 6 6 18",
            Self::Search => "M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM15.5 15.5 21 21",
            Self::Trash => "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6m4-6v6",
            Self::Edit => "M4 20h4L20 8l-4-4L4 16v4ZM13 7l4 4",
            Self::Pause => "M8 5v14M16 5v14",
            Self::Refresh => "M20 12a8 8 0 1 1-2.3-5.6M20 4v4h-4",
            Self::ChevronDown => "M6 9l6 6 6-6",
            Self::ChevronRight => "M9 6l6 6-6 6",
            Self::Check => "M5 13l4 4L19 7",
            Self::Warning => "M12 3 2 20h20L12 3Zm0 7v4m0 3h.01",
            Self::Download => "M12 3v12m0 0-4-4m4 4 4-4M4 17v3h16v-3",
            Self::Image => "M4 5h16v14H4zM8.5 9.5h.01M4 17l5-5 4 4 3-3 4 4",
            Self::Copy => "M9 9h11v11H9zM5 15H4V4h11v1",
            Self::Save => "M5 4h12l3 3v13H5V4ZM8 4v5h7V4M8 20v-6h8v6",
            Self::Power => "M12 3v8M6.3 6.3a8 8 0 1 0 11.4 0",
            Self::Grid => "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
            Self::List => "M8 6h13M8 12h13M8 18h13M4 6h.01M4 12h.01M4 18h.01",
            Self::Info => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM12 11v5m0-8h.01",
            Self::File => "M6 3h8l4 4v14H6V3ZM14 3v4h4",
            Self::Tag => "M3 3h8l10 10-8 8L3 11V3ZM7.5 7.5h.01",
            Self::Drag => "M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01",
        }
    }
}

#[component]
pub fn IconSvg(name: IconName, size: Option<u32>) -> Element {
    let size = size.unwrap_or(18);
    rsx! {
        svg {
            class: "icon",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: name.path() }
        }
    }
}