#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod durum;
mod ikon;
mod komutlar;
mod metinler;
mod pencere;
mod servis_istemci;

use durum::{MasaustuDurum, TEPSI_KIMLIK, TepsiMenuOgeleri, tepsi_gorunum_guncelle};
use metinler::{DegerYonu, TepsiDili};
use servis_istemci::{AnlikServisYaniti, ServisIstemci};
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

fn tepsi_menu_olustur(
    app: &AppHandle,
    otomatik_baslat_etkin: bool,
    dil: TepsiDili,
) -> tauri::Result<(Menu<tauri::Wry>, TepsiMenuOgeleri)> {
    let t = metinler::menu_metinleri(dil);
    let m_acilir = MenuItem::with_id(app, "acilir", t.acilir, true, None::<&str>)?;
    let m_pencere = MenuItem::with_id(app, "pencere", t.pencere, true, None::<&str>)?;
    let m_serit = CheckMenuItemBuilder::with_id("serit_goster", t.serit)
        .checked(false)
        .build(app)?;
    let m_gecirgen = CheckMenuItemBuilder::with_id("serit_gecirgen", t.serit_gecirgen)
        .checked(false)
        .build(app)?;
    let a1 = PredefinedMenuItem::separator(app)?;
    let m_yenile = MenuItem::with_id(app, "yenile", t.yenile, true, None::<&str>)?;
    let m_servis = MenuItem::with_id(app, "servis_baslat", t.servis_baslat, true, None::<&str>)?;
    let m_otomatik = CheckMenuItemBuilder::with_id("otomatik_baslat", t.otomatik_baslat)
        .checked(otomatik_baslat_etkin)
        .build(app)?;
    let a2 = PredefinedMenuItem::separator(app)?;
    let m_cikis = MenuItem::with_id(app, "cikis", t.cikis, true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &m_acilir,
            &m_pencere,
            &m_serit,
            &m_gecirgen,
            &a1,
            &m_yenile,
            &m_servis,
            &m_otomatik,
            &a2,
            &m_cikis,
        ],
    )?;

    Ok((
        menu,
        TepsiMenuOgeleri {
            acilir: m_acilir,
            pencere: m_pencere,
            serit_goster: m_serit,
            serit_gecirgen: m_gecirgen,
            yenile: m_yenile,
            servis_baslat: m_servis,
            otomatik_baslat: m_otomatik,
            cikis: m_cikis,
        },
    ))
}

fn menu_eylemi_isle(app: &AppHandle, id: &str) {
    let Some(durum) = app.try_state::<MasaustuDurum>() else {
        return;
    };
    match id {
        "acilir" => pencere::acilir_goster(app, &durum, None),
        "pencere" => pencere::pencere_ac_ve_odakla(app, &durum, None),
        "serit_goster" => {
            let yeni = !durum
                .ic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .serit_gorunur;
            pencere::serit_gorunurluk_ayarla(app, &durum, yeni);
        }
        "serit_gecirgen" => {
            let yeni = !durum
                .ic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .serit_gecirgen;
            pencere::serit_secenek_uygula(app, &durum, None, Some(yeni));
        }
        "yenile" => {
            // Sonuç servis yayınlayınca uzun yoklama ile `anlik_degisti` olarak gelir.
            let durum_k = durum.inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = durum_k.istemci.yenile_iste(None).await;
            });
        }
        "servis_baslat" => {
            let app_k = app.clone();
            let durum_k = durum.inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(saglik) = durum_k.istemci.servis_baslat().await {
                    let anlik = durum_k.istemci.anlik_getir(None, 3).await.ok().flatten();
                    {
                        let mut g = durum_k.ic.lock().unwrap_or_else(|e| e.into_inner());
                        g.servis_calisiyor = saglik.calisiyor;
                        if let Some(a) = &anlik {
                            g.son_anlik = Some(a.clone());
                        }
                    }
                    tepsi_gorunum_guncelle(&app_k, &durum_k);
                    let calisiyor = saglik.calisiyor;
                    let hata = saglik.hata.clone();
                    let _ = app_k.emit("servis_durumu_degisti", saglik);
                    let _ = app_k.emit(
                        "anlik_degisti",
                        AnlikServisYaniti {
                            servis_calisiyor: calisiyor,
                            baslatma_onerisi: !calisiyor,
                            anlik,
                            hata,
                        },
                    );
                }
            });
        }
        "otomatik_baslat" => {
            let mevcut = app.autolaunch().is_enabled().unwrap_or(false);
            if let Ok(son) = komutlar::otomatik_baslat_uygula(app, &durum, !mevcut) {
                windows_ile_baslat_kaydet(son);
                let app_k = app.clone();
                let durum_k = durum.inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(y) = cetvel_depo::CetvelYollar::ortamdan() {
                        let (ayarlar, _) = komutlar::ayarlari_yukle(&y);
                        let _ = durum_k.istemci.ayar_yenile().await;
                        let _ = app_k.emit(
                            "ayarlar_degisti",
                            komutlar::arayuz_ayarlari_uret(&ayarlar, None),
                        );
                    }
                });
            }
        }
        "cikis" => app.exit(0),
        _ => {}
    }
}

