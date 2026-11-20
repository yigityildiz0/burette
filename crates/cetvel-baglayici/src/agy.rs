use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use cetvel_depo::{AgyAyari, AgyOkumaIzni, atomik_yaz_json};
use cetvel_model::{
    Durum, GuvenEtiketi, HataBilgisi, HavuzAnlik, KaynakBilgisi, KaynakTuru, KesifBilgisi,
    KovaYasam, PencereOlcum, UtcZaman, pencere_anahtari, slug,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::job_object::{JobSuresKorumasi, askidaki_sureci_devam_ettir};
use crate::{Baglayici, BaglayiciSonucu};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Hiç okuma yapılmamışken (kapalı, kurulu değil) durum üretilen bilinen havuzlar.
const AGY_VARSAYILAN_HAVUZLARI: [&str; 3] = ["agy_gemini", "agy_claude_gpt", "agy_argon"];

/// `agy /usage` model turuna dönüştüğünde okumanın askıya alındığı süre (sn).
pub const AGY_MODEL_TURU_ASKI_SN: i64 = 24 * 3600;
/// Model turu hata türü (anlık `hata.tur`).
pub const AGY_MODEL_TURU_HATA: &str = "usage_model_turu";
const AGY_MODEL_TURU_NOTU: &str = "agy /usage model isteğine dönüştü; okuma 24 saat durduruldu";
/// Çıktı ayrıştırılamadığında ya da süreç dış zaman aşımına uğradığında bir sonraki denemeye
/// kadar beklenen süre: her deneme olası bir model turudur, hızlı geri çekilme yerine uzun bekle.
const AGY_SEMA_HATASI_BEKLEME_SN: u64 = 6 * 3600;
const AGY_CIKIS_HATASI_BEKLEME_SN: u64 = 3600;

/// Bilinen Antigravity grubu (veri tablosu; yeni grup eşlemesi buraya satır olarak eklenir).
struct BilinenGrup {
    /// Cetvel havuz kimliği (geriye uyum için sabit).
    havuz: &'static str,
    etiket: &'static str,
    /// Tam eşleşen grup adları/kimlikleri (küçük harf).
    adlar: &'static [&'static str],
    /// Grup adında geçerse eşleşen eski anahtar sözcükler (eski tablo çıktıları).
    anahtar_sozcukler: &'static [&'static str],
    /// Kova kimliği `önek + pencere` biçimindeyse eşleşir (`gemini-5h`, `3p-weekly`).
    kova_onekleri: &'static [&'static str],
}

/// Sıra önemli: daha özel önek (Argon) önce.
const BILINEN_GRUPLAR: &[BilinenGrup] = &[
    BilinenGrup {
        havuz: "agy_argon",
        etiket: "Antigravity Argon",
        adlar: &["gemini 4 argon", "argon"],
        anahtar_sozcukler: &["argon"],
        kova_onekleri: &["gemini-argon-wtu-", "gemini-argon-"],
    },
    BilinenGrup {
        havuz: "agy_claude_gpt",
        etiket: "Antigravity Claude-GPT",
        adlar: &["claude and gpt models", "3p", "claude-gpt", "claude gpt"],
        anahtar_sozcukler: &["claude", "gpt", "3p"],
        kova_onekleri: &["3p-"],
    },
    BilinenGrup {
        havuz: "agy_gemini",
        etiket: "Antigravity Gemini",
        adlar: &["gemini models", "gemini"],
        anahtar_sozcukler: &[],
        kova_onekleri: &["gemini-"],
    },
];

/// İç içe kota ilişkileri (veri): `(çocuk, ebeveyn, güven)`. Kanıt çelişkili (yerel ölçüm
/// karışık; üçüncü taraf izolasyon testi "yalnız Argon kovası düştü" diyor): güvenli, iyimser
/// olmayan varsayım Argon'u Gemini'nin içinde saymaktır. İlişki değişirse yalnız bu satır değişir.
const EBEVEYN_TABLOSU: &[(&str, &str, GuvenEtiketi)] =
    &[("agy_argon", "agy_gemini", GuvenEtiketi::Tahmini)];

fn bilinen_grup(havuz: &str) -> Option<&'static BilinenGrup> {
    BILINEN_GRUPLAR.iter().find(|g| g.havuz == havuz)
}

fn etiket_from_id(id: &str) -> String {
    bilinen_grup(id)
        .map(|g| g.etiket.to_string())
        .unwrap_or_else(|| "Antigravity".to_string())
}

/// `agy` ikilisinin kimliği: yol + boyut + değişim zamanı. İkili güncellenince askı kalkar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IkiliParmakIzi {
    pub yol: String,
    pub boyut: u64,
    pub degisim_sn: i64,
}

impl IkiliParmakIzi {
    pub fn al(yol: &Path) -> Option<Self> {
        let meta = std::fs::metadata(yol).ok()?;
        let degisim_sn = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Some(Self {
            yol: yol.to_string_lossy().into_owned(),
            boyut: meta.len(),
            degisim_sn,
        })
    }
}

/// Model turu askısı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgyAski {
    pub ikili: IkiliParmakIzi,
    pub baslangic: UtcZaman,
    pub bitis: UtcZaman,
    pub neden: String,
}

/// Süreçler arası paylaşılan kalıcı agy okuma durumu (`agy-durum.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgyKaliciDurum {
    /// Son bir saatteki `agy` çalıştırma anları (saatlik tavan servis ve CLI arasında ortak).
    #[serde(default)]
    pub cagrilar: Vec<UtcZaman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aski: Option<AgyAski>,
    /// Son başarılı okuma (servis yeniden başlarken hemen yeniden okumasın).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub son_basarili: Option<UtcZaman>,
    /// Son başarılı okumada görülen havuz kimlikleri.
    #[serde(default)]
    pub son_kimlikler: Vec<String>,
}

impl AgyKaliciDurum {
    fn yukle(yol: &Path) -> Option<Self> {
        let metin = std::fs::read_to_string(yol).ok()?;
        serde_json::from_str(&metin).ok()
    }
}

/// Kalıcı agy durumunu (yalnız okuma) diskten yükler; servis başlangıçta son başarılı okumayı
/// devralmak için kullanır.
pub fn agy_kalici_durum_oku(yol: &Path) -> Option<AgyKaliciDurum> {
    AgyKaliciDurum::yukle(yol)
}

/// Antigravity (`agy -p /usage`) kota bağlayıcısı.
/// Varsayılan olarak kapalıdır; `istek_uzerine` veya `otomatik` izin kapısıyla çalışır.
/// `~/.gemini/antigravity/daemon/hub_*.json` dosyalarına kesinlikle erişmez.
///
/// Havuz kümesi sabit değildir: sağlayıcı yeni bir grup çıkarınca `agy_<slug>` kimlikli havuz
/// kendiliğinden oluşur (bkz. [`agy_cikti_ayristir`]).
pub struct AntigravityBaglayicisi {
    ayar: AgyAyari,
    durum: Mutex<AgyKaliciDurum>,
    /// Kalıcı durum dosyası (`None`: yalnız bellek; testler).
    durum_yolu: Option<PathBuf>,
    /// Kayıttan (`kovalar.json`) tohumlanan ek havuz kimlikleri.
    tohum_kimlikler: Mutex<BTreeSet<String>>,
}

impl AntigravityBaglayicisi {
    pub fn yeni(ayar: AgyAyari) -> Self {
        Self {
            ayar,
            durum: Mutex::new(AgyKaliciDurum::default()),
            durum_yolu: None,
            tohum_kimlikler: Mutex::new(BTreeSet::new()),
        }
    }

