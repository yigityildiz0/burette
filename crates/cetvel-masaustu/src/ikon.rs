//! Plan §9.1 uyarınca 16 px mantıksal iki çubuklu tepsi ikonu (`tiny-skia`) ve ipucu metni üretimi.

use cetvel_model::{AnlikV2, Durum, UtcZaman};
use tauri::image::Image;
use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::metinler::{self, DegerYonu, TepsiDili};

/// İki çubuklu tepsi ikonu için özetlenmiş kullanım/kalan verisi.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TepsiVerisi {
    pub servis_calisiyor: bool,
    pub oran_5s: Option<f64>,
    pub oran_hafta: Option<f64>,
    pub bayat: bool,
    pub deger_yonu: DegerYonu,
}

/// `AnlikV2` anlık verisinden en sıkışık `5s` ve `hafta` oranlarını seçilen yöne göre çıkarır.
pub fn tepsi_verisi_cikar(
    anlik: Option<&AnlikV2>,
    servis_calisiyor: bool,
    deger_yonu: DegerYonu,
) -> TepsiVerisi {
    if !servis_calisiyor {
        return TepsiVerisi {
            deger_yonu,
            ..TepsiVerisi::default()
        };
    }
    let Some(anlik) = anlik else {
        return TepsiVerisi {
            servis_calisiyor: true,
            deger_yonu,
            ..TepsiVerisi::default()
        };
    };

    let mut ok_sayisi = 0usize;
    let mut bayat_sayisi = 0usize;
    let mut en_dar_5s: Option<(f64, f64)> = None;
    let mut en_dar_hafta: Option<(f64, f64)> = None;

    // Kullanım sayacı (OpenCode, dsh) kota değildir: çubuğa da bayatlık kararına da katılmaz.
    for havuz in anlik.havuzlar.values().filter(|h| !h.kullanim_sayaci_mi()) {
        match havuz.durum {
            Durum::Ok => {
                ok_sayisi += 1;
            }
            Durum::Bayat => {
                bayat_sayisi += 1;
            }
            _ => continue,
        }
        if let Some(olcum) = havuz.pencereler.get("5s") {
            let k = olcum.kalan.clamp(0.0, 1.0);
            let u = olcum.kullanilan.clamp(0.0, 1.0);
            if en_dar_5s.is_none_or(|(prev_k, _)| k < prev_k) {
                en_dar_5s = Some((k, u));
            }
        }
        if let Some(olcum) = havuz.pencereler.get("hafta") {
            let k = olcum.kalan.clamp(0.0, 1.0);
            let u = olcum.kullanilan.clamp(0.0, 1.0);
            if en_dar_hafta.is_none_or(|(prev_k, _)| k < prev_k) {
                en_dar_hafta = Some((k, u));
            }
        }
    }

    let sec = |(k, u): (f64, f64)| match deger_yonu {
        DegerYonu::Kalan => k,
        DegerYonu::Kullanilan => u,
    };

    TepsiVerisi {
        servis_calisiyor: true,
        oran_5s: en_dar_5s.map(sec),
        oran_hafta: en_dar_hafta.map(sec),
        bayat: ok_sayisi == 0 && bayat_sayisi > 0,
        deger_yonu,
    }
}

/// Görev çubuğu (sistem) teması: ikon rengi buna göre seçilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GorevCubuguTemasi {
    Koyu,
    Acik,
}

/// `HKCU\...\Themes\Personalize\SystemUsesLightTheme` (1 = açık görev çubuğu). Okunamazsa koyu
/// (Windows 11 varsayılanı). Her çizimde okunur: tema değişimi bir sonraki yenilemede yansır.
pub fn gorev_cubugu_temasi() -> GorevCubuguTemasi {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let deger: Option<u32> = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .ok()
        .and_then(|k| k.get_value("SystemUsesLightTheme").ok());
    tema_kayit_degerinden(deger)
}

/// Kayıt değerinden tema (saf).
pub fn tema_kayit_degerinden(deger: Option<u32>) -> GorevCubuguTemasi {
    match deger {
        Some(1) => GorevCubuguTemasi::Acik,
        _ => GorevCubuguTemasi::Koyu,
    }
}

