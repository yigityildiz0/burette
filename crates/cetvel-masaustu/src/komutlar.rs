//! Masaüstü ön yüzünden çağrılan Tauri komutları.
//!
//! Masaüstü yalnız görüntüleyicidir: kota verisi, Bağla taraması ve "Test et" servis üzerinden
//! (HTTP/pipe) gelir; bu süreç hiçbir bağlayıcı çalıştırmaz. Burada yalnız kullanıcı ayarları
//! (`ayarlar.toml`), Credential Manager yazımı ve onaylı Claude `statusLine` düzenlemesi yapılır.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{Ordering, compiler_fence};

use cetvel_depo::{
    Ayarlar, CetvelYollar, deger_yonu_gecerli_mi, dil_gecerli_mi, eski_tema_gorunumu,
    eski_temayi_esle, kimlik_kaydet, kip_gecerli_mi, renk_temasi_gecerli_mi,
};
use cetvel_servis::bagla::{
    StatuslinePlani, TaramaSonucu, TestSonucu, agy_izni_coz, agy_izni_metne, claude_settings_yolu,
    deepseek_env_adi, settings_yedekle_ve_yaz, statusline_cagri_metni_hazirla, statusline_plani,
};
use cetvel_servis::yerel_zaman_etiketi;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

use crate::durum::{MasaustuDurum, SeritDurumu, tepsi_gorunum_guncelle, tepsi_tercihlerini_uygula};
use crate::pencere::{pencere_ac_ve_odakla, serit_gorunurluk_ayarla, serit_secenek_uygula};
use crate::servis_istemci::{AnlikServisYaniti, ServisDurumuBilgisi, cetvel_ikilisi_bul};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnahtarKayitSonucu {
    pub platform: String,
    pub anahtar_var: bool,
    /// Servis yeni anahtarı hemen yeniden yüklesin diye bilgilendirildi mi?
    pub servis_bildirildi: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YenileKomutYaniti {
    pub istendi: bool,
    pub bekle_sn: Option<u64>,
    pub servis_calisiyor: bool,
    pub baslatma_onerisi: bool,
    pub hata: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatuslineOnizleme {
    pub zaten_tanimli: bool,
    pub zaten_cetvel: bool,
    pub onceki_komut_var: bool,
    pub uygulanabilir: bool,
    pub fark_metni: String,
    pub hata: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatuslineSonucu {
    pub yazildi: bool,
    pub yedek: Option<String>,
}

/// Arayüzün gösterdiği/değiştirdiği kalıcı ayarlar (`%APPDATA%\Cetvel\ayarlar.toml`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArayuzAyarlari {
    pub kart_sirasi: Vec<String>,
    pub gizli_kartlar: Vec<String>,
    pub sabit_kartlar: Vec<String>,
    pub pencere_bosta_sn: u64,
    /// Arayüz dili: `tr`, `en`, `de`, `fr`, `es`, `pt-BR`, `ru`, `ja`, `ko`, `zh-Hans`, `zh-Hant`.
    pub dil: String,
    /// Renk teması: `terakota`, `okyanus`, `adacayi`, `karbon`.
    pub renk_temasi: String,
    /// Görünüm kipi: `acik`, `koyu`, `sistem`.
    pub kip: String,
    /// Gösterge ve tepsi yüzde yönü: `kalan` veya `kullanilan`.
    pub deger_yonu: String,
    /// Eski arayüz sürümleri için kipten türetilen `murekkep`/`kagit` görünümü. Yeni arayüz
    /// `renk_temasi` + `kip` kullanmalı; eski arayüz kalmayınca bu alan kaldırılır.
    pub tema: String,
    pub windows_ile_baslat: bool,
    pub tepsiye_kucult: bool,
    pub agy_okuma: String,
    pub bildirim_etkin: bool,
    pub tahmin_bildirimi: bool,
    pub esik_bildirimi: bool,
    pub kredi_bildirimi: bool,
    pub bayat_bildirimi: bool,
    pub sifirlandi_bildirimi: bool,
    pub sessiz_saatler: bool,
    /// Sessiz saat başlangıcı (yerel saat, 0-23).
    pub sessiz_baslangic: u8,
    /// Sessiz saat bitişi (yerel saat, 0-23; başlangıca eşitse sessiz saat yok).
    pub sessiz_bitis: u8,
    pub claude5s_esik: u8,
    pub claude_hafta_esik: u8,
    pub diger_esik: u8,
    pub servis_port: u16,
    pub otomatik_port: bool,
    pub codex_etkin: bool,
    pub codex_etkin_dk: u64,
    pub codex_bosta_dk: u64,
    pub deepseek_etkin: bool,
    /// OpenCode kullanım sayacı açık mı (Bağla'dan).
    pub opencode_etkin: bool,
    /// DeepSeek Harness kullanım sayacı açık mı (Bağla'dan).
    pub dsh_etkin: bool,
    pub platform_esikleri: BTreeMap<String, u8>,
    pub sessiz_platformlar: Vec<String>,
    pub uyari: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AyarGuncelleme {
    pub kart_sirasi: Option<Vec<String>>,
    pub gizli_kartlar: Option<Vec<String>>,
    pub sabit_kartlar: Option<Vec<String>>,
    pub pencere_bosta_sn: Option<u64>,
    /// Desteklenen 11 dil kodundan biri.
    pub dil: Option<String>,
    /// Renk teması (`terakota`, `okyanus`, `adacayi`, `karbon`).
    pub renk_temasi: Option<String>,
    /// Görünüm kipi (`acik`, `koyu`, `sistem`); arayüzün `gorunumKipi` adı da kabul edilir.
    #[serde(alias = "gorunumKipi")]
    pub kip: Option<String>,
    /// Gösterge ve tepsi yüzde yönü (`kalan` veya `kullanilan`).
    pub deger_yonu: Option<String>,
    /// Eski tek alanlı tema (`murekkep`/`kagit`/`terminal`). Eski arayüz bunu kipin yansıması
    /// olarak gönderdiği için yalnız `kip` güncellenir, seçili `renk_temasi` ezilmez. Aynı
    /// çağrıda `kip` de varsa o kazanır.
    pub tema: Option<String>,
    pub windows_ile_baslat: Option<bool>,
    pub tepsiye_kucult: Option<bool>,
    pub agy_okuma: Option<String>,
    pub bildirim_etkin: Option<bool>,
    pub tahmin_bildirimi: Option<bool>,
    pub esik_bildirimi: Option<bool>,
    pub kredi_bildirimi: Option<bool>,
    pub bayat_bildirimi: Option<bool>,
    pub sifirlandi_bildirimi: Option<bool>,
    pub sessiz_saatler: Option<bool>,
    pub sessiz_baslangic: Option<u8>,
    pub sessiz_bitis: Option<u8>,
    pub claude5s_esik: Option<u8>,
    pub claude_hafta_esik: Option<u8>,
    pub diger_esik: Option<u8>,
    pub servis_port: Option<u16>,
    pub otomatik_port: Option<bool>,
    pub codex_etkin: Option<bool>,
    pub codex_etkin_dk: Option<u64>,
    pub codex_bosta_dk: Option<u64>,
    pub deepseek_etkin: Option<bool>,
    pub opencode_etkin: Option<bool>,
    pub dsh_etkin: Option<bool>,
    pub platform_esikleri: Option<BTreeMap<String, u8>>,
    pub sessiz_platformlar: Option<Vec<String>>,
}

fn bellek_metin_sifirla(metin: &mut String) {
    // SAFETY: `metin` içindeki tamponu sıfırlayıp ardından uzunluğu boşaltıyoruz.
    unsafe {
        let vec = metin.as_mut_vec();
        for b in vec.iter_mut() {
            std::ptr::write_volatile(b, 0);
        }
        vec.clear();
    }
    compiler_fence(Ordering::SeqCst);
}

/// Ayarlardan arayüz görünümünü üretir (saf).
pub fn arayuz_ayarlari_uret(a: &Ayarlar, uyari: Option<String>) -> ArayuzAyarlari {
    ArayuzAyarlari {
        kart_sirasi: a.masaustu.kart_sirasi.clone(),
        gizli_kartlar: a.masaustu.gizli_kartlar.clone(),
        sabit_kartlar: a.masaustu.sabit_kartlar.clone(),
        pencere_bosta_sn: a.masaustu.pencere_bosta_sn,
        dil: a.masaustu.dil.clone(),
        renk_temasi: a.masaustu.renk_temasi.clone(),
        kip: a.masaustu.kip.clone(),
        deger_yonu: a.masaustu.deger_yonu.clone(),
        tema: eski_tema_gorunumu(&a.masaustu.kip).to_string(),
        windows_ile_baslat: a.masaustu.windows_ile_baslat,
        tepsiye_kucult: a.masaustu.tepsiye_kucult,
        agy_okuma: agy_izni_metne(a.agy.okuma).to_string(),
        bildirim_etkin: a.bildirim.etkin,
        tahmin_bildirimi: a.bildirim.tahmin_bildirimi,
        esik_bildirimi: a.bildirim.esik_bildirimi,
        kredi_bildirimi: a.bildirim.kredi_bildirimi,
        bayat_bildirimi: a.bildirim.bayat_bildirimi,
        sifirlandi_bildirimi: a.bildirim.sifirlandi_bildirimi,
        sessiz_saatler: a.bildirim.sessiz_baslangic_saat != a.bildirim.sessiz_bitis_saat,
        sessiz_baslangic: a.bildirim.sessiz_baslangic_saat,
        sessiz_bitis: a.bildirim.sessiz_bitis_saat,
        claude5s_esik: a.bildirim.claude_5s_esik_yuzde,
        claude_hafta_esik: a.bildirim.claude_hafta_esik_yuzde,
        diger_esik: a.bildirim.diger_esik_yuzde,
        servis_port: a.http.port,
        otomatik_port: a.http.otomatik_port,
        codex_etkin: a.codex.etkin,
        codex_etkin_dk: a.codex.etkin_aralik_sn / 60,
        codex_bosta_dk: a.codex.bosta_aralik_sn / 60,
        deepseek_etkin: a.deepseek.etkin,
        opencode_etkin: a.opencode.etkin,
        dsh_etkin: a.dsh.etkin,
        platform_esikleri: a.bildirim.platform_esikleri.clone(),
        sessiz_platformlar: a.bildirim.sessiz_platformlar.clone(),
        uyari,
    }
}

/// Arayüz güncellemesini ayarlara uygular (saf; geçersiz değerde hata, hiçbir alan yazılmaz).
pub fn guncelleme_uygula(a: &mut Ayarlar, g: &AyarGuncelleme) -> Result<(), String> {
    let mut yeni = a.clone();
    for (liste, hedef) in [
        (&g.kart_sirasi, &mut yeni.masaustu.kart_sirasi),
        (&g.gizli_kartlar, &mut yeni.masaustu.gizli_kartlar),
        (&g.sabit_kartlar, &mut yeni.masaustu.sabit_kartlar),
    ] {
        if let Some(liste) = liste {
            if !cetvel_depo::ayarlar::kart_listesi_gecerli_mi(liste) {
                return Err(
                    "kart listesi en fazla 64 öğe, adlar [a-z0-9_]{1,40} olmalı".to_string()
                );
            }
            *hedef = liste.clone();
        }
    }
    if let Some(sn) = g.pencere_bosta_sn {
        if !(1..=3600).contains(&sn) {
            return Err("pencere bekleme süresi 1-3600 sn olmalı".to_string());
        }
        yeni.masaustu.pencere_bosta_sn = sn;
    }
    if let Some(d) = g.dil.as_deref() {
        if !dil_gecerli_mi(d) {
            return Err("desteklenmeyen dil".to_string());
        }
        yeni.masaustu.dil = d.to_string();
    }
    // Eski `tema` önce uygulanır; aynı çağrıdaki yeni alanlar onu geçersiz kılar.
    if let Some(t) = g.tema.as_deref() {
        let Some((_, kip)) = eski_temayi_esle(t) else {
            return Err("desteklenmeyen tema".to_string());
        };
        yeni.masaustu.kip = kip.to_string();
    }
    if let Some(r) = g.renk_temasi.as_deref() {
        if !renk_temasi_gecerli_mi(r) {
            return Err("desteklenmeyen renk teması".to_string());
        }
        yeni.masaustu.renk_temasi = r.to_string();
    }
    if let Some(k) = g.kip.as_deref() {
        if !kip_gecerli_mi(k) {
            return Err("desteklenmeyen kip".to_string());
        }
        yeni.masaustu.kip = k.to_string();
    }
    if let Some(y) = g.deger_yonu.as_deref() {
        if !deger_yonu_gecerli_mi(y) {
            return Err("desteklenmeyen değer yönü".to_string());
        }
        yeni.masaustu.deger_yonu = y.to_string();
    }
    if let Some(v) = g.windows_ile_baslat {
        yeni.masaustu.windows_ile_baslat = v;
    }
    if let Some(v) = g.tepsiye_kucult {
        yeni.masaustu.tepsiye_kucult = v;
    }
    if let Some(kip) = g.agy_okuma.as_deref() {
        yeni.agy.okuma = agy_izni_coz(kip)?;
    }
    if let Some(v) = g.bildirim_etkin {
        yeni.bildirim.etkin = v;
    }
    if let Some(v) = g.tahmin_bildirimi {
        yeni.bildirim.tahmin_bildirimi = v;
    }
    if let Some(v) = g.esik_bildirimi {
        yeni.bildirim.esik_bildirimi = v;
    }
    if let Some(v) = g.kredi_bildirimi {
        yeni.bildirim.kredi_bildirimi = v;
    }
    if let Some(v) = g.bayat_bildirimi {
        yeni.bildirim.bayat_bildirimi = v;
    }
    if let Some(v) = g.sifirlandi_bildirimi {
        yeni.bildirim.sifirlandi_bildirimi = v;
    }
    if let Some(sessiz) = g.sessiz_saatler {
        if sessiz {
            yeni.bildirim.sessiz_baslangic_saat = 0;
            yeni.bildirim.sessiz_bitis_saat = 8;
        } else {
            yeni.bildirim.sessiz_baslangic_saat = 0;
            yeni.bildirim.sessiz_bitis_saat = 0;
        }
    }
    // Özel aralık aynı çağrıdaki aç/kapa düğmesinden sonra uygulanır (onu geçersiz kılar).
    for (deger, hedef) in [
        (g.sessiz_baslangic, &mut yeni.bildirim.sessiz_baslangic_saat),
        (g.sessiz_bitis, &mut yeni.bildirim.sessiz_bitis_saat),
    ] {
        if let Some(v) = deger {
            if v > 23 {
                return Err("sessiz saat 0-23 arasında olmalı".to_string());
            }
            *hedef = v;
        }
    }
    if let Some(v) = g.codex_etkin {
        yeni.codex.etkin = v;
    }
    if let Some(v) = g.deepseek_etkin {
        yeni.deepseek.etkin = v;
    }
    if let Some(v) = g.opencode_etkin {
        yeni.opencode.etkin = v;
    }
    if let Some(v) = g.dsh_etkin {
        yeni.dsh.etkin = v;
    }
    if let Some(esikler) = &g.platform_esikleri {
        if esikler.len() > 64 {
            return Err("platform eşikleri en fazla 64 öğe olmalı".to_string());
        }
        for (ad, yuzde) in esikler {
            if !cetvel_depo::ayarlar::kart_adi_gecerli_mi(ad) {
                return Err("platform adı [a-z0-9_]{1,40} olmalı".to_string());
            }
            if !(1..=99).contains(yuzde) {
                return Err("eşik %1-%99 arasında olmalı".to_string());
            }
        }
        yeni.bildirim.platform_esikleri = esikler.clone();
    }
    if let Some(sessiz) = &g.sessiz_platformlar {
        if !cetvel_depo::ayarlar::kart_listesi_gecerli_mi(sessiz) {
            return Err(
                "sessiz platform listesi en fazla 64 öğe, adlar [a-z0-9_]{1,40} olmalı".to_string(),
            );
        }
        yeni.bildirim.sessiz_platformlar = sessiz.clone();
    }
    for (deger, hedef) in [
        (g.claude5s_esik, &mut yeni.bildirim.claude_5s_esik_yuzde),
        (
            g.claude_hafta_esik,
            &mut yeni.bildirim.claude_hafta_esik_yuzde,
        ),
        (g.diger_esik, &mut yeni.bildirim.diger_esik_yuzde),
    ] {
        if let Some(v) = deger {
            if !(1..=99).contains(&v) {
                return Err("eşik %1-%99 arasında olmalı".to_string());
            }
            *hedef = v;
        }
    }
    if let Some(p) = g.servis_port {
        if p != 0 && p < 1024 {
            return Err("servis portu 0 (otomatik) ya da 1024-65535 olmalı".to_string());
        }
        yeni.http.port = p;
    }
    if let Some(v) = g.otomatik_port {
        yeni.http.otomatik_port = v;
    }
    if let Some(dk) = g.codex_etkin_dk {
        if !(1..=1440).contains(&dk) {
            return Err("yenileme aralığı 1-1440 dk olmalı".to_string());
        }
        yeni.codex.etkin_aralik_sn = dk * 60;
    }
    if let Some(dk) = g.codex_bosta_dk {
        if !(1..=1440).contains(&dk) {
            return Err("yenileme aralığı 1-1440 dk olmalı".to_string());
        }
        yeni.codex.bosta_aralik_sn = dk * 60;
    }
    *a = yeni;
    Ok(())
}

fn yollar() -> Result<CetvelYollar, String> {
    CetvelYollar::ortamdan().map_err(|e| e.to_string())
}

/// Ayarları kurtarmalı yükler: bozuk dosya masaüstünü kilitlemez.
pub fn ayarlari_yukle(yollar: &CetvelYollar) -> (Ayarlar, Option<String>) {
    let y = Ayarlar::yukle_kurtarmali(&yollar.ayarlar_dosyasi(), &yerel_zaman_etiketi());
    let uyari = y.kurtarma.aciklama();
    (y.ayarlar, uyari)
}

/// Değiştirip geri yazmak için ayarları yükler. Dosya şu an okunamadıysa (kilit, paylaşım
/// ihlali, izin) `Err` döner: varsayılanlar üzerine yapılan değişiklik kullanıcının gerçek
/// dosyasını (eşikler, agy izni, sayaçlar) yedeksiz ezmesin.
pub fn ayarlari_yazmak_icin_yukle(yol: &Path) -> Result<Ayarlar, String> {
    let y = Ayarlar::yukle_kurtarmali(yol, &yerel_zaman_etiketi());
    if !y.kurtarma.yazmaya_uygun() {
        return Err(y
            .kurtarma
            .aciklama()
            .unwrap_or_else(|| "ayarlar.toml okunamadı".to_string()));
    }
    Ok(y.ayarlar)
}

/// Kaydedilen ayarı servise bildirir ve tüm pencerelere `ayarlar_degisti` yayar.
async fn ayar_degisikligini_yay(
    app: &AppHandle,
    durum: &MasaustuDurum,
    ayarlar: &Ayarlar,
) -> ArayuzAyarlari {
    let _ = durum.istemci.ayar_yenile().await;
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.tepsiye_kucult = ayarlar.masaustu.tepsiye_kucult;
        g.pencere_bosta_sn = ayarlar.masaustu.pencere_bosta_sn;
    }
    // Dil veya değer yönü değiştiyse tepsi menüsü, ikonu ve ipucu yeniden yazılır.
    tepsi_tercihlerini_uygula(
        app,
        durum,
        &ayarlar.masaustu.dil,
        &ayarlar.masaustu.deger_yonu,
    );
    let gorunum = arayuz_ayarlari_uret(ayarlar, None);
    let _ = app.emit("ayarlar_degisti", gorunum.clone());
    gorunum
}

/// Çalışan servisten güncel `AnlikV2` verisini döndürür; servis kapalıysa başlatma önerisi döner.
#[tauri::command]
pub async fn anlik_al(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
) -> Result<AnlikServisYaniti, String> {
    let onbellek = {
        let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        if g.servis_calisiyor
            && let Some(a) = &g.son_anlik
        {
            Some(a.clone())
        } else {
            None
        }
    };
    if let Some(a) = onbellek {
        return Ok(AnlikServisYaniti {
            servis_calisiyor: true,
            baslatma_onerisi: false,
            anlik: Some(a),
            hata: None,
        });
    }

    match durum.istemci.anlik_getir(None, 2).await {
        Ok(Some(anlik)) => {
            {
                let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                g.servis_calisiyor = true;
                g.son_anlik = Some(anlik.clone());
            }
            tepsi_gorunum_guncelle(&app, &durum);
            Ok(AnlikServisYaniti {
                servis_calisiyor: true,
                baslatma_onerisi: false,
                anlik: Some(anlik),
                hata: None,
            })
        }
        Ok(None) => Ok(AnlikServisYaniti {
            servis_calisiyor: true,
            baslatma_onerisi: false,
            anlik: None,
            hata: None,
        }),
        Err(hata) => {
            let calisiyor = cetvel_servis::servis_calisiyor_mu();
            {
                let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                g.servis_calisiyor = calisiyor;
            }
            tepsi_gorunum_guncelle(&app, &durum);
            Ok(AnlikServisYaniti {
                servis_calisiyor: calisiyor,
                baslatma_onerisi: !calisiyor,
                anlik: None,
                hata: Some(hata),
            })
        }
    }
}

/// Servisten yenileme ister (havuz başına 1/dk). Sonuç servis yayınlayınca `anlik_degisti` ile gelir;
/// burada sahte tazelik üretilmez.
#[tauri::command]
pub async fn yenile_iste(
    durum: State<'_, MasaustuDurum>,
    havuz: Option<String>,
) -> Result<YenileKomutYaniti, String> {
    match durum.istemci.yenile_iste(havuz.as_deref()).await {
        Ok(y) => Ok(YenileKomutYaniti {
            istendi: y.istendi,
            bekle_sn: y.bekle_sn,
            servis_calisiyor: true,
            baslatma_onerisi: false,
            hata: None,
        }),
        Err(hata) => {
            let calisiyor = cetvel_servis::servis_calisiyor_mu();
            Ok(YenileKomutYaniti {
                istendi: false,
                bekle_sn: None,
                servis_calisiyor: calisiyor,
                baslatma_onerisi: !calisiyor,
                hata: Some(hata),
            })
        }
    }
}

/// Servisten belirtilen havuz ve pencere için geçmiş örnekleri alır.
#[tauri::command]
pub async fn gecmis_al(
    durum: State<'_, MasaustuDurum>,
    havuz: String,
    pencere: String,
    gun: Option<u64>,
) -> Result<Value, String> {
    durum
        .istemci
        .gecmis_getir(&havuz, &pencere, gun.unwrap_or(1))
        .await
}

/// Arka plan servisinin çalışma durumunu kontrol eder.
#[tauri::command]
pub async fn servis_durumu(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
) -> Result<ServisDurumuBilgisi, String> {
    let bil = durum.istemci.saglik_kontrol().await;
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.servis_calisiyor = bil.calisiyor;
    }
    tepsi_gorunum_guncelle(&app, &durum);
    Ok(bil)
}

/// Çalışmıyorsa `cetvel servis` sürecini başlatır ve ilk anlığı çeker.
#[tauri::command]
pub async fn servis_baslat(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
) -> Result<AnlikServisYaniti, String> {
    let saglik = durum.istemci.servis_baslat().await?;
    let anlik = durum.istemci.anlik_getir(None, 3).await.ok().flatten();

    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.servis_calisiyor = saglik.calisiyor;
        if let Some(a) = &anlik {
            g.son_anlik = Some(a.clone());
        }
    }
    tepsi_gorunum_guncelle(&app, &durum);

    let yanit = AnlikServisYaniti {
        servis_calisiyor: saglik.calisiyor,
        baslatma_onerisi: !saglik.calisiyor,
        anlik,
        hata: saglik.hata.clone(),
    };
    let _ = app.emit("servis_durumu_degisti", saglik);
    let _ = app.emit("anlik_degisti", yanit.clone());
    Ok(yanit)
}