    /// Durumu (saatlik çağrılar, askı, son başarılı okuma) `yol`da kalıcı tutan bağlayıcı.
    /// Aynı dosyayı kullanan servis ve CLI saatlik tavanı ve askıyı paylaşır.
    pub fn kalici(ayar: AgyAyari, yol: PathBuf) -> Self {
        let durum = AgyKaliciDurum::yukle(&yol).unwrap_or_default();
        Self {
            ayar,
            durum: Mutex::new(durum),
            durum_yolu: Some(yol),
            tohum_kimlikler: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn izin(&self) -> AgyOkumaIzni {
        self.ayar.okuma
    }

    /// Kayıtta (`kovalar.json`) bilinen agy havuz kimliklerini ekler (servis yeniden başlarken
    /// dinamik havuzlar hemen yoklanan kümesine girsin).
    pub fn kimlikleri_tohumla<I: IntoIterator<Item = String>>(&self, kimlikler: I) {
        if let Ok(mut g) = self.tohum_kimlikler.lock() {
            g.extend(kimlikler.into_iter().filter(|k| k.starts_with("agy_")));
        }
    }

    /// Son başarılı okuma anı (kalıcı durumdan).
    pub fn son_basarili(&self) -> Option<UtcZaman> {
        self.durum.lock().ok().and_then(|d| d.son_basarili)
    }

    /// Etkin model turu askısı (varsa).
    pub fn aski(&self) -> Option<AgyAski> {
        self.durum.lock().ok().and_then(|d| d.aski.clone())
    }

    /// Ayar yeniden yüklenince saatlik çağrı tavanı sıfırlanmasın diye eski bağlayıcının
    /// sayacını devralır (kota harcayan `agy` çalıştırması sınırsız tekrarlanmaz).
    pub fn sayaci_devral(&self, eski: &AntigravityBaglayicisi) {
        if std::ptr::eq(self, eski) {
            return;
        }
        let kopya = eski.durum.lock().map(|g| g.clone()).unwrap_or_default();
        if let Ok(mut g) = self.durum.lock() {
            let mut cagrilar = kopya.cagrilar;
            cagrilar.extend(g.cagrilar.iter().copied());
            cagrilar.sort();
            cagrilar.dedup();
            g.cagrilar = cagrilar;
            if g.aski.is_none() {
                g.aski = kopya.aski;
            }
            g.son_basarili = g.son_basarili.max(kopya.son_basarili);
            if g.son_kimlikler.is_empty() {
                g.son_kimlikler = kopya.son_kimlikler;
            }
        }
        let eski_tohum = eski
            .tohum_kimlikler
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        self.kimlikleri_tohumla(eski_tohum);
    }

    /// Bu bağlayıcının bildiği tüm agy havuz kimlikleri: son okuma + kayıt; hiçbiri yoksa
    /// bilinen üç varsayılan.
    pub fn bilinen_kimlikler(&self) -> Vec<String> {
        let mut kume: BTreeSet<String> = self
            .durum
            .lock()
            .map(|d| d.son_kimlikler.iter().cloned().collect())
            .unwrap_or_default();
        if let Ok(t) = self.tohum_kimlikler.lock() {
            kume.extend(t.iter().cloned());
        }
        if kume.is_empty() {
            kume.extend(AGY_VARSAYILAN_HAVUZLARI.iter().map(|s| (*s).to_string()));
        }
        kume.into_iter().collect()
    }

    fn durumu_kaydet(&self, durum: &AgyKaliciDurum) {
        if let Some(yol) = &self.durum_yolu
            && let Err(e) = atomik_yaz_json(yol, durum)
        {
            tracing::warn!("agy durum dosyası yazılamadı: {e}");
        }
    }

    /// Diskteki durumu (başka süreç yazmış olabilir) bellekle birleştirir.
    fn diskten_tazele(&self, g: &mut AgyKaliciDurum) {
        let Some(yol) = &self.durum_yolu else {
            return;
        };
        let Some(disk) = AgyKaliciDurum::yukle(yol) else {
            return;
        };
        g.cagrilar.extend(disk.cagrilar);
        g.cagrilar.sort();
        g.cagrilar.dedup();
        if disk.aski.is_some() && g.aski.is_none() {
            g.aski = disk.aski;
        }
        g.son_basarili = g.son_basarili.max(disk.son_basarili);
        if g.son_kimlikler.is_empty() {
            g.son_kimlikler = disk.son_kimlikler;
        }
    }

    /// İzin kapısını, model turu askısını ve saatlik tavanı kontrol ederek `agy -p /usage`
    /// okuması yapar.
    pub async fn istekle_veya_zamanli_oku(
        &self,
        simdi: UtcZaman,
        istek_uzerine_mi: bool,
    ) -> Vec<BaglayiciSonucu> {
        match self.ayar.okuma {
            AgyOkumaIzni::Kapali => {
                return self.kapali_sonuclar(
                    "agy okuması varsayılan olarak kapalı (ayarlar.toml: [agy].okuma)",
                );
            }
            AgyOkumaIzni::IstekUzerine if !istek_uzerine_mi => {
                return self.kapali_sonuclar("agy yalnızca istek üzerine okunacak şekilde ayarlı");
            }
            _ => {}
        }

        // İkili önce çözülür: kurulu değilse saatlik kota harcanmaz.
        let Some(ikili) = self.ikili_yolu_coz() else {
            return self.baglanmadi_sonuclari("agy CLI kurulumu bulunamadı");
        };
        let parmak_izi = IkiliParmakIzi::al(&ikili);

        if let Some(aski) = self.aski_kontrol(simdi, parmak_izi.as_ref()) {
            let not = format!("{AGY_MODEL_TURU_NOTU} (bitiş {})", aski.bitis.to_iso_z());
            return self.hata_sonuclari(AGY_MODEL_TURU_HATA, &not, false, None);
        }

        if !self.saatlik_kotayi_ayir(simdi) {
            return self.hata_sonuclari(
                "saatlik_sinir",
                "agy saatlik okuma sınırı aşıldı",
                false,
                None,
            );
        }

        let asim = Duration::from_secs(self.ayar.zaman_asimi_sn.clamp(45, 120));
        match agy_surec_oku(&ikili, asim).await {
            Ok((cikti, cikis_basarili)) => {
                if agy_model_turu_mu(&cikti) {
                    self.askiya_al(simdi, parmak_izi);
                    tracing::warn!("{AGY_MODEL_TURU_NOTU}");
                    return self.hata_sonuclari(
                        AGY_MODEL_TURU_HATA,
                        AGY_MODEL_TURU_NOTU,
                        false,
                        None,
                    );
                }
                if !cikis_basarili {
                    return self.hata_sonuclari(
                        "cikis_hatasi",
                        "agy /usage hata koduyla çıktı",
                        true,
                        Some(AGY_CIKIS_HATASI_BEKLEME_SN),
                    );
                }
                let havuzlar = agy_cikti_ayristir(&cikti, simdi);
                let sema_hatasi = havuzlar.values().all(|h| h.durum == Durum::Hata);
                if sema_hatasi {
                    return havuzlar
                        .into_iter()
                        .map(|(havuz_id, havuz)| BaglayiciSonucu {
                            havuz_id,
                            havuz,
                            retry_after_sn: Some(AGY_SEMA_HATASI_BEKLEME_SN),
                            gecici_hata: true,
                        })
                        .collect();
                }
                self.basariyi_kaydet(simdi, havuzlar.keys().cloned().collect());
                havuzlar
                    .into_iter()
                    .map(|(havuz_id, havuz)| {
                        let gecici_hata = havuz.durum == Durum::Hata;
                        BaglayiciSonucu {
                            havuz_id,
                            havuz,
                            retry_after_sn: None,
                            gecici_hata,
                        }
                    })
                    .collect()
            }
            Err(err_tur) => {
                let bekleme = (err_tur == "zaman_asimi").then_some(AGY_SEMA_HATASI_BEKLEME_SN);
                self.hata_sonuclari(err_tur, "agy /usage okunamadı", true, bekleme)
            }
        }
    }

    /// Etkin askıyı döndürür; ikili değiştiyse ya da süre dolduysa askıyı kaldırır.
    fn aski_kontrol(
        &self,
        simdi: UtcZaman,
        parmak_izi: Option<&IkiliParmakIzi>,
    ) -> Option<AgyAski> {
        let mut g = self.durum.lock().ok()?;
        self.diskten_tazele(&mut g);
        let aski = g.aski.clone()?;
        let ayni_ikili = parmak_izi.is_some_and(|p| *p == aski.ikili);
        if ayni_ikili && simdi < aski.bitis {
            return Some(aski);
        }
        g.aski = None;
        let kopya = g.clone();
        drop(g);
        self.durumu_kaydet(&kopya);
        if !ayni_ikili {
            tracing::info!("agy ikilisi değişti; model turu askısı kaldırıldı");
        }
        None
    }

    fn askiya_al(&self, simdi: UtcZaman, parmak_izi: Option<IkiliParmakIzi>) {
        let Ok(mut g) = self.durum.lock() else {
            return;
        };
        g.aski = Some(AgyAski {
            ikili: parmak_izi.unwrap_or(IkiliParmakIzi {
                yol: String::new(),
                boyut: 0,
                degisim_sn: 0,
            }),
            baslangic: simdi,
            bitis: simdi.ekle_sn(AGY_MODEL_TURU_ASKI_SN),
            neden: AGY_MODEL_TURU_HATA.to_string(),
        });
        let kopya = g.clone();
        drop(g);
        self.durumu_kaydet(&kopya);
    }

    fn basariyi_kaydet(&self, simdi: UtcZaman, kimlikler: Vec<String>) {
        let Ok(mut g) = self.durum.lock() else {
            return;
        };
        g.son_basarili = Some(simdi);
        g.son_kimlikler = kimlikler;
        let kopya = g.clone();
        drop(g);
        self.durumu_kaydet(&kopya);
    }

    fn ikili_yolu_coz(&self) -> Option<PathBuf> {
        if let Some(ozel) = self.ayar.ikili_yol.as_deref() {
            let p = PathBuf::from(ozel.trim());
            if p.is_file() {
                return Some(p);
            }
        }
        agy_ikili_coz()
    }

    fn saatlik_kotayi_ayir(&self, simdi: UtcZaman) -> bool {
        let tavan = self.ayar.saatlik_tavan.clamp(1, 12) as usize;
        let bir_saat_once = simdi.ekle_sn(-3600);
        let Ok(mut guard) = self.durum.lock() else {
            return false;
        };
        self.diskten_tazele(&mut guard);
        guard.cagrilar.retain(|t| *t > bir_saat_once);
        if guard.cagrilar.len() >= tavan {
            return false;
        }
        guard.cagrilar.push(simdi);
        let kopya = guard.clone();
        drop(guard);
        self.durumu_kaydet(&kopya);
        true
    }

    fn kapali_sonuclar(&self, not: &str) -> Vec<BaglayiciSonucu> {
        self.bilinen_kimlikler()
            .into_iter()
            .map(|id| BaglayiciSonucu {
                havuz: agy_hata_havuzu(
                    &etiket_from_id(&id),
                    Durum::Kapali,
                    "kapali",
                    None,
                    vec![not.to_string()],
                ),
                havuz_id: id,
                retry_after_sn: None,
                gecici_hata: false,
            })
            .collect()
    }

    fn baglanmadi_sonuclari(&self, not: &str) -> Vec<BaglayiciSonucu> {
        self.bilinen_kimlikler()
            .into_iter()
            .map(|id| BaglayiciSonucu {
                havuz: agy_hata_havuzu(
                    &etiket_from_id(&id),
                    Durum::Baglanmadi,
                    "ikili_bulunamadi",
                    None,
                    vec![not.to_string()],
                ),
                havuz_id: id,
                retry_after_sn: None,
                gecici_hata: false,
            })
            .collect()
    }

    fn hata_sonuclari(
        &self,
        tur: &str,
        not: &str,
        gecici_hata: bool,
        retry_after_sn: Option<u64>,
    ) -> Vec<BaglayiciSonucu> {
        self.bilinen_kimlikler()
            .into_iter()
            .map(|id| {
                let mut s = hata_sonucu(&id, &etiket_from_id(&id), tur, None, not, gecici_hata);
                s.retry_after_sn = retry_after_sn;
                s
            })
            .collect()
    }
}

impl Baglayici for AntigravityBaglayicisi {
    fn havuz_id(&self) -> &str {
        "agy_gemini"
    }

    fn havuz_kimlikleri(&self) -> Vec<String> {
        self.bilinen_kimlikler()
    }

    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>> {
        Box::pin(async move {
            let mut liste = self.oku_coklu(simdi).await;
            if liste.is_empty() {
                hata_sonucu(
                    "agy_gemini",
                    "Antigravity Gemini",
                    "bos_sonuc",
                    None,
                    "agy boş sonuç döndürdü",
                    true,
                )
            } else {
                liste.remove(0)
            }
        })
    }

    fn oku_coklu<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = Vec<BaglayiciSonucu>> + Send + 'a>> {
        Box::pin(async move { self.istekle_veya_zamanli_oku(simdi, false).await })
    }
}

/// `PATH` ve bilinen kullanıcı dışı kurulum yollarından `agy` ikilisini çözer (`~/.gemini` okunmaz).
pub fn agy_ikili_coz() -> Option<PathBuf> {
    let aday_adlar = ["agy.exe", "agy.cmd", "agy.bat", "agy"];
    if let Some(path_var) = std::env::var_os("PATH") {
        for dizin in std::env::split_paths(&path_var) {
            for ad in aday_adlar {
                let aday = dizin.join(ad);
                if aday.is_file() {
                    return Some(aday);
                }
            }
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let npm_dizin = PathBuf::from(appdata).join("npm");
        for ad in ["agy.cmd", "agy.exe"] {
            let aday = npm_dizin.join(ad);
            if aday.is_file() {
                return Some(aday);
            }
        }
    }
    None
}

/// `agy -p /usage --output-format json --print-timeout 40s` komutunu MSYS olmadan doğrudan çalıştırır.
///
/// Süreç askıda başlatılır, Job Object'e eklenir, sonra devam ettirilir (erken doğan torunlar da
/// Job içinde kalır); Job kurulamazsa süreç hiç çalıştırılmaz. Çıktı, çıkış kodu başarısız olsa
/// da döner (`(çıktı, başarılı_mı)`): model turu izi hata çıkışında da aranır.
pub async fn agy_surec_oku(
    ikili_yol: &Path,
    zaman_asimi: Duration,
) -> Result<(String, bool), &'static str> {
    let cmd_uzantisi = ikili_yol
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));

    let mut cmd = if cmd_uzantisi {
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg(ikili_yol);
        c
    } else {
        Command::new(ikili_yol)
    };

    cmd.args([
        "-p",
        "/usage",
        "--output-format",
        "json",
        "--print-timeout",
        "40s",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);

    let job = JobSuresKorumasi::yeni().map_err(|_| "job_kurulamadi")?;
    let mut child = cmd.spawn().map_err(|_| "surec_baslatilamadi")?;
    let eklendi = child
        .raw_handle()
        .is_some_and(|h| job.sureci_ekle(h).is_ok());
    let devam = eklendi
        && child
            .id()
            .is_some_and(|pid| askidaki_sureci_devam_ettir(pid).is_ok());
    if !devam {
        let _ = child.kill().await;
        return Err("job_kurulamadi");
    }

    let cikti_sonucu = tokio::time::timeout(zaman_asimi, async {
        let mut stdout = child.stdout.take().ok_or("stdout_yok")?;
        let mut buf = Vec::new();
        use tokio::io::AsyncReadExt;
        stdout
            .read_to_end(&mut buf)
            .await
            .map_err(|_| "okuma_hatasi")?;
        let durum = child.wait().await.map_err(|_| "bekleme_hatasi")?;
        let metin = String::from_utf8(buf).map_err(|_| "utf8_hatasi")?;
        Ok::<_, &'static str>((metin, durum.success()))
    })
    .await;

    // Başarıda da Job kapatılır: `agy`nin bıraktığı arka plan süreçleri Cetvel adına
    // yaşamaya (ve kota harcamaya) devam etmesin.
    job.sonlandir();
    drop(job);
    let _ = child.kill().await;

    match cikti_sonucu {
        Ok(ic) => ic,
        Err(_) => Err("zaman_asimi"),
    }
}

