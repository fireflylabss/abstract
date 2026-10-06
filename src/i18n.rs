//! Interface strings, English and Portuguese (Brazil). `t`/`tf` read the
//! active language from a process-wide atomic set at startup or from the
//! spaces-menu language row.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    PtBr,
}

impl Lang {
    /// Every implemented UI language, for cross-language matching.
    pub const ALL: [Lang; 2] = [Lang::En, Lang::PtBr];
}

/// Stored in settings key `lang`: `system` | `en` | `pt-BR`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LangPref {
    System,
    En,
    PtBr,
}

impl LangPref {
    pub fn parse(s: &str) -> Self {
        match s {
            "en" => Self::En,
            "pt-BR" | "pt" => Self::PtBr,
            _ => Self::System,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }

    /// Sistema → En → PtBr → Sistema (the menu row cycle).
    pub fn next(&self) -> Self {
        match self {
            Self::System => Self::En,
            Self::En => Self::PtBr,
            Self::PtBr => Self::System,
        }
    }
}

/// OS locale → `pt*` becomes Portuguese, everything else English.
pub fn detect() -> Lang {
    match sys_locale::get_locale() {
        Some(l) if l.to_lowercase().starts_with("pt") => Lang::PtBr,
        _ => Lang::En,
    }
}

static LANG: AtomicU8 = AtomicU8::new(1);

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        0 => Lang::En,
        _ => Lang::PtBr,
    }
}

#[derive(Clone, Copy)]
pub enum Key {
    // Toolbar / status
    Sidebar,
    Search,
    Theme,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    DeleteNote,
    Saving,
    SaveFailed,
    Words,
    // Sidebar
    NewNote,
    NewFolder,
    SwitchSpace,
    Notes,
    Rename,
    MoveToTrash,
    // Notes / notices / prompts
    TrashFailed,
    TrashFolderPrompt,
    TrashFolderHint,
    Cancel,
    FileRemovedOutside,
    Untitled,
    NameConflict,
    RenameFailed,
    RelinkFailed,
    CreateNoteFailed,
    // Search palette
    SearchPlaceholder,
    NoNotesFound,
    // Wiki-links / backlinks
    ReferencedBy,
    // Editor
    EditorPlaceholder,
    EditorAria,
    // Spaces menu
    Spaces,
    OpenFolderAsSpace,
    OpenAsSpace,
    RemoveFromList,
    Language,
    LangSystem,
    // Tour
    Tour1Title,
    Tour1Body,
    Tour2Title,
    Tour2Body,
    Tour3Title,
    Tour3Body,
    Tour4Title,
    Tour4Body,
    Tour5Title,
    Tour5Body,
    Tour6Title,
    Tour6Body,
    TourSkip,
    TourBack,
    TourNext,
    TourDone,
    TourStepOf,
    TourStepAria,
    Cut,
    Copy,
    Paste,
    PastePlain,
    SelectAll,
    AddWikiLink,
    AddLink,
    SearchSelection,
    FindInNote,
    FindPlaceholder,
    ReplacePlaceholder,
    FindCount,
    FindNone,
    FindPrev,
    FindNext,
    MatchCase,
    ToggleReplace,
    ReplaceOne,
    ReplaceAll,
    CloseFind,
    Replaced,
    Format,
    Bold,
    Italic,
    Strikethrough,
    InlineCode,
    Highlight,
    Comment,
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    PlainText,
    BulletList,
    NumberedList,
    TaskList,
    Quote,
    Callout,
    Insert,
    InsertImage,
    CodeBlock,
    Divider,
    Table,
    TableColumn,
    Footnote,
    AttachFailed,
    Open,
    NewNoteHere,
    NewFolderHere,
    Duplicate,
    CopyPath,
    CopyRelativePath,
    CopyNoteLink,
    Reveal,
    OpenDefault,
    NameTaken,
    NoteStatus,
    Saved,
    Characters,
    ReadingTime,
    SelectionWords,
    Modified,
    NotSavedYet,
    JustNow,
    MinutesAgo,
    HoursAgo,
    DaysAgo,
    // Updates
    CheckUpdates,
    UpdateAvailable,
    UpdateManual,
    UpdateInstalling,
    UpdateFailed,
    UpdateRestart,
    ReleaseNotes,
    Later,
    UpdatesOff,
    // Crash report
    CrashTitle,
    CrashBody,
    CrashRecovered,
    CopyReport,
    OpenIssue,
    Dismiss,
    // Settings
    Settings,
    SettingsTip,
    Appearance,
    LightTheme,
    DarkTheme,
    EditorSection,
    RawTables,
    RawTablesHint,
    General,
    About,
    AboutTagline,
    Version,
    SourceCode,
    ReportIssue,
    License,
    Close,
    ExportAsHtml,
    CopyAsHtml,
    Exported,
    ExportFailed,
    CopyFailed,
    Zoom,
    Font,
    FontHint,
    // Discord presence
    DiscordPresence,
    DiscordPresenceHint,
    BrowsingNotes,
    // Highlight colours
    HighlightColor,
    TintRed,
    TintOrange,
    TintYellow,
    TintGreen,
    TintBlue,
    TintPurple,
    // Table structure
    TableInsertRowAbove,
    TableInsertRowBelow,
    TableInsertColumnLeft,
    TableInsertColumnRight,
    TableDeleteRow,
    TableDeleteColumn,
    // Note tabs
    OpenInNewTab,
    CloseTab,
}

