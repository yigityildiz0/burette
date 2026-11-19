//! Cetvel çekirdek servis döngüsü, HTTP/Pipe API, bildirimler ve dosya yayıncıları.

pub mod ajan_izleyici;
pub mod bagla;
pub mod bildirim;
pub mod claude_kopru;
pub mod durum;
pub mod durum_makinesi;
pub mod http_sunucu;
pub mod pipe_sunucu;
pub mod sistem;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use cetvel_baglayici::{
    AntigravityBaglayicisi, Baglayici, CodexBaglayicisi, DeepSeekBaglayicisi, YerelArac,
    YerelSayacBaglayicisi,
};
use cetvel_depo::{
    AgyOkumaIzni, AyarYukleme, Ayarlar, BaglantiBilgisi, CetvelYollar, GecmisDeposu,
    atomik_yaz_json, baglanti_bilgisi_yaz, jeton_yukle_veya_uret, kullanici_dacl_uygula,
};
use cetvel_model::{AnlikV2, HavuzAnlik, UtcZaman, uyum_uret_agy_araligi};
use thiserror::Error;
use tokio::sync::watch;
use tracing::info;

use bagla::{TestSonucu, havuz_test_ozeti};
use bildirim::{BildirimYoneticisi, WindowsToastGonderici};
use durum::{PaylasilanServisDurumu, TestIstegi};
use durum_makinesi::{
    HavuzDurumMakinesi, HavuzZamanlamaKurali, YayinKoordinatoru, bayat_esigi_hesapla, uyanma_algila,
};
use http_sunucu::http_sunucusu_baslat;
use pipe_sunucu::pipe_sunucusu_baslat;
use sistem::{TekOrnekKilidi, kullanici_etkin_mi};

pub use durum_makinesi::TakipEvresi;
pub use pipe_sunucu::{
    kullanici_pipe_adi, pipe_adaylari, pipe_istek_gonder, pipe_istek_kesifle, pipe_sunucu_pid,
};
pub use sistem::{
    cetvel_surecini_sonlandir, servis_calisiyor_mu, surec_yasiyor_mu, yerel_zaman_etiketi,
};

/// İstemcilerin (CLI, MCP, masaüstü) denediği pipe adları: `baglanti.json`'daki ad, oturum SID'li
/// ad ve eski sabit ad (bkz. [`pipe_adaylari`]).
pub fn servis_pipe_adaylari(yollar: &CetvelYollar) -> Vec<String> {
    let kayitli = cetvel_depo::baglanti_bilgisi_oku(yollar)
        .ok()
        .map(|b| b.pipe);
    pipe_adaylari(kayitli.as_deref())
}

/// [`servis_pipe_adaylari`] ile keşfedilen servis pipe'ına istek gönderir.
pub async fn servis_pipe_istegi(
    yollar: &CetvelYollar,
    yontem: &str,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    pipe_istek_kesifle(&servis_pipe_adaylari(yollar), yontem, params)
        .await
        .map(|(_, deger)| deger)
}

/// Servis katmanı hataları.
#[derive(Debug, Error)]
pub enum ServisHatasi {
    #[error("depolama hatası: {0}")]
    Depo(#[from] cetvel_depo::DepoHatasi),
    #[error("sistem G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("servis zaten başka bir örnek tarafından çalıştırılıyor")]
    ZatenCalisiyor,
    #[error("{0}")]
    Genel(String),
}

/// `cetvel oku` tek seferlik okuma filtre seçenekleri.
#[derive(Debug, Clone, Default)]
pub struct OkuSecenekleri {
    pub codex: bool,
    pub agy: bool,
    pub deepseek: bool,
}

impl OkuSecenekleri {
    fn hic_filtre_yok(&self) -> bool {
        !self.codex && !self.agy && !self.deepseek
    }
}

/// Çalıştırılacak bağlayıcı ve zamanlama kurallarını ayarlara göre oluşturur.
pub fn baglayicilari_hazirla(ayarlar: &Ayarlar) -> Vec<(Box<dyn Baglayici>, HavuzZamanlamaKurali)> {
    let mut liste: Vec<(Box<dyn Baglayici>, HavuzZamanlamaKurali)> = Vec::new();

    if ayarlar.codex.etkin {
        let etkin = ayarlar.codex.etkin_aralik_sn.max(30);
        let bosta = ayarlar.codex.bosta_aralik_sn.max(60);
        liste.push((
            Box::new(CodexBaglayicisi::yeni(ayarlar.codex.clone())),
            HavuzZamanlamaKurali {
                etkin_aralik_sn: etkin,
                bosta_aralik_sn: bosta,
                bayat_esigi_sn: bayat_esigi_hesapla(600, etkin.max(bosta)),
            },
        ));
    }

    if ayarlar.deepseek.etkin {
        let ds_ayar = ayarlar.deepseek_coz();
        let aralik = ds_ayar.aralik_sn.max(60);
        liste.push((
            Box::new(DeepSeekBaglayicisi::yeni(ds_ayar)),
            HavuzZamanlamaKurali {
                etkin_aralik_sn: aralik,
                bosta_aralik_sn: aralik,
                bayat_esigi_sn: bayat_esigi_hesapla(1800, aralik),
            },
        ));
    }

    // Kotası olmayan yerel araç sayaçları: varsayılan kapalı, yalnız Bağla'dan açılır.
    for (arac, ayar) in [
        (YerelArac::OpenCode, &ayarlar.opencode),
        (YerelArac::Dsh, &ayarlar.dsh),
    ] {
        if !ayar.etkin {
            continue;
        }
        let aralik = ayar.aralik_sn.max(60);
        liste.push((
            Box::new(YerelSayacBaglayicisi::yeni(arac, ayar.clone())),
            HavuzZamanlamaKurali {
                etkin_aralik_sn: aralik,
                bosta_aralik_sn: aralik.max(900),
                bayat_esigi_sn: bayat_esigi_hesapla(1800, aralik.max(900)),
            },
        ));
    }

    liste
}

/// Bağlayıcı başına ayar imzası (bellek içi; anahtarın kendisi değil özeti). Ayar yeniden
/// yüklenince imzası değişmeyen bağlayıcının durum makinesi (geri çekilme, devre kesici,
/// `retry_after`, son okuma) korunur.
fn baglayici_imzalari(ayarlar: &Ayarlar) -> BTreeMap<String, u64> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let ozet = |parcalar: &[&str]| {
        let mut h = DefaultHasher::new();
        for p in parcalar {
            p.hash(&mut h);
        }
        h.finish()
    };
    let mut imzalar = BTreeMap::new();
    imzalar.insert(
        "codex_plus".to_string(),
        ozet(&[&format!("{:?}", ayarlar.codex)]),
    );
    let ds = ayarlar.deepseek_coz();
    imzalar.insert(
        "deepseek".to_string(),
        ozet(&[
            &format!(
                "{}|{}|{}|{}",
                ds.etkin, ds.taban_url, ds.aralik_sn, ds.zaman_asimi_sn
            ),
            ds.anahtar.as_ref().map(|a| a.acik_metin()).unwrap_or(""),
        ]),
    );
    imzalar.insert(
        "opencode".to_string(),
        ozet(&[&format!("{:?}", ayarlar.opencode)]),
    );
    imzalar.insert("dsh".to_string(), ozet(&[&format!("{:?}", ayarlar.dsh)]));
    imzalar
}