/// `agy -p /usage` slash komutu yerine model turu açtıysa `true`: JSON çıktısında `command`
/// yok ve (`usage.input_tokens > 0` ya da `conversation_id` dolu). agy 1.5.13'te görüldü
/// (~37K token yakar); `--disable-slash-commands` eklenmez.
pub fn agy_model_turu_mu(cikti: &str) -> bool {
    let Some(kok) = json_degeri_bul(cikti) else {
        return false;
    };
    let Some(nesne) = kok.as_object() else {
        return false;
    };
    if nesne.contains_key("command") {
        return false;
    }
    let girdi_belirteci = nesne
        .get("usage")
        .and_then(|u| u.get("input_tokens"))
        .and_then(Value::as_f64)
        .is_some_and(|n| n > 0.0);
    let sohbet = nesne
        .get("conversation_id")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    girdi_belirteci || sohbet
}

fn json_degeri_bul(cikti: &str) -> Option<Value> {
    serde_json::from_str::<Value>(cikti.trim())
        .ok()
        .or_else(|| {
            cikti
                .lines()
                .rev()
                .find_map(|s| serde_json::from_str::<Value>(s.trim()).ok())
        })
}

#[derive(Debug, Deserialize)]
struct HamAgyRoot {
    command: Option<HamAgyCommand>,
    data: Option<HamAgyData>,
    #[serde(default)]
    groups: Vec<HamAgyGroup>,
}

