use std::sync::Mutex;

use cetvel_depo::{BildirimAyari, GecmisDeposu};
use cetvel_model::{AnlikV2, Durum, PencereOlcum, TahminBilgisi, TahminDurumu, UtcZaman};

/// Kullanıcıya sunulacak soyutlanmış bildirim mesajı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BildirimMesaji {
    pub tur: String,
    pub baslik: String,
    pub icerik: String,
    pub havuz: Option<String>,
    pub pencere: Option<String>,
    pub tekil_anahtar: String,
}

/// Testlerde gerçek Windows toast açmamak için kullanılan bildirim gönderici arayüzü.
pub trait BildirimGonderici: Send + Sync {
    fn gonder(&self, mesaj: &BildirimMesaji) -> Result<(), String>;
}

/// Birim ve bütünleşme testleri için bellek içi sahte toast göndericisi.
#[derive(Default)]
pub struct SahteBildirimGonderici {
    gonderilenler: Mutex<Vec<BildirimMesaji>>,
}

impl SahteBildirimGonderici {
    pub fn yeni() -> Self {
        Self::default()
    }

    pub fn kayitlar(&self) -> Vec<BildirimMesaji> {
        self.gonderilenler
            .lock()
            .expect("sahte bildirim kilidi")
            .clone()
    }
}

impl BildirimGonderici for SahteBildirimGonderici {
    fn gonder(&self, mesaj: &BildirimMesaji) -> Result<(), String> {
        self.gonderilenler
            .lock()
            .expect("sahte bildirim kilidi")
            .push(mesaj.clone());
        Ok(())
    }
}

/// Gerçek Windows masaüstü toast göndericisi (`ToastNotificationManager`).
///
/// Paketlenmemiş uygulamada toast yalnız Başlat menüsünde aynı AUMID'li kısayol varsa görünür;
/// `cetvel kur` kısayolu [`cetvel_depo::CETVEL_AUMID`] ile yazar, varsayılan gönderici de onu kullanır.
pub struct WindowsToastGonderici {
    aumid: String,
}

impl Default for WindowsToastGonderici {
    fn default() -> Self {
        Self {
            aumid: cetvel_depo::CETVEL_AUMID.to_string(),
        }
    }
}

impl WindowsToastGonderici {
    pub fn yeni(aumid: impl Into<String>) -> Self {
        Self {
            aumid: aumid.into(),
        }
    }

    /// Bildirimin gönderileceği AppUserModelID.
    pub fn aumid(&self) -> &str {
        &self.aumid
    }
}

impl BildirimGonderici for WindowsToastGonderici {
    fn gonder(&self, mesaj: &BildirimMesaji) -> Result<(), String> {
        #[cfg(windows)]
        {
            use windows::Data::Xml::Dom::XmlDocument;
            use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
            use windows::core::HSTRING;

            let xml_str = format!(
                "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
                xml_kacis(&mesaj.baslik),
                xml_kacis(&mesaj.icerik)
            );
            let doc = XmlDocument::new().map_err(|e| format!("XmlDocument hatası: {e}"))?;
            doc.LoadXml(&HSTRING::from(xml_str))
                .map_err(|e| format!("LoadXml hatası: {e}"))?;
            let toast = ToastNotification::CreateToastNotification(&doc)
                .map_err(|e| format!("CreateToastNotification hatası: {e}"))?;
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
                self.aumid.as_str(),
            ))
            .map_err(|e| format!("CreateToastNotifierWithId hatası: {e}"))?;
            notifier
                .Show(&toast)
                .map_err(|e| format!("Toast Show hatası: {e}"))?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = mesaj;
            Ok(())
        }
    }
}

