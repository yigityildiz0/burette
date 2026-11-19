//! Komut ve hata metnini yalnız bellekte türe çevirir; metnin kendisi hiçbir yerde saklanmaz.

use crate::model::{Etkinlik, HataTuru};

/// Komut metnini (kabuk satırı, sarmalayıcılar ya da kod hücresi dahil) etkinlik türüne çevirir.
/// İlk tanınan komut sözcüğü kazanır; hiçbiri tanınmazsa [`Etkinlik::Komut`].
pub fn komut_etkinligi(metin: &str) -> Etkinlik {
    let sozcukler: Vec<String> = metin
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':')))
        .filter(|s| !s.is_empty())
        .map(|s| {
            let k = s.to_ascii_lowercase();
            k.strip_suffix(".exe").map(str::to_string).unwrap_or(k)
        })
        .collect();

    for (i, s) in sozcukler.iter().enumerate() {
        let sonraki = sozcukler.get(i + 1).map(String::as_str).unwrap_or("");
        let ikinci = sozcukler.get(i + 2).map(String::as_str).unwrap_or("");
        if let Some(e) = iki_sozcuk(s, sonraki, ikinci) {
            return e;
        }
        if let Some(e) = tek_sozcuk(s) {
            return e;
        }
    }
    Etkinlik::Komut
}

fn iki_sozcuk(a: &str, b: &str, c: &str) -> Option<Etkinlik> {
    let e = match (a, b) {
        ("cargo", "test" | "nextest") => Etkinlik::Test,
        ("cargo", "build" | "check" | "clippy" | "fmt" | "doc" | "run") => Etkinlik::Derleme,
        ("cargo", "add" | "install" | "update" | "fetch") => Etkinlik::Kurulum,
        ("npm" | "pnpm" | "yarn" | "bun", "test") => Etkinlik::Test,
        ("npm" | "pnpm" | "yarn" | "bun", "run") => match c {
            "test" | "tests" | "e2e" => Etkinlik::Test,
            "build" | "compile" | "typecheck" | "lint" | "dev" => Etkinlik::Derleme,
            _ => Etkinlik::Komut,
        },
        ("npm" | "pnpm" | "yarn" | "bun", "install" | "i" | "add" | "ci") => Etkinlik::Kurulum,
        ("npm" | "pnpm" | "yarn" | "bun", "build") => Etkinlik::Derleme,
        ("go" | "dotnet" | "swift", "test") => Etkinlik::Test,
        ("go" | "dotnet" | "swift", "build" | "vet") => Etkinlik::Derleme,
        ("python" | "python3" | "py", "-m") if c == "pytest" || c == "unittest" => Etkinlik::Test,
        ("python" | "python3" | "py", "-m") if c == "pip" => Etkinlik::Kurulum,
        ("pip" | "pip3" | "uv", "install" | "add" | "sync") => Etkinlik::Kurulum,
        ("git", "grep") => Etkinlik::Arama,
        ("vite" | "next" | "astro", "build") => Etkinlik::Derleme,
        _ => return None,
    };
    Some(e)
}

fn tek_sozcuk(s: &str) -> Option<Etkinlik> {
    let e =
        match s {
            "pytest" | "vitest" | "jest" | "mocha" | "playwright" | "ctest" | "phpunit"
            | "rspec" => Etkinlik::Test,
            "tsc" | "make" | "cmake" | "ninja" | "msbuild" | "gradle" | "gradlew" | "mvn"
            | "rustc" | "gcc" | "g++" | "clang" | "cl" | "javac" | "webpack" | "esbuild"
            | "rollup" => Etkinlik::Derleme,
            "rg" | "grep" | "select-string" | "findstr" | "fd" | "ag" | "ack" | "find"
            | "where" => Etkinlik::Arama,
            "cat" | "type" | "get-content" | "gc" | "head" | "tail" | "less" | "more" | "ls"
            | "dir" | "get-childitem" | "gci" | "tree" | "stat" | "wc" | "get-item"
            | "test-path" | "nl" | "jq" => Etkinlik::Okuma,
            "set-content" | "out-file" | "add-content" | "new-item" | "mkdir" | "md" | "cp"
            | "copy" | "copy-item" | "mv" | "move" | "move-item" | "rm" | "del" | "remove-item"
            | "tee" | "touch" | "apply_patch" | "rename-item" => Etkinlik::Yazma,
            "git" | "gh" => Etkinlik::Surum,
            "curl" | "wget" | "invoke-webrequest" | "iwr" | "invoke-restmethod" | "irm" => {
                Etkinlik::Web
            }
            "winget" | "choco" | "scoop" | "brew" | "apt" | "apt-get" => Etkinlik::Kurulum,
            _ => return None,
        };
    Some(e)
}