impl Key {
    #[cfg(test)]
    pub const ALL: &'static [Key] = &[
        Key::Sidebar,
        Key::Search,
        Key::Theme,
        Key::ThemeSystem,
        Key::ThemeLight,
        Key::ThemeDark,
        Key::DeleteNote,
        Key::Saving,
        Key::SaveFailed,
        Key::Words,
        Key::NewNote,
        Key::NewFolder,
        Key::SwitchSpace,
        Key::Notes,
        Key::Rename,
        Key::MoveToTrash,
        Key::TrashFailed,
        Key::TrashFolderPrompt,
        Key::TrashFolderHint,
        Key::Cancel,
        Key::FileRemovedOutside,
        Key::Untitled,
        Key::NameConflict,
        Key::RenameFailed,
        Key::RelinkFailed,
        Key::CreateNoteFailed,
        Key::SearchPlaceholder,
        Key::NoNotesFound,
        Key::ReferencedBy,
        Key::EditorPlaceholder,
        Key::EditorAria,
        Key::Spaces,
        Key::OpenFolderAsSpace,
        Key::OpenAsSpace,
        Key::RemoveFromList,
        Key::Language,
        Key::LangSystem,
        Key::Tour1Title,
        Key::Tour1Body,
        Key::Tour2Title,
        Key::Tour2Body,
        Key::Tour3Title,
        Key::Tour3Body,
        Key::Tour4Title,
        Key::Tour4Body,
        Key::Tour5Title,
        Key::Tour5Body,
        Key::Tour6Title,
        Key::Tour6Body,
        Key::TourSkip,
        Key::TourBack,
        Key::TourNext,
        Key::TourDone,
        Key::TourStepOf,
        Key::TourStepAria,
        Key::Cut,
        Key::Copy,
        Key::Paste,
        Key::PastePlain,
        Key::SelectAll,
        Key::AddWikiLink,
        Key::AddLink,
        Key::SearchSelection,
        Key::FindInNote,
        Key::FindPlaceholder,
        Key::ReplacePlaceholder,
        Key::FindCount,
        Key::FindNone,
        Key::FindPrev,
        Key::FindNext,
        Key::MatchCase,
        Key::ToggleReplace,
        Key::ReplaceOne,
        Key::ReplaceAll,
        Key::CloseFind,
        Key::Replaced,
        Key::Format,
        Key::Bold,
        Key::Italic,
        Key::Strikethrough,
        Key::InlineCode,
        Key::Highlight,
        Key::Comment,
        Key::Paragraph,
        Key::Heading1,
        Key::Heading2,
        Key::Heading3,
        Key::PlainText,
        Key::BulletList,
        Key::NumberedList,
        Key::TaskList,
        Key::Quote,
        Key::Callout,
        Key::Insert,
        Key::InsertImage,
        Key::CodeBlock,
        Key::Divider,
        Key::Table,
        Key::TableColumn,
        Key::Footnote,
        Key::AttachFailed,
        Key::Open,
        Key::NewNoteHere,
        Key::NewFolderHere,
        Key::Duplicate,
        Key::CopyPath,
        Key::CopyRelativePath,
        Key::CopyNoteLink,
        Key::Reveal,
        Key::OpenDefault,
        Key::NameTaken,
        Key::NoteStatus,
        Key::Saved,
        Key::Characters,
        Key::ReadingTime,
        Key::SelectionWords,
        Key::Modified,
        Key::NotSavedYet,
        Key::JustNow,
        Key::MinutesAgo,
        Key::HoursAgo,
        Key::DaysAgo,
        Key::CheckUpdates,
        Key::UpdateAvailable,
        Key::UpdateManual,
        Key::UpdateInstalling,
        Key::UpdateFailed,
        Key::UpdateRestart,
        Key::ReleaseNotes,
        Key::Later,
        Key::UpdatesOff,
        Key::CrashTitle,
        Key::CrashBody,
        Key::CrashRecovered,
        Key::CopyReport,
        Key::OpenIssue,
        Key::Dismiss,
        Key::Settings,
        Key::SettingsTip,
        Key::Appearance,
        Key::LightTheme,
        Key::DarkTheme,
        Key::EditorSection,
        Key::RawTables,
        Key::RawTablesHint,
        Key::General,
        Key::About,
        Key::AboutTagline,
        Key::Version,
        Key::SourceCode,
        Key::ReportIssue,
        Key::License,
        Key::Close,
        Key::ExportAsHtml,
        Key::CopyAsHtml,
        Key::Exported,
        Key::ExportFailed,
        Key::CopyFailed,
        Key::Zoom,
        Key::Font,
        Key::FontHint,
        Key::DiscordPresence,
        Key::DiscordPresenceHint,
        Key::BrowsingNotes,
        Key::HighlightColor,
        Key::TintRed,
        Key::TintOrange,
        Key::TintYellow,
        Key::TintGreen,
        Key::TintBlue,
        Key::TintPurple,
        Key::TableInsertRowAbove,
        Key::TableInsertRowBelow,
        Key::TableInsertColumnLeft,
        Key::TableInsertColumnRight,
        Key::TableDeleteRow,
        Key::TableDeleteColumn,
        Key::OpenInNewTab,
        Key::CloseTab,
    ];
}