/// İstenen bağlayıcılardan tek seferlik okuma yapıp Anlık v2 üretir (`cetvel oku`).
pub async fn tek_sefer_oku(ayarlar: &Ayarlar, yalniz_codex: bool) -> AnlikV2 {
    tek_sefer_oku_secenekli(
        ayarlar,
        &OkuSecenekleri {
            codex: yalniz_codex,
            agy: false,
            deepseek: false,
        },
    )
    .await
}

/// Ayrıntılı kaynak seçimiyle (`--codex`, `--agy`, `--deepseek`) tek seferlik okuma yapar.
pub async fn tek_sefer_oku_secenekli(ayarlar: &Ayarlar, sec: &OkuSecenekleri) -> AnlikV2 {
    let simdi = UtcZaman::simdi();
    let tumu = sec.hic_filtre_yok();
    let hepsi = baglayicilari_hazirla(ayarlar);

    let mut havuzlar: BTreeMap<String, HavuzAnlik> = BTreeMap::new();
    for (baglayici, _) in hepsi {
        let id = baglayici.havuz_id();
        let secili = if tumu {
            true
        } else if id == "codex_plus" {
            sec.codex
        } else {
            id == "deepseek" && sec.deepseek
        };

        if !secili {
            continue;
        }
        for sonuc in baglayici.oku_coklu(UtcZaman::simdi()).await {
            havuzlar.insert(sonuc.havuz_id, sonuc.havuz);
        }
    }

    if sec.agy || (tumu && ayarlar.agy.okuma == AgyOkumaIzni::Otomatik) {
        let agy = AntigravityBaglayicisi::yeni(ayarlar.agy.clone());
        for sonuc in agy
            .istekle_veya_zamanli_oku(UtcZaman::simdi(), sec.agy)
            .await
        {
            havuzlar.insert(sonuc.havuz_id, sonuc.havuz);
        }
    }

    AnlikV2::yeni(simdi, 1, havuzlar)
}

/// `anlik.json` (v2) ve `orkestra-uyum.json` (v1) dosyalarını atomik olarak yayınlar.
pub fn anlik_ve_uyum_yayinla(
    yollar: &CetvelYollar,
    anlik: &AnlikV2,
    simdi: UtcZaman,
) -> Result<(), ServisHatasi> {
    anlik_ve_uyum_yayinla_ayarli(yollar, anlik, simdi, None)
}

/// `anlik.json` (v2) ve `orkestra-uyum.json` (v1) dosyalarını bilinen `agy` okuma aralığını
/// gözeterek atomik olarak yayınlar.
pub fn anlik_ve_uyum_yayinla_ayarli(
    yollar: &CetvelYollar,
    anlik: &AnlikV2,
    simdi: UtcZaman,
    agy_aralik_sn: Option<u64>,
) -> Result<(), ServisHatasi> {
    let uyum = uyum_uret_agy_araligi(anlik, simdi, agy_aralik_sn);
    atomik_yaz_json(&yollar.anlik_dosyasi(), anlik)?;
    atomik_yaz_json(&yollar.orkestra_uyum_dosyasi(), &uyum)?;
    Ok(())
}