/// Salt-okunur platform taraması: servis yapar (`bagla.tara`), masaüstü yalnız gösterir.
#[tauri::command]
pub async fn tara(durum: State<'_, MasaustuDurum>) -> Result<TaramaSonucu, String> {
    durum.istemci.bagla_tara().await
}

/// Bağlayıcıyı servis döngüsünde sınar (masaüstü bağlayıcı çalıştırmaz, ikinci `codex app-server` açılmaz).
#[tauri::command]
pub async fn test_et(
    durum: State<'_, MasaustuDurum>,
    platform: String,
) -> Result<TestSonucu, String> {
    durum.istemci.bagla_test(&platform).await
}

/// Kalıcı arayüz ayarlarını okur.
#[tauri::command]
pub fn ayarlar_al(window: tauri::WebviewWindow) -> Result<ArayuzAyarlari, String> {
    crate::pencere::arayuz_hazir(&window);
    let y = yollar()?;
    let (ayarlar, uyari) = ayarlari_yukle(&y);
    Ok(arayuz_ayarlari_uret(&ayarlar, uyari))
}

/// Antigravity (`agy`) okuma iznini kaydeder ve servisin hemen uygulamasını ister.
#[tauri::command]
pub async fn agy_kipi_ayarla(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    kip: String,
) -> Result<ArayuzAyarlari, String> {
    ayar_kaydet(
        app,
        durum,
        AyarGuncelleme {
            agy_okuma: Some(kip),
            ..AyarGuncelleme::default()
        },
    )
    .await
}