/// "Windows ile başlat" tercihini `ayarlar.toml`'a yazar (tepsi menüsünden değişince).
fn windows_ile_baslat_kaydet(etkin: bool) {
    // Dosya şu an okunamıyorsa yazılmaz (varsayılanlar kullanıcının dosyasını ezmesin).
    if let Ok(y) = cetvel_depo::CetvelYollar::ortamdan()
        && let Ok(mut ayarlar) = komutlar::ayarlari_yazmak_icin_yukle(&y.ayarlar_dosyasi())
        && ayarlar.masaustu.windows_ile_baslat != etkin
    {
        ayarlar.masaustu.windows_ile_baslat = etkin;
        let _ = ayarlar.kaydet(&y.ayarlar_dosyasi());
    }
}

/// Tepsi ikonunu, tepsinin bulunduğu monitörün ölçeğine göre yeniden çizer.
fn tepsi_olcegini_yenile(app: &AppHandle) {
    let Some(durum) = app.try_state::<MasaustuDurum>() else {
        return;
    };
    let olcek = pencere::tepsi_olcegi(app, &durum);
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.olcek_faktoru = olcek;
    }
    tepsi_gorunum_guncelle(app, &durum);
}

pub fn pencere_olustur(app: &AppHandle, etiket: &str) -> tauri::Result<tauri::WebviewWindow> {
    match etiket {
        "acilir" => {
            let acilir_win = WebviewWindowBuilder::new(
                app,
                "acilir",
                WebviewUrl::App("index.html?yuzey=acilir".into()),
            )
            .title("Cetvel")
            .inner_size(340.0, 540.0)
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .additional_browser_args(pencere::TARAYICI_ARGUMANLARI)
            .build()?;

            let acilir_app = app.clone();
            acilir_win.on_window_event(move |event| {
                let Some(durum) = acilir_app.try_state::<MasaustuDurum>() else {
                    return;
                };
                match event {
                    WindowEvent::Focused(false) => {
                        pencere::acilir_gizle(&acilir_app, &durum);
                    }
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        pencere::acilir_gizle(&acilir_app, &durum);
                    }
                    // Pencerenin kendi ölçeği değil, tepsinin monitörünün ölçeği kullanılır.
                    WindowEvent::ScaleFactorChanged { .. } => tepsi_olcegini_yenile(&acilir_app),
                    _ => {}
                }
            });

            Ok(acilir_win)
        }
        "pencere" => {
            let genis_win = WebviewWindowBuilder::new(
                app,
                "pencere",
                WebviewUrl::App("index.html?yuzey=pencere".into()),
            )
            .title("Cetvel")
            .inner_size(940.0, 700.0)
            .min_inner_size(720.0, 540.0)
            .decorations(true)
            .resizable(true)
            .visible(false)
            .additional_browser_args(pencere::TARAYICI_ARGUMANLARI)
            .build()?;

            let genis_app = app.clone();

            genis_win.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    let tepsiye = genis_app.try_state::<MasaustuDurum>().is_none_or(|d| {
                        d.ic.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .tepsiye_kucult
                    });
                    if tepsiye {
                        api.prevent_close();
                        pencere::ana_pencereyi_gizle_simdi(&genis_app);
                    } else {
                        genis_app.exit(0);
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => tepsi_olcegini_yenile(&genis_app),
                _ => {}
            });

            Ok(genis_win)
        }
        "serit" => {
            let serit_win = WebviewWindowBuilder::new(
                app,
                "serit",
                WebviewUrl::App("index.html?yuzey=serit".into()),
            )
            .title("Cetvel Şerit")
            .inner_size(520.0, 44.0)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .additional_browser_args(pencere::TARAYICI_ARGUMANLARI)
            .build()?;

            let serit_app = app.clone();
            serit_win.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(durum) = serit_app.try_state::<MasaustuDurum>() {
                        pencere::serit_gorunurluk_ayarla(&serit_app, &durum, false);
                    }
                }
            });

            Ok(serit_win)
        }
        _ => Err(tauri::Error::WindowNotFound),
    }
}

