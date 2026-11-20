//! Masaüstü pencere yönetimi (`acilir`, `pencere`, `serit`) ve tepsiye yapışık konumlandırma.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::durum::{MasaustuDurum, SeritDurumu};

// wry'nin varsayılan özellik listesini korur. GPU ve site izolasyonu açık kalır.
// Ek bellek bayrakları ancak ayrı ölçümle doğrulanırsa buraya alınmalıdır.
pub const TARAYICI_ARGUMANLARI: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

#[derive(Debug, Clone)]
pub enum BekleyenAcilis {
    Acilir { tepsi_rect: Option<tauri::Rect> },
    Pencere { sekme: Option<String> },
    Serit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometriGeriYukleme {
    pub konum: Option<tauri::PhysicalPosition<i32>>,
    pub boyut: Option<tauri::PhysicalSize<u32>>,
    pub buyutulmus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KisayolEylemi {
    AnaPencereyiGizle,
    AcilirGizle,
    AcilirGoster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TekOrnekHedefi {
    AnaPencere,
    Acilir,
}

#[derive(Debug, Default)]
pub struct PencereOmru {
    gizlendi: Option<Instant>,
    konum: Option<tauri::PhysicalPosition<i32>>,
    boyut: Option<tauri::PhysicalSize<u32>>,
    buyutulmus: bool,
    hazir: bool,
    bekleyen_sekme: Option<String>,
    yok_ediliyor: bool,
    bekleyen_acilis: Option<BekleyenAcilis>,
}

impl PencereOmru {
    /// Gizli pencere yok edilmek üzere işaretlenir; `Destroyed` olayı gelene dek yeniden
    /// açma istekleri bekletilir ki `WindowLabelAlreadyExists` veya ölü pencereye `show()` olmasın.
    pub fn yok_etme_baslat(&mut self) {
        self.yok_ediliyor = true;
        self.hazir = false;
        self.gizlendi = None;
    }

    /// Tauri pencereyi `WindowManager`'dan çıkardıktan sonra çağrılır.
    pub fn yok_edildi(&mut self) -> Option<BekleyenAcilis> {
        self.yok_ediliyor = false;
        self.hazir = false;
        self.gizlendi = None;
        self.bekleyen_sekme = None;
        self.bekleyen_acilis.take()
    }

    /// Pencere yok edilme aşamasındaysa açılış isteğini beklemeye alır (`true` döner).
    pub fn acilis_beklet(&mut self, acilis: BekleyenAcilis) -> bool {
        if self.yok_ediliyor {
            self.bekleyen_acilis = Some(acilis);
            true
        } else {
            false
        }
    }

    pub fn bekleyen_acilisi_iptal_et(&mut self) {
        self.bekleyen_acilis = None;
    }

    pub fn gosterildi_kaydet(&mut self) {
        self.gizlendi = None;
        self.yok_ediliyor = false;
    }

    pub fn gizlendi_kaydet(&mut self, simdi: Instant, buyutulmus: bool) {
        self.gizlendi = Some(simdi);
        self.buyutulmus = buyutulmus;
    }

    /// Gizli pencerenin gizlenme saati bir nedenle hiç yazılmadıysa sonsuza dek yaşamaması için başlatır.
    pub fn gizli_saat_eksikse_baslat(&mut self, simdi: Instant) {
        if self.gizlendi.is_none() && !self.yok_ediliyor {
            self.gizlendi = Some(simdi);
        }
    }

    pub fn sekme_iste(&mut self, sekme: &str) -> bool {
        if !self.hazir {
            self.bekleyen_sekme = Some(sekme.to_string());
        }
        self.hazir
    }

    pub fn arayuz_hazir_oldu(&mut self) -> Option<String> {
        self.hazir = true;
        self.bekleyen_sekme.take()
    }
}

/// Görünür, yok edilmekte olan veya gizlenme saati olmayan pencere süreden dolayı yok edilmez.
pub fn yok_etmeli_mi(gorunur: bool, gizli_sure: Option<Duration>, esik: Duration) -> bool {
    !gorunur && gizli_sure.is_some_and(|sure| sure >= esik)
}

/// Yeniden oluşturmada hangi pencere için hangi geometrinin geri yükleneceğini seçer (saf).
/// `acilir` sabit boyutlu olup tepsiye göre yeniden konumlanır; `serit` yalnız konumunu korur.
pub fn geri_yuklenecek_geometri(etiket: &str, omur: &PencereOmru) -> GeometriGeriYukleme {
    match etiket {
        "pencere" => GeometriGeriYukleme {
            konum: omur.konum,
            boyut: omur.boyut,
            buyutulmus: omur.buyutulmus,
        },
        "serit" => GeometriGeriYukleme {
            konum: omur.konum,
            boyut: None,
            buyutulmus: false,
        },
        _ => GeometriGeriYukleme {
            konum: None,
            boyut: None,
            buyutulmus: false,
        },
    }
}

/// `Ctrl+Alt+K` kısayolunda hangi pencere işleminin yapılacağını seçer (saf).
pub fn kisayol_eylemi_sec(
    pencere_gorunur: bool,
    pencere_odakli: bool,
    acilir_gorunur: bool,
) -> KisayolEylemi {
    if pencere_gorunur && pencere_odakli {
        KisayolEylemi::AnaPencereyiGizle
    } else if acilir_gorunur {
        KisayolEylemi::AcilirGizle
    } else {
        KisayolEylemi::AcilirGoster
    }
}

/// İkinci örnek başlatıldığında öne çıkarılacak pencereyi seçer (saf).
pub fn tek_ornek_hedefi_sec(pencere_gorunur: bool) -> TekOrnekHedefi {
    if pencere_gorunur {
        TekOrnekHedefi::AnaPencere
    } else {
        TekOrnekHedefi::Acilir
    }
}

fn bellek_hedefi(win: &WebviewWindow, dusuk: bool) {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
        };
        use windows::core::Interface;
        let _ = win.with_webview(move |webview| {
            // SAFETY: Tauri bu kapanışı WebView'in COM iş parçacığında çalıştırır.
            let sonuc = unsafe {
                webview.controller().CoreWebView2().and_then(|core| {
                    core.cast::<ICoreWebView2_19>().and_then(|core| {
                        core.SetMemoryUsageTargetLevel(if dusuk {
                            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                        } else {
                            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                        })
                    })
                })
            };
            if let Err(err) = sonuc {
                tracing::debug!(%err, "WebView bellek hedefi desteklenmiyor");
            }
        });
    }
}

/// Bu yardımcı yalnız ana olay döngüsünden çağrılır; etiket başına tek pencere oluşturulur.
fn pencere_al(app: &AppHandle, etiket: &str) -> Option<WebviewWindow> {
    let durum = app.try_state::<MasaustuDurum>()?;
    if let Some(win) = app.get_webview_window(etiket) {
        let yok_ediliyor = durum
            .ic
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pencereler
            .get(etiket)
            .is_some_and(|o| o.yok_ediliyor);
        if yok_ediliyor {
            return None;
        }
        return Some(win);
    }
    let win = match crate::pencere_olustur(app, etiket) {
        Ok(win) => win,
        Err(err) => {
            tracing::error!(%err, etiket, "Pencere oluşturulamadı");
            return None;
        }
    };
    let geometri = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let omur = g.pencereler.entry(etiket.to_string()).or_default();
        omur.hazir = false;
        omur.yok_ediliyor = false;
        omur.bekleyen_sekme = None;
        geri_yuklenecek_geometri(etiket, omur)
    };
    // Önce geçerli monitör çalışma alanlarına göre doğrula, sonra hedef monitöre yerleştir.
    let calisma_alanlari: Vec<FizikDikdortgen> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let wa = m.work_area();
            dikdortgen(wa.position, wa.size)
        })
        .collect();
    let hedef_boyut = geometri
        .boyut
        .or_else(|| win.outer_size().ok())
        .unwrap_or_else(|| tauri::PhysicalSize::new(960, 720));

    if let Some(boyut) = geometri.boyut {
        let _ = win.set_size(boyut);
    }
    if let Some(konum) = geometri.konum {
        if geometri_ekrana_uygun_mu(
            &calisma_alanlari,
            (konum.x, konum.y),
            (hedef_boyut.width, hedef_boyut.height),
        ) {
            let _ = win.set_position(konum);
        } else if etiket == "serit" {
            let tepsi = tepsi_merkezi(
                durum
                    .ic
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .son_tepsi_rect,
            );
            if let Some((wa, olcek)) = monitor_bilgisi(app, tepsi) {
                let (x, y) =
                    serit_baslangic_konum(wa, hedef_boyut.width, hedef_boyut.height, olcek);
                let _ = win.set_position(PhysicalPosition::new(x, y));
            }
        } else {
            let _ = win.center();
        }
    }
    if geometri.buyutulmus {
        let _ = win.maximize();
    }
    let app_k = app.clone();
    let etiket = etiket.to_string();
    win.on_window_event(move |olay| {
        if matches!(
            olay,
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
        ) && let Some(win) = app_k.get_webview_window(&etiket)
            && win.is_visible().unwrap_or(false)
        {
            normal_geometriyi_kaydet(&app_k, &win);
        }
    });
    Some(win)
}

