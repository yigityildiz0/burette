//! Masaüstü uygulaması paylaşılan durumu ve tepsi görsel senkronizasyonu.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use cetvel_model::AnlikV2;
use serde::Serialize;
use tauri::AppHandle;
use tauri::menu::{CheckMenuItem, MenuItem};

use crate::ikon;
use crate::metinler::{self, DegerYonu, TepsiDili};
use crate::servis_istemci::ServisIstemci;

pub const TEPSI_KIMLIK: &str = "cetvel-tepsi";

#[derive(Clone)]
pub struct TepsiMenuOgeleri {
    pub acilir: MenuItem<tauri::Wry>,
    pub pencere: MenuItem<tauri::Wry>,
    pub serit_goster: CheckMenuItem<tauri::Wry>,
    pub serit_gecirgen: CheckMenuItem<tauri::Wry>,
    pub yenile: MenuItem<tauri::Wry>,
    pub servis_baslat: MenuItem<tauri::Wry>,
    pub otomatik_baslat: CheckMenuItem<tauri::Wry>,
    pub cikis: MenuItem<tauri::Wry>,
}

impl TepsiMenuOgeleri {
    /// Tüm menü öğesi metinlerini seçilen dilde yeniden yazar (kimlikler ve onay durumları kalır).
    pub fn metinleri_uygula(&self, dil: TepsiDili) {
        let m = metinler::menu_metinleri(dil);
        let _ = self.acilir.set_text(m.acilir);
        let _ = self.pencere.set_text(m.pencere);
        let _ = self.serit_goster.set_text(m.serit);
        let _ = self.serit_gecirgen.set_text(m.serit_gecirgen);
        let _ = self.yenile.set_text(m.yenile);
        let _ = self.servis_baslat.set_text(m.servis_baslat);
        let _ = self.otomatik_baslat.set_text(m.otomatik_baslat);
        let _ = self.cikis.set_text(m.cikis);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeritDurumu {
    pub gorunur: bool,
    pub ustte: bool,
    pub gecirgen: bool,
}

pub struct DurumIc {
    pub pencere_bosta_sn: u64,
    pub pencereler: std::collections::BTreeMap<String, crate::pencere::PencereOmru>,
    pub son_anlik: Option<AnlikV2>,
    pub servis_calisiyor: bool,
    pub son_tepsi_rect: Option<tauri::Rect>,
    pub son_acilir_gizlenme: Option<Instant>,
    pub pencere_ilk_gosterim: bool,
    pub serit_ilk_konum: bool,
    pub serit_gorunur: bool,
    pub serit_ustte: bool,
    pub serit_gecirgen: bool,
    pub olcek_faktoru: f64,
    pub tepsi_menu: Option<TepsiMenuOgeleri>,
    /// Ana pencere kapatılınca tepsiye küçült (aksi hâlde uygulamadan çık).
    pub tepsiye_kucult: bool,
    /// Tepsi menüsü ve ipucu dili (ayarlardaki dilden türetilir; `tr` dışı hep `en`).
    pub tepsi_dili: TepsiDili,
    /// Tepsi çubukları ve ipucundaki yüzde yönü (`kalan` varsayılan).
    pub deger_yonu: DegerYonu,
}

impl Default for DurumIc {
    fn default() -> Self {
        Self {
            pencere_bosta_sn: 60,
            pencereler: Default::default(),
            son_anlik: None,
            servis_calisiyor: false,
            son_tepsi_rect: None,
            son_acilir_gizlenme: None,
            pencere_ilk_gosterim: false,
            serit_ilk_konum: false,
            serit_gorunur: false,
            serit_ustte: true,
            serit_gecirgen: false,
            olcek_faktoru: 1.0,
            tepsi_menu: None,
            tepsiye_kucult: true,
            tepsi_dili: TepsiDili::default(),
            deger_yonu: DegerYonu::default(),
        }
    }
}

#[derive(Clone)]
pub struct MasaustuDurum {
    pub istemci: ServisIstemci,
    pub ic: Arc<Mutex<DurumIc>>,
}

impl MasaustuDurum {
    pub fn yeni(istemci: ServisIstemci, baslangic_olcek: f64) -> Self {
        Self {
            istemci,
            ic: Arc::new(Mutex::new(DurumIc {
                olcek_faktoru: baslangic_olcek.clamp(1.0, 4.0),
                ..DurumIc::default()
            })),
        }
    }
}

/// Ayarlardaki dil koduna ve değer yönüne göre tepsiyi günceller: dil değiştiyse menü metinlerini,
/// dil veya değer yönü değiştiyse ikonu ve ipucunu yeniden yazar.
pub fn tepsi_tercihlerini_uygula(
    app: &AppHandle,
    durum: &MasaustuDurum,
    dil_kodu: &str,
    deger_yonu_kodu: &str,
) {
    let dil = TepsiDili::koddan(dil_kodu);
    let yon = DegerYonu::koddan(deger_yonu_kodu);
    // `set_text` ana iş parçacığına gönderip bekler; kilit tutulurken çağrılmaz
    // (bkz. `komutlar::otomatik_baslat_uygula`).
    let (dil_degisti, yon_degisti, menu) = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let dil_degisti = g.tepsi_dili != dil;
        let yon_degisti = g.deger_yonu != yon;
        g.tepsi_dili = dil;
        g.deger_yonu = yon;
        (dil_degisti, yon_degisti, g.tepsi_menu.clone())
    };
    if dil_degisti && let Some(menu) = menu {
        menu.metinleri_uygula(dil);
    }
    if dil_degisti || yon_degisti {
        tepsi_gorunum_guncelle(app, durum);
    }
}

/// Önbellekteki son `AnlikV2` ve servis durumuna göre tepsi ikonunu (DPI ölçekli) ve ipucu metnini günceller.
pub fn tepsi_gorunum_guncelle(app: &AppHandle, durum: &MasaustuDurum) {
    let (veri, ipucu, olcek) = {
        let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let anlik_ref = g.son_anlik.as_ref();
        let veri = ikon::tepsi_verisi_cikar(anlik_ref, g.servis_calisiyor, g.deger_yonu);
        let ipucu =
            ikon::ipucu_metni_olustur(anlik_ref, g.servis_calisiyor, g.tepsi_dili, g.deger_yonu);
        (veri, ipucu, g.olcek_faktoru)
    };
    if let Some(tepsi) = app.tray_by_id(TEPSI_KIMLIK) {
        let ikon_img = ikon::tepsi_ikonu_olustur(&veri, olcek);
        let _ = tepsi.set_icon(Some(ikon_img));
        let _ = tepsi.set_tooltip(Some(ipucu));
    }
}