fn en(k: Key) -> &'static str {
    match k {
        Key::Sidebar => "Sidebar ({MOD}+\\)",
        Key::Search => "Search notes ({MOD}+P)",
        Key::Theme => "Theme: {name} ({MOD}+Shift+L)",
        Key::ThemeSystem => "System",
        Key::ThemeLight => "Light",
        Key::ThemeDark => "Dark",
        Key::DeleteNote => "Delete note ({MOD}+Shift+Backspace)",
        Key::Saving => "Saving…",
        Key::SaveFailed => "Save failed",
        Key::Words => "{n} words",
        Key::NewNote => "New note ({MOD}+N)",
        Key::NewFolder => "New folder",
        Key::SwitchSpace => "Switch space ({MOD}+O)",
        Key::Notes => "NOTES",
        Key::Rename => "Rename",
        Key::MoveToTrash => "Move to Trash",
        Key::TrashFailed => "Could not move to Trash",
        Key::TrashFolderPrompt => "Move the folder “{name}” to Trash?",
        Key::TrashFolderHint => "Notes inside it go too.",
        Key::Cancel => "Cancel",
        Key::FileRemovedOutside => "File removed outside the app",
        Key::Untitled => "Untitled",
        Key::NameConflict => "An item with this name already exists",
        Key::RenameFailed => "Could not rename",
        Key::RelinkFailed => "Some links to this note could not be updated",
        Key::CreateNoteFailed => "Could not create the note",
        Key::SearchPlaceholder => "Search notes…",
        Key::NoNotesFound => "No notes found",
        Key::ReferencedBy => "Referenced by",
        Key::EditorPlaceholder => "Start writing…",
        Key::EditorAria => "Markdown editor",
        Key::Spaces => "SPACES",
        Key::OpenFolderAsSpace => "Open folder as space…",
        Key::OpenAsSpace => "Open as space",
        Key::RemoveFromList => "Remove from list (files stay)",
        Key::Language => "Language",
        Key::LangSystem => "System",
        Key::Tour1Title => "Spaces are real folders",
        Key::Tour1Body => {
            "Each space is a folder on your disk. Switch spaces or open another folder here ({MOD}+O)."
        }
        Key::Tour2Title => "Notes and folders",
        Key::Tour2Body => {
            "Create notes with {MOD}+N and organize them in folders. Everything becomes a plain .md file."
        }
        Key::Tour3Title => "Write in Markdown",
        Key::Tour3Body => "The first line becomes the title — and the file name.",
        Key::Tour4Title => "Auto-save",
        Key::Tour4Body => {
            "Everything saves by itself. {MOD}+S saves right away; deleting sends to Trash."
        }
        Key::Tour5Title => "Light, dark or system",
        Key::Tour5Body => "Switch the theme with {MOD}+Shift+L.",
        Key::Tour6Title => "Focus mode",
        Key::Tour6Body => "Hide the sidebar with {MOD}+\\. F1 reopens this tour.",
        Key::TourSkip => "Skip",
        Key::TourBack => "Back",
        Key::TourNext => "Next",
        Key::TourDone => "Done",
        Key::TourStepOf => "{step} of {n}",
        Key::TourStepAria => "Step {step} of {n}: {title}",
        Key::Cut => "Cut",
        Key::Copy => "Copy",
        Key::Paste => "Paste",
        Key::PastePlain => "Paste as plain text",
        Key::SelectAll => "Select all",
        Key::AddWikiLink => "Add note link",
        Key::AddLink => "Add external link",
        Key::SearchSelection => "Search for selection",
        Key::FindInNote => "Find in note…",
        Key::FindPlaceholder => "Find",
        Key::ReplacePlaceholder => "Replace with",
        Key::FindCount => "{i} of {n}",
        Key::FindNone => "No results",
        Key::FindPrev => "Previous match (Shift+Enter)",
        Key::FindNext => "Next match (Enter)",
        Key::MatchCase => "Match case",
        Key::ToggleReplace => "Replace",
        Key::ReplaceOne => "Replace (Enter)",
        Key::ReplaceAll => "Replace all ({MOD}+Enter)",
        Key::CloseFind => "Close (Esc)",
        Key::Replaced => "Replaced {n}",
        Key::Format => "Format",
        Key::Bold => "Bold",
        Key::Italic => "Italic",
        Key::Strikethrough => "Strikethrough",
        Key::InlineCode => "Code",
        Key::Highlight => "Highlight",
        Key::Comment => "Hidden comment",
        Key::Paragraph => "Paragraph",
        Key::Heading1 => "Heading 1",
        Key::Heading2 => "Heading 2",
        Key::Heading3 => "Heading 3",
        Key::PlainText => "Body text",
        Key::BulletList => "Bulleted list",
        Key::NumberedList => "Numbered list",
        Key::TaskList => "Checklist",
        Key::Quote => "Quote",
        Key::Callout => "Callout",
        Key::Insert => "Insert",
        Key::InsertImage => "Image…",
        Key::CodeBlock => "Code block",
        Key::Divider => "Divider",
        Key::Table => "Table",
        Key::TableColumn => "Column {n}",
        Key::Footnote => "Footnote",
        Key::AttachFailed => "Could not attach file",
        Key::Open => "Open",
        Key::NewNoteHere => "New note here",
        Key::NewFolderHere => "New folder here",
        Key::Duplicate => "Duplicate",
        Key::CopyPath => "Copy path",
        Key::CopyRelativePath => "Copy relative path",
        Key::CopyNoteLink => "Copy note link",
        Key::Reveal => {
            if cfg!(target_os = "macos") {
                "Show in Finder"
            } else {
                "Show in file manager"
            }
        }
        Key::OpenDefault => "Open with default app",
        Key::NameTaken => "Something with that name already exists there",
        Key::NoteStatus => "Note details",
        Key::Saved => "Saved",
        Key::Characters => "{n} characters",
        Key::ReadingTime => "{n} min read",
        Key::SelectionWords => "{n} selected",
        Key::Modified => "Modified {when}",
        Key::NotSavedYet => "Not saved yet",
        Key::JustNow => "just now",
        Key::MinutesAgo => "{n} min ago",
        Key::HoursAgo => "{n} h ago",
        Key::DaysAgo => "{n} d ago",
        Key::CheckUpdates => "Check for updates",
        Key::UpdateAvailable => "abstract {v} is available",
        Key::UpdateManual => "Update it the same way you installed it.",
        Key::UpdateInstalling => "Downloading and verifying…",
        Key::UpdateFailed => "Couldn't update: {err}",
        Key::UpdateRestart => "Update and restart",
        Key::ReleaseNotes => "What's new",
        Key::Later => "Later",
        Key::UpdatesOff => "Don't check",
        Key::CrashTitle => "abstract quit unexpectedly",
        Key::CrashBody => {
            "A report was saved on this computer and nothing was sent. Review it before sharing, since it can include bits of your note."
        }
        Key::CrashRecovered => "Text from a save that didn't finish was kept next to the report.",
        Key::CopyReport => "Copy report",
        Key::OpenIssue => "Open issue",
        Key::Dismiss => "Dismiss",
        Key::Settings => "Settings",
        Key::SettingsTip => "Settings ({MOD}+,)",
        Key::Appearance => "Appearance",
        Key::LightTheme => "Light theme",
        Key::DarkTheme => "Dark theme",
        Key::EditorSection => "Editor",
        Key::RawTables => "Raw tables",
        Key::RawTablesHint => "Show tables as Markdown source instead of a grid.",
        Key::General => "General",
        Key::About => "About",
        Key::AboutTagline => {
            "Local-first Markdown notes. Every note is a plain .md file in a folder you choose."
        }
        Key::Version => "Version {v}",
        Key::SourceCode => "Source code",
        Key::ReportIssue => "Report an issue",
        Key::License => "License",
        Key::Close => "Close",
        Key::ExportAsHtml => "Export as HTML…",
        Key::CopyAsHtml => "Copy as HTML",
        Key::Exported => "Exported {name}",
        Key::ExportFailed => "Couldn't export",
        Key::CopyFailed => "Couldn't copy",
        Key::Zoom => "Zoom {pct}%",
        Key::Font => "Font",
        Key::FontHint => "Interface and note typeface. Code stays in Noto Sans Mono.",
        Key::DiscordPresence => "Discord presence",
        Key::DiscordPresenceHint => "Show the note you're editing on your Discord profile.",
        Key::BrowsingNotes => "Browsing notes",
        Key::HighlightColor => "Highlight color",
        Key::TintRed => "Red",
        Key::TintOrange => "Orange",
        Key::TintYellow => "Yellow",
        Key::TintGreen => "Green",
        Key::TintBlue => "Blue",
        Key::TintPurple => "Purple",
        Key::TableInsertRowAbove => "Insert row above",
        Key::TableInsertRowBelow => "Insert row below",
        Key::TableInsertColumnLeft => "Insert column to the left",
        Key::TableInsertColumnRight => "Insert column to the right",
        Key::TableDeleteRow => "Delete row",
        Key::TableDeleteColumn => "Delete column",
        Key::OpenInNewTab => "Open in new tab",
        Key::CloseTab => "Close tab",
    }
}