fn normal_geometriyi_kaydet(app: &AppHandle, win: &WebviewWindow) {
    if win.is_maximized().unwrap_or(true) || win.is_minimized().unwrap_or(true) {
        return;
    }
    if let (Ok(konum), Ok(boyut)) = (win.outer_position(), win.inner_size()) {
        if boyut.width == 0 || boyut.height == 0 {
            return;
        }
        let Some(durum) = app.try_state::<MasaustuDurum>() else {
            return;
        };
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let omur = g.pencereler.entry(win.label().to_string()).or_default();
        if omur.yok_ediliyor {
            return;
        }
        omur.konum = Some(konum);
        omur.boyut = Some(boyut);
    }
}

/// Arayüz olay dinleyicilerini kurduktan sonra `ayarlar_al` çağırır. Soğuk
/// açılışta sekme olayı bu noktaya kadar bekletilir; zamanlayıcı tahmini gerekmez.
pub fn arayuz_hazir(win: &WebviewWindow) {
    let Some(durum) = win.try_state::<MasaustuDurum>() else {
        return;
    };
    let sekme = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let omur = g.pencereler.entry(win.label().to_string()).or_default();
        if omur.yok_ediliyor {
            return;
        }
        omur.arayuz_hazir_oldu()
    };
    if let Some(sekme) = sekme {
        let _ = win
            .app_handle()
            .emit_to(win.label(), "pencere_sekme_sec", sekme);
    }
}

fn gosterildi(app: &AppHandle, win: &WebviewWindow) {
    normal_geometriyi_kaydet(app, win);
    if let Some(durum) = app.try_state::<MasaustuDurum>() {
        durum
            .ic
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pencereler
            .entry(win.label().to_string())
            .or_default()
            .gosterildi_kaydet();
    }
    bellek_hedefi(win, false);
}

