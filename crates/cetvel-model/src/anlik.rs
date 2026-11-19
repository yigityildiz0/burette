use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::zaman::UtcZaman;

/// Anlık v2 kök veri yapısı (`anlik.json`).
///
/// Okuma ileri uyumludur ([`Deserialize`] elle yazıldı): ayrıştırılamayan (yeni sürüm biçimli)
/// bir havuz tüm anlığı reddettirmez; atlanır ve [`AnlikV2::notlar`]'a not düşülür. Bilinmeyen
/// `durum` değeri `bilinmiyor` sayılır, havuz kullanılamaz işaretlenir ve havuza not eklenir.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct AnlikV2 {
    /// Şema sürümü (sabit `2`).
    pub sema: u32,
    /// Üretici uygulama kimliği (`"cetvel"`).
    pub yazan: String,
    /// Üretici sürümü.
    pub surum: String,
    /// Anlık üretim zamanı (UTC `Z`).
    pub zaman: UtcZaman,
    /// Her anlamlı değişiklikte artan sıra numarası.
    pub sira: u64,
    /// Havuz kimliğine göre durum haritası.
    pub havuzlar: BTreeMap<String, HavuzAnlik>,
    /// Okuyucu tarafı notları (ör. atlanan bilinmeyen biçimli havuz). Servis yazmaz; boşsa
    /// serileştirilmez.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notlar: Vec<String>,
}

/// Bilinen `durum` değerleri (`Durum` serileştirme adları ve takma adları).
const BILINEN_DURUMLAR: [&str; 7] = [
    "ok",
    "bayat",
    "baglanmadi",
    "bagli_degil",
    "kapali",
    "hata",
    "bilinmiyor",
];

/// Yeni sürümde biçimi değişmiş olabilecek, havuzun özü için gerekmeyen alanlar: ilk ayrıştırma
/// başarısızsa bunlar çıkarılıp yeniden denenir.
const ATLANABILIR_ALANLAR: [&str; 7] = [
    "baski",
    "tahmin",
    "ek",
    "alt_sayaclar",
    "bakiye",
    "kaynak",
    "hata",
];

fn kisa_hata(e: &serde_json::Error) -> String {
    e.to_string().chars().take(120).collect()
}

/// Tek havuzu esnek ayrıştırır; `Err` ise havuz atlanmalıdır (notu döner).
fn havuz_esnek_coz(id: &str, mut deger: serde_json::Value) -> Result<HavuzAnlik, String> {
    let bilinmeyen_durum = deger
        .get("durum")
        .and_then(serde_json::Value::as_str)
        .filter(|d| !BILINEN_DURUMLAR.contains(d))
        .map(|d| d.chars().take(40).collect::<String>());

    let mut havuz = match serde_json::from_value::<HavuzAnlik>(deger.clone()) {
        Ok(h) => h,
        Err(ilk_hata) => {
            let mut atilanlar = Vec::new();
            if let Some(nesne) = deger.as_object_mut() {
                for alan in ATLANABILIR_ALANLAR {
                    if nesne.remove(alan).is_some() {
                        atilanlar.push(alan);
                    }
                }
            }
            match serde_json::from_value::<HavuzAnlik>(deger) {
                Ok(mut h) if !atilanlar.is_empty() => {
                    h.notlar.push(format!(
                        "tanınmayan alanlar yok sayıldı: {}",
                        atilanlar.join(", ")
                    ));
                    h
                }
                _ => {
                    return Err(format!(
                        "havuz '{id}' tanınmayan biçimde, atlandı ({})",
                        kisa_hata(&ilk_hata)
                    ));
                }
            }
        }
    };
    if let Some(d) = bilinmeyen_durum {
        havuz.durum = Durum::Bilinmiyor;
        havuz.kullanilabilir = Some(false);
        havuz
            .notlar
            .push(format!("bilinmeyen durum değeri '{d}': bilinmiyor sayıldı"));
    }
    Ok(havuz)
}