fn pt(k: Key) -> &'static str {
    match k {
        Key::Sidebar => "Barra lateral ({MOD}+\\)",
        Key::Search => "Buscar notas ({MOD}+P)",
        Key::Theme => "Tema: {name} ({MOD}+Shift+L)",
        Key::ThemeSystem => "Sistema",
        Key::ThemeLight => "Claro",
        Key::ThemeDark => "Escuro",
        Key::DeleteNote => "Apagar nota ({MOD}+Shift+Backspace)",
        Key::Saving => "Salvando…",
        Key::SaveFailed => "Erro ao salvar",
        Key::Words => "{n} palavras",
        Key::NewNote => "Nova nota ({MOD}+N)",
        Key::NewFolder => "Nova pasta",
        Key::SwitchSpace => "Trocar de espaço ({MOD}+O)",
        Key::Notes => "NOTAS",
        Key::Rename => "Renomear",
        Key::MoveToTrash => "Mover para a Lixeira",
        Key::TrashFailed => "Não foi possível mover para a Lixeira",
        Key::TrashFolderPrompt => "Mover a pasta “{name}” para a Lixeira?",
        Key::TrashFolderHint => "As notas dentro dela também vão.",
        Key::Cancel => "Cancelar",
        Key::FileRemovedOutside => "Arquivo removido fora do app",
        Key::Untitled => "Sem título",
        Key::NameConflict => "Já existe um item com esse nome",
        Key::RenameFailed => "Não foi possível renomear",
        Key::RelinkFailed => "Alguns links para esta nota não puderam ser atualizados",
        Key::CreateNoteFailed => "Não foi possível criar a nota",
        Key::SearchPlaceholder => "Buscar notas…",
        Key::NoNotesFound => "Nenhuma nota encontrada",
        Key::ReferencedBy => "Referenciada por",
        Key::EditorPlaceholder => "Comece a escrever…",
        Key::EditorAria => "Editor Markdown",
        Key::Spaces => "ESPAÇOS",
        Key::OpenFolderAsSpace => "Abrir pasta como espaço…",
        Key::OpenAsSpace => "Abrir como espaço",
        Key::RemoveFromList => "Remover da lista (os arquivos ficam)",
        Key::Language => "Idioma",
        Key::LangSystem => "Sistema",
        Key::Tour1Title => "Espaços são pastas reais",
        Key::Tour1Body => {
            "Cada espaço é uma pasta no seu disco. Troque de espaço ou abra outra pasta aqui ({MOD}+O)."
        }
        Key::Tour2Title => "Notas e pastas",
        Key::Tour2Body => {
            "Crie notas com {MOD}+N e organize em pastas. Tudo vira arquivo .md comum."
        }
        Key::Tour3Title => "Escreva em Markdown",
        Key::Tour3Body => "A primeira linha vira o título — e o nome do arquivo.",
        Key::Tour4Title => "Salvamento automático",
        Key::Tour4Body => {
            "Tudo é salvo sozinho. {MOD}+S salva na hora; apagar envia para a Lixeira."
        }
        Key::Tour5Title => "Claro, escuro ou sistema",
        Key::Tour5Body => "Alterne o tema com {MOD}+Shift+L.",
        Key::Tour6Title => "Modo foco",
        Key::Tour6Body => "Esconda a barra lateral com {MOD}+\\. F1 reabre este tour.",
        Key::TourSkip => "Pular",
        Key::TourBack => "Voltar",
        Key::TourNext => "Próximo",
        Key::TourDone => "Concluir",
        Key::TourStepOf => "{step} de {n}",
        Key::TourStepAria => "Passo {step} de {n}: {title}",
        Key::Cut => "Recortar",
        Key::Copy => "Copiar",
        Key::Paste => "Colar",
        Key::PastePlain => "Colar como texto simples",
        Key::SelectAll => "Selecionar tudo",
        Key::AddWikiLink => "Adicionar link de nota",
        Key::AddLink => "Adicionar link externo",
        Key::SearchSelection => "Buscar pela seleção",
        Key::FindInNote => "Buscar na nota…",
        Key::FindPlaceholder => "Buscar",
        Key::ReplacePlaceholder => "Substituir por",
        Key::FindCount => "{i} de {n}",
        Key::FindNone => "Nenhum resultado",
        Key::FindPrev => "Resultado anterior (Shift+Enter)",
        Key::FindNext => "Próximo resultado (Enter)",
        Key::MatchCase => "Diferenciar maiúsculas",
        Key::ToggleReplace => "Substituir",
        Key::ReplaceOne => "Substituir (Enter)",
        Key::ReplaceAll => "Substituir tudo ({MOD}+Enter)",
        Key::CloseFind => "Fechar (Esc)",
        Key::Replaced => "{n} substituídos",
        Key::Format => "Formatar",
        Key::Bold => "Negrito",
        Key::Italic => "Itálico",
        Key::Strikethrough => "Tachado",
        Key::InlineCode => "Código",
        Key::Highlight => "Marca-texto",
        Key::Comment => "Comentário oculto",
        Key::Paragraph => "Parágrafo",
        Key::Heading1 => "Título 1",
        Key::Heading2 => "Título 2",
        Key::Heading3 => "Título 3",
        Key::PlainText => "Texto normal",
        Key::BulletList => "Lista com marcadores",
        Key::NumberedList => "Lista numerada",
        Key::TaskList => "Lista de tarefas",
        Key::Quote => "Citação",
        Key::Callout => "Destaque",
        Key::Insert => "Inserir",
        Key::InsertImage => "Imagem…",
        Key::CodeBlock => "Bloco de código",
        Key::Divider => "Divisória",
        Key::Table => "Tabela",
        Key::TableColumn => "Coluna {n}",
        Key::Footnote => "Nota de rodapé",
        Key::AttachFailed => "Não foi possível anexar o arquivo",
        Key::Open => "Abrir",
        Key::NewNoteHere => "Nova nota aqui",
        Key::NewFolderHere => "Nova pasta aqui",
        Key::Duplicate => "Duplicar",
        Key::CopyPath => "Copiar caminho",
        Key::CopyRelativePath => "Copiar caminho relativo",
        Key::CopyNoteLink => "Copiar link da nota",
        Key::Reveal => {
            if cfg!(target_os = "macos") {
                "Mostrar no Finder"
            } else {
                "Mostrar no gerenciador de arquivos"
            }
        }
        Key::OpenDefault => "Abrir com o app padrão",
        Key::NameTaken => "Já existe algo com esse nome lá",
        Key::NoteStatus => "Detalhes da nota",
        Key::Saved => "Salvo",
        Key::Characters => "{n} caracteres",
        Key::ReadingTime => "{n} min de leitura",
        Key::SelectionWords => "{n} selecionadas",
        Key::Modified => "Modificado {when}",
        Key::NotSavedYet => "Ainda não salvo",
        Key::JustNow => "agora mesmo",
        Key::MinutesAgo => "há {n} min",
        Key::HoursAgo => "há {n} h",
        Key::DaysAgo => "há {n} d",
        Key::CheckUpdates => "Procurar atualizações",
        Key::UpdateAvailable => "O abstract {v} está disponível",
        Key::UpdateManual => "Atualize pelo mesmo lugar em que você instalou.",
        Key::UpdateInstalling => "Baixando e verificando…",
        Key::UpdateFailed => "Não deu pra atualizar: {err}",
        Key::UpdateRestart => "Atualizar e reiniciar",
        Key::ReleaseNotes => "Novidades",
        Key::Later => "Depois",
        Key::UpdatesOff => "Não verificar",
        Key::CrashTitle => "O abstract fechou inesperadamente",
        Key::CrashBody => {
            "Um relatório foi salvo neste computador e nada foi enviado. Revise antes de compartilhar, porque ele pode conter trechos da nota."
        }
        Key::CrashRecovered => {
            "O texto de um salvamento que não terminou ficou guardado junto do relatório."
        }
        Key::CopyReport => "Copiar relatório",
        Key::OpenIssue => "Abrir issue",
        Key::Dismiss => "Fechar",
        Key::Settings => "Configurações",
        Key::SettingsTip => "Configurações ({MOD}+,)",
        Key::Appearance => "Aparência",
        Key::LightTheme => "Tema claro",
        Key::DarkTheme => "Tema escuro",
        Key::EditorSection => "Editor",
        Key::RawTables => "Tabelas cruas",
        Key::RawTablesHint => "Mostra as tabelas como Markdown, em vez de uma grade.",
        Key::General => "Geral",
        Key::About => "Sobre",
        Key::AboutTagline => {
            "Notas em Markdown, locais. Cada nota é um arquivo .md comum, numa pasta que você escolhe."
        }
        Key::Version => "Versão {v}",
        Key::SourceCode => "Código-fonte",
        Key::ReportIssue => "Reportar um problema",
        Key::License => "Licença",
        Key::Close => "Fechar",
        Key::ExportAsHtml => "Exportar como HTML…",
        Key::CopyAsHtml => "Copiar como HTML",
        Key::Exported => "{name} exportado",
        Key::ExportFailed => "Não foi possível exportar",
        Key::CopyFailed => "Não foi possível copiar",
        Key::Zoom => "Zoom {pct}%",
        Key::Font => "Fonte",
        Key::FontHint => "Tipo da interface e das notas. Código continua em Noto Sans Mono.",
        Key::DiscordPresence => "Presença no Discord",
        Key::DiscordPresenceHint => {
            "Mostra no seu perfil do Discord a nota que você está editando."
        }
        Key::BrowsingNotes => "Navegando nas notas",
        Key::HighlightColor => "Cor do marca-texto",
        Key::TintRed => "Vermelho",
        Key::TintOrange => "Laranja",
        Key::TintYellow => "Amarelo",
        Key::TintGreen => "Verde",
        Key::TintBlue => "Azul",
        Key::TintPurple => "Roxo",
        Key::TableInsertRowAbove => "Inserir linha acima",
        Key::TableInsertRowBelow => "Inserir linha abaixo",
        Key::TableInsertColumnLeft => "Inserir coluna à esquerda",
        Key::TableInsertColumnRight => "Inserir coluna à direita",
        Key::TableDeleteRow => "Apagar linha",
        Key::TableDeleteColumn => "Apagar coluna",
        Key::OpenInNewTab => "Abrir em nova aba",
        Key::CloseTab => "Fechar guia",
    }
}

