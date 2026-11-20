//! Sağlayıcının ham pencere etiketinden (`"5h"`, `"Weekly"`, `"PT5H"`, `"gemini-argon-wtu-weekly"`)
//! standart Cetvel pencere anahtarı ve süresi türetir.
//!
//! Standart anahtarlar: `5dk`, `1s`, `5s`, `gun`, `hafta`, `ay`. Bilinmeyen ama çözülebilen süre
//! `"<N>dk"` olur; süre hiç çıkarılamazsa anahtar etiketin slug'ı, süre `0` (arayüz "süre
//! bilinmiyor" diye ele alır).

use std::collections::BTreeMap;

use crate::anlik::PencereOlcum;

/// Bilinen tam süreler (dakika) ve standart anahtarları.
const BILINEN_SURELER: [(u64, &str); 6] = [
    (5, "5dk"),
    (60, "1s"),
    (300, "5s"),
    (1440, "gun"),
    (10080, "hafta"),
    (43200, "ay"),
];

/// Süre (dakika) için standart anahtar; bilinmeyen süre `"<N>dk"`.
pub fn pencere_anahtari_dakikadan(dk: u64) -> String {
    BILINEN_SURELER
        .iter()
        .find(|(sure, _)| *sure == dk)
        .map(|(_, ad)| (*ad).to_string())
        .unwrap_or_else(|| format!("{dk}dk"))
}

/// Etiketi küçük harfli ASCII slug'a çevirir (`"Gemini 5 Xenon"` → `"gemini_5_xenon"`).
/// Türkçe harfler sadeleştirilir, en fazla 40 karakter; boşsa `"bilinmiyor"`.
pub fn slug(etiket: &str) -> String {
    let mut sonuc = String::new();
    let mut alt_cizgi_bekliyor = false;
    for c in etiket.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ı' => 'i',
            'ş' => 's',
            'ğ' => 'g',
            'ü' => 'u',
            'ö' => 'o',
            'ç' => 'c',
            'â' => 'a',
            'î' => 'i',
            'û' => 'u',
            diger => diger,
        };
        if c.is_ascii_alphanumeric() {
            if alt_cizgi_bekliyor && !sonuc.is_empty() {
                sonuc.push('_');
            }
            alt_cizgi_bekliyor = false;
            sonuc.push(c);
        } else {
            alt_cizgi_bekliyor = true;
        }
        if sonuc.len() >= 40 {
            break;
        }
    }
    if sonuc.is_empty() {
        "bilinmiyor".to_string()
    } else {
        sonuc
    }
}

/// Ham pencere etiketinden `(anahtar, uzunluk_dk)` türetir.
///
/// `sure_ipucu` (dakika, ör. Codex `windowDurationMins`) varsa ve sıfırdan büyükse sayısal kaynak
/// esas alınır; yoksa etiket çözülür: `5h`/`5-hour`/`session` → `5s`; `weekly`/`7d`/`week` →
/// `hafta`; `daily`/`24h`/`day` → `gun`; `5m` → `5dk`; `monthly`/`30d` → `ay`; `1h`/`hourly` →
/// `1s`; `Ns`/`Nm`/`Nh`/`Nd` (s = saat) ve ISO-8601 süreleri (`PT5H`, `P7D`) → bilinen anahtar
/// ya da `"<N>dk"`. Hiçbiri değilse `(slug(etiket), 0)`.
pub fn pencere_anahtari(etiket: &str, sure_ipucu: Option<u64>) -> (String, u64) {
    if let Some(dk) = sure_ipucu.filter(|d| *d > 0) {
        return (pencere_anahtari_dakikadan(dk), dk);
    }
    match etiket_suresi_dk(etiket) {
        Some(dk) if dk > 0 => (pencere_anahtari_dakikadan(dk), dk),
        _ => (slug(etiket), 0),
    }
}

/// Etiketten süreyi (dakika) çıkarır; çıkarılamazsa `None`.
pub fn etiket_suresi_dk(etiket: &str) -> Option<u64> {
    let temiz = etiket.trim().to_lowercase();
    if temiz.is_empty() {
        return None;
    }
    if let Some(dk) = iso8601_sure_dk(&temiz) {
        return Some(dk);
    }
    let parcalar: Vec<String> = temiz
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            p.replace('ı', "i")
                .replace('ş', "s")
                .replace('ğ', "g")
                .replace('ü', "u")
                .replace('ö', "o")
                .replace('ç', "c")
        })
        .collect();
    for (i, parca) in parcalar.iter().enumerate() {
        // Bitişik sayı + birim (`5h`, `7d`, `30d`, `24h`, `5dk`)
        if let Some(dk) = bitisik_sure_dk(parca) {
            return Some(dk);
        }
        // Ayrık sayı + birim (`5 hour`, `five-hour`, `7 day`)
        if let Some(sayi) = sayi_coz(parca)
            && let Some(birim) = parcalar.get(i + 1).and_then(|b| birim_dk(b))
        {
            return Some(sayi.saturating_mul(birim));
        }
        if let Some(dk) = kelime_suresi_dk(parca) {
            return Some(dk);
        }
    }
    None
}