fn gizle_ana_dongude(app: &AppHandle, etiket: &str) {
    let Some(durum) = app.try_state::<MasaustuDurum>() else {
        return;
    };
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let omur = g.pencereler.entry(etiket.to_string()).or_default();
        omur.bekleyen_acilisi_iptal_et();
        if omur.yok_ediliyor {
            return;
        }
    }
    let Some(win) = app.get_webview_window(etiket) else {
        return;
    };
    // Tekrarlanan odak kaybı/kapatma olayları bekleme süresini uzatmasın.
    if !win.is_visible().unwrap_or(true) {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.pencereler
            .entry(etiket.to_string())
            .or_default()
            .gizli_saat_eksikse_baslat(Instant::now());
        return;
    }
    normal_geometriyi_kaydet(app, &win);
    let buyutulmus = win.is_maximized().unwrap_or(false);
    if win.hide().is_err() {
        return;
    }
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.pencereler
            .entry(etiket.to_string())
            .or_default()
            .gizlendi_kaydet(Instant::now(), buyutulmus);
    }
    bellek_hedefi(&win, true);
}

pub fn temizlik_baslat(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let app_k = app.clone();
            // Kontrol ve destroy aynı olay döngüsünde: yeniden açma ile yarışmaz.
            let _ = app.run_on_main_thread(move || {
                let Some(durum) = app_k.try_state::<MasaustuDurum>() else {
                    return;
                };
                for etiket in ["acilir", "pencere", "serit"] {
                    let Some(win) = app_k.get_webview_window(etiket) else {
                        continue;
                    };
                    let gorunur = win.is_visible().unwrap_or(true);
                    let (yok_ediliyor, gizli_sure, esik) = {
                        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                        let esik = Duration::from_secs(g.pencere_bosta_sn);
                        let omur = g.pencereler.entry(etiket.to_string()).or_default();
                        if !gorunur {
                            omur.gizli_saat_eksikse_baslat(Instant::now());
                        }
                        (omur.yok_ediliyor, omur.gizlendi.map(|t| t.elapsed()), esik)
                    };
                    if yok_ediliyor {
                        continue;
                    }
                    if yok_etmeli_mi(gorunur, gizli_sure, esik) {
                        match win.destroy() {
                            Ok(()) => {
                                let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                                g.pencereler
                                    .entry(etiket.to_string())
                                    .or_default()
                                    .yok_etme_baslat();
                            }
                            Err(err) => {
                                tracing::warn!(%err, etiket, "Gizli pencere yok edilemedi");
                            }
                        }
                    }
                }
            });
        }
    });
}

/// Tauri pencereyi `WindowManager`'dan çıkardıktan sonra yaşam döngüsü durumunu sıfırlar
/// ve yok edilme sırasında gelmiş bir yeniden açma isteği varsa uygular.
pub fn pencere_yok_edildi(app: &AppHandle, etiket: &str) {
    let Some(durum) = app.try_state::<MasaustuDurum>() else {
        return;
    };
    let bekleyen = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let acilis = g
            .pencereler
            .get_mut(etiket)
            .and_then(PencereOmru::yok_edildi);
        match acilis {
            Some(BekleyenAcilis::Serit) if !g.serit_gorunur => None,
            diger => diger,
        }
    };
    if let Some(acilis) = bekleyen {
        let durum = durum.inner().clone();
        match acilis {
            BekleyenAcilis::Acilir { tepsi_rect } => acilir_goster(app, &durum, tepsi_rect),
            BekleyenAcilis::Pencere { sekme } => {
                pencere_ac_ve_odakla(app, &durum, sekme.as_deref());
            }
            BekleyenAcilis::Serit => serit_gorunurluk_ayarla(app, &durum, true),
        }
    }
}

// WebView2 oluşturmayı IPC'nin senkron çağrı yığınından çıkarır; tüm girişler
// (tepsi, tek örnek, kısayol, komut) aynı ana olay döngüsünde sıralanır.
pub fn acilir_goster(app: &AppHandle, durum: &MasaustuDurum, rect: Option<tauri::Rect>) {
    let (app_k, durum) = (app.clone(), durum.clone());
    let _ = app.run_on_main_thread(move || acilir_goster_ana_dongude(&app_k, &durum, rect));
}

pub fn pencere_ac_ve_odakla(app: &AppHandle, durum: &MasaustuDurum, sekme: Option<&str>) {
    let (app_k, durum, sekme) = (app.clone(), durum.clone(), sekme.map(str::to_string));
    let _ = app.run_on_main_thread(move || {
        pencere_ac_ve_odakla_ana_dongude(&app_k, &durum, sekme.as_deref())
    });
}

pub fn serit_gorunurluk_ayarla(app: &AppHandle, durum: &MasaustuDurum, gorunur: bool) {
    let (app_k, durum) = (app.clone(), durum.clone());
    let _ = app
        .run_on_main_thread(move || serit_gorunurluk_ayarla_ana_dongude(&app_k, &durum, gorunur));
}

/// Masaüstü çalışma alanı veya tepsi ikonu için fiziksel piksel dikdörtgeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FizikDikdortgen {
    pub x: i32,
    pub y: i32,
    pub genislik: u32,
    pub yukseklik: u32,
}