/// Resolve `k` in `lang` directly — `t` without touching the global.
pub fn lookup(lang: Lang, k: Key) -> &'static str {
    match lang {
        Lang::En => en(k),
        Lang::PtBr => pt(k),
    }
}

pub fn t(k: Key) -> &'static str {
    lookup(current(), k)
}

/// `t` with `{name}` placeholders replaced from `args`. `{MOD}` expands to
/// `Cmd` on macOS, `Ctrl` elsewhere — same convention as the keymap.
pub fn tf(k: Key, args: &[(&str, &str)]) -> String {
    let mut s = t(k).to_string();
    s = s.replace("{MOD}", crate::keymap::MOD);
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_keys_filled_in_both_languages() {
        for &k in Key::ALL {
            assert!(!en(k).is_empty());
            assert!(!pt(k).is_empty());
            // Placeholder sets must match across languages.
            let ph = |s: &str| {
                let mut v: Vec<String> = s
                    .split('{')
                    .skip(1)
                    .filter_map(|x| x.split('}').next().map(String::from))
                    .collect();
                v.sort();
                v
            };
            assert_eq!(ph(en(k)), ph(pt(k)), "placeholder mismatch: {:?}", en(k));
        }
    }

    #[test]
    fn tf_replaces_placeholders() {
        // No `set` — the global language races with parallel tests; the
        // default is PtBr, and MOD differs per platform.
        assert_eq!(tf(Key::Words, &[("n", "5")]), "5 palavras");
        assert_eq!(lookup(Lang::En, Key::Words), "{n} words");
        assert!(tf(Key::Sidebar, &[]).contains(&format!("{}+\\", crate::keymap::MOD)));
    }

    #[test]
    fn shortcuts_use_the_platform_modifier() {
        for &k in Key::ALL {
            for lang in [Lang::En, Lang::PtBr] {
                assert!(
                    !lookup(lang, k).contains("Ctrl"),
                    "hard-coded Ctrl: {}",
                    en(k)
                );
            }
        }
    }
}
