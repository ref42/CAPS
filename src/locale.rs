//! App-owned text which also needs to be selected before the view is created.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        if code == "zh" {
            Self::Chinese
        } else {
            Self::English
        }
    }

    pub fn search_placeholder(self) -> &'static str {
        match self {
            Self::English => "Song, artist, album",
            Self::Chinese => "歌曲、歌手、专辑",
        }
    }

    pub fn video_placeholder(self) -> &'static str {
        match self {
            Self::English => "Paste video URL",
            Self::Chinese => "粘贴视频链接",
        }
    }

    pub fn welcome(self) -> &'static str {
        match self {
            Self::English => "Search online music or import your local audio.",
            Self::Chinese => "搜索在线音乐或导入本地音频。",
        }
    }

    pub fn font_search_placeholder(self) -> &'static str {
        match self {
            Self::English => "Search fonts",
            Self::Chinese => "搜索字体",
        }
    }
}