/// Platform hata metni / kodundan hata türü çıkarır (metin saklanmaz).
pub fn hata_turu_metinden(metin: &str) -> HataTuru {
    let m = metin.to_ascii_lowercase();
    let var = |kaliplar: &[&str]| kaliplar.iter().any(|k| m.contains(k));
    if var(&[
        "429",
        "rate limit",
        "rate_limit",
        "ratelimit",
        "too many requests",
        "usage limit",
        "usage_limit",
        "quota",
        "resource_exhausted",
        "resource exhausted",
    ]) {
        HataTuru::HizSiniri
    } else if var(&[
        "context window",
        "context_window",
        "context length",
        "context_length",
        "maximum context",
        "too many tokens",
        "prompt is too long",
    ]) {
        HataTuru::BaglamAsimi
    } else if var(&[
        "timeout",
        "timed out",
        "time out",
        "deadline",
        "zaman aşımı",
    ]) {
        HataTuru::ZamanAsimi
    } else if var(&[
        "permission",
        "denied",
        "forbidden",
        "unauthorized",
        "401",
        "403",
        "not allowed",
        "sandbox",
    ]) {
        HataTuru::IzinReddi
    } else if var(&[
        "connection",
        "network",
        "dns",
        "stream disconnected",
        "stream_disconnected",
        "reset by peer",
        "unreachable",
        "http_connection",
        "response_stream",
        "502",
        "503",
        "504",
    ]) {
        HataTuru::Ag
    } else if var(&["tool", "exit code", "exit_code", "command failed"]) {
        HataTuru::AracHatasi
    } else {
        HataTuru::Bilinmiyor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn komutlar_dogru_ture_ayrilir() {
        assert_eq!(komut_etkinligi("cargo test --workspace"), Etkinlik::Test);
        assert_eq!(
            komut_etkinligi(r#"powershell.exe -NoProfile -Command "cargo clippy -- -D warnings""#),
            Etkinlik::Derleme
        );
        assert_eq!(komut_etkinligi("bash -lc 'rg -n foo src'"), Etkinlik::Arama);
        assert_eq!(komut_etkinligi("Get-Content x.txt"), Etkinlik::Okuma);
        assert_eq!(komut_etkinligi("git status --short"), Etkinlik::Surum);
        assert_eq!(komut_etkinligi("npm run build"), Etkinlik::Derleme);
        assert_eq!(komut_etkinligi("python -m pytest -q"), Etkinlik::Test);
        assert_eq!(komut_etkinligi("npm install"), Etkinlik::Kurulum);
        assert_eq!(komut_etkinligi("curl https://x.invalid"), Etkinlik::Web);
        assert_eq!(komut_etkinligi("./ozel-betik --bayrak"), Etkinlik::Komut);
        assert_eq!(
            komut_etkinligi(r#"const r = await tools.exec({cmd: "cargo test"})"#),
            Etkinlik::Test
        );
    }

    #[test]
    fn hata_metinleri_siniflanir() {
        assert_eq!(
            hata_turu_metinden("You've hit your usage limit"),
            HataTuru::HizSiniri
        );
        assert_eq!(
            hata_turu_metinden("HTTP 429 Too Many Requests"),
            HataTuru::HizSiniri
        );
        assert_eq!(
            hata_turu_metinden("context_window_exceeded"),
            HataTuru::BaglamAsimi
        );
        assert_eq!(
            hata_turu_metinden("request timed out"),
            HataTuru::ZamanAsimi
        );
        assert_eq!(hata_turu_metinden("403 Forbidden"), HataTuru::IzinReddi);
        assert_eq!(
            hata_turu_metinden("stream disconnected before completion"),
            HataTuru::Ag
        );
        assert_eq!(hata_turu_metinden("garip bir şey"), HataTuru::Bilinmiyor);
    }
}