/// Ayarlardan türetilen bağlayıcı, durum makinesi ve bayatlık kümesi. Ayar değişince yeniden kurulur.
struct DonguKurulumu {
    baglayicilar: Vec<(Box<dyn Baglayici>, HavuzZamanlamaKurali)>,
    makineler: BTreeMap<String, HavuzDurumMakinesi>,
    agy: AntigravityBaglayicisi,
    agy_makine: HavuzDurumMakinesi,
    bayat_esikleri: BTreeMap<String, u64>,
    yoklanan: BTreeSet<String>,
    imzalar: BTreeMap<String, u64>,
    agy_ayar: cetvel_depo::AgyAyari,
}

impl DonguKurulumu {
    fn olustur(ayarlar: &Ayarlar) -> Self {
        let baglayicilar = baglayicilari_hazirla(ayarlar);
        let agy = AntigravityBaglayicisi::yeni(ayarlar.agy.clone());
        let agy_aralik = ayarlar.agy.aralik_sn.max(60);
        let agy_kural = HavuzZamanlamaKurali {
            etkin_aralik_sn: agy_aralik,
            bosta_aralik_sn: agy_aralik,
            bayat_esigi_sn: bayat_esigi_hesapla(1800, agy_aralik),
        };

        let mut makineler = BTreeMap::new();
        let mut bayat_esikleri = BTreeMap::new();
        let mut yoklanan = BTreeSet::new();
        for (b, kural) in &baglayicilar {
            makineler.insert(b.havuz_id().to_string(), HavuzDurumMakinesi::yeni(*kural));
            for kimlik in b.havuz_kimlikleri() {
                bayat_esikleri.insert(kimlik.clone(), kural.bayat_esigi_sn);
                yoklanan.insert(kimlik);
            }
        }
        if ayarlar.agy.okuma != AgyOkumaIzni::Kapali {
            for kimlik in agy.havuz_kimlikleri() {
                bayat_esikleri.insert(kimlik.clone(), agy_kural.bayat_esigi_sn);
                yoklanan.insert(kimlik);
            }
        }
        bayat_esikleri.insert("claude_max".to_string(), 900);

        Self {
            baglayicilar,
            makineler,
            agy,
            agy_makine: HavuzDurumMakinesi::yeni(agy_kural),
            bayat_esikleri,
            yoklanan,
            imzalar: baglayici_imzalari(ayarlar),
            agy_ayar: ayarlar.agy.clone(),
        }
    }

    /// Ayar yeniden yüklenince eski kurulumdan durumu devralır: ayarı (imzası) değişmeyen
    /// bağlayıcının evresi korunur (devre kesici / geri çekilme silinmez, havuz hemen yeniden
    /// sorgulanmaz). agy saatlik çağrı sayacı her durumda taşınır; agy ayarı aynıysa makinesi de.
    fn devral(&mut self, mut eski: DonguKurulumu) {
        for (id, dm) in &mut self.makineler {
            if self.imzalar.get(id) != eski.imzalar.get(id) {
                continue;
            }
            if let Some(e) = eski.makineler.remove(id)
                && !matches!(
                    e.evre,
                    TakipEvresi::Baslangic | TakipEvresi::Baglanmadi { .. }
                )
            {
                dm.evre = e.evre;
            }
        }
        self.agy.sayaci_devral(&eski.agy);
        if self.agy_ayar == eski.agy_ayar {
            self.agy_makine.evre = eski.agy_makine.evre;
        }
    }

    /// Zamanı gelen, sıfırlaması geçen veya elle istenen havuzları okur.
    async fn oku(
        &mut self,
        koordinator: &mut YayinKoordinatoru,
        simdi: UtcZaman,
        kullanici_etkin: bool,
        uyanma: bool,
        hedefler: &BTreeSet<String>,
    ) {
        let hepsi = hedefler.contains("*");
        for (b, _) in &self.baglayicilar {
            let id = b.havuz_id();
            let kimlikler = b.havuz_kimlikleri();
            let istendi =
                hepsi || hedefler.contains(id) || kimlikler.iter().any(|k| hedefler.contains(k));
            let Some(dm) = self.makineler.get_mut(id) else {
                continue;
            };
            let sifirlama_gecti = kimlikler
                .iter()
                .any(|k| dm.sifirlama_gecti_mi(koordinator.havuzlar.get(k), simdi));
            let gerekli = dm.okunmali_mi(simdi, kullanici_etkin, uyanma)
                || sifirlama_gecti
                || (istendi && dm.zorla_okunabilir_mi(simdi));
            if !gerekli {
                continue;
            }
            let okuma_ani = UtcZaman::simdi();
            let sonuclar = b.oku_coklu(okuma_ani).await;
            if let Some(ilk) = sonuclar.first() {
                dm.sonuc_isle(okuma_ani, ilk);
            }
            for sonuc in sonuclar {
                koordinator.havuz_sonucu_birlestir(sonuc);
            }
        }

        let izin = self.agy.izin();
        let agy_istendi = hedefler
            .iter()
            .any(|h| h.to_ascii_lowercase().contains("agy"));
        let agy_calissin = match izin {
            AgyOkumaIzni::Kapali => false,
            _ if agy_istendi => self.agy_makine.zorla_okunabilir_mi(simdi),
            AgyOkumaIzni::Otomatik => self.agy_makine.okunmali_mi(simdi, kullanici_etkin, uyanma),
            AgyOkumaIzni::IstekUzerine => false,
        };
        if agy_calissin {
            let okuma_ani = UtcZaman::simdi();
            let sonuclar = self
                .agy
                .istekle_veya_zamanli_oku(okuma_ani, agy_istendi)
                .await;
            if let Some(ilk) = sonuclar.first() {
                self.agy_makine.sonuc_isle(okuma_ani, ilk);
            }
            for sonuc in sonuclar {
                koordinator.havuz_sonucu_birlestir(sonuc);
            }
        }
    }