/// 340 px tepsi açılır penceresini görev çubuğu kenarına ve `calisma_alani` sınırlarına göre konumlar.
pub fn acilir_konum_hesapla(
    calisma_alani: FizikDikdortgen,
    tepsi_konum: Option<FizikDikdortgen>,
    pencere_genislik: u32,
    pencere_yukseklik: u32,
    olcek: f64,
) -> (i32, i32) {
    let kenar_payi = (8.0 * olcek.clamp(1.0, 4.0)).round() as i32;
    let pw = pencere_genislik as i32;
    let ph = pencere_yukseklik as i32;
    let wa_sag = calisma_alani.x + calisma_alani.genislik as i32;
    let wa_alt = calisma_alani.y + calisma_alani.yukseklik as i32;

    let min_x = calisma_alani.x + kenar_payi;
    let max_x = (wa_sag - pw - kenar_payi).max(min_x);
    let min_y = calisma_alani.y + kenar_payi;
    let max_y = (wa_alt - ph - kenar_payi).max(min_y);

    let Some(tr) = tepsi_konum else {
        return (max_x, max_y);
    };

    let mut x = tr.x + (tr.genislik as i32) / 2 - pw / 2;
    let mut y = max_y;

    if tr.y >= wa_alt {
        // Alt görev çubuğu (standart Windows yerleşimi)
        y = max_y;
    } else if tr.y + (tr.yukseklik as i32) <= calisma_alani.y {
        // Üst görev çubuğu
        y = min_y;
    } else if tr.x >= wa_sag {
        // Sağ görev çubuğu
        x = max_x;
        y = tr.y + (tr.yukseklik as i32) / 2 - ph / 2;
    } else if tr.x + (tr.genislik as i32) <= calisma_alani.x {
        // Sol görev çubuğu
        x = min_x;
        y = tr.y + (tr.yukseklik as i32) / 2 - ph / 2;
    }

    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// Şerit penceresinin başlangıç konumunu çalışma alanının sağ-alt üstüne yerleştirir.
pub fn serit_baslangic_konum(
    calisma_alani: FizikDikdortgen,
    serit_genislik: u32,
    serit_yukseklik: u32,
    olcek: f64,
) -> (i32, i32) {
    let kenar_payi = (16.0 * olcek.clamp(1.0, 4.0)).round() as i32;
    let x = (calisma_alani.x + calisma_alani.genislik as i32 - serit_genislik as i32 - kenar_payi)
        .max(calisma_alani.x + kenar_payi);
    let y =
        (calisma_alani.y + calisma_alani.yukseklik as i32 - serit_yukseklik as i32 - kenar_payi)
            .max(calisma_alani.y + kenar_payi);
    (x, y)
}

/// Geri yüklenecek pencere konumunun etkin monitör çalışma alanlarından birinde yeterli
/// tutulabilir alana (en az 80px yatay ve başlık çubuğu erişimi) sahip olup olmadığını denetler (saf).
pub fn geometri_ekrana_uygun_mu(
    calisma_alanlari: &[FizikDikdortgen],
    pos: (i32, i32),
    size: (u32, u32),
) -> bool {
    if calisma_alanlari.is_empty() {
        return true;
    }
    let w = (size.0 as i32).max(1);
    let h = (size.1 as i32).max(1);
    let x1 = pos.0 as i64;
    let x2 = x1 + w as i64;
    let y1 = pos.1 as i64;
    let y2 = y1 + h as i64;
    let min_x_ort = 80i64.min(w as i64);
    let min_y_ort = 32i64.min(h as i64);

    calisma_alanlari.iter().any(|wa| {
        let wx1 = wa.x as i64;
        let wx2 = wx1 + wa.genislik as i64;
        let wy1 = wa.y as i64;
        let wy2 = wy1 + wa.yukseklik as i64;
        let x_ort = x2.min(wx2) - x1.max(wx1);
        let y_ort = y2.min(wy2) - y1.max(wy1);
        x_ort >= min_x_ort && y_ort >= min_y_ort && y1 >= wy1 - 16 && y1 < wy2 - min_y_ort
    })
}

/// Noktayı içeren monitörün sırasını döndürür; hiçbiri içermiyorsa en yakın monitörü seçer (saf).
pub fn noktayi_iceren_monitor(monitorler: &[FizikDikdortgen], x: i32, y: i32) -> Option<usize> {
    let icerir = |m: &FizikDikdortgen| {
        x >= m.x && y >= m.y && x < m.x + m.genislik as i32 && y < m.y + m.yukseklik as i32
    };
    if let Some(i) = monitorler.iter().position(icerir) {
        return Some(i);
    }
    monitorler
        .iter()
        .enumerate()
        .min_by_key(|(_, m)| {
            let dx = (m.x - x)
                .max(0)
                .max(x - (m.x + m.genislik as i32 - 1))
                .max(0) as i64;
            let dy = (m.y - y)
                .max(0)
                .max(y - (m.y + m.yukseklik as i32 - 1))
                .max(0) as i64;
            dx * dx + dy * dy
        })
        .map(|(i, _)| i)
}

fn dikdortgen(
    pos: tauri::PhysicalPosition<i32>,
    boyut: tauri::PhysicalSize<u32>,
) -> FizikDikdortgen {
    FizikDikdortgen {
        x: pos.x,
        y: pos.y,
        genislik: boyut.width,
        yukseklik: boyut.height,
    }
}

/// Tepsi noktasının (yoksa birincil ekranın) monitörünü döndürür: `(çalışma alanı, ölçek)`.
/// Gizli pencerenin `current_monitor`'u kullanılmaz; çoklu monitörde yanlış ekrana kıstırırdı.
fn monitor_bilgisi(
    app: &AppHandle,
    tepsi_nokta: Option<(i32, i32)>,
) -> Option<(FizikDikdortgen, f64)> {
    let monitorler = app.available_monitors().unwrap_or_default();
    let secilen = tepsi_nokta.and_then(|(x, y)| {
        let sinirlar: Vec<FizikDikdortgen> = monitorler
            .iter()
            .map(|m| dikdortgen(*m.position(), *m.size()))
            .collect();
        noktayi_iceren_monitor(&sinirlar, x, y).and_then(|i| monitorler.get(i).cloned())
    });
    let mon = secilen.or_else(|| app.primary_monitor().ok().flatten())?;
    let wa = mon.work_area();
    Some((dikdortgen(wa.position, wa.size), mon.scale_factor()))
}

fn tepsi_merkezi(rect: Option<tauri::Rect>) -> Option<(i32, i32)> {
    let r = rect?;
    let f = tauri_rect_cevir(r, 1.0);
    Some((
        f.x + (f.genislik as i32) / 2,
        f.y + (f.yukseklik as i32) / 2,
    ))
}

/// Tepsi ikonunun bulunduğu monitörün ölçeğini döndürür (ikon DPI'sı buna göre çizilir).
pub fn tepsi_olcegi(app: &AppHandle, durum: &MasaustuDurum) -> f64 {
    let rect = durum
        .ic
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .son_tepsi_rect;
    monitor_bilgisi(app, tepsi_merkezi(rect))
        .map(|(_, o)| o)
        .unwrap_or(1.0)
        .clamp(1.0, 4.0)
}

/// Ana pencereyi gizler (ana olay döngüsüne sıralar; `async` komutlardan da güvenlidir).
pub fn ana_pencereyi_gizle(app: &AppHandle) {
    let app_k = app.clone();
    let _ = app.run_on_main_thread(move || gizle_ana_dongude(&app_k, "pencere"));
}

/// Ana pencereyi ana olay döngüsü içindeyken hemen gizler.
pub fn ana_pencereyi_gizle_simdi(app: &AppHandle) {
    gizle_ana_dongude(app, "pencere");
}

fn pencere_gorunur_ve_canli_mi(app: &AppHandle, durum: &MasaustuDurum, etiket: &str) -> bool {
    let yok_ediliyor = durum
        .ic
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pencereler
        .get(etiket)
        .is_some_and(|o| o.yok_ediliyor);
    if yok_ediliyor {
        return false;
    }
    app.get_webview_window(etiket)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// İkinci örnek başlatıldığında öne çıkarılacak pencereye karar verip gösterir.
pub fn tek_ornek_pencere_goster(app: &AppHandle, durum: &MasaustuDurum) {
    match tek_ornek_hedefi_sec(pencere_gorunur_ve_canli_mi(app, durum, "pencere")) {
        TekOrnekHedefi::AnaPencere => pencere_ac_ve_odakla(app, durum, None),
        TekOrnekHedefi::Acilir => acilir_goster(app, durum, None),
    }
}

fn tauri_rect_cevir(rect: tauri::Rect, olcek: f64) -> FizikDikdortgen {
    let pos = rect.position.to_physical::<i32>(olcek);
    let size = rect.size.to_physical::<u32>(olcek);
    FizikDikdortgen {
        x: pos.x,
        y: pos.y,
        genislik: size.width,
        yukseklik: size.height,
    }
}

/// Tepsi açılır penceresini (`340 px`) tepsiye yapışık şekilde gösterir ve odaklar.
fn acilir_goster_ana_dongude(
    app: &AppHandle,
    durum: &MasaustuDurum,
    tepsi_rect: Option<tauri::Rect>,
) {
    let etkin_rect = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = tepsi_rect {
            g.son_tepsi_rect = Some(r);
        }
        let etkin = g.son_tepsi_rect;
        if g.pencereler
            .entry("acilir".to_string())
            .or_default()
            .acilis_beklet(BekleyenAcilis::Acilir { tepsi_rect })
        {
            return;
        }
        etkin
    };

    let Some(win) = pencere_al(app, "acilir") else {
        return;
    };

    if let Some((wa, mon_olcek)) = monitor_bilgisi(app, tepsi_merkezi(etkin_rect)) {
        // Hedef monitörün ölçeğiyle boyutlandır (pencere başka DPI'lı ekrandan gelebilir).
        let mevcut_olcek = win.scale_factor().unwrap_or(mon_olcek);
        let boyut = win
            .outer_size()
            .map(|b| {
                let oran = mon_olcek / mevcut_olcek.max(0.1);
                tauri::PhysicalSize::new(
                    (b.width as f64 * oran).round() as u32,
                    (b.height as f64 * oran).round() as u32,
                )
            })
            .unwrap_or_else(|_| {
                tauri::PhysicalSize::new((340.0 * mon_olcek) as u32, (540.0 * mon_olcek) as u32)
            });
        let tepsi_fizik = etkin_rect.map(|r| tauri_rect_cevir(r, mon_olcek));
        let (x, y) = acilir_konum_hesapla(wa, tepsi_fizik, boyut.width, boyut.height, mon_olcek);
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    gosterildi(app, &win);
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

/// Tepsi açılır penceresini gizler; görünürse odak kaybı geri-sekmesini engellemek için zaman damgası kaydeder.
pub fn acilir_gizle(app: &AppHandle, durum: &MasaustuDurum) {
    if pencere_gorunur_ve_canli_mi(app, durum, "acilir") {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.son_acilir_gizlenme = Some(Instant::now());
    }
    gizle_ana_dongude(app, "acilir");
}

/// Tepsi sol tıklamasında açılır pencereyi açar veya kapatır.
pub fn acilir_gecis(app: &AppHandle, durum: &MasaustuDurum, tepsi_rect: Option<tauri::Rect>) {
    if let Some(r) = tepsi_rect {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.son_tepsi_rect = Some(r);
    }

    if pencere_gorunur_ve_canli_mi(app, durum, "acilir") {
        acilir_gizle(app, durum);
        return;
    }

    // Odak kaybı nedeniyle tepsi tıklamasından hemen önce (<250ms) kapandıysa tekrar açma.
    let yakin_gizlenme = {
        let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.son_acilir_gizlenme
            .is_some_and(|t| t.elapsed() < Duration::from_millis(250))
    };
    if yakin_gizlenme {
        return;
    }

    acilir_goster(app, durum, tepsi_rect);
}

/// Ana geniş pencereyi açar, odaklar ve istenirse belirtilen sekmeye geçirir.
fn pencere_ac_ve_odakla_ana_dongude(app: &AppHandle, durum: &MasaustuDurum, sekme: Option<&str>) {
    acilir_gizle(app, durum);
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        if g.pencereler
            .entry("pencere".to_string())
            .or_default()
            .acilis_beklet(BekleyenAcilis::Pencere {
                sekme: sekme.map(str::to_string),
            })
        {
            return;
        }
    }
    let Some(win) = pencere_al(app, "pencere") else {
        return;
    };

    let ilk_kez = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        let ilk = !g.pencere_ilk_gosterim;
        g.pencere_ilk_gosterim = true;
        ilk
    };
    if ilk_kez {
        let _ = win.center();
    }
    gosterildi(app, &win);
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();

    if let Some(s) = sekme {
        let hazir = {
            let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
            g.pencereler
                .entry("pencere".to_string())
                .or_default()
                .sekme_iste(s)
        };
        if hazir {
            let _ = app.emit_to("pencere", "pencere_sekme_sec", s);
        }
    }
}

/// `Ctrl+Alt+K` kısayolu davranışı: açık ana/açılır pencere varsa gizler; yoksa açılır pencereyi gösterir.
pub fn hizli_kisayol_gecis(app: &AppHandle, durum: &MasaustuDurum) {
    let (app_k, durum_k) = (app.clone(), durum.clone());
    let _ = app.run_on_main_thread(move || hizli_kisayol_gecis_ana_dongude(&app_k, &durum_k));
}

fn hizli_kisayol_gecis_ana_dongude(app: &AppHandle, durum: &MasaustuDurum) {
    let (pencere_gorunur, pencere_odakli) = if pencere_gorunur_ve_canli_mi(app, durum, "pencere") {
        let odakli = app
            .get_webview_window("pencere")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        (true, odakli)
    } else {
        (false, false)
    };
    let acilir_gorunur = pencere_gorunur_ve_canli_mi(app, durum, "acilir");

    match kisayol_eylemi_sec(pencere_gorunur, pencere_odakli, acilir_gorunur) {
        KisayolEylemi::AnaPencereyiGizle => gizle_ana_dongude(app, "pencere"),
        KisayolEylemi::AcilirGizle => acilir_gizle(app, durum),
        KisayolEylemi::AcilirGoster => acilir_goster_ana_dongude(app, durum, None),
    }
}

/// Şerit penceresinin görünürlüğünü ayarlar ve tepsi menüsü ile ön yüzü senkronize eder.
fn serit_gorunurluk_ayarla_ana_dongude(app: &AppHandle, durum: &MasaustuDurum, gorunur: bool) {
    let (ilk_konum, ustte, gecirgen, menu_oge, bekletildi) = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.serit_gorunur = gorunur;
        let ilk = gorunur && !g.serit_ilk_konum;
        let beklet = {
            let omur = g.pencereler.entry("serit".to_string()).or_default();
            if gorunur {
                omur.acilis_beklet(BekleyenAcilis::Serit)
            } else {
                omur.bekleyen_acilisi_iptal_et();
                false
            }
        };
        (
            ilk,
            g.serit_ustte,
            g.serit_gecirgen,
            g.tepsi_menu.clone(),
            beklet,
        )
    };

    if gorunur && !bekletildi {
        let Some(win) = pencere_al(app, "serit") else {
            let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
            g.serit_gorunur = false;
            return;
        };
        let tepsi = tepsi_merkezi(
            durum
                .ic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .son_tepsi_rect,
        );
        if ilk_konum && let Some((wa, mon_olcek)) = monitor_bilgisi(app, tepsi) {
            let olcek = win.scale_factor().unwrap_or(mon_olcek);
            let boyut = win.outer_size().unwrap_or_else(|_| {
                tauri::PhysicalSize::new((520.0 * olcek) as u32, (44.0 * olcek) as u32)
            });
            let (x, y) = serit_baslangic_konum(wa, boyut.width, boyut.height, olcek);
            let _ = win.set_position(PhysicalPosition::new(x, y));
            durum
                .ic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .serit_ilk_konum = true;
        }
        let _ = win.set_always_on_top(ustte);
        let _ = win.set_ignore_cursor_events(gecirgen);
        gosterildi(app, &win);
        let _ = win.show();
    } else if !gorunur {
        gizle_ana_dongude(app, "serit");
    }

    if let Some(menu) = menu_oge {
        let _ = menu.serit_goster.set_checked(gorunur);
    }

    let _ = app.emit(
        "serit_durumu_degisti",
        SeritDurumu {
            gorunur,
            ustte,
            gecirgen,
        },
    );
}