#[cfg(windows)]
fn xml_kacis(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Eşik geçişleri, sıfırlamalar, kredi ve bayatlık durumlarını tekilleştirerek yönetir.
pub struct BildirimYoneticisi<G: BildirimGonderici> {
    gonderici: G,
    onceki: Option<AnlikV2>,
}

impl<G: BildirimGonderici> BildirimYoneticisi<G> {
    pub fn yeni(gonderici: G) -> Self {
        Self {
            gonderici,
            onceki: None,
        }
    }

    pub fn gonderici(&self) -> &G {
        &self.gonderici
    }

    /// Yeni anlık durumu öncekiyle karşılaştırır; gönderilen bildirimleri döndürür.
    pub fn degerlendir(
        &mut self,
        yeni: &AnlikV2,
        ayar: &BildirimAyari,
        depo: &GecmisDeposu,
        yerel_saat: u8,
        simdi: UtcZaman,
    ) -> Vec<BildirimMesaji> {
        let mut adaylar = Vec::new();

        if ayar.etkin && !sessiz_saat_mi(yerel_saat, ayar) {
            for (havuz_id, yeni_havuz) in &yeni.havuzlar {
                if ayar.platform_sessiz_mi(&yeni_havuz.platform) {
                    continue;
                }
                let eski_havuz = self.onceki.as_ref().and_then(|o| o.havuzlar.get(havuz_id));

                // 1. Eşik altına iniş ve tükenen pencere sıfırlanması
                if yeni_havuz.durum == Durum::Ok {
                    for (pencere_ad, yeni_olcum) in &yeni_havuz.pencereler {
                        let esik =
                            ayar.platform_esik_orani(&yeni_havuz.platform, havuz_id, pencere_ad);
                        let eski_kalan = eski_havuz
                            .and_then(|eh| eh.pencereler.get(pencere_ad))
                            .map(|ep| ep.kalan);
                        let sifir_etiket =
                            sifirlama_anahtari(yeni_olcum.sifirlama, yeni_olcum.uzunluk_dk, simdi);

                        if yeni_olcum.kalan <= esik && eski_kalan.is_none_or(|ek| ek > esik) {
                            let kalan_yuzde = (yeni_olcum.kalan * 100.0).round() as i64;
                            adaylar.push(BildirimMesaji {
                                tur: "esik".to_string(),
                                baslik: format!("{} kotası azaldı", yeni_havuz.etiket),
                                icerik: format!(
                                    "{} {} penceresinde %{kalan_yuzde} kota kaldı.",
                                    yeni_havuz.etiket, pencere_ad
                                ),
                                havuz: Some(havuz_id.clone()),
                                pencere: Some(pencere_ad.clone()),
                                tekil_anahtar: tekilleme_anahtari(
                                    "esik",
                                    havuz_id,
                                    pencere_ad,
                                    &sifir_etiket,
                                ),
                            });
                        }

                        if eski_kalan.is_some_and(|ek| ek <= 0.001) && yeni_olcum.kalan >= 0.90 {
                            adaylar.push(BildirimMesaji {
                                tur: "sifirlandi".to_string(),
                                baslik: format!("{} penceresi sıfırlandı", yeni_havuz.etiket),
                                icerik: format!(
                                    "{} {} penceresi yeniden kullanılabilir.",
                                    yeni_havuz.etiket, pencere_ad
                                ),
                                havuz: Some(havuz_id.clone()),
                                pencere: Some(pencere_ad.clone()),
                                tekil_anahtar: tekilleme_anahtari(
                                    "sifirlandi",
                                    havuz_id,
                                    pencere_ad,
                                    &sifir_etiket,
                                ),
                            });
                        }
                    }
                }

                // 1b. Akıllı tahmin uyarısı: bu hızla pencere sıfırlanmadan biter.
                if yeni_havuz.durum == Durum::Ok {
                    for (pencere_ad, yeni_olcum) in &yeni_havuz.pencereler {
                        // Aynı turda aynı pencere için eşik bildirimi çıktıysa ikinci toast yok.
                        let esik_turu_var = adaylar.iter().any(|m| {
                            m.tur == "esik"
                                && m.havuz.as_deref() == Some(havuz_id.as_str())
                                && m.pencere.as_deref() == Some(pencere_ad.as_str())
                        });
                        if esik_turu_var {
                            continue;
                        }
                        let eski_tahmin = eski_havuz.and_then(|eh| eh.tahmin.get(pencere_ad));
                        let Some(yeni_tahmin) = yeni_havuz.tahmin.get(pencere_ad) else {
                            continue;
                        };
                        let Some(uyari) =
                            tahmin_uyarisi(yeni_olcum, yeni_tahmin, eski_tahmin, simdi)
                        else {
                            continue;
                        };
                        let sifir_etiket =
                            sifirlama_anahtari(yeni_olcum.sifirlama, yeni_olcum.uzunluk_dk, simdi);
                        let pencere_metni = pencere_gorunen_adi(pencere_ad);
                        adaylar.push(BildirimMesaji {
                            tur: "tahmin".to_string(),
                            baslik: format!(
                                "{} {} sıfırlanmadan bitebilir",
                                yeni_havuz.etiket, pencere_metni
                            ),
                            icerik: format!(
                                "Bu hızla yaklaşık {} sonra biter; sıfırlanmasına {} var. Şu an %{} kaldı.",
                                sure_metni(uyari.bitise_sn),
                                sure_metni(uyari.sifirlamaya_sn),
                                (yeni_olcum.kalan.clamp(0.0, 1.0) * 100.0).round() as i64
                            ),
                            havuz: Some(havuz_id.clone()),
                            pencere: Some(pencere_ad.clone()),
                            tekil_anahtar: tekilleme_anahtari(
                                "tahmin",
                                havuz_id,
                                pencere_ad,
                                &sifir_etiket,
                            ),
                        });
                    }
                }

                // 2. Codex sıfırlama kredisi
                if let Some(yeni_kredi) = yeni_havuz
                    .ek
                    .as_ref()
                    .and_then(|ek| ek.sifirlama_kredisi.as_ref())
                    && yeni_kredi.adet > 0
                {
                    let eski_adet = eski_havuz
                        .and_then(|eh| eh.ek.as_ref())
                        .and_then(|ek| ek.sifirlama_kredisi.as_ref())
                        .map(|sk| sk.adet)
                        .unwrap_or(0);
                    if yeni_kredi.adet > eski_adet {
                        let etiket = yeni_kredi
                            .son
                            .map(|z| z.to_iso_z())
                            .unwrap_or_else(|| yeni_kredi.adet.to_string());
                        adaylar.push(BildirimMesaji {
                            tur: "sifirlama_kredisi".to_string(),
                            baslik: format!("{} sıfırlama kredisi", yeni_havuz.etiket),
                            icerik: format!("{} sıfırlama kredisi tanımlandı.", yeni_havuz.etiket),
                            havuz: Some(havuz_id.clone()),
                            pencere: None,
                            tekil_anahtar: tekilleme_anahtari(
                                "sifirlama_kredisi",
                                havuz_id,
                                "kredi",
                                &etiket,
                            ),
                        });
                    }
                }

                // 3. Kaynak bayatladı (havuzun kendi bayat eşiği aşıldı ve > 30 dk; günde en fazla
                // 1 kez). Yalnız yaş yetmez: uzun boşta yoklama aralığı bayatlık değildir.
                let yas_sn = yeni_havuz.kaynak.as_ref().map(|k| k.yas_sn).unwrap_or(0);
                if yeni_havuz.durum == Durum::Bayat
                    && yas_sn > 1800
                    && depo
                        .son_24_saat_olay_sayisi("bayat", havuz_id, simdi)
                        .unwrap_or(0)
                        == 0
                {
                    let gun_etiket = &simdi.to_iso_z()[..10];
                    adaylar.push(BildirimMesaji {
                        tur: "bayat".to_string(),
                        baslik: format!("{} verisi bayatladı", yeni_havuz.etiket),
                        icerik: format!(
                            "{} kaynağı 30 dakikadan uzun süredir güncellenemedi.",
                            yeni_havuz.etiket
                        ),
                        havuz: Some(havuz_id.clone()),
                        pencere: None,
                        tekil_anahtar: tekilleme_anahtari("bayat", havuz_id, "kaynak", gun_etiket),
                    });
                }
            }
        }

        // Kullanıcının kapattığı bildirim türlerini ayıkla (Ayarlar sekmesi).
        adaylar.retain(|m| ayar.tur_acik_mi(&m.tur));

        let mut gonderilenler = Vec::new();
        for mesaj in adaylar {
            if depo.olay_var_mi(&mesaj.tekil_anahtar).unwrap_or(false) {
                continue;
            }
            if self.gonderici.gonder(&mesaj).is_ok() {
                let _ = depo.olay_kaydet(
                    simdi,
                    &mesaj.tur,
                    mesaj.havuz.as_deref(),
                    mesaj.pencere.as_deref(),
                    Some(&mesaj.tekil_anahtar),
                    Some(&mesaj.icerik),
                );
                gonderilenler.push(mesaj);
            }
        }

        self.onceki = Some(yeni.clone());
        gonderilenler
    }
}

/// Tahmin uyarısının sayısal özeti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TahminUyarisi {
    /// Şu andan tahmini tükenmeye kalan süre (sn).
    pub bitise_sn: i64,
    /// Şu andan pencerenin sıfırlanmasına kalan süre (sn).
    pub sifirlamaya_sn: i64,
}