/// `ayarlar.toml` oku-değiştir-yaz adımlarını süreç içinde sıraya sokar.
static AYAR_DOSYA_KILIDI: Mutex<()> = Mutex::new(());

/// Ayarlar sekmesindeki değişikliği `ayarlar.toml`'a yazar, servise bildirir ve pencerelere yayar.
#[tauri::command]
pub async fn ayar_kaydet(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    guncelleme: AyarGuncelleme,
) -> Result<ArayuzAyarlari, String> {
    let y = yollar()?;
    let ayarlar = {
        let _kilit = AYAR_DOSYA_KILIDI.lock().unwrap_or_else(|e| e.into_inner());
        let mut ayarlar = ayarlari_yazmak_icin_yukle(&y.ayarlar_dosyasi())?;
        guncelleme_uygula(&mut ayarlar, &guncelleme)?;

        if let Some(etkin) = guncelleme.windows_ile_baslat {
            let son = otomatik_baslat_uygula(&app, &durum, etkin)?;
            ayarlar.masaustu.windows_ile_baslat = son;
        }

        ayarlar
            .kaydet(&y.ayarlar_dosyasi())
            .map_err(|e| e.to_string())?;
        ayarlar
    };
    Ok(ayar_degisikligini_yay(&app, &durum, &ayarlar).await)
}