impl<'de> Deserialize<'de> for AnlikV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Ham {
            sema: u32,
            yazan: String,
            surum: String,
            zaman: UtcZaman,
            sira: u64,
            havuzlar: BTreeMap<String, serde_json::Value>,
            #[serde(default)]
            notlar: Vec<String>,
        }
        let ham = Ham::deserialize(deserializer)?;
        let mut notlar = ham.notlar;
        let mut havuzlar = BTreeMap::new();
        for (id, deger) in ham.havuzlar {
            match havuz_esnek_coz(&id, deger) {
                Ok(h) => {
                    havuzlar.insert(id, h);
                }
                Err(not) => notlar.push(not),
            }
        }
        Ok(Self {
            sema: ham.sema,
            yazan: ham.yazan,
            surum: ham.surum,
            zaman: ham.zaman,
            sira: ham.sira,
            havuzlar,
            notlar,
        })
    }
}

impl AnlikV2 {
    /// Yeni bir v2 anlık belgesi oluşturur.
    pub fn yeni(zaman: UtcZaman, sira: u64, havuzlar: BTreeMap<String, HavuzAnlik>) -> Self {
        Self {
            sema: 2,
            yazan: "cetvel".to_string(),
            surum: env!("CARGO_PKG_VERSION").to_string(),
            zaman,
            sira,
            havuzlar,
            notlar: Vec::new(),
        }
    }
}

/// Havuzun anlık durumu (`plan.md §5.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HavuzAnlik {
    /// Platform kimliği (`codex`, `deepseek`, `claude`, vb.).
    pub platform: String,
    /// İnsan okunur etiket.
    pub etiket: String,
    /// Operasyonel okuma/havuz durumu.
    pub durum: Durum,
    /// Tüm pencerelerin en küçüğü olan etkin kalan oran (`0.0..=1.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etkin_kalan: Option<f64>,
    /// Etkin kalanı belirleyen en kısıtlayıcı pencere adı (ör. `"hafta"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baglayici_pencere: Option<String>,
    /// Havuzun şu an kullanılabilir olup olmadığı (`etkin_kalan > 0` ve `durum == Ok`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kullanilabilir: Option<bool>,
    /// Veri kaynağı ve tazelik bilgisi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kaynak: Option<KaynakBilgisi>,
    /// Pencere adına göre ölçümler (`5dk`, `5s`, `gun`, `hafta`, `ay`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pencereler: BTreeMap<String, PencereOlcum>,
    /// Alt sayaçlar (ör. model-bazlı ek limit kimlikleri).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub alt_sayaclar: BTreeMap<String, AltSayac>,
    /// Pencere bazlı tahmin bilgisi (M4'te dolar).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tahmin: BTreeMap<String, TahminBilgisi>,
    /// Kullanım baskısı etiketi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baski: Option<Baski>,
    /// Bakiye bilgisi (bakiye tabanlı sağlayıcılar için).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bakiye: Option<BakiyeBilgisi>,
    /// Sağlayıcıya özgü ek alanlar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ek: Option<EkBilgi>,
    /// Varsa güvenli hata bilgisi (üçüncü taraf ham metni içermez).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hata: Option<HataBilgisi>,
    /// Yerel bilgilendirme notları.
    #[serde(default)]
    pub notlar: Vec<String>,
}

/// Resmî kotası olmayan, yalnız yerel kullanım sayacı üreten araçların platform kimlikleri.
pub const KULLANIM_SAYACI_PLATFORMLARI: &[&str] = &["opencode", "dsh"];

