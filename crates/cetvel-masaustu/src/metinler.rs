//! Rust tarafındaki kullanıcıya görünen metinler (tepsi menüsü ve tepsi ipucu) için tr/en sözlüğü.
//!
//! Arayüz 11 dil sunar; tepsi için yalnız Türkçe ve İngilizce bulunur. Türkçe dışındaki her dil
//! kodu (`de`, `ja`, `pt-BR` ...) İngilizceye düşer. Sözlük saftır (Tauri gerektirmez), test
//! edilebilir.

/// Tepsi metinlerinin dili.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TepsiDili {
    #[default]
    Tr,
    En,
}

impl TepsiDili {
    /// Ayarlardaki dil kodundan tepsi dilini seçer: `tr` Türkçe, diğer her şey İngilizce.
    pub fn koddan(kod: &str) -> Self {
        if kod == "tr" { Self::Tr } else { Self::En }
    }
}

/// Tepsi çubukları ve ipucunda gösterilecek yüzde yönü (`kalan` varsayılan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DegerYonu {
    #[default]
    Kalan,
    Kullanilan,
}

impl DegerYonu {
    /// Ayarlardaki değer yönünden (`kalan` / `kullanilan`) enum seçer; bilinmeyen değerde `Kalan`.
    pub fn koddan(kod: &str) -> Self {
        if kod == "kullanilan" {
            Self::Kullanilan
        } else {
            Self::Kalan
        }
    }
}

/// Tepsi menüsü öğelerinin metinleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuMetinleri {
    pub acilir: &'static str,
    pub pencere: &'static str,
    pub serit: &'static str,
    pub serit_gecirgen: &'static str,
    pub yenile: &'static str,
    pub servis_baslat: &'static str,
    pub otomatik_baslat: &'static str,
    pub cikis: &'static str,
}

pub fn menu_metinleri(dil: TepsiDili) -> MenuMetinleri {
    match dil {
        TepsiDili::Tr => MenuMetinleri {
            acilir: "Açılır Pencere",
            pencere: "Genişlet (Pencere)",
            serit: "Şerit Modu",
            serit_gecirgen: "Şerit Tıklama Geçirgen",
            yenile: "Yenile",
            servis_baslat: "Servisi Başlat",
            otomatik_baslat: "Otomatik Başlat",
            cikis: "Çıkış",
        },
        TepsiDili::En => MenuMetinleri {
            acilir: "Quick View",
            pencere: "Expand (Window)",
            serit: "Strip Mode",
            serit_gecirgen: "Strip Click-Through",
            yenile: "Refresh",
            servis_baslat: "Start Service",
            otomatik_baslat: "Start with Windows",
            cikis: "Exit",
        },
    }
}

pub fn ipucu_servis_calismiyor(dil: TepsiDili) -> &'static str {
    match dil {
        TepsiDili::Tr => "Cetvel · Servis çalışmıyor",
        TepsiDili::En => "Cetvel · Service not running",
    }
}

pub fn ipucu_veri_bekleniyor(dil: TepsiDili) -> &'static str {
    match dil {
        TepsiDili::Tr => "Cetvel · Servis bağlı (veri bekleniyor)",
        TepsiDili::En => "Cetvel · Service connected (waiting for data)",
    }
}

pub fn ipucu_veri_yok(dil: TepsiDili) -> &'static str {
    match dil {
        TepsiDili::Tr => "Cetvel · veri yok",
        TepsiDili::En => "Cetvel · no data",
    }
}

fn yuzde_metni(dil: TepsiDili, yuzde: u32, yon: DegerYonu) -> String {
    match (dil, yon) {
        (TepsiDili::Tr, DegerYonu::Kalan) => format!("%{yuzde} kalan"),
        (TepsiDili::Tr, DegerYonu::Kullanilan) => format!("%{yuzde} kullanıldı"),
        (TepsiDili::En, DegerYonu::Kalan) => format!("{yuzde}% left"),
        (TepsiDili::En, DegerYonu::Kullanilan) => format!("{yuzde}% used"),
    }
}

