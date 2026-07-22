use std::path::Path;

use tree_sitter::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LanguageKind {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    CSharp,
    Java,
    Css,
    Html,
    Vue,
    Ruby,
    Php,
    Shell,
    Lua,
    Kotlin,
    Swift,
    Yaml,
    Dockerfile,
    Json,
    Toml,
    Markdown,
}

pub(crate) fn detect_language(file_path: &Path, text: Option<&str>) -> Option<LanguageKind> {
    detect_language_by_file_name(file_path)
        .or_else(|| detect_language_by_extension(file_path))
        .or_else(|| detect_language_by_content(text?))
}

fn detect_language_by_extension(file_path: &Path) -> Option<LanguageKind> {
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();

    match extension.as_str() {
        "rs" => Some(LanguageKind::Rust),
        "ts" | "mts" | "cts" | "ets" => Some(LanguageKind::TypeScript),
        "tsx" => Some(LanguageKind::Tsx),
        "js" | "mjs" | "cjs" | "jsx" => Some(LanguageKind::JavaScript),
        "py" | "pyw" => Some(LanguageKind::Python),
        "go" => Some(LanguageKind::Go),
        "c" => Some(LanguageKind::C),
        "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(LanguageKind::Cpp),
        "cs" => Some(LanguageKind::CSharp),
        "java" => Some(LanguageKind::Java),
        "css" => Some(LanguageKind::Css),
        "html" | "htm" => Some(LanguageKind::Html),
        "vue" => Some(LanguageKind::Vue),
        "rb" | "rake" | "gemspec" => Some(LanguageKind::Ruby),
        "php" | "phtml" | "php3" | "php4" | "php5" | "phps" => Some(LanguageKind::Php),
        "sh" | "bash" | "bats" | "zsh" => Some(LanguageKind::Shell),
        "lua" => Some(LanguageKind::Lua),
        "kt" | "kts" => Some(LanguageKind::Kotlin),
        "swift" => Some(LanguageKind::Swift),
        "yaml" | "yml" => Some(LanguageKind::Yaml),
        "dockerfile" => Some(LanguageKind::Dockerfile),
        "json" => Some(LanguageKind::Json),
        "toml" => Some(LanguageKind::Toml),
        "md" | "markdown" => Some(LanguageKind::Markdown),
        _ => None,
    }
}

fn detect_language_by_file_name(file_path: &Path) -> Option<LanguageKind> {
    let file_name = file_path.file_name()?.to_str()?.to_ascii_lowercase();

    match file_name.as_str() {
        "dockerfile" | "containerfile" => Some(LanguageKind::Dockerfile),
        "gemfile" | "rakefile" | "guardfile" | "podfile" | "capfile" => Some(LanguageKind::Ruby),
        _ => None,
    }
}

fn detect_language_by_content(text: &str) -> Option<LanguageKind> {
    let first_line = text.lines().next()?.trim().to_ascii_lowercase();

    if !first_line.starts_with("#!") {
        return None;
    }

    if first_line.contains("python") {
        Some(LanguageKind::Python)
    } else if first_line.contains("node") || first_line.contains("deno") {
        Some(LanguageKind::JavaScript)
    } else if first_line.contains("ruby") {
        Some(LanguageKind::Ruby)
    } else if first_line.contains("bash")
        || first_line.contains("/sh")
        || first_line.contains("zsh")
    {
        Some(LanguageKind::Shell)
    } else if first_line.contains("lua") {
        Some(LanguageKind::Lua)
    } else {
        None
    }
}

impl LanguageKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Css => "css",
            Self::Html => "html",
            Self::Vue => "vue",
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Shell => "shell",
            Self::Lua => "lua",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Yaml => "yaml",
            Self::Dockerfile => "dockerfile",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
        }
    }

    pub(crate) fn tree_sitter_language(self) -> Option<Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Self::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Self::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Self::Go => Some(tree_sitter_go::LANGUAGE.into()),
            Self::C => Some(tree_sitter_c::LANGUAGE.into()),
            Self::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
            Self::CSharp => Some(tree_sitter_c_sharp::LANGUAGE.into()),
            Self::Java => Some(tree_sitter_java::LANGUAGE.into()),
            Self::Css => Some(tree_sitter_css::LANGUAGE.into()),
            Self::Html => Some(tree_sitter_htmlx::LANGUAGE.into()),
            Self::Vue => Some(tree_sitter_vue_updated::language()),
            Self::Ruby => Some(tree_sitter_ruby::LANGUAGE.into()),
            Self::Php => Some(tree_sitter_php::LANGUAGE_PHP.into()),
            Self::Shell => Some(tree_sitter_bash::LANGUAGE.into()),
            Self::Lua => Some(tree_sitter_lua::LANGUAGE.into()),
            Self::Kotlin => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
            Self::Swift => Some(tree_sitter_swift::LANGUAGE.into()),
            Self::Yaml => Some(tree_sitter_yaml::LANGUAGE.into()),
            Self::Dockerfile => Some(tree_sitter_containerfile::LANGUAGE.into()),
            Self::Json => Some(tree_sitter_json::LANGUAGE.into()),
            Self::Toml => Some(tree_sitter_toml_ng::LANGUAGE.into()),
            Self::Markdown => None,
        }
    }
}