#[derive(Debug, Deserialize)]
struct HamAgyCommand {
    data: Option<HamAgyData>,
}

#[derive(Debug, Deserialize)]
struct HamAgyData {
    #[serde(default)]
    groups: Vec<HamAgyGroup>,
}

#[derive(Debug, Deserialize)]
struct HamAgyGroup {
    #[serde(default)]
    id: Option<Value>,
    #[serde(
        default,
        alias = "display_name",
        alias = "displayName",
        alias = "label"
    )]
    name: Option<Value>,
    #[serde(default)]
    buckets: Vec<HamAgyBucket>,
}

#[derive(Debug, Deserialize)]
struct HamAgyBucket {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    window: Option<Value>,
    #[serde(default, alias = "windowMinutes", alias = "window_duration_mins")]
    window_minutes: Option<u64>,
    remaining_fraction: Option<f64>,
    used_fraction: Option<f64>,
    remaining_percent: Option<f64>,
    reset_time: Option<Value>,
}

fn metin(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Ayrıştırma sırasında biriken havuz taslağı.
struct HavuzTaslagi {
    etiket: String,
    grup: Option<String>,
    dinamik: bool,
    pencereler: BTreeMap<String, PencereOlcum>,
    notlar: Vec<String>,
    /// Kova kimliklerinin ortak öneki (`gemini-`, `gemini-argon-wtu-`); ebeveyn kuralı için.
    kova_onegi: Option<String>,
}

/// Grup → havuz eşlemesi sonucu.
struct GrupEslesmesi {
    havuz_id: String,
    etiket: String,
    dinamik: bool,
}

fn kucuk(s: Option<&str>) -> Option<String> {
    s.map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty())
}

/// Bilinen gruba tam ad/kimlik ya da eski anahtar sözcükle eşler (kova kimliği kullanmadan).
fn bilinen_grup_adla(ad: Option<&str>, id: Option<&str>) -> Option<&'static BilinenGrup> {
    let adaylar: Vec<String> = [kucuk(ad), kucuk(id)].into_iter().flatten().collect();
    if adaylar.is_empty() {
        return None;
    }
    BILINEN_GRUPLAR
        .iter()
        .find(|g| adaylar.iter().any(|a| g.adlar.contains(&a.as_str())))
        .or_else(|| {
            BILINEN_GRUPLAR.iter().find(|g| {
                adaylar
                    .iter()
                    .any(|a| g.anahtar_sozcukler.iter().any(|k| a.contains(k)))
            })
        })
}

/// Kova kimliği `önek + pencere` (`gemini-5h`, `gemini-argon-wtu-weekly`) biçimindeyse bilinen
/// grubu döndürür. `gemini-xenon-5h` gibi önekten sonra başka parça taşıyan kimlik eşleşmez.
fn bilinen_grup_kovayla(kova_kimlikleri: &[String]) -> Option<&'static BilinenGrup> {
    BILINEN_GRUPLAR.iter().find(|g| {
        kova_kimlikleri.iter().any(|k| {
            let k = k.to_lowercase();
            g.kova_onekleri.iter().any(|onek| {
                k.strip_prefix(onek).is_some_and(|kalan| {
                    !kalan.is_empty() && !kalan.contains('-') && pencere_anahtari(kalan, None).1 > 0
                })
            })
        })
    })
}

fn grup_esle(ad: Option<&str>, id: Option<&str>, kova_kimlikleri: &[String]) -> GrupEslesmesi {
    let tam = BILINEN_GRUPLAR.iter().find(|g| {
        [kucuk(ad), kucuk(id)]
            .into_iter()
            .flatten()
            .any(|a| g.adlar.contains(&a.as_str()))
    });
    if let Some(g) = tam
        .or_else(|| bilinen_grup_kovayla(kova_kimlikleri))
        .or_else(|| bilinen_grup_adla(ad, id))
    {
        return GrupEslesmesi {
            havuz_id: g.havuz.to_string(),
            etiket: g.etiket.to_string(),
            dinamik: false,
        };
    }
    // Bilinmeyen grup atılmaz: `agy_<slug(grup id ya da ad)>` kimlikli yeni havuz.
    let kaynak = id
        .or(ad)
        .map(str::to_string)
        .or_else(|| {
            kova_kimlikleri.first().map(|k| {
                kova_onegi(std::slice::from_ref(k))
                    .map(|o| o.trim_end_matches('-').to_string())
                    .unwrap_or_else(|| k.clone())
            })
        })
        .unwrap_or_else(|| "diger".to_string());
    let gorunen = ad.or(id).unwrap_or(&kaynak).to_string();
    GrupEslesmesi {
        havuz_id: dinamik_kimlik(&kaynak),
        etiket: format!("Antigravity {gorunen}"),
        dinamik: true,
    }
}

/// Kova kimliklerinin ortak öneki, son `-` dahil (`["gemini-5h","gemini-weekly"]` → `gemini-`).
/// Dinamik havuz kimliği `agy_<slug>`; masaüstü kart adı sınırı (40) içinde kalır.
fn dinamik_kimlik(kaynak: &str) -> String {
    let mut govde = slug(kaynak);
    govde.truncate(36);
    format!("agy_{}", govde.trim_end_matches('_'))
}

fn kova_onegi(kimlikler: &[String]) -> Option<String> {
    let ilk = kimlikler.first()?.to_lowercase();
    let mut ortak = ilk.clone();
    for k in &kimlikler[1..] {
        let k = k.to_lowercase();
        let n = ortak
            .chars()
            .zip(k.chars())
            .take_while(|(a, b)| a == b)
            .count();
        ortak = ortak.chars().take(n).collect();
    }
    if kimlikler.len() == 1 {
        // Tek kova: pencere parçası (son `-` sonrası) önek sayılmaz.
        ortak = ilk;
    }
    let son = ortak.rfind('-')?;
    let onek = &ortak[..=son];
    (onek.len() > 1).then(|| onek.to_string())
}

/// Kovanın pencere anahtarını, süresini ve ham etiketini `window` (yoksa kova kimliğinin son
/// parçası) alanından [`pencere_anahtari`] ile türetir; sabit "Argon yalnız haftalık" varsayımı yok.
fn kova_penceresi(kova: &HamAgyBucket) -> (String, u64, Option<String>) {
    let pencere = metin(kova.window.as_ref());
    let id = metin(kova.id.as_ref());
    if let Some(dk) = kova.window_minutes.filter(|d| *d > 0) {
        let (ad, dk) = pencere_anahtari(pencere.as_deref().unwrap_or(""), Some(dk));
        return (ad, dk, pencere.or(id));
    }
    if let Some(w) = pencere.as_deref() {
        let (ad, dk) = pencere_anahtari(w, None);
        if dk > 0 {
            return (ad, dk, pencere);
        }
    }
    if let Some(k) = id.as_deref() {
        let son_parca = k.rsplit('-').next().unwrap_or(k);
        for aday in [son_parca, k] {
            let (ad, dk) = pencere_anahtari(aday, None);
            if dk > 0 {
                return (ad, dk, pencere.or_else(|| id.clone()));
            }
        }
    }
    let ham = pencere.clone().or(id.clone()).unwrap_or_default();
    let (ad, dk) = pencere_anahtari(&ham, None);
    (ad, dk, pencere.or(id))
}

/// Kovanın kalan oranı. `> 1` gelen kesir 0-100 ölçeği sayılır (not düşülür); aralık dışı değer
/// geçersizdir (sessizce kıstırılmaz).
fn kova_kalan_orani(kova: &HamAgyBucket) -> Result<(f64, Option<&'static str>), ()> {
    let kesir = |v: f64| -> Result<(f64, Option<&'static str>), ()> {
        if !v.is_finite() || v < 0.0 {
            Err(())
        } else if v <= 1.0 {
            Ok((v, None))
        } else if v <= 100.0 {
            Ok((v / 100.0, Some("oran 0-100 ölçeğinde geldi, yüzde sayıldı")))
        } else {
            Err(())
        }
    };
    if let Some(rem) = kova.remaining_fraction {
        return kesir(rem);
    }
    if let Some(used) = kova.used_fraction {
        return kesir(used).map(|(u, n)| ((1.0 - u).clamp(0.0, 1.0), n));
    }
    if let Some(pct) = kova.remaining_percent {
        if pct.is_finite() && (0.0..=100.0).contains(&pct) {
            return Ok((pct / 100.0, None));
        }
        return Err(());
    }
    Err(())
}