fn kelime_suresi_dk(kelime: &str) -> Option<u64> {
    Some(match kelime {
        "session" | "oturum" => 300,
        "weekly" | "week" | "hafta" | "haftalik" | "wk" => 10080,
        "daily" | "day" | "gun" | "gunluk" => 1440,
        "monthly" | "month" | "ay" | "aylik" => 43200,
        "hourly" | "saatlik" => 60,
        _ => return None,
    })
}

fn birim_dk(birim: &str) -> Option<u64> {
    Some(match birim {
        "m" | "min" | "mins" | "minute" | "minutes" | "dk" | "dakika" => 1,
        "s" | "sa" | "saat" | "h" | "hr" | "hrs" | "hour" | "hours" => 60,
        "d" | "day" | "days" | "g" | "gun" => 1440,
        "w" | "week" | "weeks" | "hafta" => 10080,
        "mo" | "month" | "months" | "ay" => 43200,
        _ => return None,
    })
}

fn sayi_coz(s: &str) -> Option<u64> {
    if let Ok(n) = s.parse::<u64>() {
        return (n > 0 && n <= 100_000).then_some(n);
    }
    Some(match s {
        "one" | "bir" => 1,
        "two" | "iki" => 2,
        "three" | "uc" => 3,
        "four" | "dort" => 4,
        "five" | "bes" => 5,
        "six" | "alti" => 6,
        "seven" | "yedi" => 7,
        "ten" => 10,
        "twelve" => 12,
        "thirty" | "otuz" => 30,
        _ => return None,
    })
}

fn bitisik_sure_dk(parca: &str) -> Option<u64> {
    let rakam_sonu = parca.find(|c: char| !c.is_ascii_digit())?;
    if rakam_sonu == 0 {
        return None;
    }
    let sayi = sayi_coz(&parca[..rakam_sonu])?;
    let birim = birim_dk(&parca[rakam_sonu..])?;
    Some(sayi.saturating_mul(birim))
}

/// Basit ISO-8601 süre çözümü (`P7D`, `PT5H`, `PT300M`, `P1W`, `P1DT12H`).
fn iso8601_sure_dk(s: &str) -> Option<u64> {
    let govde = s.strip_prefix('p')?;
    if govde.is_empty() {
        return None;
    }
    let mut toplam: u64 = 0;
    let mut zaman_kismi = false;
    let mut sayi = String::new();
    let mut herhangi = false;
    for c in govde.chars() {
        match c {
            't' => {
                if !sayi.is_empty() {
                    return None;
                }
                zaman_kismi = true;
            }
            '0'..='9' => sayi.push(c),
            _ => {
                let n: u64 = sayi.parse().ok()?;
                sayi.clear();
                let carpan = match (c, zaman_kismi) {
                    ('w', false) => 10080,
                    ('d', false) => 1440,
                    ('m', false) => 43200,
                    ('h', true) => 60,
                    ('m', true) => 1,
                    ('s', true) => {
                        // Saniye: yalnız tam dakikaya katkı.
                        toplam = toplam.saturating_add(n / 60);
                        herhangi = true;
                        continue;
                    }
                    _ => return None,
                };
                toplam = toplam.saturating_add(n.saturating_mul(carpan));
                herhangi = true;
            }
        }
    }
    (sayi.is_empty() && herhangi && toplam > 0).then_some(toplam)
}

