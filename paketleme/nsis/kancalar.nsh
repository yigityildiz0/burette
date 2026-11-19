; Cetvel NSIS kurulum kancaları (Tauri 2 `bundle > windows > nsis > installerHooks`).
;
; Bu dosya Tauri şablonunun başında, ${PRODUCTNAME} gibi tanımlardan ÖNCE !include edilir:
; - İşlev gövdeleri burada derlenir, o yüzden onlarda sabit adlar kullanılır.
; - Makrolar eklendikleri yerde (Install/Uninstall bölümleri) açılır; orada şablon tanımları ve
;   değişkenleri ($UpdateMode, $DeleteAppDataCheckboxState, ${MAINBINARYNAME}) kullanılabilir.
;
; Akış
;   kurulum  : (eski servis varsa durdur; başka süreç kilitliyorsa eski kopyayı kenara al)
;              -> cetvel.exe ve lisans belgelerini kopyala -> Tauri dosyaları, kısayol,
;              kaldırma kaydı -> `cetvel kur --uygulama <masaüstü> --baslat`
;              (servis kaydı: Görev Zamanlayıcı kökünde kullanıcının SID'ine bağlı `CetvelServis`; reddedilirse yedek
;              HKCU\...\Run\CetvelServis = `cetvel.exe servis --gizli`; ikisi de yönetici istemez)
;   kaldırma : Tauri dosyaları silinir (açık uygulama kapatılır) -> `cetvel kaldir [--veri-sil]`
;              (görev, eski `Cetvel\CetvelServis` görevi ve Run yedeği birlikte silinir)
;              -> cetvel.exe ve kurulum klasörü silinir
;   sessiz   : kurulum `/S`, kaldırma `/S` (verileri silmek için ayrıca `/VERISIL`)

!include LogicLib.nsh
!include FileFunc.nsh

; cetvel.exe (servis + CLI) `beforeBuildCommand` ile derlenir ve buradan pakete alınır.
; Yol bu dosyanın konumuna göredir (depo kökü/target/release). Tauri `externalBin`/`resources`
; kullanılmadı: ikisi de dosyayı her `cargo build`'de `target/<profil>/cetvel.exe` üzerine
; kopyalayıp geliştirme derlemesindeki gerçek cetvel.exe'yi ezer.
!define CETVEL_CLI_KAYNAK "${__FILEDIR__}\..\..\target\release\cetvel.exe"
; Lisans belgeleri kurulum klasörüne de konur (Hakkında ekranı ikiliye gömülü kopyayı gösterir).
!define CETVEL_DEPO_KOKU "${__FILEDIR__}\..\.."

; Tauri'nin kullanıcı başına varsayılan dizini %LOCALAPPDATA%\Cetvel, Cetvel'in veri klasörüyle
; aynıdır. Programlar Windows'un kullanıcı başına uygulama yerine alınır:
; %LOCALAPPDATA%\Programs\Cetvel. Dizin sayfası açılmadan önce (GUI) burada, sessiz kurulumda
; NSIS_HOOK_PREINSTALL içinde düzeltilir. Kullanıcının seçtiği başka bir dizine dokunulmaz.
!define MUI_CUSTOMFUNCTION_GUIINIT CetvelKurulumDiziniDuzelt

Function CetvelKurulumDiziniDuzelt
  ${If} $INSTDIR == "$LOCALAPPDATA\Cetvel"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Cetvel"
  ${EndIf}
FunctionEnd

; DOSYA yazmaya kapalıysa (çalışan exe) ONEK-<sayaç>.exe adına taşır; açıksa dokunmaz.
; Kullandığı $1 ve $2 korunur.
!macro CetvelKilitliyseKenaraAl DOSYA ONEK
  Push $1
  Push $2
  ClearErrors
  FileOpen $1 "${DOSYA}" a
  ${If} ${Errors}
    System::Call 'kernel32::GetTickCount() i .r2'
    ClearErrors
    Rename "${DOSYA}" "${ONEK}-$2.exe"
    ${If} ${Errors}
      DetailPrint "cetvel.exe: kilitli, kenara alınamadı / locked, could not be moved aside"
    ${Else}
      DetailPrint "cetvel.exe: kullanımda, eski kopya kenara alındı / in use, old copy moved aside"
      Delete "${ONEK}-$2.exe"
    ${EndIf}
  ${Else}
    FileClose $1
  ${EndIf}
  ClearErrors
  Pop $2
  Pop $1
!macroend