/// `agy -p /usage` JSON çıktısını (veya yedek metin tablosunu) Cetvel havuzlarına ayrıştırır
///. Bilinen gruplar eski kimliklere (`agy_gemini`, `agy_claude_gpt`,
/// `agy_argon`) eşlenir; bilinmeyen grup `agy_<slug>` kimlikli dinamik havuz olur.
pub fn agy_cikti_ayristir(cikti: &str, simdi: UtcZaman) -> BTreeMap<String, HavuzAnlik> {
    if agy_model_turu_mu(cikti) {
        let mut hata_map = BTreeMap::new();
        for id in AGY_VARSAYILAN_HAVUZLARI {
            hata_map.insert(
                id.to_string(),
                agy_hata_havuzu(
                    &etiket_from_id(id),
                    Durum::Hata,
                    AGY_MODEL_TURU_HATA,
                    None,
                    vec![AGY_MODEL_TURU_NOTU.into()],
                ),
            );
        }
        return hata_map;
    }
    if let Some(havuzlar) = agy_json_ayristir(cikti, simdi) {
        return havuzlar;
    }
    if let Some(havuzlar) = agy_tablo_ayristir(cikti, simdi) {
        return havuzlar;
    }

    let mut hata_map = BTreeMap::new();
    hata_map.insert(
        "agy_gemini".to_string(),
        agy_hata_havuzu(
            "Antigravity Gemini",
            Durum::Hata,
            "sema_degisti",
            None,
            vec!["agy /usage çıktısı ayrıştırılamadı".into()],
        ),
    );
    hata_map
}

fn agy_json_ayristir(cikti: &str, simdi: UtcZaman) -> Option<BTreeMap<String, HavuzAnlik>> {
    let json_deger = json_degeri_bul(cikti)?;
    let root: HamAgyRoot = serde_json::from_value(json_deger).ok()?;
    let mut gruplar = Vec::new();
    if let Some(cmd) = root.command
        && let Some(d) = cmd.data
    {
        gruplar.extend(d.groups);
    }
    if let Some(d) = root.data {
        gruplar.extend(d.groups);
    }
    gruplar.extend(root.groups);

    if gruplar.is_empty() {
        return None;
    }

    let mut taslaklar: BTreeMap<String, HavuzTaslagi> = BTreeMap::new();
    for grup in gruplar {
        let grup_ad = metin(grup.name.as_ref());
        let grup_id = metin(grup.id.as_ref());
        let kova_kimlikleri: Vec<String> = grup
            .buckets
            .iter()
            .filter_map(|k| metin(k.id.as_ref()))
            .collect();
        let eslesme = grup_esle(grup_ad.as_deref(), grup_id.as_deref(), &kova_kimlikleri);
        let taslak = taslaklar
            .entry(eslesme.havuz_id.clone())
            .or_insert_with(|| HavuzTaslagi {
                etiket: eslesme.etiket.clone(),
                grup: grup_ad.clone().or(grup_id.clone()),
                dinamik: eslesme.dinamik,
                pencereler: BTreeMap::new(),
                notlar: Vec::new(),
                kova_onegi: kova_onegi(&kova_kimlikleri),
            });
        for kova in grup.buckets {
            let kova_ad = metin(kova.id.as_ref()).unwrap_or_else(|| "?".to_string());
            let (kalan, olcek_notu) = match kova_kalan_orani(&kova) {
                Ok(v) => v,
                Err(()) => {
                    taslak.notlar.push(format!(
                        "kova {kova_ad}: geçersiz ya da eksik oran, atlandı"
                    ));
                    continue;
                }
            };
            if let Some(n) = olcek_notu {
                taslak.notlar.push(format!("kova {kova_ad}: {n}"));
            }
            let (pencere_ad, uzunluk_dk, etiket) = kova_penceresi(&kova);
            let sifirlama = kova.reset_time.as_ref().and_then(zaman_degeri_coz);
            let olcum = PencereOlcum {
                uzunluk_dk,
                kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
                kalan,
                sifirlama,
                etiket,
            };
            // Oynaklık nedeniyle aynı pencere için en kısıtlayıcı (en düşük kalan) değer alınır;
            // farklı süreli pencere aynı ada düşerse ezilmez.
            cetvel_model::pencere_cakismasiz_ekle(&mut taslak.pencereler, pencere_ad, olcum);
        }
    }

    taslaklar.retain(|_, t| !t.pencereler.is_empty());
    if taslaklar.is_empty() {
        return None;
    }

    Some(havuzlari_insa_et(taslaklar, simdi))
}

/// Tablo satırının grup etiketi: ilk hücre (boru/sekme) ya da ilk rakam/`%` öncesi metin.
fn satir_etiketi(satir: &str, hucreler: &[&str]) -> Option<String> {
    let aday = if let Some(ilk) = hucreler.first() {
        (*ilk).to_string()
    } else if satir.contains('\t') {
        satir.split('\t').next().unwrap_or_default().to_string()
    } else {
        satir
            .split(|c: char| c.is_ascii_digit() || c == '%' || c == '|')
            .next()
            .unwrap_or_default()
            .to_string()
    };
    let temiz = aday.trim().to_string();
    temiz.chars().any(char::is_alphabetic).then_some(temiz)
}

/// `used`/`kullanılan` kelimesi (alt dize değil): `unused` ya da `kullanılabilir` eşleşmez.
fn kullanilan_kelimesi_var(alt: &str) -> bool {
    alt.split(|c: char| !c.is_alphanumeric()).any(|k| {
        matches!(
            k,
            "used" | "usage" | "kullanilan" | "kullanılan" | "kullanim" | "kullanım"
        )
    })
}

fn tablo_olcumu(
    yuzde: f64,
    used_mi: bool,
    uzunluk_dk: u64,
    sifirlama: Option<UtcZaman>,
    etiket: Option<String>,
) -> PencereOlcum {
    let kalan = yuzdeyi_kalana_cevir(yuzde, used_mi);
    PencereOlcum {
        uzunluk_dk,
        kullanilan: (1.0 - kalan).clamp(0.0, 1.0),
        kalan,
        sifirlama,
        etiket,
    }
}