/// Tahmin uyarısının koşulları (yanlış alarmı bastırmak için bilinçli olarak tutucu):
///
/// - Tahmin var (`deneysel`/`hazir`), `sifirlanmadan_biter == true`, bitiş ve sıfırlama zamanı belli,
///   pencerede hâlâ kota var.
/// - Önceki değerlendirmede de aynı pencere "sıfırlanmadan biter" diyordu (tek örneklik sıçrama
///   uyarı üretmez).
/// - Bitiş, sıfırlamadan anlamlı ölçüde önce: en az 15 dk ve pencere uzunluğunun %5'i.
/// - Bitiş yakın: kısa pencerelerde (< 1 gün) pencere uzunluğu içinde; günlük ve daha uzun
///   pencerelerde 24 saat içinde. Deneysel uzun pencere tahmini kısa bir yoğun oturumu haftaya
///   yayabildiği için yalnız bitiş 12 saat içindeyse uyarılır.
pub fn tahmin_uyarisi(
    olcum: &PencereOlcum,
    tahmin: &TahminBilgisi,
    onceki: Option<&TahminBilgisi>,
    simdi: UtcZaman,
) -> Option<TahminUyarisi> {
    if tahmin.durum == TahminDurumu::TahminYok || tahmin.sifirlanmadan_biter != Some(true) {
        return None;
    }
    if onceki.and_then(|o| o.sifirlanmadan_biter) != Some(true) {
        return None;
    }
    if !(olcum.kalan.is_finite() && olcum.kalan > 0.0) {
        return None;
    }
    let bitis = tahmin.bitis_tahmini?;
    let sifirlama = olcum.sifirlama?;
    let bitise_sn = bitis.fark_sn(simdi);
    let sifirlamaya_sn = sifirlama.fark_sn(simdi);
    if bitise_sn <= 0 || sifirlamaya_sn <= bitise_sn {
        return None;
    }
    let pencere_sn = (olcum.uzunluk_dk.max(1) * 60) as i64;
    let asgari_fark = (pencere_sn / 20).max(900);
    if sifirlamaya_sn - bitise_sn < asgari_fark {
        return None;
    }
    let ufuk_sn = if pencere_sn < 86_400 {
        pencere_sn
    } else if tahmin.durum == TahminDurumu::Hazir {
        86_400
    } else {
        43_200
    };
    if bitise_sn > ufuk_sn {
        return None;
    }
    Some(TahminUyarisi {
        bitise_sn,
        sifirlamaya_sn,
    })
}