impl HavuzAnlik {
    /// Havuz bir kota değil, kotası olmayan yerel aracın kullanım sayacıysa `true`: `ek.kullanim`
    /// dolu ya da (okuma hatası, kapalı, kurulu değil gibi `ek` taşımayan durumlarda) platform
    /// [`KULLANIM_SAYACI_PLATFORMLARI`] içinde. Böyle havuzlar kota özetlerine (tepsi, sağlık,
    /// orkestra uyumu) katılmaz.
    pub fn kullanim_sayaci_mi(&self) -> bool {
        self.ek.as_ref().is_some_and(|e| e.kullanim.is_some())
            || KULLANIM_SAYACI_PLATFORMLARI.contains(&self.platform.as_str())
    }

    /// Pencerelerden `etkin_kalan`, `baglayici_pencere`, `kullanilabilir` ve temel `baski` alanlarını hesaplar.
    ///
    /// Haftalık pencere `%0` (`kalan == 0.0`) iken `5s` `%100` (`kalan == 1.0`) görünse dahi
    /// `etkin_kalan = 0.0`, `baglayici_pencere = "hafta"` ve `kullanilabilir = false` olur.
    pub fn hesapla_etkin_durum(&mut self) {
        let mut en_kucuk: Option<(&String, &PencereOlcum)> = None;
        for (ad, olcum) in &self.pencereler {
            if !olcum.kalan.is_finite() {
                continue;
            }
            match en_kucuk {
                None => en_kucuk = Some((ad, olcum)),
                Some((mevcut_ad, mevcut)) => {
                    let fark = olcum.kalan - mevcut.kalan;
                    if fark < -1e-9
                        || (fark.abs() <= 1e-9
                            && (olcum.uzunluk_dk > mevcut.uzunluk_dk
                                || (olcum.uzunluk_dk == mevcut.uzunluk_dk && ad < mevcut_ad)))
                    {
                        en_kucuk = Some((ad, olcum));
                    }
                }
            }
        }

        if let Some((pencere_ad, olcum)) = en_kucuk {
            let etkin = olcum.kalan.clamp(0.0, 1.0);
            let pencere = pencere_ad.clone();
            self.etkin_kalan = Some(etkin);
            self.baglayici_pencere = Some(pencere.clone());

            let aktif_ve_kalan_var = self.durum == Durum::Ok && etkin > 0.0;
            self.kullanilabilir = Some(aktif_ve_kalan_var);

            if self.durum == Durum::Ok {
                let baski = if etkin <= 0.0 {
                    Baski::Kacin
                } else if etkin <= 0.20 {
                    Baski::Dikkat
                } else {
                    Baski::Rahat
                };
                self.baski = Some(baski);
            } else if self.durum == Durum::Bayat && etkin <= 0.0 {
                self.baski = Some(Baski::Kacin);
            }

            if self.durum == Durum::Ok && etkin <= 0.0 {
                let uyari = format!("havuz kullanılamaz: {pencere} penceresi %0 kalan");
                if !self.notlar.iter().any(|n| n == &uyari) {
                    self.notlar.push(uyari);
                }
            }
        } else {
            self.etkin_kalan = None;
            self.baglayici_pencere = None;
            let muaf = self.ek.as_ref().and_then(|e| e.muaf) == Some(true);
            let bakiye_var = self
                .bakiye
                .as_ref()
                .is_some_and(|b| b.toplam > 0.0 && b.kullanilabilir.unwrap_or(true));
            // Kullanım sayacı (kotası olmayan yerel araç) kotaya takılmaz: okunuyorsa kullanılabilir.
            let sayac = self.ek.as_ref().is_some_and(|e| e.kullanim.is_some());
            let kullanilir = self.durum == Durum::Ok && (muaf || bakiye_var || sayac);
            self.kullanilabilir = Some(kullanilir);
            if self.durum == Durum::Ok && self.bakiye.is_some() {
                self.baski = Some(if kullanilir {
                    Baski::Rahat
                } else {
                    Baski::Kacin
                });
            }
        }
    }