/// JSON yerine düz tablo metni (`Gemini ... 5s ...`, `Claude-GPT ...`, `Argon ...`) gelirse
/// ayrıştırır. Satır etiketi bilinen gruba eşlenmezse ve satırda tanınan bir pencere varsa
/// dinamik havuz oluşur.
fn agy_tablo_ayristir(cikti: &str, simdi: UtcZaman) -> Option<BTreeMap<String, HavuzAnlik>> {
    let mut taslaklar: BTreeMap<String, HavuzTaslagi> = BTreeMap::new();
    // Başlık satırından çıkarılan pencere sütunları: (sütun, anahtar, dakika, ham etiket).
    let mut sutunlar: Vec<(usize, String, u64, String)> = Vec::new();
    let mut tablo_used_mi = false;

    for satir in cikti.lines() {
        let alt = satir.to_lowercase();
        let hucreler: Vec<&str> = if satir.contains('|') {
            satir
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect()
        } else {
            Vec::new()
        };
        let yuzdeler = satirdan_yuzdeleri_cikar(satir);

        if yuzdeler.is_empty() {
            // Yalnız harf içeren başlık satırı sütunları günceller; Markdown ayırıcı satırı
            // (`|---|:--:|`) başlığın `used` bayrağını ve sütun konumlarını silmesin.
            let baslik_satiri = satir.chars().any(char::is_alphabetic);
            if hucreler.len() >= 2 && baslik_satiri {
                sutunlar = hucreler
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter_map(|(i, h)| {
                        let (ad, dk) = pencere_anahtari(h, None);
                        (dk > 0).then(|| (i, ad, dk, (*h).to_string()))
                    })
                    .collect();
                tablo_used_mi = kullanilan_kelimesi_var(&alt);
            }
            continue;
        }

        let Some(etiket) = satir_etiketi(satir, &hucreler) else {
            continue;
        };
        let used_mi = tablo_used_mi || kullanilan_kelimesi_var(&alt);
        let sifirlama = satirdan_sifirlama_cikar(satir);
        let mut yeni: Vec<(String, PencereOlcum)> = Vec::new();

        if !hucreler.is_empty() && !sutunlar.is_empty() {
            for (idx, ad, dk, ham) in &sutunlar {
                if let Some(hucre) = hucreler.get(*idx)
                    && let Some(&yuzde) = satirdan_yuzdeleri_cikar(hucre).first()
                {
                    yeni.push((
                        ad.clone(),
                        tablo_olcumu(yuzde, used_mi, *dk, None, Some(ham.clone())),
                    ));
                }
            }
        }

        if yeni.is_empty() {
            // Sekme/boru hücrelerinden ilk pencere hücresi (`Five Hour Limit Remaining`).
            let ek_hucreler: Vec<&str> = if hucreler.is_empty() {
                satir.split('\t').skip(1).collect()
            } else {
                hucreler.iter().skip(1).copied().collect()
            };
            let pencere_hucresi = ek_hucreler
                .iter()
                .filter(|h| satirdan_yuzdeleri_cikar(h).is_empty())
                .find_map(|h| {
                    let (ad, dk) = pencere_anahtari(h, None);
                    (dk > 0).then(|| (ad, dk, h.trim().to_string()))
                });
            if let Some((ad, dk, ham)) = pencere_hucresi {
                yeni.push((
                    ad,
                    tablo_olcumu(yuzdeler[0], used_mi, dk, sifirlama, Some(ham)),
                ));
            }
        }

        if yeni.is_empty() {
            // Eski serbest metin: `5s`/`hafta` anahtar sözcükleri ve yüzde sırası.
            let has_5s = bes_saatlik_baslik_mi(&alt);
            let has_hafta = haftalik_baslik_mi(&alt);
            if has_5s {
                yeni.push((
                    "5s".into(),
                    tablo_olcumu(yuzdeler[0], used_mi, 300, sifirlama, None),
                ));
            }
            if has_hafta {
                let idx = if has_5s && yuzdeler.len() >= 2 { 1 } else { 0 };
                yeni.push((
                    "hafta".into(),
                    tablo_olcumu(yuzdeler[idx], used_mi, 10080, sifirlama, None),
                ));
            }
        }

        let eslesme = bilinen_grup_adla(Some(&etiket), None);
        if yeni.is_empty() {
            // Bilinen grupta pencere belirtilmemiş satır: eski yerleşik kural.
            if eslesme.is_none() {
                continue;
            }
            if yuzdeler.len() >= 2 {
                yeni.push((
                    "5s".into(),
                    tablo_olcumu(yuzdeler[0], used_mi, 300, None, None),
                ));
                yeni.push((
                    "hafta".into(),
                    tablo_olcumu(yuzdeler[1], used_mi, 10080, None, None),
                ));
            } else {
                yeni.push((
                    "hafta".into(),
                    tablo_olcumu(yuzdeler[0], used_mi, 10080, None, None),
                ));
            }
        }

        let (havuz_id, gorunen, dinamik) = match eslesme {
            Some(g) => (g.havuz.to_string(), g.etiket.to_string(), false),
            None => (
                dinamik_kimlik(&etiket),
                format!("Antigravity {etiket}"),
                true,
            ),
        };
        let taslak = taslaklar.entry(havuz_id).or_insert_with(|| HavuzTaslagi {
            etiket: gorunen,
            grup: Some(etiket.clone()),
            dinamik,
            pencereler: BTreeMap::new(),
            notlar: Vec::new(),
            kova_onegi: None,
        });
        for (ad, olcum) in yeni {
            // Aynı satır grubunda sonraki satır aynı pencereyi yenilerse son değer geçerli.
            taslak.pencereler.insert(ad, olcum);
        }
    }

    taslaklar.retain(|_, t| !t.pencereler.is_empty());
    if taslaklar.is_empty() {
        None
    } else {
        Some(havuzlari_insa_et(taslaklar, simdi))
    }
}

fn bes_saatlik_baslik_mi(alt: &str) -> bool {
    alt.contains("5s")
        || alt.contains("5h")
        || alt.contains("5 saat")
        || alt.contains("five hour")
        || alt.contains("five-hour")
}

fn haftalik_baslik_mi(alt: &str) -> bool {
    alt.contains("hafta") || alt.contains("week") || alt.contains("7d")
}

fn satirdan_sifirlama_cikar(satir: &str) -> Option<UtcZaman> {
    satir
        .split(|c: char| c.is_whitespace() || c == '|')
        .find_map(|tok| UtcZaman::parse_tarih_veya_zaman(tok).ok())
}

fn satirdan_yuzdeleri_cikar(satir: &str) -> Vec<f64> {
    let mut sonuclar = Vec::new();
    let bayt = satir.as_bytes();
    let mut i = 0usize;
    while i < bayt.len() {
        if bayt[i] == b'%' {
            // Önce `%85` biçimi
            let mut j = i + 1;
            while j < bayt.len() && (bayt[j].is_ascii_digit() || bayt[j] == b'.') {
                j += 1;
            }
            if j > i + 1
                && let Ok(v) = satir[i + 1..j].parse::<f64>()
                && (0.0..=100.0).contains(&v)
            {
                sonuclar.push(v);
                i = j;
                continue;
            }
            // Sonra `85%` biçimi
            let mut k = i;
            while k > 0 && (bayt[k - 1].is_ascii_digit() || bayt[k - 1] == b'.') {
                k -= 1;
            }
            if k < i
                && let Ok(v) = satir[k..i].parse::<f64>()
                && (0.0..=100.0).contains(&v)
            {
                sonuclar.push(v);
            }
        }
        i += 1;
    }
    sonuclar
}

fn yuzdeyi_kalana_cevir(yuzde: f64, used_mi: bool) -> f64 {
    let oran = (yuzde / 100.0).clamp(0.0, 1.0);
    if used_mi {
        (1.0 - oran).clamp(0.0, 1.0)
    } else {
        oran
    }
}

fn zaman_degeri_coz(val: &Value) -> Option<UtcZaman> {
    match val {
        Value::String(s) => UtcZaman::parse_tarih_veya_zaman(s).ok(),
        Value::Number(n) => n.as_i64().and_then(UtcZaman::from_unix_timestamp),
        _ => None,
    }
}

/// Taslaklardan havuzları kurar; keşif bilgisini (dinamik, grup, ebeveyn) ve iç içe kota
/// zincirini doldurur.
fn havuzlari_insa_et(
    taslaklar: BTreeMap<String, HavuzTaslagi>,
    simdi: UtcZaman,
) -> BTreeMap<String, HavuzAnlik> {
    // Ebeveyn: önce veri tablosu, sonra genel önek kuralı (bir grubun kova öneki başka bir
    // grubun önekini taşıyorsa, en uzun eşleşen önek ebeveyndir; güven tahmini).
    let onekler: Vec<(String, String)> = taslaklar
        .iter()
        .filter_map(|(id, t)| t.kova_onegi.clone().map(|o| (id.clone(), o)))
        .collect();
    let ebeveyn_bul = |id: &str, onek: Option<&str>| -> Option<(String, GuvenEtiketi)> {
        if let Some((_, e, g)) = EBEVEYN_TABLOSU.iter().find(|(c, _, _)| *c == id) {
            return Some(((*e).to_string(), *g));
        }
        let onek = onek?;
        onekler
            .iter()
            .filter(|(digeri, o)| {
                digeri != id && onek.starts_with(o.as_str()) && onek.len() > o.len()
            })
            .max_by_key(|(_, o)| o.len())
            .map(|(e, _)| (e.clone(), GuvenEtiketi::Tahmini))
    };

    let mut sonuc = BTreeMap::new();
    for (havuz_id, taslak) in &taslaklar {
        let ebeveyn = ebeveyn_bul(havuz_id, taslak.kova_onegi.as_deref());
        let mut notlar = vec!["tek okuma (±%1 oynaklık olabilir)".to_string()];
        notlar.extend(taslak.notlar.iter().cloned());
        let mut havuz = HavuzAnlik {
            platform: "agy".to_string(),
            etiket: taslak.etiket.clone(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::AgyUsage,
                guven: GuvenEtiketi::Tahmini,
                okuma: simdi,
                yas_sn: 0,
                hassasiyet: None,
            }),
            pencereler: taslak.pencereler.clone(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar,
            kesif: Some(KesifBilgisi {
                dinamik: taslak.dinamik,
                grup: taslak.grup.clone(),
                ilk_gorulme: simdi,
                son_gorulme: simdi,
                yasam: KovaYasam::Aktif,
                ebeveyn_guven: ebeveyn.as_ref().map(|(_, g)| *g),
                ebeveyn_havuz: ebeveyn.map(|(e, _)| e),
            }),
            etkin_kalan_zincir: None,
        };
        havuz.hesapla_etkin_durum();
        sonuc.insert(havuz_id.clone(), havuz);
    }
    let mut gecici = cetvel_model::AnlikV2::yeni(simdi, 0, sonuc);
    gecici.zincirleri_hesapla();
    gecici.havuzlar
}