/// Pencere kodunun bildirimde okunur adı (`5s` → `5 saatlik kotası`).
pub fn pencere_gorunen_adi(pencere: &str) -> String {
    match pencere {
        "5dk" => "5 dakikalık kotası".to_string(),
        "5s" => "5 saatlik kotası".to_string(),
        "gun" => "günlük kotası".to_string(),
        "hafta" => "haftalık kotası".to_string(),
        "ay" => "aylık kotası".to_string(),
        diger => format!("{diger} kotası"),
    }
}

/// Süreyi kısa Türkçe metne çevirir: `45 dk`, `2 sa 05 dk`, `3 gün 4 sa`.
pub fn sure_metni(saniye: i64) -> String {
    let dk = (saniye.max(0) + 59) / 60;
    if dk < 60 {
        return format!("{dk} dk");
    }
    let sa = dk / 60;
    if sa < 24 {
        let kalan_dk = dk % 60;
        return if kalan_dk == 0 {
            format!("{sa} sa")
        } else {
            format!("{sa} sa {kalan_dk:02} dk")
        };
    }
    let gun = sa / 24;
    let kalan_sa = sa % 24;
    if kalan_sa == 0 {
        format!("{gun} gün")
    } else {
        format!("{gun} gün {kalan_sa} sa")
    }
}

/// Standart tekrar bastırma anahtarı (`tur:havuz/pencere/sıfırlama`).
pub fn tekilleme_anahtari(tur: &str, havuz: &str, pencere: &str, sifirlama: &str) -> String {
    format!("{tur}:{havuz}/{pencere}/{sifirlama}")
}

/// Yerel saat bilgisini güvenli aralıkta döndürür.
pub fn yerel_saat_now() -> u8 {
    #[cfg(windows)]
    {
        // SAFETY: `GetLocalTime` salt okuma ile geçerli yerel duvar saatini döndürür.
        let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        (st.wHour as u8).min(23)
    }
    #[cfg(not(windows))]
    {
        time::OffsetDateTime::now_utc().hour()
    }
}

pub fn sessiz_saat_mi(yerel_saat: u8, ayar: &BildirimAyari) -> bool {
    let bas = ayar.sessiz_baslangic_saat % 24;
    let bit = ayar.sessiz_bitis_saat % 24;
    if bas == bit {
        return false;
    }
    if bas < bit {
        yerel_saat >= bas && yerel_saat < bit
    } else {
        yerel_saat >= bas || yerel_saat < bit
    }
}