fn ilk_kurulum(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();
    let istemci = ServisIstemci::ortamdan().map_err(std::io::Error::other)?;
    let ilk_olcek = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |m| m.scale_factor());

    let durum = MasaustuDurum::yeni(istemci, ilk_olcek);
    let otomatik_baslat_etkin = app_handle.autolaunch().is_enabled().unwrap_or(false);
    // Kalıcı tercih: kapatınca tepsiye küçült. HKCU Run durumu gözlenen gerçektir; dosya ona eşitlenir.
    if let Ok(y) = cetvel_depo::CetvelYollar::ortamdan() {
        let (ayarlar, _) = komutlar::ayarlari_yukle(&y);
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.tepsiye_kucult = ayarlar.masaustu.tepsiye_kucult;
        g.pencere_bosta_sn = ayarlar.masaustu.pencere_bosta_sn;
        g.tepsi_dili = TepsiDili::koddan(&ayarlar.masaustu.dil);
        g.deger_yonu = DegerYonu::koddan(&ayarlar.masaustu.deger_yonu);
    }
    windows_ile_baslat_kaydet(otomatik_baslat_etkin);
    let (tepsi_dili, deger_yonu) = {
        let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        (g.tepsi_dili, g.deger_yonu)
    };
    let (menu, menu_ogeleri) = tepsi_menu_olustur(&app_handle, otomatik_baslat_etkin, tepsi_dili)?;
    {
        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
        g.tepsi_menu = Some(menu_ogeleri);
    }

    let ilk_ikon = ikon::tepsi_ikonu_olustur(
        &ikon::TepsiVerisi {
            deger_yonu,
            ..Default::default()
        },
        ilk_olcek,
    );
    let ilk_ipucu = ikon::ipucu_metni_olustur(None, false, tepsi_dili, deger_yonu);

    app.manage(durum.clone());

    TrayIconBuilder::with_id(TEPSI_KIMLIK)
        .icon(ilk_ikon)
        .tooltip(ilk_ipucu)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app_h = tray.app_handle();
                if let Some(durum) = app_h.try_state::<MasaustuDurum>() {
                    pencere::acilir_gecis(app_h, &durum, Some(rect));
                    tepsi_olcegini_yenile(app_h);
                }
            }
        })
        .on_menu_event(|app_h, event| {
            menu_eylemi_isle(app_h, event.id().as_ref());
        })
        .build(app)?;

    pencere::temizlik_baslat(&app_handle);

    if let Err(err) =
        app_handle
            .global_shortcut()
            .on_shortcut("Ctrl+Alt+K", |app_h, _shortcut, event| {
                if event.state == ShortcutState::Pressed
                    && let Some(durum) = app_h.try_state::<MasaustuDurum>()
                {
                    pencere::hizli_kisayol_gecis(app_h, &durum);
                }
            })
    {
        tracing::warn!(%err, "Ctrl+Alt+K genel kısayolu kaydedilemedi");
    }

    servis_istemci::servis_izleyici_baslat(app_handle.clone(), durum);

    // Başlangıç durumunu arka planda senkronize et (çevrimdışıysa UI'da servis başlatma önerilir).
    let ilk_durum_app = app_handle;
    let ilk_durum_yeni = app.state::<MasaustuDurum>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let saglik = ilk_durum_yeni.istemci.saglik_kontrol().await;
        let anlik = if saglik.calisiyor {
            ilk_durum_yeni
                .istemci
                .anlik_getir(None, 1)
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        {
            let mut g = ilk_durum_yeni.ic.lock().unwrap_or_else(|e| e.into_inner());
            g.servis_calisiyor = saglik.calisiyor;
            if let Some(a) = &anlik {
                g.son_anlik = Some(a.clone());
            }
        }
        tepsi_gorunum_guncelle(&ilk_durum_app, &ilk_durum_yeni);
        let _ = ilk_durum_app.emit("servis_durumu_degisti", saglik.clone());
        let _ = ilk_durum_app.emit(
            "anlik_degisti",
            AnlikServisYaniti {
                servis_calisiyor: saglik.calisiyor,
                baslatma_onerisi: !saglik.calisiyor,
                anlik,
                hata: saglik.hata,
            },
        );
    });

    Ok(())
}

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Masaüstü çalışma zamanı oluşturulamadı");
    tauri::async_runtime::set(runtime.handle().clone());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(durum) = app.try_state::<MasaustuDurum>() {
                pencere::tek_ornek_pencere_goster(app, &durum);
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Cetvel")
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            komutlar::anlik_al,
            komutlar::yenile_iste,
            komutlar::gecmis_al,
            komutlar::servis_durumu,
            komutlar::servis_baslat,
            komutlar::tara,
            komutlar::test_et,
            komutlar::agy_kipi_ayarla,
            komutlar::anahtar_kaydet,
            komutlar::claude_statusline_onizle,
            komutlar::claude_statusline_uygula,
            komutlar::ayarlar_al,
            komutlar::ayar_kaydet,
            komutlar::ayarlari_sifirla,
            komutlar::pencere_ac,
            komutlar::acilir_gizle,
            komutlar::pencere_gizle,
            komutlar::serit_gizle,
            komutlar::serit_durumu_al,
            komutlar::serit_tiklama_gecirgen_ayarla,
            komutlar::serit_ustte_ayarla,
            komutlar::otomatik_baslat_durum,
            komutlar::otomatik_baslat_ayarla,
            komutlar::lisans_metni_al,
            komutlar::gunluk_klasorunu_ac,
            komutlar::gecmis_disa_aktar,
            komutlar::uygulamadan_cik,
        ])
        .setup(ilk_kurulum)
        .build(tauri::generate_context!())
        .expect("Cetvel Tauri uygulaması oluşturulamadı");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Destroyed,
            ..
        } => {
            pencere::pencere_yok_edildi(app_handle, &label);
        }
        tauri::RunEvent::ExitRequested { api, code, .. } if code.is_none() => {
            api.prevent_exit();
        }
        _ => {}
    });
}