!macro NSIS_HOOK_PREINSTALL
  ${If} $INSTDIR == "$LOCALAPPDATA\Cetvel"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Cetvel"
    SetOutPath "$INSTDIR"
  ${EndIf}

  ; Önceki bir yükseltmede kilitli olduğu için kenara alınmış eski kopyalar (artık serbestse).
  Delete "$INSTDIR\cetvel-eski-*.exe"

  ; Yükseltme/yeniden kurma: çalışan servis cetvel.exe'yi kilitler, önce durdurulur.
  ${If} ${FileExists} "$INSTDIR\cetvel.exe"
    ${If} $LANGUAGE = 1055
      DetailPrint "Çalışan Cetvel servisi durduruluyor..."
    ${Else}
      DetailPrint "Stopping the running Cetvel service..."
    ${EndIf}
    nsExec::ExecToLog '"$INSTDIR\cetvel.exe" durdur'
    Pop $0

    ; Servis dışındaki süreçler de cetvel.exe'yi açık tutabilir: Claude Code statusLine köprüsü
    ; (`cetvel kopru statusline`) ya da bir MCP istemcisinin başlattığı `cetvel mcp`. Çalışan bir
    ; exe'nin üzerine yazılamaz ama aynı klasörde yeniden adlandırılabilir; o süreçler eski
    ; kopyayla biter, yeni kopya her durumda yazılır. Kilit yoksa dosyaya dokunulmaz (File üzerine
    ; yazar), böylece kopyalama başarısız olursa eski cetvel.exe yerinde kalır.
    !insertmacro CetvelKilitliyseKenaraAl "$INSTDIR\cetvel.exe" "$INSTDIR\cetvel-eski"
  ${EndIf}

  File "/oname=cetvel.exe" "${CETVEL_CLI_KAYNAK}"
  File "/oname=LICENSE-MIT.txt" "${CETVEL_DEPO_KOKU}\LICENSE-MIT"
  File "/oname=LICENSE-APACHE.txt" "${CETVEL_DEPO_KOKU}\LICENSE-APACHE"
  File "/oname=THIRD-PARTY-NOTICES.txt" "${CETVEL_DEPO_KOKU}\THIRD-PARTY-NOTICES.txt"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $LANGUAGE = 1055
    DetailPrint "Cetvel servisi kaydediliyor (Görev Zamanlayıcı ya da oturum açılışı kaydı, Başlat menüsü kısayolu)..."
  ${Else}
    DetailPrint "Registering the Cetvel service (Task Scheduler or sign-in entry, Start menu shortcut)..."
  ${EndIf}
  nsExec::ExecToLog '"$INSTDIR\cetvel.exe" kur --uygulama "$INSTDIR\${MAINBINARYNAME}.exe" --baslat'
  Pop $0
  ${If} $0 != 0
    ${If} $LANGUAGE = 1055
      DetailPrint "Uyarı: 'cetvel kur' $0 koduyla bitti."
      ${IfNot} ${Silent}
        MessageBox MB_ICONEXCLAMATION|MB_OK "Cetvel kuruldu ama arka plan servisi kaydedilemedi (kod $0).$\n$\nUygulamayı açıp 'Servisi başlat' düğmesini kullanabilir ya da bir terminalde şunu çalıştırabilirsiniz:$\n$\n$\"$INSTDIR\cetvel.exe$\" kur --baslat"
      ${EndIf}
    ${Else}
      DetailPrint "Warning: 'cetvel kur' exited with code $0."
      ${IfNot} ${Silent}
        MessageBox MB_ICONEXCLAMATION|MB_OK "Cetvel is installed but the background service could not be registered (code $0).$\n$\nOpen the app and use 'Start service', or run this in a terminal:$\n$\n$\"$INSTDIR\cetvel.exe$\" kur --baslat"
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Sessiz kaldırmada onay kutusu yok: `/VERISIL` (ya da `/DELETEDATA`) kutunun işaretlenmesiyle
  ; aynıdır. Tauri'nin kutusu (varsayılan: işaretsiz) böylece Cetvel verisini de kapsar.
  ClearErrors
  ${GetOptions} $CMDLINE "/VERISIL" $R0
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState 1
  ${EndIf}
  ClearErrors
  ${GetOptions} $CMDLINE "/DELETEDATA" $R0
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState 1
  ${EndIf}
  ClearErrors
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Masaüstü uygulaması bu noktada kapatılmış ve dosyaları silinmiştir; cetvel.exe kalır.
  ${If} ${FileExists} "$INSTDIR\cetvel.exe"
    ${If} $UpdateMode = 1
      ; Güncelleme: yalnız servis durdurulur; servis kaydı (görev ya da Run), kısayol, ayar ve
      ; veri korunur.
      nsExec::ExecToLog '"$INSTDIR\cetvel.exe" durdur'
      Pop $0
    ${ElseIf} $DeleteAppDataCheckboxState = 1
      ${If} $LANGUAGE = 1055
        DetailPrint "Cetvel kaldırılıyor (veriler, ayarlar ve kayıtlı anahtarlar dahil)..."
      ${Else}
        DetailPrint "Removing Cetvel (including data, settings and saved keys)..."
      ${EndIf}
      nsExec::ExecToLog '"$INSTDIR\cetvel.exe" kaldir --veri-sil'
      Pop $0
    ${Else}
      ${If} $LANGUAGE = 1055
        DetailPrint "Cetvel kaldırılıyor (veriler ve ayarlar korunuyor)..."
      ${Else}
        DetailPrint "Removing Cetvel (keeping data and settings)..."
      ${EndIf}
      nsExec::ExecToLog '"$INSTDIR\cetvel.exe" kaldir'
      Pop $0
    ${EndIf}
    ${If} $0 != 0
      DetailPrint "cetvel: $0"
    ${EndIf}
    Delete "$INSTDIR\cetvel.exe"
    ; Hâlâ duruyorsa başka bir süreç (statusLine köprüsü, `cetvel mcp`) onu çalıştırıyordur.
    ; Kullanıcı başına kaldırıcı "yeniden başlatınca sil" kaydı yazamaz (yönetici ister); kopya
    ; geçici klasöre taşınır ki kurulum klasörü silinebilsin. Taşınamazsa (başka birim) eski yol.
    ${If} ${FileExists} "$INSTDIR\cetvel.exe"
      !insertmacro CetvelKilitliyseKenaraAl "$INSTDIR\cetvel.exe" "$TEMP\cetvel-kaldirilan"
      ${If} ${FileExists} "$INSTDIR\cetvel.exe"
        Delete /REBOOTOK "$INSTDIR\cetvel.exe"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Delete "$INSTDIR\cetvel-eski-*.exe"
  ${If} $UpdateMode <> 1
    Delete "$INSTDIR\LICENSE-MIT.txt"
    Delete "$INSTDIR\LICENSE-APACHE.txt"
    Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  ${EndIf}
  RMDir "$INSTDIR"
!macroend
