use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, RwLock as StdRwLock};
use std::time::Duration;

use cetvel_ajan::AjanAnlik;
use cetvel_depo::{
    Ayarlar, BildirimAyari, CetvelYollar, GecmisDeposu, GecmisOrnegi, kullanici_dacl_uygula,
};
use cetvel_model::{AnlikV2, Durum, HavuzAnlik, UtcZaman};
use cetvel_tahmin::{OrnekNoktasi, havuz_tahminlerini_doldur};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Notify, RwLock, mpsc, oneshot, watch};

use crate::bagla::{TaramaSonucu, TestSonucu, platform_normalize, tara};
use crate::bildirim::{BildirimGonderici, BildirimYoneticisi, yerel_saat_now};
use crate::claude_kopru::{
    ClaudeKopruHatasi, KiraIstegi, KiraSonucu, KiraYoneticisi, claude_havuzu_guncelle,
    suresi_dolan_pencereleri_hesapla,
};
use crate::durum_makinesi::{YenileSinirlayici, kopru_havuzu_mu, yayin_birlestir};
use crate::kova_kaydi::KovaKaydi;

/// Köprüden (`POST /v1/claude`) yaratılabilecek en fazla havuz sayısı: yetkili bir istemci
/// sınırsız kalıcı havuz üretemesin.
pub const KOPRU_HAVUZ_TAVANI: usize = 8;
/// İçerik değişmeyen köprü gönderimi bu süre içinde yeniden yazılmaz (sira, SQLite, disk).
const KOPRU_DEGISMEYEN_ATLAMA_SN: i64 = 60;

/// "Test et" isteğinin servis döngüsüne iletilen biçimi (bağlayıcıyı yalnız döngü çalıştırır).
pub struct TestIstegi {
    pub platform: String,
    pub yanit: oneshot::Sender<TestSonucu>,
}