/// Claude 5 saatlik pencere özeti; `sifirlanma` yerel `HH:MM`.
pub fn ipucu_claude_5s(
    dil: TepsiDili,
    yuzde: u32,
    yon: DegerYonu,
    sifirlanma: Option<&str>,
    en_dar_mi: bool,
) -> String {
    let deger = yuzde_metni(dil, yuzde, yon);
    let mut metin = match dil {
        TepsiDili::Tr => format!("Claude 5s: {deger}"),
        TepsiDili::En => format!("Claude 5h: {deger}"),
    };
    if en_dar_mi {
        metin.push_str(match dil {
            TepsiDili::Tr => " (en dar)",
            TepsiDili::En => " (tightest)",
        });
    }
    if let Some(saat) = sifirlanma {
        match dil {
            TepsiDili::Tr => metin.push_str(&format!(" · {saat} sıfırlanır")),
            TepsiDili::En => metin.push_str(&format!(" · resets {saat}")),
        }
    }
    metin
}

/// En dar havuz özeti; `pencere` boşsa pencere adı yazılmaz.
pub fn ipucu_havuz(
    dil: TepsiDili,
    etiket: &str,
    pencere: &str,
    yuzde: u32,
    yon: DegerYonu,
) -> String {
    let ad = if pencere.is_empty() {
        etiket.to_string()
    } else {
        format!("{etiket} {pencere}")
    };
    let deger = yuzde_metni(dil, yuzde, yon);
    match dil {
        TepsiDili::Tr => format!("En dar: {ad}: {deger}"),
        TepsiDili::En => format!("Tightest: {ad}: {deger}"),
    }
}

/// Bağlayıcı pencere kodunun (`5s`, `hafta`, `gun`, `ay`) kısa gösterimi; bilinmeyen kod olduğu gibi.
pub fn pencere_kisa(dil: TepsiDili, kod: &str) -> &str {
    match (dil, kod) {
        (_, "5s") => match dil {
            TepsiDili::Tr => "5s",
            TepsiDili::En => "5h",
        },
        (TepsiDili::Tr, "hafta") => "hafta",
        (TepsiDili::En, "hafta") => "week",
        (TepsiDili::Tr, "gun") => "gün",
        (TepsiDili::En, "gun") => "day",
        (TepsiDili::Tr, "ay") => "ay",
        (TepsiDili::En, "ay") => "month",
        (_, diger) => diger,
    }
}