/// Tüm ayarları `Ayarlar::default()` durumuna döndürür, kaydeder ve pencerelere yayar.
#[tauri::command]
pub async fn ayarlari_sifirla(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
) -> Result<ArayuzAyarlari, String> {
    let y = yollar()?;
    let ayarlar = {
        let _kilit = AYAR_DOSYA_KILIDI.lock().unwrap_or_else(|e| e.into_inner());
        let _ = ayarlari_yazmak_icin_yukle(&y.ayarlar_dosyasi())?;
        let mut ayarlar = Ayarlar::default();
        if let Ok(son) = otomatik_baslat_uygula(&app, &durum, ayarlar.masaustu.windows_ile_baslat) {
            ayarlar.masaustu.windows_ile_baslat = son;
        }
        ayarlar
            .kaydet(&y.ayarlar_dosyasi())
            .map_err(|e| e.to_string())?;
        ayarlar
    };
    Ok(ayar_degisikligini_yay(&app, &durum, &ayarlar).await)
}

/// Anahtar kaydı ön denetimi (saf; ayar okumadan, Credential Manager'a dokunmadan): platform
/// yalnız `deepseek`; anahtar boş, kısa, aşırı uzun ya da boşluk/kontrol karakterli
/// olamaz. Hata metni anahtar değerini içermez. Dönüş: normalize platform adı.
pub fn anahtar_kaydi_on_kontrol(platform: &str, anahtar: &str) -> Result<String, String> {
    let p_norm = platform.trim().to_ascii_lowercase();
    if p_norm != "deepseek" {
        return Err("yalnız deepseek anahtarı kaydedilebilir".to_string());
    }
    cetvel_depo::api_anahtari_dogrula(anahtar).map_err(|e| match e {
        cetvel_depo::DepoHatasi::Guvenlik(m) => m,
        diger => diger.to_string(),
    })?;
    Ok(p_norm)
}

/// DeepSeek API anahtarını Windows Credential Manager'a kaydeder (değer geri okunmaz,
/// günlüğe yazılmaz); servis yeniden yüklemeye çağrılır.
#[tauri::command]
pub async fn anahtar_kaydet(
    durum: State<'_, MasaustuDurum>,
    platform: String,
    mut anahtar: String,
) -> Result<AnahtarKayitSonucu, String> {
    let on_kontrol = anahtar_kaydi_on_kontrol(&platform, &anahtar);
    let p_norm = match on_kontrol {
        Ok(p) => p,
        Err(e) => {
            bellek_metin_sifirla(&mut anahtar);
            return Err(e);
        }
    };
    let hedef_env = yollar().ok().map(|y| {
        let (ayarlar, _) = ayarlari_yukle(&y);
        deepseek_env_adi(&ayarlar)
    });
    let Some(hedef_env) = hedef_env else {
        bellek_metin_sifirla(&mut anahtar);
        return Err("kullanıcı klasörleri çözülemedi; anahtar kaydedilmedi".to_string());
    };

    let kayit_sonucu = kimlik_kaydet(&hedef_env, &anahtar);
    bellek_metin_sifirla(&mut anahtar);
    kayit_sonucu.map_err(|e| e.to_string())?;
    tracing::info!(platform = %p_norm, "API anahtarı Windows Credential Manager'a kaydedildi");

    let servis_bildirildi = durum.istemci.ayar_yenile().await.is_ok();
    Ok(AnahtarKayitSonucu {
        platform: p_norm,
        anahtar_var: true,
        servis_bildirildi,
    })
}