/// Pencereyi ezmeden ekler: aynı anahtarda farklı süreli bir pencere varsa yeni ölçüm
/// `"<N>dk"` anahtarına (o da doluysa `"<N>dk_2"`…) yazılır. Aynı süreli kayıt varsa en
/// kısıtlayıcı (en düşük kalan) korunur. Yazılan anahtarı döndürür.
pub fn pencere_cakismasiz_ekle(
    pencereler: &mut BTreeMap<String, PencereOlcum>,
    anahtar: String,
    olcum: PencereOlcum,
) -> String {
    match pencereler.get_mut(&anahtar) {
        None => {
            pencereler.insert(anahtar.clone(), olcum);
            anahtar
        }
        Some(mevcut) if mevcut.uzunluk_dk == olcum.uzunluk_dk => {
            if olcum.kalan < mevcut.kalan {
                *mevcut = olcum;
            }
            anahtar
        }
        Some(_) => {
            let taban = format!("{}dk", olcum.uzunluk_dk);
            let mut aday = taban.clone();
            let mut n = 2;
            while let Some(m) = pencereler.get(&aday) {
                if m.uzunluk_dk == olcum.uzunluk_dk {
                    break;
                }
                aday = format!("{taban}_{n}");
                n += 1;
            }
            match pencereler.get_mut(&aday) {
                Some(m) if olcum.kalan < m.kalan => *m = olcum,
                Some(_) => {}
                None => {
                    pencereler.insert(aday.clone(), olcum);
                }
            }
            aday
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(etiket: &str) -> (String, u64) {
        pencere_anahtari(etiket, None)
    }

    #[test]
    fn standart_etiketler_bilinen_anahtarlara_eslenir() {
        for e in [
            "5h",
            "5-hour",
            "5 hour limit",
            "Five-Hour Limit Remaining",
            "session",
            "Current session",
            "gemini-5h",
            "3p-5h",
            "5s",
            "PT5H",
        ] {
            assert_eq!(a(e), ("5s".to_string(), 300), "{e}");
        }
        for e in [
            "weekly",
            "Weekly",
            "7d",
            "week",
            "gemini-argon-wtu-weekly",
            "Weekly · all models",
            "P7D",
            "P1W",
            "seven_day",
        ] {
            assert_eq!(a(e), ("hafta".to_string(), 10080), "{e}");
        }
        for e in ["daily", "24h", "day", "1d", "P1D"] {
            assert_eq!(a(e), ("gun".to_string(), 1440), "{e}");
        }
        for e in ["monthly", "30d", "month"] {
            assert_eq!(a(e), ("ay".to_string(), 43200), "{e}");
        }
        for e in ["1h", "hourly", "60m", "PT1H"] {
            assert_eq!(a(e), ("1s".to_string(), 60), "{e}");
        }
        assert_eq!(a("5m"), ("5dk".to_string(), 5));
        assert_eq!(a("5dk"), ("5dk".to_string(), 5));
    }

    #[test]
    fn bilinmeyen_sure_dakika_anahtari_olur() {
        assert_eq!(a("8h"), ("480dk".to_string(), 480));
        assert_eq!(a("2d"), ("2880dk".to_string(), 2880));
        assert_eq!(a("PT90M"), ("90dk".to_string(), 90));
        assert_eq!(a("P1DT12H"), ("2160dk".to_string(), 2160));
        assert_eq!(a("3 hours"), ("180dk".to_string(), 180));
    }

    #[test]
    fn sure_ipucu_sayisal_kaynak_olarak_esas_alinir() {
        assert_eq!(pencere_anahtari("", Some(300)), ("5s".to_string(), 300));
        assert_eq!(
            pencere_anahtari("weekly", Some(480)),
            ("480dk".to_string(), 480)
        );
        assert_eq!(
            pencere_anahtari("weekly", Some(0)),
            ("hafta".to_string(), 10080)
        );
    }

    #[test]
    fn cozulemeyen_etiket_slug_ve_sifir_sure() {
        assert_eq!(a("Burst Credits"), ("burst_credits".to_string(), 0));
        assert_eq!(a("Özel Kota"), ("ozel_kota".to_string(), 0));
        assert_eq!(a("   "), ("bilinmiyor".to_string(), 0));
        // `3p` ve `4` birim değil: yanlışlıkla süre üretmez.
        assert_eq!(a("3p"), ("3p".to_string(), 0));
        assert_eq!(a("Gemini 4 Argon"), ("gemini_4_argon".to_string(), 0));
    }

    #[test]
    fn slug_turkce_ve_sinir() {
        assert_eq!(slug("Gemini 5 Xenon"), "gemini_5_xenon");
        assert_eq!(slug("Claude and GPT models"), "claude_and_gpt_models");
        assert_eq!(slug("  Şirin--Ğüç  "), "sirin_guc");
        assert!(slug(&"x".repeat(100)).len() <= 40);
    }

    #[test]
    fn dakikadan_anahtar_tam_eslesir() {
        assert_eq!(pencere_anahtari_dakikadan(300), "5s");
        assert_eq!(pencere_anahtari_dakikadan(480), "480dk");
        assert_eq!(pencere_anahtari_dakikadan(10080), "hafta");
        assert_eq!(pencere_anahtari_dakikadan(43200), "ay");
    }

    #[test]
    fn cakisan_anahtar_ezmez() {
        let olcum = |dk: u64, kalan: f64| PencereOlcum {
            uzunluk_dk: dk,
            kullanilan: 1.0 - kalan,
            kalan,
            sifirlama: None,
            etiket: None,
        };
        let mut m = BTreeMap::new();
        assert_eq!(
            pencere_cakismasiz_ekle(&mut m, "gun".into(), olcum(1440, 0.5)),
            "gun"
        );
        // Aynı süre: en kısıtlayıcı korunur.
        pencere_cakismasiz_ekle(&mut m, "gun".into(), olcum(1440, 0.3));
        assert!((m["gun"].kalan - 0.3).abs() < 1e-9);
        // Farklı süre aynı ada: ezmez, dakika anahtarına yazılır.
        assert_eq!(
            pencere_cakismasiz_ekle(&mut m, "gun".into(), olcum(1500, 0.9)),
            "1500dk"
        );
        assert_eq!(m.len(), 2);
    }
}