    /// Okuma yaşını günceller ve eşik aşıldıysa `Bayat` durumuna geçirir.
    pub fn yas_guncelle(&mut self, simdi: UtcZaman, bayat_esigi_sn: u64) {
        if let Some(kaynak) = self.kaynak.as_mut() {
            let yas = simdi.fark_sn(kaynak.okuma).max(0) as u64;
            kaynak.yas_sn = yas;
            if self.durum == Durum::Ok && yas > bayat_esigi_sn {
                self.durum = Durum::Bayat;
                self.kullanilabilir = Some(false);
            }
        }
    }
}

/// Havuzun okuma / erişim durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Durum {
    Ok,
    Bayat,
    #[serde(alias = "bagli_degil")]
    Baglanmadi,
    Kapali,
    Hata,
    /// Bilinmeyen (yeni sürüm) durum değerleri de buraya düşer; tüm anlık reddedilmez.
    #[serde(other)]
    Bilinmiyor,
}

/// Ölçüm kaynağı metaverisi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct KaynakBilgisi {
    pub tur: KaynakTuru,
    pub guven: GuvenEtiketi,
    pub okuma: UtcZaman,
    pub yas_sn: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hassasiyet: Option<f64>,
}

/// Okuma yöntemi / kaynak türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KaynakTuru {
    ModKoprusu,
    Statusline,
    CodexAppServer,
    RolloutYedek,
    AgyUsage,
    ResmiApi,
    BakiyeApi,
    Hesap,
    // Yerel aracın kendi oturum veritabanı ya da günlüğü (salt okuma; OpenCode, DeepSeek Harness).
    YerelGunluk,
    #[serde(other)]
    Bilinmiyor,
}

/// Veri güvenilirlik etiketi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuvenEtiketi {
    ResmiArayuz,
    ResmiApi,
    Tahmini,
    Bayat,
    // Yerel kayıttan sayılmış kesin sayaç (kota değil).
    YerelKayit,
    #[serde(other)]
    Bilinmiyor,
}

/// Tek bir kota penceresinin normalize edilmiş ölçümü.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PencereOlcum {
    pub uzunluk_dk: u64,
    pub kullanilan: f64,
    pub kalan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama: Option<UtcZaman>,
}

/// Ek alt sayaç bilgisi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AltSayac {
    pub kullanilan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kalan: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uzunluk_dk: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama: Option<UtcZaman>,
}

/// Tahmin bilgisi (`plan.md §7`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TahminBilgisi {
    pub durum: TahminDurumu,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etiket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neden: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitis_tahmini: Option<UtcZaman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitis_erken: Option<UtcZaman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitis_gec: Option<UtcZaman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saatlik_hiz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlanmadan_biter: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gunluk_butce: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TahminDurumu {
    Deneysel,
    Hazir,
    #[serde(other)]
    TahminYok,
}

/// Kota kullanım baskısı etiketi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Baski {
    Rahat,
    Dikkat,
    Kacin,
}

/// Bakiye bilgisi (bakiye tabanlı API'ler için).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BakiyeBilgisi {
    pub para_birimi: String,
    pub toplam: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hibe: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yuklenen: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kullanilabilir: Option<bool>,
}

/// Platforma özgü ek alanlar.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EkBilgi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama_kredisi: Option<SifirlamaKredisi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub olagan_kullanim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_turu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kredi_bakiye: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sinirsiz_kredi: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_turu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kampanya_bitis: Option<UtcZaman>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eszamanli_istek: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eszamanli_tavan: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seviye: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muaf: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kalan_istek_baslik: Option<u64>,
    /// Resmî kotası olmayan yerel araçlar (OpenCode, DeepSeek Harness) için kullanım sayacı.
    /// Bu alan doluysa havuz bir kota değil, yalnız sayaçtır: yüzde/pencere yoktur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kullanim: Option<KullanimSayaci>,
}