    /// "Test et": bağlayıcıyı tek okuyucu olan servis döngüsünde çalıştırır. Hız sınırı veya
    /// geri çekilme varsa yeniden okumaz, son bilinen sonucu yaşıyla döndürür.
    async fn test_et(
        &mut self,
        platform: &str,
        koordinator: &mut YayinKoordinatoru,
        durum: &PaylasilanServisDurumu,
    ) -> TestSonucu {
        let baslangic = std::time::Instant::now();
        let simdi = UtcZaman::simdi();
        let ms = |b: std::time::Instant| b.elapsed().as_millis() as u64;

        if platform == "claude" {
            let anlik = durum.anlik_kopya().await;
            return match anlik.havuzlar.get("claude_max") {
                Some(cl) => onbellek_test_sonucu(platform, cl, simdi, ms(baslangic)),
                None => TestSonucu::basarisiz(
                    platform,
                    ms(baslangic),
                    "köprü verisi yok: kota-bandi köprüsü (POST /v1/claude) henüz veri göndermedi",
                    "no bridge data: kota-bandi bridge (POST /v1/claude) has not sent data yet",
                ),
            };
        }

        if platform == "agy" {
            if self.agy.izin() == AgyOkumaIzni::Kapali {
                return TestSonucu::basarisiz(
                    platform,
                    ms(baslangic),
                    "Antigravity okuma izni kapalı",
                    "Antigravity read permission is off",
                );
            }
            if !self.agy_makine.zorla_okunabilir_mi(simdi) {
                return son_bilinen_veya_bekle(
                    platform,
                    koordinator
                        .havuzlar
                        .iter()
                        .find(|(id, _)| id.starts_with("agy_"))
                        .map(|(_, h)| h),
                    simdi,
                    ms(baslangic),
                );
            }
            let sonuclar = self.agy.istekle_veya_zamanli_oku(simdi, true).await;
            if let Some(ilk) = sonuclar.first() {
                self.agy_makine.sonuc_isle(simdi, ilk);
            }
            return okuma_test_sonucu(platform, sonuclar, koordinator, ms(baslangic));
        }

        let hedef_id = match platform {
            "codex" => "codex_plus",
            "deepseek" => "deepseek",
            "opencode" => "opencode",
            "dsh" => "dsh",
            diger => diger,
        };
        let Some((b, _)) = self
            .baglayicilar
            .iter()
            .find(|(b, _)| b.havuz_id() == hedef_id)
        else {
            return TestSonucu::basarisiz(
                platform,
                ms(baslangic),
                "bağlayıcı ayarlarda kapalı",
                "connector is disabled in settings",
            );
        };
        let Some(dm) = self.makineler.get_mut(hedef_id) else {
            return TestSonucu::basarisiz(platform, ms(baslangic), "durum yok", "no state");
        };
        if !dm.zorla_okunabilir_mi(simdi) {
            return son_bilinen_veya_bekle(
                platform,
                koordinator.havuzlar.get(hedef_id),
                simdi,
                ms(baslangic),
            );
        }
        let sonuclar = b.oku_coklu(simdi).await;
        if let Some(ilk) = sonuclar.first() {
            dm.sonuc_isle(simdi, ilk);
        }
        okuma_test_sonucu(platform, sonuclar, koordinator, ms(baslangic))
    }
}

fn okuma_test_sonucu(
    platform: &str,
    sonuclar: Vec<cetvel_baglayici::BaglayiciSonucu>,
    koordinator: &mut YayinKoordinatoru,
    sure_ms: u64,
) -> TestSonucu {
    let ozet = sonuclar.first().map(|s| havuz_test_ozeti(&s.havuz));
    for sonuc in sonuclar {
        koordinator.havuz_sonucu_birlestir(sonuc);
    }
    match ozet {
        Some((basarili, tr, en)) => TestSonucu {
            platform: platform.to_string(),
            basarili,
            sure_ms,
            ozet_tr: format!("{sure_ms} ms · {tr}"),
            ozet_en: format!("{sure_ms} ms · {en}"),
            onbellekten: false,
            yas_sn: Some(0),
        },
        None => TestSonucu::basarisiz(
            platform,
            sure_ms,
            &format!("{sure_ms} ms · bağlayıcı yanıt üretmedi"),
            &format!("{sure_ms} ms · connector produced no data"),
        ),
    }
}

fn onbellek_test_sonucu(
    platform: &str,
    havuz: &HavuzAnlik,
    simdi: UtcZaman,
    sure_ms: u64,
) -> TestSonucu {
    let (basarili, tr, en) = havuz_test_ozeti(havuz);
    let yas = havuz
        .kaynak
        .as_ref()
        .map(|k| simdi.fark_sn(k.okuma).max(0) as u64);
    let yas_metin = yas
        .map(|y| format!("{y} sn"))
        .unwrap_or_else(|| "?".to_string());
    TestSonucu {
        platform: platform.to_string(),
        basarili,
        sure_ms,
        ozet_tr: format!("son okuma {yas_metin} önce · {tr}"),
        ozet_en: format!("last read {yas_metin} ago · {en}"),
        onbellekten: true,
        yas_sn: yas,
    }
}