/// Kullanım oranının görsel bandı: renk yalnız dikkat ve kaçın bandında kullanılır (plan §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IkonBandi {
    Rahat,
    Dikkat,
    Kacin,
}

/// Kullanılan oranından band (≥ %85 kaçın, ≥ %70 dikkat).
pub fn ikon_bandi(kullanilan: f64) -> IkonBandi {
    if kullanilan >= 0.85 {
        IkonBandi::Kacin
    } else if kullanilan >= 0.70 {
        IkonBandi::Dikkat
    } else {
        IkonBandi::Rahat
    }
}

/// Çubuğun dolgu rengi (RGBA). Rahat bantta tema ile zıt nötr ton (koyu çubukta açık, açık
/// çubukta koyu); dikkat amber, kaçın kırmızı (açık temada kontrast için koyulaştırılmış).
/// Servis kapalı/bayat: nötr gri.
pub fn dolgu_rengi(
    oran: f64,
    servis_calisiyor: bool,
    bayat: bool,
    tema: GorevCubuguTemasi,
) -> [u8; 4] {
    let koyu = tema == GorevCubuguTemasi::Koyu;
    if !servis_calisiyor || bayat {
        return if koyu {
            [150, 156, 166, 255]
        } else {
            [110, 116, 126, 255]
        };
    }
    match (ikon_bandi(oran), koyu) {
        (IkonBandi::Rahat, true) => [236, 239, 243, 255],
        (IkonBandi::Rahat, false) => [30, 34, 42, 255],
        (IkonBandi::Dikkat, true) => [245, 166, 35, 255],
        (IkonBandi::Dikkat, false) => [196, 118, 0, 255],
        (IkonBandi::Kacin, true) => [239, 83, 80, 255],
        (IkonBandi::Kacin, false) => [200, 30, 30, 255],
    }
}

/// Boş çubuk izi: temanın zıt tonunda yarı saydam (her iki temada okunur, arka plan kapsülü yok).
fn iz_rengi(tema: GorevCubuguTemasi) -> [u8; 4] {
    match tema {
        GorevCubuguTemasi::Koyu => [255, 255, 255, 72],
        GorevCubuguTemasi::Acik => [0, 0, 0, 60],
    }
}

fn boya(rgba: [u8; 4]) -> Paint<'static> {
    let mut p = Paint {
        anti_alias: false,
        ..Paint::default()
    };
    p.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    p
}

/// DPI ölçek çarpanına ve görev çubuğu temasına göre 16 px mantıksal iki çubuklu tepsi ikonunu
/// düz (unpremultiplied) RGBA olarak çizer.
pub fn tepsi_ikonu_ciz_rgba(
    veri: &TepsiVerisi,
    olcek_faktoru: f64,
    tema: GorevCubuguTemasi,
) -> (Vec<u8>, u32, u32) {
    let olcek = olcek_faktoru.clamp(1.0, 4.0);
    let boyut = (16.0 * olcek).round().clamp(16.0, 64.0) as u32;
    let mut pixmap = Pixmap::new(boyut, boyut).unwrap_or_else(|| Pixmap::new(16, 16).unwrap());
    let sx = boyut as f32 / 16.0;
    let tf = Transform::from_scale(sx, sx);

    let cubuklar = [(3.0_f32, veri.oran_5s), (9.0_f32, veri.oran_hafta)];
    let iz_paint = boya(iz_rengi(tema));

    for (y, oran_opt) in cubuklar {
        if let Some(iz_rect) = Rect::from_xywh(1.5, y, 13.0, 4.0) {
            pixmap.fill_rect(iz_rect, &iz_paint, tf, None);
        }
        if !veri.servis_calisiyor {
            let kapali_paint = boya(dolgu_rengi(0.0, false, false, tema));
            if let Some(nokta) = Rect::from_xywh(1.5, y + 1.5, 3.0, 1.0) {
                pixmap.fill_rect(nokta, &kapali_paint, tf, None);
            }
        } else if let Some(oran) = oran_opt {
            let oran = oran.clamp(0.0, 1.0);
            let genislik = (13.0 * oran as f32).max(1.5);
            let kullanilan = match veri.deger_yonu {
                DegerYonu::Kalan => (1.0 - oran).clamp(0.0, 1.0),
                DegerYonu::Kullanilan => oran,
            };
            let dolgu_paint = boya(dolgu_rengi(kullanilan, true, veri.bayat, tema));
            if let Some(dolgu_rect) = Rect::from_xywh(1.5, y, genislik, 4.0) {
                pixmap.fill_rect(dolgu_rect, &dolgu_paint, tf, None);
            }
        }
    }

    let mut rgba = Vec::with_capacity((boyut * boyut * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.push(c.red());
        rgba.push(c.green());
        rgba.push(c.blue());
        rgba.push(c.alpha());
    }
    (rgba, boyut, boyut)
}

/// Tauri tepsi ikonu nesnesini oluşturur (görev çubuğu teması kayıt defterinden okunur).
pub fn tepsi_ikonu_olustur(veri: &TepsiVerisi, olcek_faktoru: f64) -> Image<'static> {
    let (rgba, w, h) = tepsi_ikonu_ciz_rgba(veri, olcek_faktoru, gorev_cubugu_temasi());
    Image::new_owned(rgba, w, h)
}