/// `POST /v1/yenile` ve pipe `yenile` yanıtı.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct YenileYaniti {
    pub istendi: bool,
    pub sira: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub havuz: Option<String>,
    /// Hız sınırına takıldıysa yeniden denemeden önce beklenecek saniye.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bekle_sn: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HavuzSaglikOzeti {
    pub durum: Durum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yas_sn: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kullanilabilir: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaglikYaniti {
    pub durum: String,
    pub surum: String,
    pub sira: u64,
    pub zaman: UtcZaman,
    pub calisma_sn: u64,
    pub port: u16,
    pub pipe: String,
    pub havuz_sayisi: usize,
    pub havuzlar: BTreeMap<String, HavuzSaglikOzeti>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GecmisKaydiJson {
    pub zaman: UtcZaman,
    pub kullanilan: f64,
    pub kalan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama: Option<UtcZaman>,
    pub kaynak: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hassasiyet: Option<f64>,
}

impl From<GecmisOrnegi> for GecmisKaydiJson {
    fn from(o: GecmisOrnegi) -> Self {
        Self {
            zaman: o.zaman,
            kullanilan: o.kullanilan,
            kalan: o.kalan,
            sifirlama: o.sifirlama,
            kaynak: o.kaynak,
            hassasiyet: o.hassasiyet,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GecmisYaniti {
    pub havuz: String,
    pub pencere: String,
    pub gun: u64,
    pub ornekler: Vec<GecmisKaydiJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeGuncellemeYaniti {
    pub kabul: bool,
    pub uygulandi: bool,
    pub sira: u64,
    pub havuz: HavuzAnlik,
}

type BildirimGeriCagirisi =
    Box<dyn FnMut(&AnlikV2, &GecmisDeposu, UtcZaman, &BildirimAyari) + Send>;

/// HTTP sunucusu, Named Pipe ve arka plan servis döngüsünün paylaştığı durum.
pub struct PaylasilanServisDurumu {
    /// Güncel ayarlar; `ayar.yenile` ile servis yeniden başlatılmadan değiştirilir.
    pub ayarlar: StdRwLock<Arc<Ayarlar>>,
    /// Bozuk `ayarlar.toml` kurtarıldıysa kullanıcıya gösterilecek açıklama.
    pub ayar_uyarisi: Mutex<Option<String>>,
    pub yollar: Option<CetvelYollar>,
    pub jeton: String,
    pub pipe_adi: String,
    pub http_port: Mutex<u16>,
    pub baslangic: UtcZaman,
    pub anlik: RwLock<AnlikV2>,
    pub sira_yayini: watch::Sender<u64>,
    pub claude_kira: Mutex<KiraYoneticisi>,
    pub gecmis: Mutex<Option<GecmisDeposu>>,
    pub bildirim: Mutex<Option<BildirimGeriCagirisi>>,
    pub yenile_bildir: Notify,
    pub ayar_yenile_iste: Notify,
    pub kapat_iste: Notify,
    /// Claude dışı ajan ağacı (`cetvel-ajan`); izleyici görevi günceller.
    ajanlar: StdRwLock<Arc<AjanAnlik>>,
    yenile_hedefleri: Mutex<BTreeSet<String>>,
    yenile_sinirlayici: Mutex<YenileSinirlayici>,
    test_tx: mpsc::UnboundedSender<TestIstegi>,
    test_rx: Mutex<Option<mpsc::UnboundedReceiver<TestIstegi>>>,
    /// Dinamik kova kaydı (`kovalar.json`).
    kova_kaydi: Mutex<KovaKaydi>,
    /// Orkestra uyumunda havuz başına pencere yaş eşiği (sn); servis döngüsü kurar.
    uyum_yas_esikleri: Mutex<BTreeMap<String, u64>>,
}

impl PaylasilanServisDurumu {
    pub fn yeni(
        ayarlar: Ayarlar,
        yollar: Option<CetvelYollar>,
        jeton: impl Into<String>,
        pipe_adi: impl Into<String>,
        gecmis: Option<GecmisDeposu>,
    ) -> Self {
        let baslangic_anlik = AnlikV2::yeni(UtcZaman::simdi(), 0, BTreeMap::new());
        let (sira_tx, _) = watch::channel(0u64);
        let (test_tx, test_rx) = mpsc::unbounded_channel();
        Self {
            ayarlar: StdRwLock::new(Arc::new(ayarlar)),
            ayar_uyarisi: Mutex::new(None),
            yollar,
            jeton: jeton.into(),
            pipe_adi: pipe_adi.into(),
            http_port: Mutex::new(0),
            baslangic: UtcZaman::simdi(),
            anlik: RwLock::new(baslangic_anlik),
            sira_yayini: sira_tx,
            claude_kira: Mutex::new(KiraYoneticisi::default()),
            gecmis: Mutex::new(gecmis),
            bildirim: Mutex::new(None),
            yenile_bildir: Notify::new(),
            ayar_yenile_iste: Notify::new(),
            kapat_iste: Notify::new(),
            ajanlar: StdRwLock::new(Arc::new(AjanAnlik::bos(UtcZaman::simdi()))),
            yenile_hedefleri: Mutex::new(BTreeSet::new()),
            yenile_sinirlayici: Mutex::new(YenileSinirlayici::default()),
            test_tx,
            test_rx: Mutex::new(Some(test_rx)),
            kova_kaydi: Mutex::new(KovaKaydi::default()),
            uyum_yas_esikleri: Mutex::new(BTreeMap::new()),
        }
    }

    /// `kovalar.json` kaydını diskten yükler (servis başlangıcı).
    pub fn kova_kaydini_yukle(&self) {
        if let Some(yollar) = &self.yollar
            && let Ok(mut g) = self.kova_kaydi.lock()
        {
            *g = KovaKaydi::yukle(&yollar.kovalar_dosyasi());
        }
    }

    /// Kayıtlı havuz kimlikleri (platform filtresiyle).
    pub fn kayitli_kimlikler(&self, platform: &str) -> Vec<String> {
        self.kova_kaydi
            .lock()
            .map(|k| k.kimlikler(platform))
            .unwrap_or_default()
    }

    /// Uyum yaş eşiklerini ayarlar (bağlayıcı yoklama aralıklarından türetilmiş).
    pub fn uyum_esiklerini_ayarla(&self, esikler: BTreeMap<String, u64>) {
        if let Ok(mut g) = self.uyum_yas_esikleri.lock() {
            *g = esikler;
        }
    }

    fn uyum_esikleri(&self) -> BTreeMap<String, u64> {
        self.uyum_yas_esikleri
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Başarıyla okunan platformlar için yaşam döngüsünü uygular: yanıtta olmayan kayıtlı
    /// havuzlar `gorulmuyor`, 7 günden eskiler düşer. Düşen kimlikleri döndürür.
    pub fn kova_yasam_uygula(
        &self,
        okunanlar: &[(String, BTreeSet<String>)],
        havuzlar: &mut BTreeMap<String, HavuzAnlik>,
        simdi: UtcZaman,
    ) -> Vec<String> {
        let Ok(mut kayit) = self.kova_kaydi.lock() else {
            return Vec::new();
        };
        let mut dusen = Vec::new();
        for (platform, gorulen) in okunanlar {
            dusen.extend(kayit.yasam_uygula(platform, gorulen, havuzlar, simdi));
        }
        if !dusen.is_empty() {
            self.kova_kaydini_yaz(&kayit);
        }
        dusen
    }

    fn kova_kaydini_yaz(&self, kayit: &KovaKaydi) {
        if let Some(yollar) = &self.yollar
            && let Err(e) = cetvel_depo::atomik_yaz_json(&yollar.kovalar_dosyasi(), kayit)
        {
            tracing::warn!("kovalar.json yazılamadı: {e}");
        }
    }

    /// Yayınlanacak havuzları kayda işler (ilk/son görülme, alt sayaç ilk görülme) ve iç içe
    /// kota zincirini hesaplar.
    fn kayda_isle(&self, anlik: &mut AnlikV2) {
        if let Ok(mut kayit) = self.kova_kaydi.lock() {
            let mut degisti = false;
            for (id, havuz) in &anlik.havuzlar {
                degisti |= kayit.gozlemle(id, havuz);
            }
            for (id, havuz) in &mut anlik.havuzlar {
                kayit.havuza_isle(id, havuz);
            }
            if degisti {
                self.kova_kaydini_yaz(&kayit);
            }
        }
        anlik.zincirleri_hesapla();
    }

    /// Geçmiş veritabanında 90 günden eski kayıtları siler (servis döngüsü günde bir çağırır).
    pub fn gecmis_temizle(&self, simdi: UtcZaman) {
        if let Ok(guard) = self.gecmis.lock()
            && let Some(depo) = guard.as_ref()
            && let Err(e) = depo.eski_verileri_temizle(simdi, 90)
        {
            tracing::warn!("geçmiş temizliği başarısız: {e}");
        }
    }

    /// Son ajan anlığı (`GET /v1/ajanlar`, pipe `ajanlar.al`).
    pub fn ajanlar_kopya(&self) -> Arc<AjanAnlik> {
        self.ajanlar
            .read()
            .map(|g| Arc::clone(&g))
            .unwrap_or_else(|e| Arc::clone(&e.into_inner()))
    }

    /// Yeniden başlatmada önceki `ajanlar.json`'un `sira`'sından sürdürür (ETag `"ajan-<sira>"`
    /// tekrar etmesin, `sira` artışını izleyen tüketici yeni içeriği kaçırmasın). Boş başlangıç
    /// anlığı yeni bir içerik sürümü sayılır: `onceki + 1`.
    pub fn ajan_sirasini_tohumla(&self, onceki: u64) {
        if onceki == 0 {
            return;
        }
        let mut g = match self.ajanlar.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if g.sira <= onceki {
            let mut yeni = (**g).clone();
            yeni.sira = onceki.saturating_add(1);
            *g = Arc::new(yeni);
        }
    }

    /// Yeni ajan anlığını yerleştirir. İçerik değiştiyse `sira` artar ve `true` döner; yalnız
    /// `zaman` değiştiyse sira korunur.
    pub fn ajanlar_guncelle(&self, mut yeni: AjanAnlik) -> (bool, Arc<AjanAnlik>) {
        let mut g = match self.ajanlar.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        let degisti = !g.icerik_ayni_mi(&yeni);
        yeni.sira = if degisti { g.sira + 1 } else { g.sira };
        let yeni = Arc::new(yeni);
        *g = Arc::clone(&yeni);
        (degisti, yeni)
    }

    /// Güncel ayarların paylaşılan kopyası.
    pub fn ayarlar_kopya(&self) -> Arc<Ayarlar> {
        self.ayarlar
            .read()
            .map(|g| Arc::clone(&g))
            .unwrap_or_else(|e| Arc::clone(&e.into_inner()))
    }

    /// Ayarları çalışma anında değiştirir (bildirim eşikleri bir sonraki yayında geçerli olur).
    pub fn ayarlari_degistir(&self, yeni: Ayarlar) {
        match self.ayarlar.write() {
            Ok(mut g) => *g = Arc::new(yeni),
            Err(e) => *e.into_inner() = Arc::new(yeni),
        }
    }

    pub fn ayar_uyarisi_ayarla(&self, uyari: Option<String>) {
        if let Ok(mut g) = self.ayar_uyarisi.lock() {
            *g = uyari;
        }
    }

    pub fn ayar_uyarisi(&self) -> Option<String> {
        self.ayar_uyarisi.lock().ok().and_then(|g| g.clone())
    }

    /// Servis yeniden başlarken diskteki son anlıktan `sira` ve köprü (Claude) havuzlarını geri yükler.
    /// Yoklanan havuzlar burada alınmaz; döngü onları koordinatöre tohumlar.
    pub async fn baslangic_anligi_kur(&self, onceki: &AnlikV2) {
        let havuzlar: BTreeMap<String, HavuzAnlik> = onceki
            .havuzlar
            .iter()
            .filter(|(_, h)| kopru_havuzu_mu(h))
            .map(|(id, h)| (id.clone(), h.clone()))
            .collect();
        let sira = onceki.sira;
        {
            let mut g = self.anlik.write().await;
            *g = AnlikV2::yeni(onceki.zaman, sira, havuzlar);
        }
        let _ = self.sira_yayini.send(sira);
    }

    pub fn bildirim_yoneticisi_bagla<G: BildirimGonderici + 'static>(
        &self,
        mut yonetici: BildirimYoneticisi<G>,
    ) {
        let cb: BildirimGeriCagirisi = Box::new(move |anlik, depo, simdi, ayar| {
            let saat = yerel_saat_now();
            let _ = yonetici.degerlendir(anlik, ayar, depo, saat, simdi);
        });
        if let Ok(mut guard) = self.bildirim.lock() {
            *guard = Some(cb);
        }
    }

    pub fn http_port_ayarla(&self, port: u16) {
        if let Ok(mut guard) = self.http_port.lock() {
            *guard = port;
        }
    }

    /// Mevcut anlık snapshot'ı (varsa süresi dolan pencereleri hesaplayarak) döndürür.
    pub async fn anlik_kopya(&self) -> AnlikV2 {
        let simdi = UtcZaman::simdi();
        let mut anlik = self.anlik.read().await.clone();
        for havuz in anlik.havuzlar.values_mut() {
            let _ = suresi_dolan_pencereleri_hesapla(havuz, simdi);
        }
        anlik
    }

    /// Tek bir havuzu ada göre döndürür.
    pub async fn havuz_kopya(&self, ad: &str) -> Option<HavuzAnlik> {
        let simdi = UtcZaman::simdi();
        let anlik = self.anlik.read().await;
        let mut havuz = anlik.havuzlar.get(ad)?.clone();
        let _ = suresi_dolan_pencereleri_hesapla(&mut havuz, simdi);
        Some(havuz)
    }

    /// Yeni anlığı (yazma kilidi alınmışken) geçmişe kaydeder, tahmin/bildirim işler, diske yazar ve yayınlar.
    fn kilitli_yayinla(&self, hedef: &mut AnlikV2, mut yeni: AnlikV2, simdi: UtcZaman) -> u64 {
        for havuz in yeni.havuzlar.values_mut() {
            let _ = suresi_dolan_pencereleri_hesapla(havuz, simdi);
        }
        self.kayda_isle(&mut yeni);
        let guncel_ayarlar = self.ayarlar_kopya();
        let bildirim_ayari = &guncel_ayarlar.bildirim;

        if let Ok(mut gecmis_guard) = self.gecmis.lock()
            && let Some(depo) = gecmis_guard.as_mut()
        {
            let _ = depo.anlik_kaydet(&yeni);
            tahminleri_havuzlara_doldur(&mut yeni, depo, simdi, bildirim_ayari);
            if let Ok(mut b_guard) = self.bildirim.lock()
                && let Some(cb) = b_guard.as_mut()
            {
                cb(&yeni, depo, simdi, bildirim_ayari);
            }
        }

        if let Some(yollar) = &self.yollar {
            dosyaya_anlik_ve_uyum_yaz(
                yollar,
                &yeni,
                Some(guncel_ayarlar.agy.aralik_sn),
                &self.uyum_esikleri(),
            );
        }

        let sira = yeni.sira;
        *hedef = yeni;
        let _ = self.sira_yayini.send(sira);
        sira
    }

    /// Verilen anlığı olduğu gibi (kendi `sira`sıyla) yayınlar. Testler ve tek tur için.
    pub async fn anlik_yayinla(&self, yeni: AnlikV2, simdi: UtcZaman) {
        let mut guard = self.anlik.write().await;
        self.kilitli_yayinla(&mut guard, yeni, simdi);
    }

    /// Servis döngüsü yayını: okuma ile yayın aynı yazma kilidi altında birleştirilir; köprü
    /// güncellemesi kaybolmaz ve her içerik değişimi yeni (tekil) bir `sira` alır.
    pub async fn dongu_yayinla(
        &self,
        yoklanan: BTreeMap<String, HavuzAnlik>,
        yoklanan_kimlikler: &BTreeSet<String>,
        simdi: UtcZaman,
        kalp_atisi_geldi: bool,
    ) -> Option<u64> {
        let mut guard = self.anlik.write().await;
        let yeni = yayin_birlestir(
            &guard,
            yoklanan,
            yoklanan_kimlikler,
            simdi,
            900,
            kalp_atisi_geldi,
        )?;
        Some(self.kilitli_yayinla(&mut guard, yeni, simdi))
    }

    /// Claude köprüsünden (`POST /v1/claude`) gelen ölçümü doğrular, havuzu ve anlık dosyalarını günceller.
    pub async fn claude_guncelle(
        &self,
        ham: Value,
        simdi: UtcZaman,
    ) -> Result<ClaudeGuncellemeYaniti, ClaudeKopruHatasi> {
        let havuz_id = ham
            .get("havuz")
            .or_else(|| ham.get("havuz_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("claude_max")
            .to_string();

        let mut guncellenmis = self.anlik.write().await;
        let mevcut = guncellenmis.havuzlar.get(&havuz_id).cloned();
        if mevcut.is_none()
            && guncellenmis
                .havuzlar
                .values()
                .filter(|h| kopru_havuzu_mu(h))
                .count()
                >= KOPRU_HAVUZ_TAVANI
        {
            return Err(ClaudeKopruHatasi::GecersizBicim(format!(
                "köprü havuz sayısı tavanı ({KOPRU_HAVUZ_TAVANI}) dolu"
            )));
        }
        let (mut havuz, mut uygulandi) = claude_havuzu_guncelle(mevcut.as_ref(), ham, simdi)?;
        // Statusline sık gönderir: içerik (pencereler, alt sayaçlar, bakiye) değişmediyse ve son
        // uygulama 60 sn'den yeniyse sira artışı, SQLite işlemi ve disk yazımı atlanır.
        if uygulandi
            && let Some(eski) = &mevcut
            && eski.pencereler == havuz.pencereler
            && eski.alt_sayaclar.keys().eq(havuz.alt_sayaclar.keys())
            && eski
                .alt_sayaclar
                .iter()
                .zip(havuz.alt_sayaclar.iter())
                .all(|((_, a), (_, b))| a.kullanilan == b.kullanilan && a.sifirlama == b.sifirlama)
            && eski.bakiye == havuz.bakiye
            && eski
                .kaynak
                .as_ref()
                .is_some_and(|k| simdi.fark_sn(k.okuma) < KOPRU_DEGISMEYEN_ATLAMA_SN)
        {
            uygulandi = false;
            havuz = eski.clone();
        }

        let sira = if uygulandi {
            let guncel_ayarlar = self.ayarlar_kopya();
            let bildirim_ayari = &guncel_ayarlar.bildirim;
            guncellenmis.sira = guncellenmis.sira.saturating_add(1).max(1);
            guncellenmis.zaman = simdi;
            if let Ok(mut gecmis_guard) = self.gecmis.lock()
                && let Some(depo) = gecmis_guard.as_mut()
            {
                // Tekil havuzu geçici snapshot ile kaydedip tahmin hesapla
                let mut tek = AnlikV2::yeni(
                    simdi,
                    guncellenmis.sira,
                    BTreeMap::from([(havuz_id.clone(), havuz.clone())]),
                );
                let _ = depo.anlik_kaydet(&tek);
                tahminleri_havuzlara_doldur(&mut tek, depo, simdi, bildirim_ayari);
                if let Some(t_havuz) = tek.havuzlar.remove(&havuz_id) {
                    havuz = t_havuz;
                }
            }
            guncellenmis
                .havuzlar
                .insert(havuz_id.clone(), havuz.clone());
            self.kayda_isle(&mut guncellenmis);
            if let Some(h) = guncellenmis.havuzlar.get(&havuz_id) {
                havuz = h.clone();
            }

            if let Ok(gecmis_guard) = self.gecmis.lock()
                && let Some(depo) = gecmis_guard.as_ref()
                && let Ok(mut b_guard) = self.bildirim.lock()
                && let Some(cb) = b_guard.as_mut()
            {
                cb(&guncellenmis, depo, simdi, bildirim_ayari);
            }

            if let Some(yollar) = &self.yollar {
                dosyaya_anlik_ve_uyum_yaz(
                    yollar,
                    &guncellenmis,
                    Some(guncel_ayarlar.agy.aralik_sn),
                    &self.uyum_esikleri(),
                );
            }
            let _ = self.sira_yayini.send(guncellenmis.sira);
            guncellenmis.sira
        } else {
            guncellenmis.sira
        };

        Ok(ClaudeGuncellemeYaniti {
            kabul: true,
            uygulandi,
            sira,
            havuz,
        })
    }

    pub fn claude_kira_talep(&self, istek: &KiraIstegi, simdi: UtcZaman) -> KiraSonucu {
        let mut guard = self.claude_kira.lock().expect("claude kira kilidi");
        guard.talep_et(&istek.istemci, istek.sure_sn, simdi)
    }

    pub fn gecmis_sorgula(
        &self,
        havuz: &str,
        pencere: &str,
        gun: u64,
        simdi: UtcZaman,
    ) -> GecmisYaniti {
        let gun_sinirli = gun.clamp(1, 90);
        let baslangic = simdi.ekle_sn(-((gun_sinirli as i64) * 86_400));
        let ornekler = if let Ok(guard) = self.gecmis.lock()
            && let Some(depo) = guard.as_ref()
        {
            depo.pencere_ornekleri_seyrek(havuz, pencere, baslangic, simdi, 2_000)
                .unwrap_or_default()
                .into_iter()
                .map(GecmisKaydiJson::from)
                .collect()
        } else {
            Vec::new()
        };

        GecmisYaniti {
            havuz: havuz.to_string(),
            pencere: pencere.to_string(),
            gun: gun_sinirli,
            ornekler,
        }
    }

    pub async fn saglik_ozeti(&self) -> SaglikYaniti {
        let simdi = UtcZaman::simdi();
        let anlik = self.anlik.read().await;
        let mut havuzlar = BTreeMap::new();
        for (ad, havuz) in &anlik.havuzlar {
            havuzlar.insert(
                ad.clone(),
                HavuzSaglikOzeti {
                    durum: havuz.durum,
                    yas_sn: havuz.kaynak.as_ref().map(|k| k.yas_sn),
                    kullanilabilir: havuz.kullanilabilir,
                },
            );
        }
        let calisma_sn = simdi.fark_sn(self.baslangic).max(0) as u64;
        let port = self.http_port.lock().map(|g| *g).unwrap_or(0);
        let saglik_durum = if saglik_uyarisi_mi(&anlik) {
            "uyari"
        } else {
            "ok"
        };

        SaglikYaniti {
            durum: saglik_durum.to_string(),
            surum: env!("CARGO_PKG_VERSION").to_string(),
            sira: anlik.sira,
            zaman: simdi,
            calisma_sn,
            port,
            pipe: self.pipe_adi.clone(),
            havuz_sayisi: havuzlar.len(),
            havuzlar,
        }
    }

    /// Elle yenileme isteği (havuz başına 1/dk). `havuz` yoksa tüm havuzlar (`*`) istenir;
    /// `agy` içeren hedef istek üzerine Antigravity okumasını tetikler (izin kapısına bağlı).
    pub async fn yenile_iste(&self, havuz: Option<&str>, simdi: UtcZaman) -> YenileYaniti {
        let hedef = havuz
            .map(str::trim)
            .filter(|h| !h.is_empty() && h.len() <= 64)
            .unwrap_or("*")
            .to_string();
        let izin = self
            .yenile_sinirlayici
            .lock()
            .map(|mut s| s.izin_ver(&hedef, simdi))
            .unwrap_or(Ok(()));
        let sira = self.anlik.read().await.sira;
        match izin {
            Ok(()) => {
                if let Ok(mut h) = self.yenile_hedefleri.lock() {
                    h.insert(hedef.clone());
                }
                self.yenile_bildir.notify_one();
                YenileYaniti {
                    istendi: true,
                    sira,
                    havuz: Some(hedef),
                    bekle_sn: None,
                }
            }
            Err(bekle) => YenileYaniti {
                istendi: false,
                sira,
                havuz: Some(hedef),
                bekle_sn: Some(bekle),
            },
        }
    }

    /// Bekleyen yenileme hedeflerini alır ve temizler (servis döngüsü çağırır).
    pub fn yenile_hedeflerini_al(&self) -> BTreeSet<String> {
        self.yenile_hedefleri
            .lock()
            .map(|mut h| std::mem::take(&mut *h))
            .unwrap_or_default()
    }

    /// "Test et" alıcısını servis döngüsüne devreder (bir kez).
    pub fn test_alicisini_al(&self) -> Option<mpsc::UnboundedReceiver<TestIstegi>> {
        self.test_rx.lock().ok().and_then(|mut g| g.take())
    }

    /// Bağlayıcıyı servis döngüsünde (tek okuyucu) sınar; masaüstü kendisi okumaz.
    pub async fn bagla_test(&self, platform: &str) -> Result<TestSonucu, String> {
        let p =
            platform_normalize(platform).ok_or_else(|| "desteklenmeyen platform".to_string())?;
        let (tx, rx) = oneshot::channel();
        self.test_tx
            .send(TestIstegi {
                platform: p.to_string(),
                yanit: tx,
            })
            .map_err(|_| "servis döngüsü çalışmıyor".to_string())?;
        match tokio::time::timeout(Duration::from_secs(90), rx).await {
            Ok(Ok(sonuc)) => Ok(sonuc),
            Ok(Err(_)) => Err("test yanıtı alınamadı".to_string()),
            Err(_) => Err("test zaman aşımına uğradı".to_string()),
        }
    }

    /// Salt-okunur Bağla taraması (`bagla.tara`).
    pub async fn bagla_tara(&self) -> TaramaSonucu {
        let simdi = UtcZaman::simdi();
        let (claude, codex) = {
            let anlik = self.anlik.read().await;
            (
                anlik.havuzlar.get("claude_max").cloned(),
                anlik.havuzlar.get("codex_plus").cloned(),
            )
        };
        let ayarlar = self.ayarlar_kopya();
        tara(&ayarlar, claude.as_ref(), codex.as_ref(), simdi)
    }
}

/// Tahmin için örnek ufku: pencerenin gerçek başlangıcı (`sifirlama - uzunluk`, plan §7
/// `u_pencere_basi`); en fazla 8 gün geriye. Sıfırlama bilinmiyorsa son 4 saat (tahmin zaten yok).
/// Sağlık uyarısı: izlenen kota havuzlarının (kullanım sayacı, kapalı ve bağlanmamış havuzlar
/// hariç) en az biri var ve hepsi `Hata` ya da `Bayat`. Taze okunan bir kullanım sayacı (OpenCode,
/// dsh) bayat kota verisini "ok" gösteremez.
pub(crate) fn saglik_uyarisi_mi(anlik: &AnlikV2) -> bool {
    let izlenen: Vec<&HavuzAnlik> = anlik
        .havuzlar
        .values()
        .filter(|h| {
            !h.kullanim_sayaci_mi() && !matches!(h.durum, Durum::Kapali | Durum::Baglanmadi)
        })
        .collect();
    !izlenen.is_empty()
        && izlenen
            .iter()
            .all(|h| matches!(h.durum, Durum::Hata | Durum::Bayat))
}

fn tahmin_ufku(olcum: Option<&cetvel_model::PencereOlcum>, simdi: UtcZaman) -> UtcZaman {
    let en_eski = simdi.ekle_sn(-8 * 86_400);
    match olcum.and_then(|o| o.sifirlama.map(|s| (s, o.uzunluk_dk))) {
        Some((s, uzunluk_dk)) => {
            let bas = s.ekle_sn(-((uzunluk_dk as i64) * 60) - 600);
            if bas < en_eski {
                en_eski
            } else if bas > simdi {
                simdi
            } else {
                bas
            }
        }
        None => simdi.ekle_sn(-4 * 3600),
    }
}

fn tahminleri_havuzlara_doldur(
    anlik: &mut AnlikV2,
    depo: &GecmisDeposu,
    simdi: UtcZaman,
    bildirim_ayari: &BildirimAyari,
) {
    for (havuz_id, havuz) in &mut anlik.havuzlar {
        if havuz.durum != Durum::Ok || havuz.pencereler.is_empty() {
            continue;
        }
        let hassasiyet = havuz.kaynak.as_ref().and_then(|k| k.hassasiyet);
        let tahmini_kaynak = havuz
            .kaynak
            .as_ref()
            .is_some_and(|k| k.guven == cetvel_model::GuvenEtiketi::Tahmini);
        let pencere_kopya = havuz.pencereler.clone();

        let platform = havuz.platform.clone();
        let taban =
            |pencere_ad: &str| bildirim_ayari.platform_esik_orani(&platform, havuz_id, pencere_ad);
        havuz_tahminlerini_doldur(
            havuz,
            simdi,
            |pencere_ad| {
                let baslangic = tahmin_ufku(pencere_kopya.get(pencere_ad), simdi);
                let ornekler = depo
                    .pencere_ornekleri(havuz_id, pencere_ad, baslangic, 10_000)
                    .unwrap_or_default();
                let noktalar: Vec<OrnekNoktasi> = ornekler
                    .into_iter()
                    .map(|o| OrnekNoktasi {
                        zaman: o.zaman,
                        kullanilan: o.kullanilan,
                        sifirlama: o.sifirlama,
                        // Kaynak etiketi sabit listeye bağlı değil: havuzun kendi güveni
                        // tahminiyse ya da örnek hesaplanmış/agy kaynaklıysa gürültü eşiği uygulanır.
                        tahmini_kaynak: tahmini_kaynak
                            || matches!(o.kaynak.as_str(), "hesap" | "agy_usage" | "tahmini"),
                        hassasiyet: o.hassasiyet.or(hassasiyet),
                    })
                    .collect();
                if !noktalar.is_empty() {
                    noktalar
                } else if let Some(olcum) = pencere_kopya.get(pencere_ad) {
                    vec![OrnekNoktasi {
                        zaman: simdi,
                        kullanilan: olcum.kullanilan,
                        sifirlama: olcum.sifirlama,
                        tahmini_kaynak,
                        hassasiyet,
                    }]
                } else {
                    Vec::new()
                }
            },
            taban,
        );
    }
}

pub fn dosyaya_anlik_ve_uyum_yaz(
    yollar: &CetvelYollar,
    anlik: &AnlikV2,
    agy_aralik_sn: Option<u64>,
    uyum_esikleri: &BTreeMap<String, u64>,
) {
    if let Err(e) = std::fs::create_dir_all(&yollar.local_kok) {
        tracing::warn!("Cetvel klasörü oluşturulamadı: {e}");
        return;
    }
    let _ = kullanici_dacl_uygula(&yollar.local_kok);
    if let Err(e) = crate::anlik_ve_uyum_yayinla_esikli(
        yollar,
        anlik,
        anlik.zaman,
        agy_aralik_sn,
        uyum_esikleri,
    ) {
        tracing::warn!("anlik / orkestra-uyum yazılamadı: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cetvel_model::PencereOlcum;

    #[test]
    fn tahmin_ufku_pencere_basindan_baslar() {
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T12:00:00Z").unwrap();
        // Haftalık pencere, sıfırlamaya 92 sa: başlangıç 76 sa önce (4 sa değil).
        let hafta = PencereOlcum {
            uzunluk_dk: 10080,
            kullanilan: 0.35,
            kalan: 0.65,
            sifirlama: Some(simdi.ekle_sn(92 * 3600)),
            etiket: None,
        };
        let ufuk = tahmin_ufku(Some(&hafta), simdi);
        assert_eq!(simdi.fark_sn(ufuk), 76 * 3600 + 600);
        // 5s pencere 1 sa önce başladı.
        let bes = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.2,
            kalan: 0.8,
            sifirlama: Some(simdi.ekle_sn(4 * 3600)),
            etiket: None,
        };
        assert_eq!(simdi.fark_sn(tahmin_ufku(Some(&bes), simdi)), 3600 + 600);
        assert_eq!(simdi.fark_sn(tahmin_ufku(None, simdi)), 4 * 3600);
    }

    fn havuz(platform: &str, durum: Durum, sayac: bool) -> HavuzAnlik {
        HavuzAnlik {
            platform: platform.to_string(),
            etiket: platform.to_string(),
            durum,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: sayac.then(|| cetvel_model::EkBilgi {
                kullanim: Some(cetvel_model::KullanimSayaci {
                    kota_yok: true,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            hata: None,
            notlar: Vec::new(),
            kesif: None,
            etkin_kalan_zincir: None,
        }
    }

    #[test]
    fn saglik_uyarisi_kullanim_sayacini_ve_kapali_havuzu_saymaz() {
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T12:00:00Z").unwrap();
        let mut h = BTreeMap::new();
        h.insert(
            "claude_max".to_string(),
            havuz("claude", Durum::Bayat, false),
        );
        h.insert("codex_plus".to_string(), havuz("codex", Durum::Hata, false));
        h.insert("opencode".to_string(), havuz("opencode", Durum::Ok, true));
        h.insert("dsh".to_string(), havuz("dsh", Durum::Kapali, false));
        h.insert("agy".to_string(), havuz("agy", Durum::Baglanmadi, false));
        let mut anlik = AnlikV2::yeni(simdi, 1, h);
        assert!(
            saglik_uyarisi_mi(&anlik),
            "taze sayaç bayat kotayı örtmemeli"
        );

        anlik.havuzlar.get_mut("claude_max").unwrap().durum = Durum::Ok;
        assert!(!saglik_uyarisi_mi(&anlik));

        // Yalnız sayaç ve kapalı havuz varsa uyarı yok (izlenen kota yok).
        anlik.havuzlar.remove("claude_max");
        anlik.havuzlar.remove("codex_plus");
        assert!(!saglik_uyarisi_mi(&anlik));
        // Okuma hatasındaki sayaç (`ek` yok) da platformundan sayaç tanınır.
        anlik.havuzlar.insert(
            "opencode".to_string(),
            havuz("opencode", Durum::Hata, false),
        );
        assert!(!saglik_uyarisi_mi(&anlik));
    }
}