fn son_bilinen_veya_bekle(
    platform: &str,
    havuz: Option<&HavuzAnlik>,
    simdi: UtcZaman,
    sure_ms: u64,
) -> TestSonucu {
    match havuz {
        Some(h) => onbellek_test_sonucu(platform, h, simdi, sure_ms),
        None => TestSonucu::basarisiz(
            platform,
            sure_ms,
            "okuma şu an sınırlı (hız sınırı ya da geri çekilme); biraz sonra yeniden deneyin",
            "reading is limited right now (rate limit or backoff); try again shortly",
        ),
    }
}

/// Döngüyü uyandıran olay.
enum Uyandiran {
    Kapat,
    Yenile,
    AyarYenile,
    Test(Option<TestIstegi>),
    Zaman,
}

async fn test_bekle(
    rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<TestIstegi>>,
) -> Option<TestIstegi> {
    match rx {
        Some(r) => r.recv().await,
        None => std::future::pending().await,
    }
}

fn dosya_degisim_ani(yol: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(yol).and_then(|m| m.modified()).ok()
}

/// Ayarları kurtarmalı yükler ve kurtarma olduysa uyarıyı günlüğe yazar.
pub fn ayarlari_kurtarmali_yukle(yollar: &CetvelYollar) -> AyarYukleme {
    let yukleme = Ayarlar::yukle_kurtarmali(&yollar.ayarlar_dosyasi(), &yerel_zaman_etiketi());
    if let Some(aciklama) = yukleme.kurtarma.aciklama() {
        tracing::warn!("{aciklama}");
    }
    yukleme
}