fn settings_oku(yol: &Path) -> Result<Option<(String, Value)>, String> {
    match std::fs::read_to_string(yol) {
        Ok(ham) => {
            let v = serde_json::from_str::<Value>(&ham)
                .map_err(|e| format!("Claude settings.json ayrıştırılamadı: {e}"))?;
            Ok(Some((ham, v)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// statusLine'a yazılacak cetvel.exe çağrı metni (Git Bash / PowerShell uyumlu; bkz.
/// `statusline_exe_metni`).
fn cetvel_exe_metni() -> Result<String, String> {
    let yol = cetvel_ikilisi_bul();
    if !yol.is_absolute() || !yol.is_file() {
        return Err("cetvel.exe masaüstü uygulamasının yanında bulunamadı".to_string());
    }
    Ok(statusline_cagri_metni_hazirla(&yol.display().to_string()))
}

/// statusLine önizlemesi (saf dosya mantığı; testlerde geçici dizinle çağrılır).
pub fn statusline_onizle_dosyadan(
    settings_yolu: &Path,
    cetvel_exe: &str,
) -> ClaudeStatuslineOnizleme {
    let mevcut = match settings_oku(settings_yolu) {
        Ok(m) => m,
        Err(hata) => {
            return ClaudeStatuslineOnizleme {
                zaten_tanimli: false,
                zaten_cetvel: false,
                onceki_komut_var: false,
                uygulanabilir: false,
                fark_metni: String::new(),
                hata: Some(hata),
            };
        }
    };
    let zaten_tanimli = mevcut
        .as_ref()
        .is_some_and(|(_, v)| v.get("statusLine").is_some());
    match statusline_plani(mevcut.as_ref().map(|(_, v)| v), cetvel_exe) {
        Ok(StatuslinePlani {
            onceki_komut,
            zaten_cetvel,
            fark_metni,
            ..
        }) => ClaudeStatuslineOnizleme {
            zaten_tanimli,
            zaten_cetvel,
            onceki_komut_var: onceki_komut.is_some(),
            uygulanabilir: !zaten_cetvel,
            fark_metni,
            hata: None,
        },
        Err(hata) => ClaudeStatuslineOnizleme {
            zaten_tanimli,
            zaten_cetvel: false,
            onceki_komut_var: false,
            uygulanabilir: false,
            fark_metni: String::new(),
            hata: Some(hata),
        },
    }
}

/// statusLine'ı uygular: önizlenen fark hâlâ geçerliyse tarihli (üzerine yazılmayan) yedek alır,
/// sonra yazar. Önizlemeden sonra dosya değiştiyse yazmaz.
pub fn statusline_uygula_dosyaya(
    settings_yolu: &Path,
    cetvel_exe: &str,
    beklenen_fark: &str,
    zaman_etiketi: &str,
) -> Result<ClaudeStatuslineSonucu, String> {
    let mevcut = settings_oku(settings_yolu)?;
    let plan = statusline_plani(mevcut.as_ref().map(|(_, v)| v), cetvel_exe)?;
    if plan.zaten_cetvel {
        return Ok(ClaudeStatuslineSonucu {
            yazildi: false,
            yedek: None,
        });
    }
    if plan.fark_metni != beklenen_fark {
        return Err("settings.json önizlemeden sonra değişti; yeniden önizleyin".to_string());
    }

    let yedek_adi = settings_yedekle_ve_yaz(
        settings_yolu,
        mevcut.as_ref().map(|(ham, _)| ham.as_str()),
        &plan.yeni_settings,
        zaman_etiketi,
    )?;
    Ok(ClaudeStatuslineSonucu {
        yazildi: true,
        yedek: yedek_adi,
    })
}

/// Claude `settings.json` için `statusLine` zincirleme önizlemesi üretir (dosyaya yazmaz).
#[tauri::command]
pub fn claude_statusline_onizle() -> Result<ClaudeStatuslineOnizleme, String> {
    let yol = claude_settings_yolu().ok_or_else(|| "kullanıcı ev dizini bulunamadı".to_string())?;
    let exe = cetvel_exe_metni()?;
    Ok(statusline_onizle_dosyadan(&yol, &exe))
}

/// Açık onayla (`onay: true`) ve önizlenen farkla, tarihli yedek alıp statusLine'ı zincirler.
#[tauri::command]
pub fn claude_statusline_uygula(
    onay: bool,
    beklenen_fark: String,
) -> Result<ClaudeStatuslineSonucu, String> {
    if !onay {
        return Err("statusLine yazımı için açık onay gerekli".to_string());
    }
    let yol = claude_settings_yolu().ok_or_else(|| "kullanıcı ev dizini bulunamadı".to_string())?;
    let exe = cetvel_exe_metni()?;
    statusline_uygula_dosyaya(&yol, &exe, &beklenen_fark, &yerel_zaman_etiketi())
}

/// Masaüstü yüzeyleri (`acilir`, `pencere`, `serit`) arasında geçiş yapar.
#[tauri::command]
pub async fn pencere_ac(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    yuzey: String,
    sekme: Option<String>,
) -> Result<(), String> {
    match yuzey.trim() {
        "acilir" => crate::pencere::acilir_goster(&app, &durum, None),
        "pencere" => pencere_ac_ve_odakla(&app, &durum, sekme.as_deref()),
        "serit" => serit_gorunurluk_ayarla(&app, &durum, true),
        _ => return Err("bilinmeyen pencere yüzeyi".to_string()),
    }
    Ok(())
}

/// Açılır pencereyi gizler (Esc).
#[tauri::command]
pub fn acilir_gizle(app: AppHandle, durum: State<'_, MasaustuDurum>) {
    crate::pencere::acilir_gizle(&app, &durum);
}

/// Ana pencereyi gizler (Esc); `acilir_goster` verilirse açılır pencereye döner ("Küçült").
#[tauri::command]
pub async fn pencere_gizle(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    acilir_goster: Option<bool>,
) -> Result<(), String> {
    crate::pencere::ana_pencereyi_gizle(&app);
    if acilir_goster.unwrap_or(false) {
        crate::pencere::acilir_goster(&app, &durum, None);
    }
    Ok(())
}

/// Şerit penceresini gizler.
#[tauri::command]
pub fn serit_gizle(app: AppHandle, durum: State<'_, MasaustuDurum>) {
    serit_gorunurluk_ayarla(&app, &durum, false);
}

#[tauri::command]
pub fn serit_durumu_al(durum: State<'_, MasaustuDurum>) -> SeritDurumu {
    let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
    SeritDurumu {
        gorunur: g.serit_gorunur,
        ustte: g.serit_ustte,
        gecirgen: g.serit_gecirgen,
    }
}

#[tauri::command]
pub fn serit_tiklama_gecirgen_ayarla(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    gecirgen: bool,
) -> SeritDurumu {
    serit_secenek_uygula(&app, &durum, None, Some(gecirgen))
}

#[tauri::command]
pub fn serit_ustte_ayarla(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    ustte: bool,
) -> SeritDurumu {
    serit_secenek_uygula(&app, &durum, Some(ustte), None)
}

#[tauri::command]
pub fn otomatik_baslat_durum(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// HKCU `Run` otomatik başlatmayı uygular ve tepsi menüsünü eşitler.
pub fn otomatik_baslat_uygula(
    app: &AppHandle,
    durum: &MasaustuDurum,
    etkin: bool,
) -> Result<bool, String> {
    let al = app.autolaunch();
    if etkin {
        al.enable().map_err(|e| e.to_string())?;
    } else {
        al.disable().map_err(|e| e.to_string())?;
    }
    let son = al.is_enabled().map_err(|e| e.to_string())?;
    // Menü öğesi kilit bırakıldıktan sonra güncellenir: `set_checked` ana iş parçacığına gönderip
    // bekler; `if let` gövdesinde MutexGuard yaşarken (edition 2024'te de) ana iş parçacığı aynı
    // kilidi isterse karşılıklı kilitlenme olurdu.
    let menu = {
        let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.tepsi_menu.clone()
    };
    if let Some(menu) = menu {
        let _ = menu.otomatik_baslat.set_checked(son);
    }
    Ok(son)
}

#[tauri::command]
pub async fn otomatik_baslat_ayarla(
    app: AppHandle,
    durum: State<'_, MasaustuDurum>,
    etkin: bool,
) -> Result<bool, String> {
    let gorunum = ayar_kaydet(
        app,
        durum,
        AyarGuncelleme {
            windows_ile_baslat: Some(etkin),
            ..AyarGuncelleme::default()
        },
    )
    .await?;
    Ok(gorunum.windows_ile_baslat)
}

/// Hakkında ekranının gösterdiği lisans belgeleri (ikiliye gömülü; kurulum klasöründe de durur).
const LISANS_MIT: &str = include_str!("../../../LICENSE-MIT");
const LISANS_APACHE: &str = include_str!("../../../LICENSE-APACHE");
const UCUNCU_TARAF: &str = include_str!("../../../THIRD-PARTY-NOTICES.txt");

/// Lisans belgesini döndürür: `mit`, `apache`, `ucuncu_taraf`.
pub fn lisans_metni(tur: &str) -> Result<&'static str, String> {
    match tur.trim() {
        "mit" => Ok(LISANS_MIT),
        "apache" => Ok(LISANS_APACHE),
        "ucuncu_taraf" => Ok(UCUNCU_TARAF),
        _ => Err("bilinmeyen lisans belgesi".to_string()),
    }
}

#[tauri::command]
pub fn lisans_metni_al(tur: String) -> Result<String, String> {
    lisans_metni(&tur).map(str::to_string)
}

/// Klasörü (yoksa oluşturup) Dosya Gezgini'nde açar; `sec` verilirse o dosya seçili gelir.
fn gezginde_ac(klasor: &Path, sec: Option<&Path>) -> Result<(), String> {
    std::fs::create_dir_all(klasor).map_err(|e| e.to_string())?;
    let mut komut = std::process::Command::new("explorer.exe");
    match sec {
        Some(dosya) => {
            komut.arg(format!("/select,{}", dosya.display()));
        }
        None => {
            komut.arg(klasor);
        }
    }
    // explorer.exe başarıda bile 1 döndürebilir; yalnız başlatılamaması hatadır.
    komut.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Servis günlüklerinin klasörünü açar (`%LOCALAPPDATA%\Cetvel\gunluk`).
#[tauri::command]
pub fn gunluk_klasorunu_ac() -> Result<String, String> {
    let y = yollar()?;
    let klasor = y.gunluk_dizini();
    gezginde_ac(&klasor, None)?;
    Ok(klasor.display().to_string())
}

/// CSV alanını kaçışlar (virgül, tırnak, satır sonu).
fn csv_alani(deger: &str) -> String {
    if deger.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", deger.replace('"', "\"\""))
    } else {
        deger.to_string()
    }
}

/// `GET /v1/gecmis` yanıtlarından CSV üretir (saf). Satır: havuz, pencere, zaman, kullanılan,
/// kalan, sıfırlama, kaynak. Oranlar 0-1 aralığında, zamanlar ISO 8601 UTC.
pub fn gecmis_csv_uret(parcalar: &[(String, String, Value)]) -> String {
    let mut csv = String::from("havuz,pencere,zaman,kullanilan,kalan,sifirlama,kaynak\n");
    for (havuz, pencere, yanit) in parcalar {
        let Some(ornekler) = yanit.get("ornekler").and_then(Value::as_array) else {
            continue;
        };
        for o in ornekler {
            let metin = |a: &str| o.get(a).and_then(Value::as_str).unwrap_or("").to_string();
            let sayi = |a: &str| {
                o.get(a)
                    .and_then(Value::as_f64)
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_default()
            };
            let satir = [
                csv_alani(havuz),
                csv_alani(pencere),
                csv_alani(&metin("zaman")),
                sayi("kullanilan"),
                sayi("kalan"),
                csv_alani(&metin("sifirlama")),
                csv_alani(&metin("kaynak")),
            ];
            csv.push_str(&satir.join(","));
            csv.push('\n');
        }
    }
    csv
}

/// Dışa aktarım dosya adı: `Cetvel Geçmiş 2026-11-16 14.30.csv` (tarih sonda).
pub fn disa_aktarim_adi(zaman_etiketi: &str) -> String {
    let temiz: String = zaman_etiketi
        .chars()
        .map(|c| {
            if matches!(c, ':' | '/' | '\\') {
                '.'
            } else {
                c
            }
        })
        .collect();
    let kisa: String = temiz.trim().chars().take(16).collect();
    format!("Cetvel Geçmiş {kisa}.csv")
}

/// Son 90 günün okuma geçmişini (tüm bağlı havuz ve pencereler) CSV olarak dışa aktarır ve
/// dosyayı Dosya Gezgini'nde seçili gösterir. Dönüş: dosya adı.
#[tauri::command]
pub async fn gecmis_disa_aktar(durum: State<'_, MasaustuDurum>) -> Result<String, String> {
    let anlik = match durum.istemci.anlik_getir(None, 3).await {
        Ok(Some(a)) => a,
        Ok(None) => return Err("servisten anlık alınamadı".to_string()),
        Err(e) => return Err(e),
    };
    let mut parcalar = Vec::new();
    for (havuz_id, havuz) in &anlik.havuzlar {
        for pencere in havuz.pencereler.keys() {
            if let Ok(yanit) = durum.istemci.gecmis_getir(havuz_id, pencere, 90).await {
                parcalar.push((havuz_id.clone(), pencere.clone(), yanit));
            }
        }
    }
    let csv = gecmis_csv_uret(&parcalar);
    let y = yollar()?;
    let klasor = y.local_kok.join("Dışa Aktarım");
    std::fs::create_dir_all(&klasor).map_err(|e| e.to_string())?;
    let ad = disa_aktarim_adi(&yerel_zaman_etiketi());
    let dosya = klasor.join(&ad);
    // Excel Türkçe karakterleri doğru açsın diye UTF-8 BOM.
    let mut baytlar = vec![0xEF, 0xBB, 0xBF];
    baytlar.extend_from_slice(csv.as_bytes());
    cetvel_depo::atomik_yaz(&dosya, &baytlar).map_err(|e| e.to_string())?;
    gezginde_ac(&klasor, Some(&dosya))?;
    Ok(ad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cetvel_depo::AgyOkumaIzni;
    use std::path::PathBuf;

    #[test]
    fn anahtar_kaydi_bos_uzun_bosluklu_ve_bilinmeyen_platformu_reddeder() {
        assert_eq!(
            anahtar_kaydi_on_kontrol(" DeepSeek ", "sk-ornek-123456").as_deref(),
            Ok("deepseek")
        );
        assert_eq!(
            anahtar_kaydi_on_kontrol("deepseek", "  ds-anahtar-123  ").as_deref(),
            Ok("deepseek")
        );
        for (platform, anahtar) in [
            ("deepseek", ""),
            ("deepseek", "    "),
            ("deepseek", "kisa"),
            ("deepseek", "sk-gizli deger-123"),
            ("deepseek", "sk-gizli\ndeger-123"),
            ("opencode", "sk-ornek-123456"),
            ("codex", "sk-ornek-123456"),
        ] {
            let hata = anahtar_kaydi_on_kontrol(platform, anahtar).unwrap_err();
            assert!(
                !hata.contains("gizli"),
                "hata anahtarı sızdırmamalı: {hata}"
            );
        }
        let uzun = "a".repeat(cetvel_depo::AZAMI_ANAHTAR_BAYT + 1);
        assert!(
            anahtar_kaydi_on_kontrol("deepseek", &uzun)
                .unwrap_err()
                .contains("uzun")
        );
    }

    fn gecici(ad: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cetvel-masaustu-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("gecici");
        d
    }

    #[test]
    fn bellek_metin_sifirlama_tamponu_temizler() {
        let mut s = String::from("sk-gizli-anahtar-123");
        bellek_metin_sifirla(&mut s);
        assert!(s.is_empty());
    }

    #[test]
    fn arayuz_guncellemesi_ayarlara_uygulanir_ve_gecersiz_deger_hicbirini_yazmaz() {
        let mut a = Ayarlar::default();
        let g = AyarGuncelleme {
            dil: Some("ja".into()),
            renk_temasi: Some("okyanus".into()),
            kip: Some("acik".into()),
            deger_yonu: Some("kullanilan".into()),
            tepsiye_kucult: Some(false),
            agy_okuma: Some("istek_uzerine".into()),
            claude5s_esik: Some(25),
            sessiz_saatler: Some(false),
            servis_port: Some(0),
            codex_etkin_dk: Some(10),
            ..AyarGuncelleme::default()
        };
        guncelleme_uygula(&mut a, &g).expect("gecerli");
        assert_eq!(a.masaustu.dil, "ja");
        assert_eq!(a.masaustu.renk_temasi, "okyanus");
        assert_eq!(a.masaustu.kip, "acik");
        assert_eq!(a.masaustu.deger_yonu, "kullanilan");
        assert!(!a.masaustu.tepsiye_kucult);
        assert_eq!(a.agy.okuma, AgyOkumaIzni::IstekUzerine);
        assert_eq!(a.bildirim.claude_5s_esik_yuzde, 25);
        assert_eq!(a.http.port, 0);
        assert_eq!(a.codex.etkin_aralik_sn, 600);
        let gorunum = arayuz_ayarlari_uret(&a, None);
        assert!(!gorunum.sessiz_saatler);
        assert_eq!(gorunum.codex_etkin_dk, 10);
        assert_eq!(gorunum.agy_okuma, "istek_uzerine");

        let onceki = a.clone();
        let hatali = AyarGuncelleme {
            dil: Some("tr".into()),
            claude_hafta_esik: Some(0),
            ..AyarGuncelleme::default()
        };
        assert!(guncelleme_uygula(&mut a, &hatali).is_err());
        assert_eq!(a, onceki, "geçersiz güncelleme kısmen uygulanmamalı");
        assert!(
            guncelleme_uygula(
                &mut a,
                &AyarGuncelleme {
                    servis_port: Some(80),
                    ..AyarGuncelleme::default()
                }
            )
            .is_err()
        );
        assert!(
            guncelleme_uygula(
                &mut a,
                &AyarGuncelleme {
                    agy_okuma: Some("her_sey".into()),
                    ..AyarGuncelleme::default()
                }
            )
            .is_err()
        );
    }

    fn tek_guncelleme(g: AyarGuncelleme) -> (Ayarlar, Result<(), String>) {
        let mut a = Ayarlar::default();
        let sonuc = guncelleme_uygula(&mut a, &g);
        (a, sonuc)
    }

    #[test]
    fn guncelleme_11_dilin_hepsini_kabul_eder_gecersizi_reddeder() {
        for dil in [
            "tr", "en", "de", "fr", "es", "pt-BR", "ru", "ja", "ko", "zh-Hans", "zh-Hant",
        ] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                dil: Some(dil.into()),
                ..AyarGuncelleme::default()
            });
            assert!(sonuc.is_ok(), "{dil}");
            assert_eq!(a.masaustu.dil, dil);
        }
        for dil in ["", "xx", "pt", "pt-br", "zh", "EN", "zh-hans"] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                dil: Some(dil.into()),
                ..AyarGuncelleme::default()
            });
            assert_eq!(sonuc, Err("desteklenmeyen dil".to_string()), "{dil}");
            assert_eq!(a, Ayarlar::default(), "reddedilen dil yazılmamalı");
        }
    }

    #[test]
    fn guncelleme_renk_temasi_ve_kipi_dogrular() {
        for renk in ["terakota", "okyanus", "adacayi", "karbon"] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                renk_temasi: Some(renk.into()),
                ..AyarGuncelleme::default()
            });
            assert!(sonuc.is_ok(), "{renk}");
            assert_eq!(a.masaustu.renk_temasi, renk);
            assert_eq!(a.masaustu.kip, "koyu", "renk kipi değiştirmez");
        }
        for kip in ["acik", "koyu", "sistem"] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                kip: Some(kip.into()),
                ..AyarGuncelleme::default()
            });
            assert!(sonuc.is_ok(), "{kip}");
            assert_eq!(a.masaustu.kip, kip);
            assert_eq!(a.masaustu.renk_temasi, "terakota", "kip rengi değiştirmez");
        }
        for renk in ["", "murekkep", "kagit", "terminal", "Karbon"] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                renk_temasi: Some(renk.into()),
                ..AyarGuncelleme::default()
            });
            assert_eq!(
                sonuc,
                Err("desteklenmeyen renk teması".to_string()),
                "{renk}"
            );
            assert_eq!(a, Ayarlar::default());
        }
        for kip in ["", "dark", "Koyu", "murekkep"] {
            let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
                kip: Some(kip.into()),
                ..AyarGuncelleme::default()
            });
            assert_eq!(sonuc, Err("desteklenmeyen kip".to_string()), "{kip}");
            assert_eq!(a, Ayarlar::default());
        }
    }

    #[test]
    fn eski_tema_degeri_yalniz_kipi_gunceller_rengi_ezmez() {
        for (eski, kip) in [
            ("murekkep", "koyu"),
            ("kagit", "acik"),
            ("terminal", "koyu"),
        ] {
            let mut a = Ayarlar::default();
            a.masaustu.renk_temasi = "adacayi".to_string();
            guncelleme_uygula(
                &mut a,
                &AyarGuncelleme {
                    tema: Some(eski.into()),
                    ..AyarGuncelleme::default()
                },
            )
            .expect("eski tema gecerli");
            assert_eq!(a.masaustu.kip, kip, "{eski}");
            assert_eq!(a.masaustu.renk_temasi, "adacayi", "{eski}: renk korunur");
        }
        // Yeni `kip` aynı çağrıdaki eski `tema`yı geçersiz kılar.
        let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
            tema: Some("murekkep".into()),
            kip: Some("sistem".into()),
            renk_temasi: Some("karbon".into()),
            ..AyarGuncelleme::default()
        });
        assert!(sonuc.is_ok());
        assert_eq!(a.masaustu.kip, "sistem");
        assert_eq!(a.masaustu.renk_temasi, "karbon");
        // Bilinmeyen eski değer reddedilir ve hiçbir şey yazılmaz.
        let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
            tema: Some("gokkusagi".into()),
            dil: Some("de".into()),
            ..AyarGuncelleme::default()
        });
        assert_eq!(sonuc, Err("desteklenmeyen tema".to_string()));
        assert_eq!(a, Ayarlar::default());
    }

    #[test]
    fn guncelleme_json_camel_case_ve_gorunum_kipi_takma_adini_okur() {
        let g: AyarGuncelleme =
            serde_json::from_str(r#"{"dil":"zh-Hant","renkTemasi":"karbon","gorunumKipi":"koyu"}"#)
                .expect("json");
        assert_eq!(g.dil.as_deref(), Some("zh-Hant"));
        assert_eq!(g.renk_temasi.as_deref(), Some("karbon"));
        assert_eq!(g.kip.as_deref(), Some("koyu"));
        let g: AyarGuncelleme =
            serde_json::from_str(r#"{"kip":"acik","tema":"kagit"}"#).expect("json");
        assert_eq!(g.kip.as_deref(), Some("acik"));
        assert_eq!(g.tema.as_deref(), Some("kagit"));
    }

    #[test]
    fn arayuz_gorunumu_yeni_alanlari_ve_eski_tema_gorunumunu_uretir() {
        let mut a = Ayarlar::default();
        let g = arayuz_ayarlari_uret(&a, None);
        assert_eq!(
            (g.dil.as_str(), g.renk_temasi.as_str(), g.kip.as_str()),
            ("tr", "terakota", "koyu")
        );
        a.masaustu.dil = "pt-BR".to_string();
        a.masaustu.renk_temasi = "adacayi".to_string();
        a.masaustu.kip = "koyu".to_string();
        let g = arayuz_ayarlari_uret(&a, None);
        assert_eq!(g.dil, "pt-BR");
        assert_eq!(g.renk_temasi, "adacayi");
        assert_eq!(g.kip, "koyu");
        assert_eq!(g.tema, "murekkep");
        a.masaustu.kip = "acik".to_string();
        assert_eq!(arayuz_ayarlari_uret(&a, None).tema, "kagit");
        let json = serde_json::to_value(arayuz_ayarlari_uret(&a, None)).expect("json");
        assert_eq!(json["renkTemasi"], "adacayi");
        assert_eq!(json["kip"], "acik");
    }

    #[test]
    fn statusline_onizle_onay_ve_tarihli_yedekle_zincirlenir() {
        let d = gecici("statusline");
        let yol = d.join("settings.json");
        let exe = r"C:\Araclar\Cetvel\cetvel.exe";
        std::fs::write(
            &yol,
            r#"{ "model": "opus", "statusLine": { "type": "command", "command": "bash sl.sh" } }"#,
        )
        .expect("yaz");

        let onizleme = statusline_onizle_dosyadan(&yol, exe);
        assert!(onizleme.uygulanabilir);
        assert!(onizleme.onceki_komut_var);
        assert!(onizleme.hata.is_none());

        // Önizlemeden farklı beklenen fark: yazılmaz
        assert!(statusline_uygula_dosyaya(&yol, exe, "baska", "2026-11-16 10.00.00").is_err());

        let s1 = statusline_uygula_dosyaya(&yol, exe, &onizleme.fark_metni, "2026-11-16 10.00.00")
            .expect("uygula");
        assert!(s1.yazildi);
        assert_eq!(
            s1.yedek.as_deref(),
            Some("settings.cetvel-yedek 2026-11-16 10.00.00.json")
        );
        let yeni: Value =
            serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).expect("json");
        assert_eq!(yeni["model"], "opus");
        assert!(
            yeni["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("--onceki-hex")
        );
        let yedek =
            std::fs::read_to_string(d.join("settings.cetvel-yedek 2026-11-16 10.00.00.json"))
                .expect("yedek");
        assert!(yedek.contains("bash sl.sh"));

        // İkinci uygulama: zaten Cetvel, değişiklik yok, yedek üzerine yazılmaz
        let tekrar = statusline_onizle_dosyadan(&yol, exe);
        assert!(tekrar.zaten_cetvel);
        let s2 = statusline_uygula_dosyaya(&yol, exe, &tekrar.fark_metni, "2026-11-16 10.00.00")
            .expect("tekrar");
        assert!(!s2.yazildi);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn okunamayan_ayar_dosyasi_yazmak_icin_yuklenmez() {
        // Dizin yolu okunamaz (NotFound değil) → geçici okuma hatası gibi.
        let d = gecici("ayar-okunamaz");
        assert!(ayarlari_yazmak_icin_yukle(&d).is_err());
        let yol = d.join("ayarlar.toml");
        assert!(
            ayarlari_yazmak_icin_yukle(&yol).is_ok(),
            "dosya yoksa varsayılan yazılabilir"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bozuk_settings_json_onizlemede_hata_verir_dosyaya_dokunulmaz() {
        let d = gecici("bozuk-settings");
        let yol = d.join("settings.json");
        std::fs::write(&yol, "{ bozuk").expect("yaz");
        let o = statusline_onizle_dosyadan(&yol, r"C:\x\cetvel.exe");
        assert!(!o.uygulanabilir);
        assert!(o.hata.is_some());
        assert!(statusline_uygula_dosyaya(&yol, r"C:\x\cetvel.exe", "", "e").is_err());
        assert_eq!(std::fs::read_to_string(&yol).unwrap(), "{ bozuk");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ozel_sessiz_saat_ve_yerel_sayaclar_guncellenir_gecersiz_saat_reddedilir() {
        let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
            sessiz_saatler: Some(true),
            sessiz_baslangic: Some(23),
            sessiz_bitis: Some(7),
            opencode_etkin: Some(true),
            dsh_etkin: Some(true),
            ..AyarGuncelleme::default()
        });
        assert!(sonuc.is_ok());
        assert_eq!(
            (
                a.bildirim.sessiz_baslangic_saat,
                a.bildirim.sessiz_bitis_saat
            ),
            (23, 7)
        );
        assert!(a.opencode.etkin && a.dsh.etkin);
        let g = arayuz_ayarlari_uret(&a, None);
        assert!(g.sessiz_saatler && g.opencode_etkin && g.dsh_etkin);
        assert_eq!((g.sessiz_baslangic, g.sessiz_bitis), (23, 7));
        let json = serde_json::to_value(&g).unwrap();
        assert_eq!(json["sessizBaslangic"], 23);
        assert_eq!(json["opencodeEtkin"], true);

        let (a, sonuc) = tek_guncelleme(AyarGuncelleme {
            sessiz_bitis: Some(24),
            dsh_etkin: Some(true),
            ..AyarGuncelleme::default()
        });
        assert!(sonuc.is_err());
        assert_eq!(a, Ayarlar::default(), "geçersiz saat hiçbir alanı yazmaz");
        // Varsayılan: yerel sayaçlar kapalı (yalnız Bağla'dan açılır).
        let v = arayuz_ayarlari_uret(&Ayarlar::default(), None);
        assert!(!v.opencode_etkin && !v.dsh_etkin);
        let g: AyarGuncelleme =
            serde_json::from_str(r#"{"opencodeEtkin":true,"sessizBaslangic":22}"#).unwrap();
        assert_eq!(g.opencode_etkin, Some(true));
        assert_eq!(g.sessiz_baslangic, Some(22));
    }

    #[test]
    fn lisans_belgeleri_gomulu_ve_yerel_yol_icermez() {
        assert!(lisans_metni("mit").unwrap().contains("MIT License"));
        assert!(lisans_metni("apache").unwrap().contains("Apache License"));
        let ucuncu = lisans_metni("ucuncu_taraf").unwrap();
        assert!(ucuncu.contains("THIRD-PARTY NOTICES"));
        assert!(ucuncu.contains("tauri "), "Tauri bileşeni listelenmeli");
        assert!(!ucuncu.contains(r"\Users\"), "yerel yol sızmamalı");
        assert!(lisans_metni("gpl").is_err());
    }

    #[test]
    fn gecmis_csv_kacis_ve_dosya_adi() {
        let yanit = serde_json::json!({"ornekler": [
            {"zaman": "2026-11-16T10:00:00Z", "kullanilan": 0.25, "kalan": 0.75,
             "sifirlama": "2026-11-16T15:00:00Z", "kaynak": "mod_koprusu"},
            {"zaman": "2026-11-16T10:05:00Z", "kullanilan": 0.3, "kalan": 0.7, "kaynak": "a,\"b\""}
        ]});
        let csv = gecmis_csv_uret(&[("claude_max".into(), "5s".into(), yanit)]);
        let satirlar: Vec<&str> = csv.lines().collect();
        assert_eq!(
            satirlar[0],
            "havuz,pencere,zaman,kullanilan,kalan,sifirlama,kaynak"
        );
        assert_eq!(
            satirlar[1],
            "claude_max,5s,2026-11-16T10:00:00Z,0.2500,0.7500,2026-11-16T15:00:00Z,mod_koprusu"
        );
        assert!(satirlar[2].ends_with(",\"a,\"\"b\"\"\""), "{}", satirlar[2]);
        assert_eq!(
            disa_aktarim_adi("2026-11-16 14:30:05"),
            "Cetvel Geçmiş 2026-11-16 14.30.csv"
        );
    }
}

#[cfg(test)]
mod kart_testleri {
    use super::*;

    #[test]
    fn camel_case_kart_ayarlari_kalici_bicime_ve_olay_yukune_yansir() {
        let g: AyarGuncelleme = serde_json::from_value(serde_json::json!({
            "kartSirasi": ["codex_plus", "claude_max"],
            "gizliKartlar": ["deepseek"], "sabitKartlar": ["codex_plus"],
            "pencereBostaSn": 30
        }))
        .unwrap();
        let mut a = Ayarlar::default();
        guncelleme_uygula(&mut a, &g).unwrap();
        assert_eq!(a.masaustu.kart_sirasi, ["codex_plus", "claude_max"]);
        let olay = serde_json::to_value(arayuz_ayarlari_uret(&a, None)).unwrap();
        assert_eq!(olay["gizliKartlar"], serde_json::json!(["deepseek"]));
        assert_eq!(olay["sabitKartlar"], serde_json::json!(["codex_plus"]));
        assert_eq!(
            olay["kartSirasi"],
            serde_json::json!(["codex_plus", "claude_max"])
        );
        assert_eq!(olay["pencereBostaSn"], 30);
    }

    #[test]
    fn gecersiz_kart_listesi_hicbir_alani_degistirmez() {
        let once = Ayarlar::default();
        for alan in ["kartSirasi", "gizliKartlar", "sabitKartlar"] {
            for liste in [
                vec!["x"; 65],
                vec![""],
                vec!["ABC"],
                vec!["a-b"],
                vec!["ş"],
                vec!["a b"],
                vec!["a\n"],
            ] {
                let g: AyarGuncelleme = serde_json::from_value(serde_json::json!({
                    "dil": "en", alan: liste
                }))
                .unwrap();
                let mut a = once.clone();
                assert!(guncelleme_uygula(&mut a, &g).is_err());
                assert_eq!(a, once);
            }
        }
        for sn in [0, 3601] {
            let mut a = once.clone();
            assert!(
                guncelleme_uygula(
                    &mut a,
                    &AyarGuncelleme {
                        pencere_bosta_sn: Some(sn),
                        ..Default::default()
                    }
                )
                .is_err()
            );
            assert_eq!(a, once);
        }
    }

    #[test]
    fn kart_listeleri_bosaltilebilir_eksik_alanlar_korunur() {
        let mut a = Ayarlar::default();
        a.masaustu.kart_sirasi = vec!["codex".into()];
        a.masaustu.sabit_kartlar = vec!["deepseek".into()];
        guncelleme_uygula(
            &mut a,
            &AyarGuncelleme {
                kart_sirasi: Some(Vec::new()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(a.masaustu.kart_sirasi.is_empty());
        assert_eq!(a.masaustu.sabit_kartlar, ["deepseek"]);

        // Sınır değerleri (1..=3600 sn, 40 karakter kart adı, 64 öğe) kabul edilir.
        let uzun_ad = "a".repeat(40);
        let mut a = Ayarlar::default();
        guncelleme_uygula(
            &mut a,
            &AyarGuncelleme {
                kart_sirasi: Some(vec![uzun_ad.clone(); 64]),
                pencere_bosta_sn: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(a.masaustu.kart_sirasi.len(), 64);
        assert_eq!(a.masaustu.kart_sirasi[0], uzun_ad);
        assert_eq!(a.masaustu.pencere_bosta_sn, 1);
        guncelleme_uygula(
            &mut a,
            &AyarGuncelleme {
                pencere_bosta_sn: Some(3600),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(a.masaustu.pencere_bosta_sn, 3600);
    }

    #[test]
    fn platform_anahtarlari_ve_bildirim_esikleri_dogrulanir() {
        let mut esikler = BTreeMap::new();
        esikler.insert("codex".to_string(), 25);
        esikler.insert("claude".to_string(), 15);
        let mut a = Ayarlar::default();
        assert!(!a.deepseek.etkin);
        assert!(a.codex.etkin);

        let g = AyarGuncelleme {
            codex_etkin: Some(false),
            deepseek_etkin: Some(true),
            platform_esikleri: Some(esikler.clone()),
            sessiz_platformlar: Some(vec!["codex".to_string()]),
            ..Default::default()
        };
        guncelleme_uygula(&mut a, &g).expect("gecerli");
        assert!(!a.codex.etkin);
        assert!(a.deepseek.etkin);
        assert_eq!(a.bildirim.platform_esikleri, esikler);
        assert_eq!(a.bildirim.sessiz_platformlar, ["codex"]);

        let gorunum = arayuz_ayarlari_uret(&a, None);
        assert!(!gorunum.codex_etkin);
        assert!(gorunum.deepseek_etkin);
        assert_eq!(gorunum.platform_esikleri, esikler);
        assert_eq!(gorunum.sessiz_platformlar, ["codex"]);

        // Geçersiz eşik (0 veya 100) ya da geçersiz platform adı hiçbir alanı değiştirmez.
        let once = a.clone();
        let mut bozuk_esik = BTreeMap::new();
        bozuk_esik.insert("codex".to_string(), 100);
        assert!(
            guncelleme_uygula(
                &mut a,
                &AyarGuncelleme {
                    deepseek_etkin: Some(false),
                    platform_esikleri: Some(bozuk_esik),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(a, once);

        assert!(
            guncelleme_uygula(
                &mut a,
                &AyarGuncelleme {
                    sessiz_platformlar: Some(vec!["GECERSIZ!".to_string()]),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(a, once);
    }
}

/// Arayüzdeki "Çıkış" menüsü: tepsi menüsündeki Çıkış ile aynı (servise dokunmaz).
#[tauri::command]
pub fn uygulamadan_cik(app: tauri::AppHandle) {
    app.exit(0);
}