/// Tekilleştirme anahtarının sıfırlama parçası. Sıfırlama bilinmiyorsa (agy metin tablosu,
/// sıfırlaması geçip hesaplanan pencere) pencere uzunluğu kadar (en az 1 sa) dönem
/// damgası kullanılır; aksi halde ilk bildirimden sonra o pencere için bir daha toast gelmezdi.
fn sifirlama_anahtari(sifirlama: Option<UtcZaman>, uzunluk_dk: u64, simdi: UtcZaman) -> String {
    match sifirlama {
        Some(z) => z.to_iso_z(),
        None => {
            let donem_sn = (uzunluk_dk.max(60) * 60) as i64;
            let donem = simdi.as_offset().unix_timestamp().div_euclid(donem_sn);
            format!("belirsiz-{donem}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cetvel_model::{GuvenEtiketi, HavuzAnlik, KaynakBilgisi, KaynakTuru};

    use super::*;

    #[test]
    fn varsayilan_toast_aumid_baslat_kisayoluyla_ayni() {
        // Gerçek toast açılmaz: yalnız göndericinin kimliği kısayolun AUMID'iyle karşılaştırılır.
        let g = WindowsToastGonderici::default();
        assert_eq!(g.aumid(), cetvel_depo::CETVEL_AUMID);
        assert!(cetvel_depo::kisayol::aumid_gecerli_mi(g.aumid()));
        assert_eq!(WindowsToastGonderici::yeni("X.Y").aumid(), "X.Y");
    }

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).unwrap()
    }

    fn ornek_anlik(havuz_id: &str, kalan_5s: f64, yas_sn: u64, simdi: UtcZaman) -> AnlikV2 {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: (1.0 - kalan_5s).clamp(0.0, 1.0),
                kalan: kalan_5s,
                sifirlama: Some(z("2026-11-15T22:00:00Z")),
            },
        );
        let mut havuz = HavuzAnlik {
            platform: "claude".to_string(),
            etiket: "Claude".to_string(),
            durum: if yas_sn > 1800 {
                Durum::Bayat
            } else {
                Durum::Ok
            },
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::ModKoprusu,
                guven: GuvenEtiketi::ResmiArayuz,
                okuma: simdi.ekle_sn(-(yas_sn as i64)),
                yas_sn,
                hassasiyet: Some(0.1),
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        havuz.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert(havuz_id.to_string(), havuz);
        AnlikV2::yeni(simdi, 1, havuzlar)
    }

    #[test]
    fn esik_gecisi_bildirim_uretilir_ve_ayni_pencerede_tekillestirilir() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let ayar = BildirimAyari::default();
        let simdi = z("2026-11-15T14:00:00Z");
        let mut yonetici = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());

        // Eşikten yüksek -> bildirim yok
        let d1 = ornek_anlik("claude_max", 0.50, 0, simdi);
        assert!(
            yonetici
                .degerlendir(&d1, &ayar, &depo, 14, simdi)
                .is_empty()
        );

        // %15 eşiğine düşüş -> 1 toast gönderilir
        let d2 = ornek_anlik("claude_max", 0.14, 0, simdi.ekle_sn(60));
        let g2 = yonetici.degerlendir(&d2, &ayar, &depo, 14, simdi.ekle_sn(60));
        assert_eq!(g2.len(), 1);
        assert_eq!(g2[0].tur, "esik");

        // Toparlanıp yeniden düşse bile aynı sıfırlama döneminde tekrar gönderilmez
        let d3 = ornek_anlik("claude_max", 0.20, 0, simdi.ekle_sn(120));
        assert!(
            yonetici
                .degerlendir(&d3, &ayar, &depo, 14, simdi.ekle_sn(120))
                .is_empty()
        );
        let d4 = ornek_anlik("claude_max", 0.10, 0, simdi.ekle_sn(180));
        assert!(
            yonetici
                .degerlendir(&d4, &ayar, &depo, 14, simdi.ekle_sn(180))
                .is_empty()
        );

        assert_eq!(yonetici.gonderici().kayitlar().len(), 1);
    }

    #[test]
    fn sifirlamasi_bilinmeyen_pencere_sonraki_donemde_yeniden_bildirilir() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let ayar = BildirimAyari::default();
        let simdi = z("2026-11-15T14:00:00Z");
        let mut yonetici = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());
        let anlik = |kalan: f64, t: UtcZaman| {
            let mut a = ornek_anlik("deepseek", kalan, 0, t);
            let h = a.havuzlar.get_mut("deepseek").unwrap();
            let p = h.pencereler.get_mut("5s").unwrap();
            p.sifirlama = None;
            p.uzunluk_dk = 5;
            a
        };
        let _ = yonetici.degerlendir(&anlik(0.9, simdi), &ayar, &depo, 14, simdi);
        let g1 = yonetici.degerlendir(&anlik(0.1, simdi.ekle_sn(60)), &ayar, &depo, 14, simdi);
        assert_eq!(g1.len(), 1);
        // Aynı saat içinde toparlanıp düşerse tekrar gelmez.
        let _ = yonetici.degerlendir(
            &anlik(0.9, simdi.ekle_sn(120)),
            &ayar,
            &depo,
            14,
            simdi.ekle_sn(120),
        );
        assert!(
            yonetici
                .degerlendir(
                    &anlik(0.1, simdi.ekle_sn(180)),
                    &ayar,
                    &depo,
                    14,
                    simdi.ekle_sn(180)
                )
                .is_empty()
        );
        // Ertesi gün yeniden eşiğin altına düşüş yeniden bildirilir.
        let yarin = simdi.ekle_sn(86_400);
        let _ = yonetici.degerlendir(&anlik(0.9, yarin), &ayar, &depo, 14, yarin);
        let g2 = yonetici.degerlendir(
            &anlik(0.1, yarin.ekle_sn(60)),
            &ayar,
            &depo,
            14,
            yarin.ekle_sn(60),
        );
        assert_eq!(g2.len(), 1);
    }

    #[test]
    fn uzun_yoklama_araligi_bayat_bildirimi_uretmez() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let ayar = BildirimAyari::default();
        let simdi = z("2026-11-15T14:00:00Z");
        let mut yonetici = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());
        // Yaş 30 dk'yı geçti ama havuz kendi eşiğine göre hâlâ `ok` (boşta 30 dk aralık).
        let mut a = ornek_anlik("codex_plus", 0.80, 0, simdi);
        a.havuzlar
            .get_mut("codex_plus")
            .unwrap()
            .kaynak
            .as_mut()
            .unwrap()
            .yas_sn = 1850;
        assert!(yonetici.degerlendir(&a, &ayar, &depo, 14, simdi).is_empty());
    }

    #[test]
    fn sessiz_saatlerde_bildirim_baskilanir_ve_bayat_gunde_bir_kez() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let ayar = BildirimAyari::default(); // 00..08 sessiz
        let simdi = z("2026-11-15T14:00:00Z");
        let mut yonetici = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());

        // Saat 03:00'te eşik altı bildirim gönderilmez
        let d1 = ornek_anlik("codex_plus", 0.10, 0, simdi);
        assert!(yonetici.degerlendir(&d1, &ayar, &depo, 3, simdi).is_empty());

        // Bayat kaynak gün içinde en fazla 1 bildirim üretir
        let bayat1 = ornek_anlik("codex_plus", 0.80, 2400, simdi.ekle_sn(60));
        let g1 = yonetici.degerlendir(&bayat1, &ayar, &depo, 14, simdi.ekle_sn(60));
        assert_eq!(g1.len(), 1);
        assert_eq!(g1[0].tur, "bayat");

        let bayat2 = ornek_anlik("codex_plus", 0.80, 2600, simdi.ekle_sn(300));
        assert!(
            yonetici
                .degerlendir(&bayat2, &ayar, &depo, 14, simdi.ekle_sn(300))
                .is_empty()
        );
    }

    fn tahminli_anlik(
        kalan: f64,
        simdi: UtcZaman,
        bitis_sonra_sn: i64,
        sifirlama_sonra_sn: i64,
        biter: bool,
    ) -> AnlikV2 {
        let mut a = ornek_anlik("claude_max", kalan, 0, simdi);
        let h = a.havuzlar.get_mut("claude_max").unwrap();
        let p = h.pencereler.get_mut("5s").unwrap();
        p.sifirlama = Some(simdi.ekle_sn(sifirlama_sonra_sn));
        h.tahmin.insert(
            "5s".to_string(),
            TahminBilgisi {
                durum: TahminDurumu::Deneysel,
                etiket: Some("deneysel".to_string()),
                neden: None,
                bitis_tahmini: Some(simdi.ekle_sn(bitis_sonra_sn)),
                bitis_erken: None,
                bitis_gec: None,
                saatlik_hiz: Some(0.4),
                sifirlanmadan_biter: Some(biter),
                gunluk_butce: None,
            },
        );
        a
    }

    #[test]
    fn tahmin_uyarisi_iki_ardisik_turda_dogrulanir_ve_pencere_basina_bir_kez_gelir() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let ayar = BildirimAyari::default();
        assert!(ayar.tahmin_bildirimi, "akıllı uyarı varsayılan açık");
        let t0 = z("2026-11-15T12:00:00Z");
        let mut y = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());

        // İlk tur: tek örnek uyarı üretmez (sıçrama bastırma).
        let g = y.degerlendir(
            &tahminli_anlik(0.60, t0, 3600, 4 * 3600, true),
            &ayar,
            &depo,
            14,
            t0,
        );
        assert!(g.is_empty());
        // İkinci tur aynı yönde: 1 saatte biter, sıfırlamaya 4 saat → uyarı.
        let t1 = t0.ekle_sn(60);
        let g = y.degerlendir(
            &tahminli_anlik(0.58, t1, 3540, 4 * 3600 - 60, true),
            &ayar,
            &depo,
            14,
            t1,
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].tur, "tahmin");
        assert!(
            g[0].baslik.contains("sıfırlanmadan bitebilir"),
            "{}",
            g[0].baslik
        );
        assert!(g[0].icerik.contains("59 dk"), "{}", g[0].icerik);
        assert!(g[0].icerik.contains("3 sa 59 dk"), "{}", g[0].icerik);
        // Aynı sıfırlama döneminde tekrar gelmez.
        for i in 2..6 {
            let t = t0.ekle_sn(60 * i);
            let g = y.degerlendir(
                &tahminli_anlik(0.55, t, 3000, 4 * 3600 - 60 * i, true),
                &ayar,
                &depo,
                14,
                t,
            );
            assert!(g.is_empty(), "tur {i}");
        }
        assert_eq!(y.gonderici().kayitlar().len(), 1);
    }

    #[test]
    fn tahmin_uyarisi_sessiz_saatte_ve_kapaliyken_gelmez_sonra_gelir() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let mut ayar = BildirimAyari::default();
        let t0 = z("2026-11-15T01:00:00Z");
        let mut y = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());
        let a = |t: UtcZaman| tahminli_anlik(0.60, t, 3600, 4 * 3600, true);
        assert!(y.degerlendir(&a(t0), &ayar, &depo, 3, t0).is_empty());
        assert!(
            y.degerlendir(&a(t0.ekle_sn(60)), &ayar, &depo, 3, t0.ekle_sn(60))
                .is_empty()
        );
        // Kullanıcı kapattıysa sessiz saat dışında da gelmez.
        ayar.tahmin_bildirimi = false;
        assert!(
            y.degerlendir(&a(t0.ekle_sn(120)), &ayar, &depo, 9, t0.ekle_sn(120))
                .is_empty()
        );
        // Açınca (sessiz saat bitti) ertelenen uyarı gelir.
        ayar.tahmin_bildirimi = true;
        let g = y.degerlendir(&a(t0.ekle_sn(180)), &ayar, &depo, 9, t0.ekle_sn(180));
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn tahmin_uyarisi_kosullari_yanlis_alarmi_bastirir() {
        let simdi = z("2026-11-15T12:00:00Z");
        let olcum = |kalan: f64, uzunluk_dk: u64, sifirlama_sn: i64| PencereOlcum {
            uzunluk_dk,
            kullanilan: 1.0 - kalan,
            kalan,
            sifirlama: Some(simdi.ekle_sn(sifirlama_sn)),
        };
        let tahmin = |durum: TahminDurumu, bitis_sn: i64, biter: bool| TahminBilgisi {
            durum,
            etiket: None,
            neden: None,
            bitis_tahmini: Some(simdi.ekle_sn(bitis_sn)),
            bitis_erken: None,
            bitis_gec: None,
            saatlik_hiz: None,
            sifirlanmadan_biter: Some(biter),
            gunluk_butce: None,
        };
        let d = TahminDurumu::Deneysel;
        let onceki = tahmin(d, 3600, true);
        // Olumlu durum.
        assert!(
            tahmin_uyarisi(
                &olcum(0.5, 300, 3 * 3600),
                &tahmin(d, 3600, true),
                Some(&onceki),
                simdi
            )
            .is_some()
        );
        // Önceki tur yok ya da "biter" demiyordu.
        assert!(
            tahmin_uyarisi(
                &olcum(0.5, 300, 3 * 3600),
                &tahmin(d, 3600, true),
                None,
                simdi
            )
            .is_none()
        );
        assert!(
            tahmin_uyarisi(
                &olcum(0.5, 300, 3 * 3600),
                &tahmin(d, 3600, true),
                Some(&tahmin(d, 3600, false)),
                simdi
            )
            .is_none()
        );
        // Sıfırlamadan yalnız 10 dk önce biter: fark çok küçük.
        assert!(
            tahmin_uyarisi(
                &olcum(0.5, 300, 4200),
                &tahmin(d, 3600, true),
                Some(&onceki),
                simdi
            )
            .is_none()
        );
        // Kota zaten bitti ya da tahmin yok.
        assert!(
            tahmin_uyarisi(
                &olcum(0.0, 300, 3 * 3600),
                &tahmin(d, 3600, true),
                Some(&onceki),
                simdi
            )
            .is_none()
        );
        assert!(
            tahmin_uyarisi(
                &olcum(0.5, 300, 3 * 3600),
                &tahmin(TahminDurumu::TahminYok, 3600, true),
                Some(&onceki),
                simdi
            )
            .is_none()
        );
        // Haftalık deneysel: 20 saatte biter → ufuk (12 sa) dışı; 6 saatte biter → uyarı.
        let hafta = 7 * 24 * 60;
        assert!(
            tahmin_uyarisi(
                &olcum(0.3, hafta, 3 * 86_400),
                &tahmin(d, 20 * 3600, true),
                Some(&onceki),
                simdi
            )
            .is_none()
        );
        assert!(
            tahmin_uyarisi(
                &olcum(0.3, hafta, 3 * 86_400),
                &tahmin(d, 6 * 3600, true),
                Some(&onceki),
                simdi
            )
            .is_some()
        );
        // Hazır tahminde ufuk 24 saat.
        assert!(
            tahmin_uyarisi(
                &olcum(0.3, hafta, 3 * 86_400),
                &tahmin(TahminDurumu::Hazir, 20 * 3600, true),
                Some(&onceki),
                simdi
            )
            .is_some()
        );
    }

    #[test]
    fn sure_ve_pencere_metinleri_okunur() {
        assert_eq!(sure_metni(30), "1 dk");
        assert_eq!(sure_metni(45 * 60), "45 dk");
        assert_eq!(sure_metni(2 * 3600 + 5 * 60), "2 sa 05 dk");
        assert_eq!(sure_metni(3 * 3600), "3 sa");
        assert_eq!(sure_metni(3 * 86_400 + 4 * 3600), "3 gün 4 sa");
        assert_eq!(pencere_gorunen_adi("5s"), "5 saatlik kotası");
        assert_eq!(pencere_gorunen_adi("hafta"), "haftalık kotası");
    }

    #[test]
    fn platform_esigi_ve_sessize_alma_bildirimi_yonetir() {
        let depo = GecmisDeposu::bellekte().unwrap();
        let simdi = z("2026-11-15T14:00:00Z");

        // Platform özel eşiği %40 iken %35 kalan eşik bildirimi tetikler
        let mut ozel_esik_ayar = BildirimAyari::default();
        ozel_esik_ayar
            .platform_esikleri
            .insert("claude".to_string(), 40);
        let mut yonetici = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());
        let d_ust = ornek_anlik("claude_max", 0.50, 0, simdi);
        assert!(
            yonetici
                .degerlendir(&d_ust, &ozel_esik_ayar, &depo, 14, simdi)
                .is_empty()
        );
        let d_alt = ornek_anlik("claude_max", 0.35, 0, simdi.ekle_sn(60));
        let giden = yonetici.degerlendir(&d_alt, &ozel_esik_ayar, &depo, 14, simdi.ekle_sn(60));
        assert_eq!(giden.len(), 1);
        assert_eq!(giden[0].tur, "esik");

        // Sessize alınan platformda aynı eşik altı geçiş bildirim üretmez
        let depo_sessiz = GecmisDeposu::bellekte().unwrap();
        let mut sessiz_ayar = ozel_esik_ayar;
        sessiz_ayar.sessiz_platformlar.push("claude".to_string());
        let mut yonetici_sessiz = BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni());
        let giden_sessiz = yonetici_sessiz.degerlendir(
            &d_alt,
            &sessiz_ayar,
            &depo_sessiz,
            14,
            simdi.ekle_sn(120),
        );
        assert!(giden_sessiz.is_empty());
    }
}