/// Arka plan servis döngüsünü, yerel HTTP/Pipe API'lerini ve SQLite geçmişini başlatır (`cetvel servis`).
pub async fn servis_calistir(yollar: &CetvelYollar, tek_tur: bool) -> Result<(), ServisHatasi> {
    let _kilit = TekOrnekKilidi::al()?.ok_or(ServisHatasi::ZatenCalisiyor)?;

    std::fs::create_dir_all(&yollar.local_kok)?;
    std::fs::create_dir_all(&yollar.roaming_kok)?;
    let _ = kullanici_dacl_uygula(&yollar.local_kok);
    let _ = kullanici_dacl_uygula(&yollar.roaming_kok);

    let ilk_yukleme = ayarlari_kurtarmali_yukle(yollar);
    let ayarlar = ilk_yukleme.ayarlar.clone();
    // Dosya şu an okunamadıysa (kilit/paylaşım) ilk tikte yeniden denenir.
    let ilk_okunamadi = !ilk_yukleme.kurtarma.yazmaya_uygun();

    let depo = GecmisDeposu::ac(&yollar.gecmis_veritabani())?;
    let _ = depo.eski_verileri_temizle(UtcZaman::simdi(), 90);
    let jeton = jeton_yukle_veya_uret(yollar)?;
    let pipe_adi = kullanici_pipe_adi();

    let durum = Arc::new(PaylasilanServisDurumu::yeni(
        ayarlar.clone(),
        Some(yollar.clone()),
        jeton.clone(),
        pipe_adi.clone(),
        Some(depo),
    ));
    durum.ayar_uyarisi_ayarla(ilk_yukleme.kurtarma.aciklama());
    // Ajan ağacının sira'sı da (anlık gibi) yeniden başlatmada sürer; HTTP açılmadan önce.
    if let Some(sira) = onceki_ajan_sirasi_oku(&yollar.ajanlar_dosyasi()) {
        durum.ajan_sirasini_tohumla(sira);
    }
    durum.bildirim_yoneticisi_bagla(BildirimYoneticisi::yeni(WindowsToastGonderici::default()));

    let (kap_tx, kap_rx) = watch::channel(false);
    let (baglanan_port, adresler, http_gorev) = http_sunucusu_baslat(
        Arc::clone(&durum),
        ayarlar.http.port,
        ayarlar.http.otomatik_port,
        kap_rx.clone(),
    )
    .await?;
    let pipe_gorev =
        pipe_sunucusu_baslat(Arc::clone(&durum), pipe_adi.clone(), kap_rx.clone()).await?;

    // Claude dışı ajan ağacı: tek turda bir kez, sürekli kipte ayrı yoklama görevi.
    let ajan_gorev = if tek_tur {
        let ayar = durum.ayarlar_kopya().ajan.clone();
        let okuyucu = cetvel_ajan::AjanOkuyucu::yeni(ajan_izleyici::yapilandirma_olustur(&ayar));
        let mut son_yazim = None;
        let _ = ajan_izleyici::tur_calistir(okuyucu, &durum, Some(yollar), &mut son_yazim).await;
        None
    } else {
        Some(tokio::spawn(ajan_izleyici::ajan_izleyicisi_calistir(
            Arc::clone(&durum),
            Some(yollar.clone()),
            kap_rx,
        )))
    };

    let baglanti = BaglantiBilgisi {
        port: baglanan_port,
        adresler: adresler.clone(),
        jeton,
        pipe: pipe_adi,
        surum: env!("CARGO_PKG_VERSION").to_string(),
        pid: Some(std::process::id()),
    };
    baglanti_bilgisi_yaz(yollar, &baglanti)?;

    let mut kurulum = DonguKurulumu::olustur(&ayarlar);
    let mut koordinator = YayinKoordinatoru::yeni(kurulum.bayat_esikleri.clone());

    // Yeniden başlatmada sira ve son bilinen değerler (Claude dahil) korunur.
    if let Some(onceki) = onceki_anlik_oku(&yollar.anlik_dosyasi()) {
        durum.baslangic_anligi_kur(&onceki).await;
        koordinator.tohumla(
            onceki
                .havuzlar
                .into_iter()
                .filter(|(id, _)| kurulum.yoklanan.contains(id))
                .collect(),
        );
    }

    let tik_araligi_sn = 15u64;
    let mut onceki_tik = UtcZaman::simdi();
    let mut son_yayin: Option<UtcZaman> = None;
    let mut ayar_yenile = ilk_okunamadi;
    let mut ayar_ani = dosya_degisim_ani(&yollar.ayarlar_dosyasi());
    let mut test_rx = durum.test_alicisini_al();

    info!(
        havuz_sayisi = kurulum.baglayicilar.len(),
        port = baglanan_port,
        "Cetvel servisi başlatıldı"
    );

    loop {
        let simdi = UtcZaman::simdi();
        let uyanma = uyanma_algila(onceki_tik, simdi, tik_araligi_sn);
        if uyanma {
            info!("sistem uyanması algılandı; havuzlar yenileniyor");
        }
        onceki_tik = simdi;

        // Ayar dosyası dışarıdan (masaüstü, elle) değiştiyse ya da açıkça istendiyse yeniden yükle.
        let guncel_ani = dosya_degisim_ani(&yollar.ayarlar_dosyasi());
        if guncel_ani != ayar_ani {
            ayar_yenile = true;
        }
        if ayar_yenile {
            let yukleme = ayarlari_kurtarmali_yukle(yollar);
            durum.ayar_uyarisi_ayarla(yukleme.kurtarma.aciklama());
            if yukleme.kurtarma.yazmaya_uygun() {
                let eski =
                    std::mem::replace(&mut kurulum, DonguKurulumu::olustur(&yukleme.ayarlar));
                kurulum.devral(eski);
                koordinator.kimlikleri_koru(&kurulum.yoklanan);
                koordinator.bayat_esikleri = kurulum.bayat_esikleri.clone();
                durum.ayarlari_degistir(yukleme.ayarlar);
                ayar_ani = dosya_degisim_ani(&yollar.ayarlar_dosyasi());
                ayar_yenile = false;
                info!("ayarlar yeniden yüklendi");
            } else {
                // Geçici okuma hatası: önceki ayarlar korunur, sonraki tikte yeniden denenir.
                tracing::warn!("ayarlar.toml şu an okunamadı; önceki ayarlar korunuyor");
            }
        }

        let hedefler = durum.yenile_hedeflerini_al();
        let guncel_ayarlar = durum.ayarlar_kopya();
        let kullanici_etkin = kullanici_etkin_mi(guncel_ayarlar.servis.bosta_esik_sn);
        kurulum
            .oku(&mut koordinator, simdi, kullanici_etkin, uyanma, &hedefler)
            .await;

        let yayin_ani = UtcZaman::simdi();
        let kalp_atisi_sn = guncel_ayarlar.servis.kalp_atisi_sn.max(10) as i64;
        let kalp_atisi_geldi =
            tek_tur || son_yayin.is_none_or(|s| yayin_ani.fark_sn(s) >= kalp_atisi_sn);
        if durum
            .dongu_yayinla(
                koordinator.yaslandirilmis(yayin_ani),
                &kurulum.yoklanan,
                yayin_ani,
                kalp_atisi_geldi,
            )
            .await
            .is_some()
        {
            son_yayin = Some(yayin_ani);
        }

        // `baglanti.json` silindiyse yeniden yaz (masaüstü/CLI bağlanabilsin).
        if !yollar.baglanti_dosyasi().exists()
            && let Err(e) = baglanti_bilgisi_yaz(yollar, &baglanti)
        {
            tracing::warn!("baglanti.json yeniden yazılamadı: {e}");
        }

        if tek_tur {
            break;
        }

        let olay = tokio::select! {
            _ = tokio::signal::ctrl_c() => Uyandiran::Kapat,
            _ = durum.kapat_iste.notified() => Uyandiran::Kapat,
            _ = durum.yenile_bildir.notified() => Uyandiran::Yenile,
            _ = durum.ayar_yenile_iste.notified() => Uyandiran::AyarYenile,
            istek = test_bekle(&mut test_rx) => Uyandiran::Test(istek),
            _ = tokio::time::sleep(Duration::from_secs(tik_araligi_sn)) => Uyandiran::Zaman,
        };
        match olay {
            Uyandiran::Kapat => {
                info!("kapatma isteği alındı; Cetvel servisi durduruluyor");
                break;
            }
            Uyandiran::AyarYenile => ayar_yenile = true,
            Uyandiran::Test(Some(istek)) => {
                let sonuc = kurulum
                    .test_et(&istek.platform, &mut koordinator, &durum)
                    .await;
                let _ = istek.yanit.send(sonuc);
            }
            Uyandiran::Test(None) => test_rx = None,
            Uyandiran::Yenile | Uyandiran::Zaman => {}
        }
    }

    let _ = kap_tx.send(true);
    let _ = tokio::time::timeout(Duration::from_secs(1), http_gorev).await;
    let _ = tokio::time::timeout(Duration::from_secs(1), pipe_gorev).await;
    if let Some(g) = ajan_gorev {
        let _ = tokio::time::timeout(Duration::from_secs(2), g).await;
    }

    Ok(())
}