/// Kotası olmayan yerel bir aracın kullanım sayacı (son 24 saat, son 7 gün, toplam).
///
/// Belirteç sayıları yalnız kaynak açıkça verdiyse doludur (ör. sıkıştırılmış günlükte `None`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KullanimSayaci {
    /// Her zaman `true`: arayüz "kota yok, kullanım sayacı" etiketini gösterir.
    pub kota_yok: bool,
    pub oturum_24s: u64,
    pub oturum_7g: u64,
    pub oturum_toplam: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub belirtec_24s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub belirtec_7g: Option<u64>,
    /// En son oturum etkinliği.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub son_etkinlik: Option<UtcZaman>,
}

/// Codex sıfırlama kredisi özeti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SifirlamaKredisi {
    pub adet: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub son: Option<UtcZaman>,
}

/// Ham üçüncü taraf hata metni taşımayan sadeleştirilmiş hata yapısı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HataBilgisi {
    pub tur: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_kodu: Option<u16>,
}

/// Pencere süresini (dakika) standart pencere adına çevirir (`plan.md §5.1`).
pub fn pencere_adi_dakikadan(uzunluk_dk: u64) -> String {
    match uzunluk_dk {
        1..=15 => "5dk".to_string(),
        16..=600 => "5s".to_string(),
        601..=2880 => "gun".to_string(),
        2881..=20160 => "hafta".to_string(),
        20161.. => "ay".to_string(),
        0 => "0dk".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek_zaman(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("gecerli zaman")
    }

    #[test]
    fn bilinmeyen_enum_degerleri_tum_anligi_reddetmez() {
        let ham = r#"{
            "sema": 2, "yazan": "cetvel", "surum": "9.9.9",
            "zaman": "2026-11-16T10:00:00Z", "sira": 5,
            "havuzlar": {
                "yeni": {
                    "platform": "yeni", "etiket": "Yeni", "durum": "gelecek_durum",
                    "kaynak": { "tur": "yeni_kaynak", "guven": "yeni_guven",
                                "okuma": "2026-11-16T10:00:00Z", "yas_sn": 3 },
                    "tahmin": { "5s": { "durum": "yeni_tahmin" } }
                }
            }
        }"#;
        let anlik: AnlikV2 = serde_json::from_str(ham).expect("ileri uyumlu ayrıştırma");
        let h = &anlik.havuzlar["yeni"];
        assert_eq!(h.durum, Durum::Bilinmiyor);
        assert_eq!(h.kullanilabilir, Some(false));
        assert!(h.notlar.iter().any(|n| n.contains("gelecek_durum")));
        let k = h.kaynak.as_ref().expect("kaynak");
        assert_eq!(k.tur, KaynakTuru::Bilinmiyor);
        assert_eq!(k.guven, GuvenEtiketi::Bilinmiyor);
        assert_eq!(h.tahmin["5s"].durum, TahminDurumu::TahminYok);
        // Takma ad hâlâ çalışır
        let d: Durum = serde_json::from_str("\"bagli_degil\"").expect("alias");
        assert_eq!(d, Durum::Baglanmadi);
    }

    #[test]
    fn bozuk_havuz_atlanir_not_duser_digerleri_korunur() {
        let ham = r#"{
            "sema": 2, "yazan": "cetvel", "surum": "9.9.9",
            "zaman": "2026-11-16T10:00:00Z", "sira": 7,
            "havuzlar": {
                "codex_plus": { "platform": "codex", "etiket": "Codex", "durum": "ok",
                    "pencereler": { "5s": { "uzunluk_dk": 300, "kullanilan": 0.2, "kalan": 0.8 } } },
                "gelecek": { "platform": 42, "durum": "ok" },
                "baski_yeni": { "platform": "x", "etiket": "X", "durum": "ok",
                    "baski": "panik", "notlar": [] }
            }
        }"#;
        let anlik: AnlikV2 = serde_json::from_str(ham).expect("tüm anlık reddedilmemeli");
        assert_eq!(anlik.sira, 7);
        assert!(anlik.havuzlar.contains_key("codex_plus"));
        assert!(!anlik.havuzlar.contains_key("gelecek"));
        assert_eq!(anlik.notlar.len(), 1);
        assert!(anlik.notlar[0].contains("gelecek"));
        // Bilinmeyen `baski` değeri: alan yok sayılır, havuz korunur ve not düşer.
        let b = &anlik.havuzlar["baski_yeni"];
        assert_eq!(b.baski, None);
        assert!(b.notlar.iter().any(|n| n.contains("baski")));
        // Gidiş-dönüş: okuyucu notları yazılır ve yeniden okunur; boşsa alan hiç yazılmaz.
        let tekrar: AnlikV2 =
            serde_json::from_str(&serde_json::to_string(&anlik).unwrap()).unwrap();
        assert_eq!(tekrar.notlar, anlik.notlar);
        let bos = AnlikV2::yeni(anlik.zaman, 1, BTreeMap::new());
        assert!(!serde_json::to_string(&bos).unwrap().contains("notlar"));
    }

    #[test]
    fn haftalik_sifir_bes_saatlik_yuz_iken_etkin_kalan_sifir_ve_kullanilamaz() {
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
                sifirlama: Some(ornek_zaman("2026-11-20T07:00:00Z")),
            },
        );

        let mut havuz = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::CodexAppServer,
                guven: GuvenEtiketi::ResmiArayuz,
                okuma: ornek_zaman("2026-11-15T18:00:00Z"),
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

        havuz.hesapla_etkin_durum();

        assert_eq!(havuz.etkin_kalan, Some(0.0));
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("hafta"));
        assert_eq!(havuz.kullanilabilir, Some(false));
        assert_eq!(havuz.baski, Some(Baski::Kacin));

        let json = serde_json::to_value(&havuz).expect("json");
        assert_eq!(json["etkin_kalan"], 0.0);
        assert_eq!(json["baglayici_pencere"], "hafta");
        assert_eq!(json["kullanilabilir"], false);
    }

    #[test]
    fn iki_pencere_esit_kalan_olunca_uzun_pencere_baglayici_secilir() {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.5,
                kalan: 0.5,
                sifirlama: None,
            },
        );
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                uzunluk_dk: 10080,
                kullanilan: 0.5,
                kalan: 0.5,
                sifirlama: Some(ornek_zaman("2026-11-20T07:00:00Z")),
            },
        );

        let mut havuz = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex".to_string(),
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

        havuz.hesapla_etkin_durum();
        assert_eq!(havuz.etkin_kalan, Some(0.5));
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("hafta"));
        assert_eq!(havuz.kullanilabilir, Some(true));
        assert_eq!(havuz.baski, Some(Baski::Rahat));
    }

    #[test]
    fn durum_bagli_degil_takma_adini_kabul_eder_ve_bakiye_etkin_durum_hesaplar() {
        let durum: Durum = serde_json::from_str(r#""bagli_degil""#).expect("alias deserialize");
        assert_eq!(durum, Durum::Baglanmadi);
        assert_eq!(
            serde_json::to_string(&durum).expect("serialize"),
            r#""baglanmadi""#
        );

        let mut havuz = HavuzAnlik {
            platform: "deepseek".to_string(),
            etiket: "DeepSeek".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: Some(BakiyeBilgisi {
                para_birimi: "USD".to_string(),
                toplam: 10.0,
                hibe: Some(2.0),
                yuklenen: Some(8.0),
                kullanilabilir: Some(false),
            }),
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        havuz.hesapla_etkin_durum();
        assert_eq!(havuz.etkin_kalan, None);
        assert_eq!(havuz.kullanilabilir, Some(false));
        assert_eq!(havuz.baski, Some(Baski::Kacin));

        havuz.bakiye.as_mut().unwrap().kullanilabilir = Some(true);
        havuz.hesapla_etkin_durum();
        assert_eq!(havuz.etkin_kalan, None);
        assert_eq!(havuz.kullanilabilir, Some(true));
        assert_eq!(havuz.baski, Some(Baski::Rahat));
    }
}