fn agy_hata_havuzu(
    etiket: &str,
    durum: Durum,
    hata_turu: &str,
    http_kodu: Option<u16>,
    notlar: Vec<String>,
) -> HavuzAnlik {
    HavuzAnlik {
        platform: "agy".to_string(),
        etiket: etiket.to_string(),
        durum,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: Some(false),
        kaynak: None,
        pencereler: BTreeMap::new(),
        alt_sayaclar: BTreeMap::new(),
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: None,
        ek: None,
        hata: (durum == Durum::Hata).then(|| HataBilgisi {
            tur: hata_turu.to_string(),
            http_kodu,
        }),
        notlar,
        kesif: None,
        etkin_kalan_zincir: None,
    }
}

fn hata_sonucu(
    id: &str,
    etiket: &str,
    tur: &str,
    http_kodu: Option<u16>,
    not: &str,
    gecici_hata: bool,
) -> BaglayiciSonucu {
    BaglayiciSonucu {
        havuz_id: id.to_string(),
        havuz: agy_hata_havuzu(etiket, Durum::Hata, tur, http_kodu, vec![not.to_string()]),
        retry_after_sn: None,
        gecici_hata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[tokio::test]
    async fn varsayilan_kapali_izinle_surec_baslatmaz() {
        let baglayici = AntigravityBaglayicisi::yeni(AgyAyari::default());
        assert_eq!(baglayici.izin(), AgyOkumaIzni::Kapali);

        let sonuclar = baglayici.oku_coklu(z("2026-11-15T18:00:00Z")).await;
        assert_eq!(sonuclar.len(), 3);
        assert!(sonuclar.iter().all(|s| s.havuz.durum == Durum::Kapali));
        assert!(sonuclar.iter().all(|s| !s.gecici_hata));
    }

    #[tokio::test]
    async fn istek_uzerine_ayari_otomatik_okumada_calismaz() {
        let ayar = AgyAyari {
            okuma: AgyOkumaIzni::IstekUzerine,
            ..Default::default()
        };
        let baglayici = AntigravityBaglayicisi::yeni(ayar);
        let otomatik_sonuc = baglayici.oku_coklu(z("2026-11-15T18:00:00Z")).await;
        assert!(
            otomatik_sonuc
                .iter()
                .all(|s| s.havuz.durum == Durum::Kapali)
        );
    }

    #[test]
    fn agy_json_ciktisini_uc_havuza_ayristirir() {
        let json = r#"{
            "command": {
                "data": {
                    "groups": [
                        {
                            "id": "gemini",
                            "buckets": [
                                { "id": "gemini-5h", "window": "5h", "remaining_fraction": 0.94, "reset_time": "2026-11-15T22:00:00Z" },
                                { "id": "gemini-weekly", "window": "weekly", "remaining_fraction": 0.62, "reset_time": "2026-11-20T00:00:00Z" }
                            ]
                        },
                        {
                            "id": "3p",
                            "buckets": [
                                { "id": "3p-5h", "window": "5h", "remaining_fraction": 0.45, "reset_time": "2026-11-15T21:00:00Z" },
                                { "id": "3p-weekly", "window": "weekly", "remaining_fraction": 0.80, "reset_time": "2026-11-20T00:00:00Z" }
                            ]
                        },
                        {
                            "id": "argon",
                            "buckets": [
                                { "id": "gemini-argon-wtu-weekly", "window": "weekly", "remaining_fraction": 0.99, "reset_time": "2026-11-21T00:00:00Z" }
                            ]
                        }
                    ]
                }
            }
        }"#;

        let havuzlar = agy_cikti_ayristir(json, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuzlar.len(), 3);

        let gemini = &havuzlar["agy_gemini"];
        assert_eq!(gemini.durum, Durum::Ok);
        assert_eq!(gemini.baglayici_pencere.as_deref(), Some("hafta"));
        assert!((gemini.etkin_kalan.unwrap() - 0.62).abs() < 1e-9);
        assert_eq!(
            gemini.kaynak.as_ref().map(|k| (k.tur, k.guven)),
            Some((KaynakTuru::AgyUsage, GuvenEtiketi::Tahmini))
        );

        let claude_gpt = &havuzlar["agy_claude_gpt"];
        assert_eq!(claude_gpt.baglayici_pencere.as_deref(), Some("5s"));
        assert!((claude_gpt.etkin_kalan.unwrap() - 0.45).abs() < 1e-9);

        let argon = &havuzlar["agy_argon"];
        assert_eq!(argon.baglayici_pencere.as_deref(), Some("hafta"));
        assert!((argon.etkin_kalan.unwrap() - 0.99).abs() < 1e-9);
        let kesif = argon.kesif.as_ref().unwrap();
        assert_eq!(kesif.ebeveyn_havuz.as_deref(), Some("agy_gemini"));
        assert_eq!(kesif.ebeveyn_guven, Some(GuvenEtiketi::Tahmini));
        assert!(!kesif.dinamik);
        // Zincir: min(0.99, Gemini 0.62).
        assert!((argon.etkin_kalan_zincir.unwrap() - 0.62).abs() < 1e-9);
        assert_eq!(gemini.etkin_kalan_zincir, None);
    }

    #[test]
    fn gercek_gruplar_ve_bilinmeyen_dorduncu_grup_dinamik_havuz_olur() {
        let ham = include_str!("../../../fixtures/agy-usage-gruplar.json");
        let havuzlar = agy_cikti_ayristir(ham, z("2026-11-20T10:00:00Z"));
        assert_eq!(
            havuzlar.len(),
            4,
            "{:?}",
            havuzlar.keys().collect::<Vec<_>>()
        );
        for bilinen in ["agy_gemini", "agy_claude_gpt", "agy_argon"] {
            let h = &havuzlar[bilinen];
            assert_eq!(h.durum, Durum::Ok, "{bilinen}");
            assert!(!h.kesif.as_ref().unwrap().dinamik, "{bilinen}");
        }
        assert_eq!(havuzlar["agy_gemini"].etiket, "Antigravity Gemini");
        assert!(havuzlar["agy_gemini"].pencereler.contains_key("5s"));
        assert!(havuzlar["agy_gemini"].pencereler.contains_key("hafta"));
        assert_eq!(
            havuzlar["agy_gemini"].pencereler["hafta"].etiket.as_deref(),
            Some("weekly")
        );

        let xenon = &havuzlar["agy_gemini_5_xenon"];
        assert_eq!(xenon.etiket, "Antigravity Gemini 5 Xenon");
        assert_eq!(xenon.durum, Durum::Ok);
        let k = xenon.kesif.as_ref().unwrap();
        assert!(k.dinamik);
        assert_eq!(k.grup.as_deref(), Some("Gemini 5 Xenon"));
        assert_eq!(k.yasam, KovaYasam::Aktif);
        // Genel önek kuralı: gemini-xenon-* ⊂ gemini-* → ebeveyn Gemini (tahmini).
        assert_eq!(k.ebeveyn_havuz.as_deref(), Some("agy_gemini"));
        assert_eq!(k.ebeveyn_guven, Some(GuvenEtiketi::Tahmini));
        assert_eq!(xenon.pencereler["5s"].uzunluk_dk, 300);
        assert_eq!(xenon.pencereler["hafta"].uzunluk_dk, 10080);
        // Argon artık "yalnız hafta" varsayımıyla değil, kovanın kendi penceresiyle.
        assert_eq!(havuzlar["agy_argon"].pencereler.len(), 1);
        assert_eq!(
            havuzlar["agy_argon"]
                .kesif
                .as_ref()
                .unwrap()
                .ebeveyn_havuz
                .as_deref(),
            Some("agy_gemini")
        );
    }

    #[test]
    fn bilinmeyen_pencere_ve_yuzde_olcekli_kesir_ezilmez() {
        let json = r#"{"command":{"data":{"groups":[
            {"name":"Gemini Models","buckets":[
                {"id":"gemini-5h","window":"5h","remaining_fraction":0.5},
                {"id":"gemini-daily","window":"daily","remaining_fraction":85},
                {"id":"gemini-weekly","window":"weekly","remaining_fraction":0.9},
                {"id":"gemini-bozuk","window":"weekly","remaining_fraction":-3}
            ]}]}}}"#;
        let h = &agy_cikti_ayristir(json, z("2026-11-20T10:00:00Z"))["agy_gemini"];
        assert_eq!(h.pencereler.len(), 3);
        assert!((h.pencereler["gun"].kalan - 0.85).abs() < 1e-9);
        assert!(h.notlar.iter().any(|n| n.contains("0-100")));
        assert!(h.notlar.iter().any(|n| n.contains("gemini-bozuk")));
    }

    #[test]
    fn model_turu_ciktisi_algilanir() {
        let ham = include_str!("../../../fixtures/agy-usage-model-turu.json");
        assert!(agy_model_turu_mu(ham));
        let havuzlar = agy_cikti_ayristir(ham, z("2026-11-20T10:00:00Z"));
        assert!(
            havuzlar
                .values()
                .all(|h| h.hata.as_ref().map(|e| e.tur.as_str()) == Some(AGY_MODEL_TURU_HATA))
        );
        // Normal `command` yanıtı model turu sayılmaz.
        assert!(!agy_model_turu_mu(include_str!(
            "../../../fixtures/agy-usage-gruplar.json"
        )));
        assert!(!agy_model_turu_mu("tablo metni"));
        assert!(!agy_model_turu_mu(
            r#"{"status":"OK","usage":{"input_tokens":0}}"#
        ));
    }

    /// Sahte `agy.cmd`: model turu JSON'u basar ve her çalıştırmayı sayaç dosyasına yazar.
    fn sahte_agy(dizin: &Path, json: &str, ek: &str) -> PathBuf {
        let yol = dizin.join("agy.cmd");
        let sayac = dizin.join("sayac.txt");
        let icerik = format!(
            "@echo off\r\necho x>>\"{}\"\r\necho {json}\r\nexit /b 1\r\nrem {ek}\r\n",
            sayac.display()
        );
        std::fs::write(&yol, icerik).unwrap();
        yol
    }

    fn calisma_sayisi(dizin: &Path) -> usize {
        std::fs::read_to_string(dizin.join("sayac.txt"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn model_turu_bagliyiciyi_askiya_alir_ikili_degisince_kalkar() {
        let dizin = std::env::temp_dir().join(format!("cetvel-agy-aski-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dizin);
        std::fs::create_dir_all(&dizin).unwrap();
        let json = include_str!("../../../fixtures/agy-usage-model-turu.json")
            .lines()
            .map(str::trim)
            .collect::<String>();
        let ikili = sahte_agy(&dizin, &json, "a");
        let ayar = AgyAyari {
            okuma: AgyOkumaIzni::Otomatik,
            ikili_yol: Some(ikili.to_string_lossy().into_owned()),
            saatlik_tavan: 12,
            ..Default::default()
        };
        let durum_yolu = dizin.join("agy-durum.json");
        let b = AntigravityBaglayicisi::kalici(ayar.clone(), durum_yolu.clone());
        let t = UtcZaman::simdi();

        let ilk = b.istekle_veya_zamanli_oku(t, false).await;
        assert_eq!(calisma_sayisi(&dizin), 1);
        assert!(ilk.iter().all(|s| s.havuz.hata.as_ref().map(|h| h.tur.as_str())
            == Some(AGY_MODEL_TURU_HATA)));
        assert!(ilk.iter().all(|s| !s.gecici_hata));
        assert!(ilk[0].havuz.notlar[0].contains("24 saat"));
        assert!(b.aski().is_some());

        // Askıdayken zamanlı ve elle okuma süreci çalıştırmaz; yeni süreç (CLI) de askıyı görür.
        let _ = b.istekle_veya_zamanli_oku(t.ekle_sn(120), true).await;
        let cli = AntigravityBaglayicisi::kalici(ayar.clone(), durum_yolu.clone());
        let _ = cli.istekle_veya_zamanli_oku(t.ekle_sn(200), true).await;
        assert_eq!(calisma_sayisi(&dizin), 1, "askıda süreç başlatılmamalı");

        // İkili değişince (boyut farklı) askı kalkar ve yeniden okunur.
        sahte_agy(&dizin, &json, "yeni-surum-uzun-yorum");
        let _ = b.istekle_veya_zamanli_oku(t.ekle_sn(300), false).await;
        assert_eq!(calisma_sayisi(&dizin), 2);

        let _ = std::fs::remove_dir_all(&dizin);
    }

    #[test]
    fn agy_metin_tablosunu_yedek_olarak_ayristirir() {
        let tablo = "\
| Model / Havuz | 5s Kalan | Haftalık Kalan |
| Gemini        | %95      | %70            |
| Claude-GPT    | %30      | %85            |
| Argon         | -        | %100           |
";
        let havuzlar = agy_cikti_ayristir(tablo, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuzlar.len(), 3);
        assert!((havuzlar["agy_gemini"].pencereler["5s"].kalan - 0.95).abs() < 1e-9);
        assert!((havuzlar["agy_gemini"].pencereler["hafta"].kalan - 0.70).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].etkin_kalan.unwrap() - 0.30).abs() < 1e-9);
        assert!((havuzlar["agy_argon"].etkin_kalan.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn markdown_ayirici_satiri_used_basligini_silmez() {
        let tablo = "\
| Model | 5s Used | Haftalik Used |
|---|:---:|---|
| Gemini | %5 | %30 |
| Claude-GPT | %70 | %15 |
";
        let havuzlar = agy_cikti_ayristir(tablo, z("2026-11-15T18:00:00Z"));
        assert!((havuzlar["agy_gemini"].pencereler["5s"].kalan - 0.95).abs() < 1e-9);
        assert!((havuzlar["agy_gemini"].pencereler["hafta"].kalan - 0.70).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].pencereler["5s"].kalan - 0.30).abs() < 1e-9);
        assert!((havuzlar["agy_claude_gpt"].pencereler["hafta"].kalan - 0.85).abs() < 1e-9);
    }

    #[test]
    fn unused_basligi_kullanilan_sayilmaz() {
        let tablo = "\
| Model | 5h Unused | Weekly Unused |
| Gemini | %80 | %60 |
";
        let havuzlar = agy_cikti_ayristir(tablo, z("2026-11-15T18:00:00Z"));
        assert!((havuzlar["agy_gemini"].pencereler["5s"].kalan - 0.80).abs() < 1e-9);
    }

    #[test]
    fn saatlik_sayac_devralinir() {
        let eski = AntigravityBaglayicisi::yeni(AgyAyari::default());
        let t = z("2026-11-15T18:00:00Z");
        eski.durum
            .lock()
            .unwrap()
            .cagrilar
            .extend([t, t.ekle_sn(60)]);
        let yeni = AntigravityBaglayicisi::yeni(AgyAyari::default());
        yeni.sayaci_devral(&eski);
        assert_eq!(yeni.durum.lock().unwrap().cagrilar.len(), 2);
    }

    #[test]
    fn agy_metin_yedeği_five_hour_ve_five_hour_tireli_basliklari_tanir() {
        let girdi = "\
Gemini Models\tFive-Hour Limit Remaining\t80%\t2026-11-16T18:05:17Z
Gemini Models\tWeekly Limit Remaining\t23%\t2026-11-21T15:00:50Z
Claude and GPT models\t5h Limit Remaining\t65%\t2026-11-16T18:05:17Z
";
        let havuzlar = agy_cikti_ayristir(girdi, z("2026-11-16T16:00:00Z"));
        assert_eq!(havuzlar.len(), 2);
        let gemini = &havuzlar["agy_gemini"];
        assert!((gemini.pencereler["5s"].kalan - 0.80).abs() < 1e-9);
        assert!((gemini.pencereler["5s"].kullanilan - 0.20).abs() < 1e-9);
        assert!((gemini.pencereler["hafta"].kalan - 0.23).abs() < 1e-9);
        assert_eq!(
            gemini.pencereler["5s"].sifirlama,
            Some(z("2026-11-16T18:05:17Z"))
        );

        let claude_gpt = &havuzlar["agy_claude_gpt"];
        assert!((claude_gpt.pencereler["5s"].kalan - 0.65).abs() < 1e-9);
    }
}