/// Verilen saat dilimi farkıyla `HH:MM` üretir (saf; test edilebilir).
pub fn saat_hh_mm_dilimle(zaman: UtcZaman, ofset: time::UtcOffset) -> String {
    let dt = zaman.as_offset().to_offset(ofset);
    format!("{:02}:{:02}", dt.hour(), dt.minute())
}

/// Yerel saatle (o tarihteki yaz saati dahil) `HH:MM`; yerel dilim çözülemezse UTC + `Z`.
fn saat_hh_mm(zaman: UtcZaman) -> String {
    match time::UtcOffset::local_offset_at(zaman.as_offset()) {
        Ok(ofset) => saat_hh_mm_dilimle(zaman, ofset),
        Err(_) => format!("{}Z", saat_hh_mm_dilimle(zaman, time::UtcOffset::UTC)),
    }
}

/// Tepsi ikonu fare üstüne gelince gösterilen özet ipucu metnini (Windows 127 karakter sınırı içinde)
/// seçilen tepsi dilinde ve değer yönünde üretir.
pub fn ipucu_metni_olustur(
    anlik: Option<&AnlikV2>,
    servis_calisiyor: bool,
    dil: TepsiDili,
    deger_yonu: DegerYonu,
) -> String {
    if !servis_calisiyor {
        return metinler::ipucu_servis_calismiyor(dil).to_string();
    }
    let Some(anlik) = anlik else {
        return metinler::ipucu_veri_bekleniyor(dil).to_string();
    };

    // En dar havuzu bul: çubuğun hangi havuzu yansıttığını açıkça bildirir.
    let mut en_dar: Option<(&str, &cetvel_model::HavuzAnlik, f64)> = None;
    // Yalnız ölçülmüş kalanı olan kota havuzları: kullanım sayacı ya da kalanı bilinmeyen havuz için
    // yüzde uydurulmaz.
    for (id, h) in &anlik.havuzlar {
        if h.durum != Durum::Ok || h.kullanim_sayaci_mi() {
            continue;
        }
        let Some(kalan) = h.etkin_kalan else {
            continue;
        };
        if en_dar.is_none_or(|(_, _, ek)| kalan < ek) {
            en_dar = Some((id.as_str(), h, kalan));
        }
    }

    let claude_5s_en_dar = en_dar.is_some_and(|(id, h, _)| {
        id == "claude_max" && h.baglayici_pencere.as_deref() == Some("5s")
    });

    let mut parcalar = Vec::new();
    if let Some(cl) = anlik.havuzlar.get("claude_max")
        && cl.durum == Durum::Ok
        && let Some(p5) = cl.pencereler.get("5s")
    {
        let oran = match deger_yonu {
            DegerYonu::Kalan => p5.kalan,
            DegerYonu::Kullanilan => p5.kullanilan,
        }
        .clamp(0.0, 1.0);
        let yuzde = (oran * 100.0).round() as u32;
        let sifirlanma = p5.sifirlama.map(saat_hh_mm);
        parcalar.push(metinler::ipucu_claude_5s(
            dil,
            yuzde,
            deger_yonu,
            sifirlanma.as_deref(),
            claude_5s_en_dar,
        ));
    }

    if let Some((_, dar, kalan)) = en_dar
        && !claude_5s_en_dar
    {
        let p_kisa = dar
            .baglayici_pencere
            .as_deref()
            .map_or("", |kod| metinler::pencere_kisa(dil, kod));
        let oran = match deger_yonu {
            DegerYonu::Kalan => kalan,
            DegerYonu::Kullanilan => 1.0 - kalan,
        }
        .clamp(0.0, 1.0);
        let yuzde = (oran * 100.0).round() as u32;
        parcalar.push(metinler::ipucu_havuz(
            dil,
            &dar.etiket,
            p_kisa,
            yuzde,
            deger_yonu,
        ));
    }

    let ham = if !parcalar.is_empty() {
        parcalar.join(" | ")
    } else if anlik.havuzlar.is_empty() {
        metinler::ipucu_veri_yok(dil).to_string()
    } else {
        metinler::ipucu_havuz_sayisi(dil, anlik.havuzlar.len())
    };

    // NOTIFYICONDATAW.szTip 128 UTF-16 kodu (127 char + NUL) ile sınırlıdır.
    ham.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cetvel_model::{HavuzAnlik, PencereOlcum};

    use super::*;

    fn ornek_anlik_cift(u5: f64, uhafta: f64) -> AnlikV2 {
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T20:00:00Z").unwrap();
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                kullanilan: u5,
                kalan: 1.0 - u5,
                sifirlama: Some(simdi.ekle_sn(3600)),
                uzunluk_dk: 300,
            },
        );
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                kullanilan: uhafta,
                kalan: 1.0 - uhafta,
                sifirlama: Some(simdi.ekle_sn(86400)),
                uzunluk_dk: 10080,
            },
        );
        let mut claude = HavuzAnlik {
            platform: "claude".to_string(),
            etiket: "Claude".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        claude.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("claude_max".to_string(), claude);
        AnlikV2::yeni(simdi, 10, havuzlar)
    }

    /// OpenCode benzeri kullanım sayacı havuzu: pencere/yüzde yok, `ek.kullanim` dolu, durum `Ok`.
    fn sayac_havuzu() -> HavuzAnlik {
        let mut h = HavuzAnlik {
            platform: "opencode".to_string(),
            etiket: "OpenCode".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: Some(cetvel_model::EkBilgi {
                kullanim: Some(cetvel_model::KullanimSayaci {
                    kota_yok: true,
                    oturum_24s: 2,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            hata: None,
            notlar: Vec::new(),
        };
        h.hesapla_etkin_durum();
        h
    }

    #[test]
    fn kullanim_sayaci_bayat_kota_verisini_taze_gostermez() {
        let mut anlik = ornek_anlik_cift(0.45, 0.90);
        anlik.havuzlar.get_mut("claude_max").unwrap().durum = Durum::Bayat;
        anlik
            .havuzlar
            .insert("opencode".to_string(), sayac_havuzu());
        let veri_kalan = tepsi_verisi_cikar(Some(&anlik), true, DegerYonu::Kalan);
        assert!(
            veri_kalan.bayat,
            "yalnız sayaç Ok iken kota verisi bayat çizilmeli"
        );
        assert!((veri_kalan.oran_5s.unwrap() - 0.55).abs() < 1e-6);

        let veri_kul = tepsi_verisi_cikar(Some(&anlik), true, DegerYonu::Kullanilan);
        assert!((veri_kul.oran_5s.unwrap() - 0.45).abs() < 1e-6);
    }

    #[test]
    fn ipucu_sayac_havuzuna_uydurma_yuzde_yazmaz() {
        let mut anlik = ornek_anlik_cift(0.45, 0.90);
        anlik.havuzlar.get_mut("claude_max").unwrap().durum = Durum::Hata;
        anlik
            .havuzlar
            .insert("opencode".to_string(), sayac_havuzu());
        for dil in [TepsiDili::Tr, TepsiDili::En] {
            let ipucu = ipucu_metni_olustur(Some(&anlik), true, dil, DegerYonu::Kalan);
            assert!(!ipucu.contains("OpenCode"), "{ipucu}");
            assert!(!ipucu.contains('%'), "{ipucu}");
        }
        // Kota havuzu okunuyorsa sayaç en dar havuz seçilmez.
        let mut anlik = ornek_anlik_cift(0.58, 0.88);
        anlik
            .havuzlar
            .insert("opencode".to_string(), sayac_havuzu());
        let ipucu = ipucu_metni_olustur(Some(&anlik), true, TepsiDili::Tr, DegerYonu::Kalan);
        assert!(
            ipucu.contains("Claude 5s: %42 kalan") && !ipucu.contains("OpenCode"),
            "{ipucu}"
        );
    }

    #[test]
    fn tepsi_ikonu_dpi_olcegine_gore_boyut_uretir() {
        let anlik = ornek_anlik_cift(0.45, 0.90);
        let veri = tepsi_verisi_cikar(Some(&anlik), true, DegerYonu::Kalan);
        assert!(veri.servis_calisiyor);
        assert!((veri.oran_5s.unwrap() - 0.55).abs() < 1e-6);
        assert!((veri.oran_hafta.unwrap() - 0.10).abs() < 1e-6);

        let koyu = GorevCubuguTemasi::Koyu;
        let (rgba1, w1, h1) = tepsi_ikonu_ciz_rgba(&veri, 1.0, koyu);
        assert_eq!((w1, h1), (16, 16));
        assert_eq!(rgba1.len(), 16 * 16 * 4);

        let (rgba15, w15, h15) = tepsi_ikonu_ciz_rgba(&veri, 1.5, koyu);
        assert_eq!((w15, h15), (24, 24));
        assert_eq!(rgba15.len(), 24 * 24 * 4);

        let (rgba2, w2, h2) = tepsi_ikonu_ciz_rgba(&veri, 2.0, koyu);
        assert_eq!((w2, h2), (32, 32));
        assert_eq!(rgba2.len(), 32 * 32 * 4);

        // %125 / %175 / %250 ölçekleri (plan §9.1: DPI'ya göre ayrı çizim)
        for tema in [GorevCubuguTemasi::Koyu, GorevCubuguTemasi::Acik] {
            for (olcek, beklenen) in [(1.25, 20u32), (1.75, 28), (2.5, 40)] {
                let (rgba, w, h) = tepsi_ikonu_ciz_rgba(&veri, olcek, tema);
                assert_eq!((w, h), (beklenen, beklenen), "ölçek {olcek} {tema:?}");
                assert_eq!(rgba.len(), (beklenen * beklenen * 4) as usize);
                assert!(rgba.chunks(4).any(|p| p[3] > 0), "boş ikon çizilmemeli");
            }
        }
    }

    fn bagil_parlaklik(c: [u8; 4]) -> f64 {
        (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) / 255.0
    }

    fn notr_mu(c: &[u8]) -> bool {
        let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
        (r - g).abs() <= 16 && (g - b).abs() <= 16 && (r - b).abs() <= 16
    }

    #[test]
    fn tema_kayit_degeri_acik_ve_koyuya_cevrilir() {
        assert_eq!(tema_kayit_degerinden(Some(1)), GorevCubuguTemasi::Acik);
        assert_eq!(tema_kayit_degerinden(Some(0)), GorevCubuguTemasi::Koyu);
        assert_eq!(tema_kayit_degerinden(None), GorevCubuguTemasi::Koyu);
        // Gerçek kayıt okuması çökmeden bir tema döndürür.
        let _ = gorev_cubugu_temasi();
    }

    #[test]
    fn renk_yalniz_dikkat_ve_kacin_bandinda_ve_temaya_gore_kontrastli() {
        assert_eq!(ikon_bandi(0.69), IkonBandi::Rahat);
        assert_eq!(ikon_bandi(0.70), IkonBandi::Dikkat);
        assert_eq!(ikon_bandi(0.85), IkonBandi::Kacin);
        for tema in [GorevCubuguTemasi::Koyu, GorevCubuguTemasi::Acik] {
            let rahat = dolgu_rengi(0.30, true, false, tema);
            assert!(notr_mu(&rahat), "rahat bant renksiz olmalı: {tema:?}");
            let dikkat = dolgu_rengi(0.75, true, false, tema);
            let kacin = dolgu_rengi(0.95, true, false, tema);
            assert!(!notr_mu(&dikkat) && !notr_mu(&kacin), "{tema:?}");
            assert!(kacin[0] > kacin[2] + 100, "kaçın kırmızı: {tema:?}");
            assert!(
                dikkat[0] > dikkat[2] + 100 && dikkat[1] > kacin[1],
                "{tema:?}"
            );
            assert!(notr_mu(&dolgu_rengi(0.95, true, true, tema)), "bayat gri");
            assert!(
                notr_mu(&dolgu_rengi(0.95, false, false, tema)),
                "kapalı gri"
            );
        }
        // Koyu görev çubuğunda açık, açık görev çubuğunda koyu nötr ton.
        assert!(bagil_parlaklik(dolgu_rengi(0.3, true, false, GorevCubuguTemasi::Koyu)) > 0.8);
        assert!(bagil_parlaklik(dolgu_rengi(0.3, true, false, GorevCubuguTemasi::Acik)) < 0.2);
    }

    #[test]
    fn cizilen_ikon_rahatta_renksiz_kacinda_kirmizi_tema_degisince_farkli() {
        let rahat = TepsiVerisi {
            servis_calisiyor: true,
            oran_5s: Some(0.6),
            oran_hafta: Some(0.8),
            bayat: false,
            deger_yonu: DegerYonu::Kalan,
        };
        let kacin = TepsiVerisi {
            oran_5s: Some(0.05),
            ..rahat.clone()
        };
        for olcek in [1.0, 1.25, 1.75, 2.5] {
            for tema in [GorevCubuguTemasi::Koyu, GorevCubuguTemasi::Acik] {
                let (px, _, _) = tepsi_ikonu_ciz_rgba(&rahat, olcek, tema);
                assert!(
                    px.chunks(4).filter(|p| p[3] > 0).all(notr_mu),
                    "rahat ikonda renkli piksel olmamalı ({olcek}, {tema:?})"
                );
                let (px, _, _) = tepsi_ikonu_ciz_rgba(&kacin, olcek, tema);
                assert!(
                    px.chunks(4)
                        .any(|p| p[3] == 255 && p[0] as i32 - p[2] as i32 > 100),
                    "kaçın ikonda kırmızı piksel olmalı ({olcek}, {tema:?})"
                );
            }
            let (koyu, _, _) = tepsi_ikonu_ciz_rgba(&rahat, olcek, GorevCubuguTemasi::Koyu);
            let (acik, _, _) = tepsi_ikonu_ciz_rgba(&rahat, olcek, GorevCubuguTemasi::Acik);
            assert_ne!(koyu, acik, "tema çizimi değiştirmeli ({olcek})");
        }

        // Kullanılan kipinde ters yönde aynı bantlar korunur.
        let kacin_kullanilan = TepsiVerisi {
            servis_calisiyor: true,
            oran_5s: Some(0.95),
            oran_hafta: Some(0.20),
            bayat: false,
            deger_yonu: DegerYonu::Kullanilan,
        };
        let (px, _, _) = tepsi_ikonu_ciz_rgba(&kacin_kullanilan, 1.0, GorevCubuguTemasi::Koyu);
        assert!(
            px.chunks(4)
                .any(|p| p[3] == 255 && p[0] as i32 - p[2] as i32 > 100),
            "kullanılan kipinde %95 kırmızı olmalı"
        );
    }

    #[test]
    fn ipucu_saati_yerel_dilimle_bicimlenir_utc_z_yazmaz() {
        let z = UtcZaman::parse_tarih_veya_zaman("2026-11-15T18:10:00Z").unwrap();
        let istanbul = time::UtcOffset::from_hms(3, 0, 0).unwrap();
        assert_eq!(saat_hh_mm_dilimle(z, istanbul), "21:10");
        let yaz_saati = time::UtcOffset::from_hms(2, 0, 0).unwrap();
        assert_eq!(saat_hh_mm_dilimle(z, yaz_saati), "20:10");
        assert_eq!(saat_hh_mm_dilimle(z, time::UtcOffset::UTC), "18:10");
    }

    #[test]
    fn ipucu_metni_servis_durumuna_ve_limiti_gore_uretilir() {
        assert!(
            ipucu_metni_olustur(None, false, TepsiDili::Tr, DegerYonu::Kalan)
                .contains("çalışmıyor")
        );
        // Claude 5s %20 kullanıldı -> %80 kalan, hafta %10 kullanıldı -> %90 kalan (en dar Claude 5s).
        let anlik = ornek_anlik_cift(0.20, 0.10);
        let ipucu_kalan = ipucu_metni_olustur(Some(&anlik), true, TepsiDili::Tr, DegerYonu::Kalan);
        assert!(
            ipucu_kalan.contains("Claude 5s: %80 kalan (en dar)"),
            "{ipucu_kalan}"
        );
        assert!(ipucu_kalan.chars().count() <= 120);

        let ipucu_kul =
            ipucu_metni_olustur(Some(&anlik), true, TepsiDili::Tr, DegerYonu::Kullanilan);
        assert!(
            ipucu_kul.contains("Claude 5s: %20 kullanıldı (en dar)"),
            "{ipucu_kul}"
        );
    }

    #[test]
    fn ipucu_metni_ingilizce_ve_diger_dillerde_ingilizce_uretilir() {
        assert!(
            ipucu_metni_olustur(None, false, TepsiDili::En, DegerYonu::Kalan)
                .contains("not running")
        );
        let anlik = ornek_anlik_cift(0.20, 0.88);
        assert!(
            ipucu_metni_olustur(Some(&anlik), false, TepsiDili::En, DegerYonu::Kalan)
                .contains("not running")
        );
        for kod in ["en", "de", "ja", "pt-BR", "zh-Hans"] {
            let ipucu =
                ipucu_metni_olustur(Some(&anlik), true, TepsiDili::koddan(kod), DegerYonu::Kalan);
            assert!(ipucu.contains("Claude 5h: 80% left"), "{kod}: {ipucu}");
            assert!(
                ipucu.contains("Tightest: Claude week: 12% left"),
                "{kod}: {ipucu}"
            );
            assert!(!ipucu.contains("%80"), "{kod}: {ipucu}");
            assert!(ipucu.chars().count() <= 120);
        }
        let tr = ipucu_metni_olustur(
            Some(&anlik),
            true,
            TepsiDili::koddan("tr"),
            DegerYonu::Kalan,
        );
        assert!(tr.contains("Claude 5s: %80 kalan"), "{tr}");
        assert!(tr.contains("En dar: Claude hafta: %12 kalan"), "{tr}");
    }

    #[test]
    fn ipucu_baska_havuz_en_darsa_onu_acikca_soyler() {
        let mut anlik = ornek_anlik_cift(0.20, 0.10);
        let simdi = anlik.zaman;
        let mut codex_pencere = BTreeMap::new();
        codex_pencere.insert(
            "hafta".to_string(),
            PencereOlcum {
                kullanilan: 0.60,
                kalan: 0.40,
                sifirlama: Some(simdi.ekle_sn(86_400)),
                uzunluk_dk: 10080,
            },
        );
        let mut codex = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: codex_pencere,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        codex.hesapla_etkin_durum();
        anlik.havuzlar.insert("codex_plus".to_string(), codex);

        let tr_kalan = ipucu_metni_olustur(Some(&anlik), true, TepsiDili::Tr, DegerYonu::Kalan);
        assert!(tr_kalan.contains("Claude 5s: %80 kalan"), "{tr_kalan}");
        assert!(
            tr_kalan.contains("En dar: Codex hafta: %40 kalan"),
            "{tr_kalan}"
        );

        let en_kul = ipucu_metni_olustur(Some(&anlik), true, TepsiDili::En, DegerYonu::Kullanilan);
        assert!(en_kul.contains("Claude 5h: 20% used"), "{en_kul}");
        assert!(
            en_kul.contains("Tightest: Codex week: 60% used"),
            "{en_kul}"
        );

        let veri = tepsi_verisi_cikar(Some(&anlik), true, DegerYonu::Kalan);
        assert!((veri.oran_5s.unwrap() - 0.80).abs() < 1e-9);
        assert!((veri.oran_hafta.unwrap() - 0.40).abs() < 1e-9);
    }
}