fn onceki_anlik_oku(anlik_yol: &std::path::Path) -> Option<AnlikV2> {
    std::fs::read_to_string(anlik_yol)
        .ok()
        .and_then(|s| serde_json::from_str::<AnlikV2>(&s).ok())
}

/// Önceki `ajanlar.json`'dan yalnız `sira` (şema değişse de okunur).
fn onceki_ajan_sirasi_oku(yol: &std::path::Path) -> Option<u64> {
    let metin = std::fs::read_to_string(yol).ok()?;
    serde_json::from_str::<serde_json::Value>(&metin)
        .ok()?
        .get("sira")?
        .as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bildirim::SahteBildirimGonderici;
    use cetvel_model::{Durum, GuvenEtiketi, KaynakBilgisi, KaynakTuru, PencereOlcum};

    #[test]
    fn anlik_ve_uyum_yayinla_dosyalari_atomik_yazar_ve_dogrular() {
        let kok = std::env::temp_dir().join(format!("cetvel-servis-yayin-{}", std::process::id()));
        let yollar = CetvelYollar::ozel_koklerle(kok.join("local"), kok.join("roaming"));
        let okuma = UtcZaman::parse_tarih_veya_zaman("2026-11-15T18:00:00Z").expect("zaman");

        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.0,
                kalan: 1.0,
                sifirlama: None,
            },
        );
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                uzunluk_dk: 10080,
                kullanilan: 1.0,
                kalan: 0.0,
                sifirlama: Some(
                    UtcZaman::parse_tarih_veya_zaman("2026-11-16T07:00:00Z").expect("sifir"),
                ),
            },
        );
        let mut codex = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::CodexAppServer,
                guven: GuvenEtiketi::ResmiArayuz,
                okuma,
                yas_sn: 0,
                hassasiyet: None,
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
        codex.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("codex_plus".to_string(), codex);
        let anlik = AnlikV2::yeni(okuma, 1, havuzlar);

        anlik_ve_uyum_yayinla(&yollar, &anlik, okuma).expect("yayin");

        let v2_str = std::fs::read_to_string(yollar.anlik_dosyasi()).expect("v2 oku");
        let v2: AnlikV2 = serde_json::from_str(&v2_str).expect("v2 json");
        cetvel_model::anlik_dogrula(&v2).expect("v2 dogrula");
        assert_eq!(v2.havuzlar["codex_plus"].etkin_kalan, Some(0.0));
        assert_eq!(
            v2.havuzlar["codex_plus"].baglayici_pencere.as_deref(),
            Some("hafta")
        );
        assert_eq!(v2.havuzlar["codex_plus"].kullanilabilir, Some(false));

        let v1_str = std::fs::read_to_string(yollar.orkestra_uyum_dosyasi()).expect("v1 oku");
        let v1: cetvel_model::OrkestraUyumV1 = serde_json::from_str(&v1_str).expect("v1 json");
        cetvel_model::uyum_dogrula(&v1).expect("v1 dogrula");
        assert_eq!(v1.yazan, "rust-servis");
        assert!(!v1.havuzlar["codex_plus"].kullanilabilir);
        assert_eq!(v1.havuzlar["codex_plus"].etkin_kalan, Some(0.0));
        assert_eq!(
            v1.havuzlar["codex_plus"].baglayici_pencere.as_deref(),
            Some("hafta")
        );

        let _ = std::fs::remove_dir_all(&kok);
    }

    #[tokio::test]
    async fn paylasilan_durum_gecmis_tahmin_ve_sahte_bildirim_isleme() {
        let kok = std::env::temp_dir().join(format!("cetvel-servis-ent-{}", std::process::id()));
        let yollar = CetvelYollar::ozel_koklerle(kok.join("local"), kok.join("roaming"));
        let depo = GecmisDeposu::bellekte().unwrap();
        let durum = PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            Some(yollar.clone()),
            "test-jeton-abc",
            r"\\.\pipe\cetvel-test-ent",
            Some(depo),
        );
        durum.bildirim_yoneticisi_bagla(BildirimYoneticisi::yeni(SahteBildirimGonderici::yeni()));

        let simdi = UtcZaman::simdi();
        for (i, yuzde) in [10.0, 14.0, 18.0, 22.0].iter().enumerate() {
            let t = simdi.ekle_sn((i as i64 - 3) * 600);
            let yuk = serde_json::json!({
                "five_hour": { "utilization": yuzde, "resets_at": simdi.ekle_sn(7200).to_iso_z() }
            });
            let _ = durum.claude_guncelle(yuk, t).await.unwrap();
        }

        let anlik = durum.anlik_kopya().await;
        let cm = &anlik.havuzlar["claude_max"];
        assert!(cm.tahmin.contains_key("5s"));
        assert_eq!(cm.tahmin["5s"].etiket.as_deref(), Some("deneysel"));
        assert!(yollar.anlik_dosyasi().exists());

        let _ = std::fs::remove_dir_all(&kok);
    }

    /// Gerçek Credential Manager / ortam anahtarlarına dokunmayan test ayarı.
    fn anahtarsiz_ayar() -> Ayarlar {
        let mut ayar = Ayarlar::default();
        ayar.deepseek.etkin = true;
        ayar.deepseek.anahtar_env = Some("CETVEL_TEST_YOK_DS_91".to_string());
        ayar
    }

    #[test]
    fn ayar_yenilemesi_degismeyen_baglayicinin_devre_kesicisini_ve_agy_evresini_korur() {
        let mut ayar = anahtarsiz_ayar();
        ayar.agy.okuma = AgyOkumaIzni::Otomatik;
        let mut eski = DonguKurulumu::olustur(&ayar);
        let t = UtcZaman::simdi();
        let acik = TakipEvresi::DevreAcik {
            acilis: t,
            sonraki_deneme: t.ekle_sn(1800),
            ardisik_hata: 5,
        };
        eski.makineler.get_mut("deepseek").expect("deepseek").evre = acik.clone();
        eski.makineler.get_mut("codex_plus").expect("codex").evre =
            TakipEvresi::Aktif { son_okuma: t };
        eski.agy_makine.evre = TakipEvresi::Aktif { son_okuma: t };

        // Yalnız tema ve Codex aralığı değişti.
        ayar.masaustu.renk_temasi = "okyanus".to_string();
        ayar.codex.etkin_aralik_sn = 900;
        let mut yeni = DonguKurulumu::olustur(&ayar);
        yeni.devral(eski);
        assert_eq!(yeni.makineler["deepseek"].evre, acik, "devre kesici korunur");
        assert_eq!(
            yeni.makineler["codex_plus"].evre,
            TakipEvresi::Baslangic,
            "ayarı değişen bağlayıcı sıfırdan başlar"
        );
        assert!(matches!(yeni.agy_makine.evre, TakipEvresi::Aktif { .. }));

        // agy izni değişirse agy makinesi yeniden başlar.
        let mut ayar2 = ayar.clone();
        ayar2.agy.okuma = AgyOkumaIzni::IstekUzerine;
        let mut ucuncu = DonguKurulumu::olustur(&ayar2);
        ucuncu.devral(yeni);
        assert_eq!(ucuncu.agy_makine.evre, TakipEvresi::Baslangic);
    }

    #[test]
    fn codex_bosta_araligi_bayat_esiginden_kisa() {
        let ayar = anahtarsiz_ayar();
        for (b, kural) in baglayicilari_hazirla(&ayar) {
            assert!(
                kural.bayat_esigi_sn > kural.bosta_aralik_sn.max(kural.etkin_aralik_sn),
                "{}: eşik {} ≤ aralık {}",
                b.havuz_id(),
                kural.bayat_esigi_sn,
                kural.bosta_aralik_sn
            );
        }
    }

    #[tokio::test]
    async fn tek_sefer_oku_deepseek_anahtarsiz_baglanmadi_dondurur() {
        let mut ayar = Ayarlar::default();
        ayar.deepseek.etkin = true;
        ayar.deepseek.anahtar_env = Some("CETVEL_VAR_OLMAYAN_DS_KEY_12345".to_string());

        let anlik = tek_sefer_oku_secenekli(
            &ayar,
            &OkuSecenekleri {
                codex: false,
                agy: false,
                deepseek: true,
            },
        )
        .await;
        assert_eq!(anlik.havuzlar.len(), 1);
        assert_eq!(anlik.havuzlar["deepseek"].durum, Durum::Baglanmadi);
    }

    #[test]
    fn anlik_ve_uyum_yayinla_ayarli_agy_araligini_korur() {
        let kok =
            std::env::temp_dir().join(format!("cetvel-servis-agy-uyum-{}", std::process::id()));
        let yollar = CetvelYollar::ozel_koklerle(kok.join("local"), kok.join("roaming"));
        let okuma = UtcZaman::parse_tarih_veya_zaman("2026-11-16T10:00:00Z").expect("zaman");
        let simdi = okuma.ekle_sn(3300); // 55 dk sonra

        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.15,
                kalan: 0.85,
                sifirlama: Some(okuma.ekle_sn(18_000)),
            },
        );
        let mut agy = HavuzAnlik {
            platform: "agy".to_string(),
            etiket: "Antigravity Gemini".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::AgyUsage,
                guven: GuvenEtiketi::Tahmini,
                okuma,
                yas_sn: 3300,
                hassasiyet: None,
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
        agy.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("agy_gemini".to_string(), agy);
        let anlik = AnlikV2::yeni(simdi, 1, havuzlar);

        anlik_ve_uyum_yayinla_ayarli(&yollar, &anlik, simdi, Some(3600)).expect("yayin");
        let v1_str = std::fs::read_to_string(yollar.orkestra_uyum_dosyasi()).expect("v1 oku");
        let v1: cetvel_model::OrkestraUyumV1 = serde_json::from_str(&v1_str).expect("v1 json");
        assert_eq!(v1.havuzlar["agy_gemini"].durum, cetvel_model::UyumDurum::Ok);
        assert!(v1.havuzlar["agy_gemini"].kullanilabilir);
        assert_eq!(v1.havuzlar["agy_gemini"].pencereler["5s"].kalan, 0.85);

        let _ = std::fs::remove_dir_all(&kok);
    }
}