pub fn ipucu_havuz_sayisi(dil: TepsiDili, sayi: usize) -> String {
    match dil {
        TepsiDili::Tr => format!("Cetvel · {sayi} havuz"),
        TepsiDili::En => format!("Cetvel · {sayi} pools"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dil_kodu_turkce_disinda_ingilizceye_duser() {
        assert_eq!(TepsiDili::koddan("tr"), TepsiDili::Tr);
        assert_eq!(TepsiDili::koddan("en"), TepsiDili::En);
        for kod in [
            "de", "fr", "es", "pt-BR", "ru", "ja", "ko", "zh-Hans", "zh-Hant", "", "xx",
        ] {
            assert_eq!(TepsiDili::koddan(kod), TepsiDili::En, "{kod}");
        }
        assert_eq!(TepsiDili::default(), TepsiDili::Tr);
        assert_eq!(DegerYonu::default(), DegerYonu::Kalan);
        assert_eq!(DegerYonu::koddan("kalan"), DegerYonu::Kalan);
        assert_eq!(DegerYonu::koddan("kullanilan"), DegerYonu::Kullanilan);
        assert_eq!(DegerYonu::koddan(""), DegerYonu::Kalan);
        assert_eq!(DegerYonu::koddan("bilinmiyor"), DegerYonu::Kalan);
    }

    #[test]
    fn menu_metinleri_iki_dilde_dolu_ve_farklidir() {
        let tr = menu_metinleri(TepsiDili::Tr);
        let en = menu_metinleri(TepsiDili::En);
        let tr_liste = [
            tr.acilir,
            tr.pencere,
            tr.serit,
            tr.serit_gecirgen,
            tr.yenile,
            tr.servis_baslat,
            tr.otomatik_baslat,
            tr.cikis,
        ];
        let en_liste = [
            en.acilir,
            en.pencere,
            en.serit,
            en.serit_gecirgen,
            en.yenile,
            en.servis_baslat,
            en.otomatik_baslat,
            en.cikis,
        ];
        for (t, e) in tr_liste.iter().zip(en_liste.iter()) {
            assert!(!t.is_empty() && !e.is_empty());
            assert_ne!(t, e, "çeviri aynı kalmış: {t}");
        }
        assert_eq!(tr.cikis, "Çıkış");
        assert_eq!(en.cikis, "Exit");
        // Menü kimlikleri değil metinler çevrilir; öğeler birbirinden ayrışmalı.
        for liste in [tr_liste, en_liste] {
            let mut sirali = liste.to_vec();
            sirali.sort_unstable();
            sirali.dedup();
            assert_eq!(sirali.len(), liste.len());
        }
    }

    #[test]
    fn ipucu_parcalari_iki_dilde_bicimlenir() {
        assert!(ipucu_servis_calismiyor(TepsiDili::Tr).contains("çalışmıyor"));
        assert!(ipucu_servis_calismiyor(TepsiDili::En).contains("not running"));
        assert!(ipucu_veri_bekleniyor(TepsiDili::Tr).contains("veri bekleniyor"));
        assert!(ipucu_veri_bekleniyor(TepsiDili::En).contains("waiting for data"));
        assert_eq!(ipucu_veri_yok(TepsiDili::Tr), "Cetvel · veri yok");
        assert_eq!(ipucu_veri_yok(TepsiDili::En), "Cetvel · no data");
        assert_eq!(
            ipucu_claude_5s(TepsiDili::Tr, 80, DegerYonu::Kalan, Some("21:10"), false),
            "Claude 5s: %80 kalan · 21:10 sıfırlanır"
        );
        assert_eq!(
            ipucu_claude_5s(TepsiDili::En, 80, DegerYonu::Kalan, Some("21:10"), false),
            "Claude 5h: 80% left · resets 21:10"
        );
        assert_eq!(
            ipucu_claude_5s(TepsiDili::Tr, 80, DegerYonu::Kalan, None, true),
            "Claude 5s: %80 kalan (en dar)"
        );
        assert_eq!(
            ipucu_claude_5s(TepsiDili::En, 80, DegerYonu::Kalan, None, true),
            "Claude 5h: 80% left (tightest)"
        );
        assert_eq!(
            ipucu_claude_5s(TepsiDili::Tr, 20, DegerYonu::Kullanilan, None, false),
            "Claude 5s: %20 kullanıldı"
        );
        assert_eq!(
            ipucu_claude_5s(TepsiDili::En, 20, DegerYonu::Kullanilan, None, false),
            "Claude 5h: 20% used"
        );
        assert_eq!(
            ipucu_havuz(TepsiDili::Tr, "Codex", "hafta", 0, DegerYonu::Kalan),
            "En dar: Codex hafta: %0 kalan"
        );
        assert_eq!(
            ipucu_havuz(TepsiDili::En, "Codex", "week", 100, DegerYonu::Kullanilan),
            "Tightest: Codex week: 100% used"
        );
        assert_eq!(
            ipucu_havuz(TepsiDili::En, "DeepSeek", "", 40, DegerYonu::Kalan),
            "Tightest: DeepSeek: 40% left"
        );
        assert_eq!(ipucu_havuz_sayisi(TepsiDili::Tr, 3), "Cetvel · 3 havuz");
        assert_eq!(ipucu_havuz_sayisi(TepsiDili::En, 3), "Cetvel · 3 pools");
    }

    #[test]
    fn pencere_kisaltmalari_dile_gore_cevrilir_bilinmeyen_korunur() {
        assert_eq!(pencere_kisa(TepsiDili::Tr, "5s"), "5s");
        assert_eq!(pencere_kisa(TepsiDili::En, "5s"), "5h");
        assert_eq!(pencere_kisa(TepsiDili::Tr, "gun"), "gün");
        assert_eq!(pencere_kisa(TepsiDili::En, "gun"), "day");
        assert_eq!(pencere_kisa(TepsiDili::Tr, "hafta"), "hafta");
        assert_eq!(pencere_kisa(TepsiDili::En, "hafta"), "week");
        assert_eq!(pencere_kisa(TepsiDili::En, "ay"), "month");
        assert_eq!(pencere_kisa(TepsiDili::En, "30d"), "30d");
    }
}