/// Şerit penceresi `her zaman üstte` ve `tıklama geçirgen` seçeneklerini günceller.
pub fn serit_secenek_uygula(
    app: &AppHandle,
    durum: &MasaustuDurum,
    ustte: Option<bool>,
    gecirgen: Option<bool>,
) -> SeritDurumu {
    let (durum_ozeti, menu_oge, yok_ediliyor) = {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(u) = ustte {
            g.serit_ustte = u;
        }
        if let Some(gec) = gecirgen {
            g.serit_gecirgen = gec;
        }
        let yok_ediliyor = g.pencereler.get("serit").is_some_and(|o| o.yok_ediliyor);
        (
            SeritDurumu {
                gorunur: g.serit_gorunur,
                ustte: g.serit_ustte,
                gecirgen: g.serit_gecirgen,
            },
            g.tepsi_menu.clone(),
            yok_ediliyor,
        )
    };

    if !yok_ediliyor {
        let app_k = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(win) = app_k.get_webview_window("serit") {
                if let Some(u) = ustte {
                    let _ = win.set_always_on_top(u);
                }
                if let Some(gec) = gecirgen {
                    let _ = win.set_ignore_cursor_events(gec);
                }
            }
        });
    }

    if let Some(menu) = menu_oge {
        let _ = menu.serit_gecirgen.set_checked(durum_ozeti.gecirgen);
    }

    let _ = app.emit("serit_durumu_degisti", durum_ozeti.clone());
    durum_ozeti
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yalniz_gizli_ve_suresi_dolmus_pencere_yok_edilir() {
        let esik = Duration::from_secs(60);
        assert!(!yok_etmeli_mi(false, None, esik));
        assert!(!yok_etmeli_mi(false, Some(Duration::from_secs(59)), esik));
        assert!(yok_etmeli_mi(false, Some(esik), esik));
        assert!(!yok_etmeli_mi(true, Some(Duration::from_secs(120)), esik));
        // Yeniden gösterme gizlenme saatini temizler; eski süre yeni pencereyi silemez.
        assert!(!yok_etmeli_mi(true, None, esik));
        assert!(!yok_etmeli_mi(false, Some(Duration::from_secs(1)), esik));
    }

    #[test]
    fn pencere_omru_ayarlanabilir_sureyi_kullanir() {
        let sure = Some(Duration::from_secs(20));
        assert!(yok_etmeli_mi(false, sure, Duration::from_secs(10)));
        assert!(!yok_etmeli_mi(false, sure, Duration::from_secs(30)));
    }

    #[test]
    fn acilir_pencere_alt_gorev_cubugunda_tepsiye_yapisik_konumlanir() {
        let wa = FizikDikdortgen {
            x: 0,
            y: 0,
            genislik: 1920,
            yukseklik: 1032,
        };
        let tr = FizikDikdortgen {
            x: 1750,
            y: 1040,
            genislik: 24,
            yukseklik: 24,
        };
        let (x, y) = acilir_konum_hesapla(wa, Some(tr), 340, 540, 1.0);
        // Tepsi merkezi 1762 - 170 = 1592 ama max_x = 1920 - 340 - 8 = 1572 -> kıstırılır
        assert_eq!(x, 1572);
        assert_eq!(y, 1032 - 540 - 8);
    }

    #[test]
    fn acilir_pencere_ust_gorev_cubugunda_tepeye_konumlanir() {
        let wa = FizikDikdortgen {
            x: 0,
            y: 48,
            genislik: 1920,
            yukseklik: 1032,
        };
        let tr = FizikDikdortgen {
            x: 1600,
            y: 12,
            genislik: 24,
            yukseklik: 24,
        };
        let (x, y) = acilir_konum_hesapla(wa, Some(tr), 340, 540, 1.0);
        assert_eq!(x, 1600 + 12 - 170);
        assert_eq!(y, 48 + 8);
    }

    #[test]
    fn tepsi_noktasi_ikincil_monitordeyse_o_monitor_secilir() {
        let birincil = FizikDikdortgen {
            x: 0,
            y: 0,
            genislik: 1920,
            yukseklik: 1080,
        };
        // Solda, %150 ölçekli 4K ikincil ekran (negatif koordinatlar)
        let ikincil = FizikDikdortgen {
            x: -3840,
            y: -200,
            genislik: 3840,
            yukseklik: 2160,
        };
        let m = [birincil, ikincil];
        assert_eq!(noktayi_iceren_monitor(&m, 1800, 1060), Some(0));
        assert_eq!(noktayi_iceren_monitor(&m, -100, 1900), Some(1));
        // Hiçbirinde değilse en yakın
        assert_eq!(noktayi_iceren_monitor(&m, 1925, 500), Some(0));
        assert_eq!(noktayi_iceren_monitor(&m, -4000, 0), Some(1));
        assert_eq!(noktayi_iceren_monitor(&[], 0, 0), None);
    }

    #[test]
    fn acilir_pencere_sol_ve_sag_gorev_cubugunda_kenara_yapisir() {
        let wa = FizikDikdortgen {
            x: 48,
            y: 0,
            genislik: 1872,
            yukseklik: 1080,
        };
        let sol_tepsi = FizikDikdortgen {
            x: 12,
            y: 1000,
            genislik: 24,
            yukseklik: 24,
        };
        let (x, y) = acilir_konum_hesapla(wa, Some(sol_tepsi), 340, 540, 1.0);
        assert_eq!(x, 48 + 8);
        assert_eq!(y, 1080 - 540 - 8);

        let wa_sag = FizikDikdortgen {
            x: 0,
            y: 0,
            genislik: 1872,
            yukseklik: 1080,
        };
        let sag_tepsi = FizikDikdortgen {
            x: 1884,
            y: 300,
            genislik: 24,
            yukseklik: 24,
        };
        let (x2, y2) = acilir_konum_hesapla(wa_sag, Some(sag_tepsi), 340, 540, 1.0);
        assert_eq!(x2, 1872 - 340 - 8);
        assert_eq!(y2, 300 + 12 - 270);
    }

    #[test]
    fn acilir_pencere_tepsi_rect_yoksa_sag_alt_kosede_acilir() {
        let wa = FizikDikdortgen {
            x: 0,
            y: 0,
            genislik: 2560,
            yukseklik: 1380,
        };
        let (x, y) = acilir_konum_hesapla(wa, None, 510, 810, 1.5);
        assert_eq!(x, 2560 - 510 - 12);
        assert_eq!(y, 1380 - 810 - 12);
    }

    #[test]
    fn yok_edilirken_gelen_acilis_istegi_yok_edilme_sonrasina_ertelenir() {
        let mut omur = PencereOmru::default();
        assert!(!omur.acilis_beklet(BekleyenAcilis::Acilir { tepsi_rect: None }));

        let gizleme = Instant::now();
        omur.gizlendi_kaydet(gizleme, false);
        omur.arayuz_hazir_oldu();
        assert!(omur.hazir);

        omur.yok_etme_baslat();
        assert!(omur.yok_ediliyor);
        assert!(!omur.hazir);
        assert!(omur.gizlendi.is_none());

        // Yok edilme tamamlanana dek yeni açılış isteği beklemeye alınır.
        assert!(omur.acilis_beklet(BekleyenAcilis::Pencere {
            sekme: Some("ayarlar".into()),
        }));
        let bekleyen = omur.yok_edildi();
        assert!(!omur.yok_ediliyor);
        assert!(matches!(
            bekleyen,
            Some(BekleyenAcilis::Pencere { sekme: Some(ref s) }) if s == "ayarlar"
        ));
        // İkinci kez yok_edildi boş döner.
        assert!(omur.yok_edildi().is_none());
    }

    #[test]
    fn yok_edilirken_gizle_cagrisi_bekleyen_acilisi_iptal_eder() {
        let mut omur = PencereOmru::default();
        omur.yok_etme_baslat();
        assert!(omur.acilis_beklet(BekleyenAcilis::Serit));
        omur.bekleyen_acilisi_iptal_et();
        assert!(omur.yok_edildi().is_none());
    }

    #[test]
    fn gizli_saati_eksik_pencere_icin_baslatilir_ama_mevcut_saat_ezilmez() {
        let mut omur = PencereOmru::default();
        let ilk = Instant::now() - Duration::from_secs(30);
        omur.gizlendi_kaydet(ilk, true);
        let sonra = Instant::now();
        omur.gizli_saat_eksikse_baslat(sonra);
        assert_eq!(omur.gizlendi, Some(ilk));

        let mut saatsiz = PencereOmru::default();
        saatsiz.gizli_saat_eksikse_baslat(sonra);
        assert_eq!(saatsiz.gizlendi, Some(sonra));

        saatsiz.yok_etme_baslat();
        saatsiz.gizli_saat_eksikse_baslat(Instant::now());
        assert!(saatsiz.gizlendi.is_none());
    }

    #[test]
    fn sekme_hazir_olmadan_bekletilir_yok_edilince_temizlenir() {
        let mut omur = PencereOmru::default();
        assert!(!omur.sekme_iste("gecmis"));
        omur.yok_etme_baslat();
        assert!(omur.yok_edildi().is_none());
        // Yeni açılan pencerede eski bekleyen sekme taşınmamalı.
        assert_eq!(omur.arayuz_hazir_oldu(), None);
        assert!(omur.sekme_iste("ayarlar"));
    }

    #[test]
    fn geometri_geri_yukleme_pencere_turune_gore_secilir() {
        let omur = PencereOmru {
            konum: Some(tauri::PhysicalPosition::new(120, 80)),
            boyut: Some(tauri::PhysicalSize::new(1024, 768)),
            buyutulmus: true,
            ..Default::default()
        };

        assert_eq!(
            geri_yuklenecek_geometri("pencere", &omur),
            GeometriGeriYukleme {
                konum: Some(tauri::PhysicalPosition::new(120, 80)),
                boyut: Some(tauri::PhysicalSize::new(1024, 768)),
                buyutulmus: true,
            }
        );
        assert_eq!(
            geri_yuklenecek_geometri("serit", &omur),
            GeometriGeriYukleme {
                konum: Some(tauri::PhysicalPosition::new(120, 80)),
                boyut: None,
                buyutulmus: false,
            }
        );
        assert_eq!(
            geri_yuklenecek_geometri("acilir", &omur),
            GeometriGeriYukleme {
                konum: None,
                boyut: None,
                buyutulmus: false,
            }
        );
    }

    #[test]
    fn kisayol_ve_tek_ornek_eylem_secimleri_tutarlidir() {
        assert_eq!(
            kisayol_eylemi_sec(true, true, false),
            KisayolEylemi::AnaPencereyiGizle
        );
        assert_eq!(
            kisayol_eylemi_sec(true, false, true),
            KisayolEylemi::AcilirGizle
        );
        assert_eq!(
            kisayol_eylemi_sec(true, false, false),
            KisayolEylemi::AcilirGoster
        );
        assert_eq!(
            kisayol_eylemi_sec(false, false, false),
            KisayolEylemi::AcilirGoster
        );
        assert_eq!(tek_ornek_hedefi_sec(true), TekOrnekHedefi::AnaPencere);
        assert_eq!(tek_ornek_hedefi_sec(false), TekOrnekHedefi::Acilir);
    }

    #[test]
    fn sokulen_veya_tasinan_monitor_konumu_dislanir() {
        let ekranlar = [FizikDikdortgen {
            x: 0,
            y: 0,
            genislik: 1920,
            yukseklik: 1040,
        }];
        assert!(geometri_ekrana_uygun_mu(&ekranlar, (200, 120), (960, 720)));
        // Sağ taraftan sökülen 2. monitör koordinatı reddedilir
        assert!(!geometri_ekrana_uygun_mu(
            &ekranlar,
            (2100, 120),
            (960, 720)
        ));
        // Başlık çubuğu ekranın çok üstüne taşmış pencere reddedilir
        assert!(!geometri_ekrana_uygun_mu(
            &ekranlar,
            (200, -100),
            (960, 720)
        ));
        // Boş monitör listesinde zorlamadan kabul edilir
        assert!(geometri_ekrana_uygun_mu(&[], (2100, 120), (960, 720)));
    }
}
