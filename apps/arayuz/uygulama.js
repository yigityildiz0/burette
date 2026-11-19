/* ==========================================================================
   Cetvel — Masaüstü Arayüzü (Tauri 2 + ?demo=1 Önizleme)
   - Dış CDN yok, CSP 'self' uyumlu.
   - Canlı masaüstü kipinde veri yalnız servisten (invoke/listen) gelir.
   - Sahte veri yalnız ?demo=1 kipinde etkinleşir.
   ========================================================================== */

(function () {
  "use strict";

  // --------------------------------------------------------------------------
  // 1. Eşzamanlı Türkçe Kaynak Sözlük (diller/<kod>.json yüklenmeden önce ve
  //    eksik anahtarlar için yedek kaynak)
  // --------------------------------------------------------------------------
  const YEDEK_TR = {
    meta: {
      kod: "tr",
      ad: "Türkçe",
      yerliAd: "Türkçe",
      tamMi: true,
    },
    uygulama: {
      ad: "CETVEL",
      acilirEtiket: "Cetvel Kota Özeti",
      pencereEtiket: "Genişletilmiş Cetvel penceresi",
      seritEtiket: "Cetvel şerit modu",
    },
    proto: {
      baslik: "CETVEL ARAYÜZ",
      yuzeyler: {
        acilir: "Açılır (340 px)",
        pencere: "Pencere",
        serit: "Şerit",
      },
      senaryoBaslik: "Önizleme senaryosu:",
      senaryolar: {
        normal: "Normal",
        sikisik: "Sıkışık",
        bayat: "Bayat",
        hepsi_bitti: "Hepsi bitti",
      },
      senaryoKisayol: "Alt+S ile gizle",
    },
    sekmeler: {
      simdi: "Şimdi",
      gecmis: "Geçmiş",
      bagla: "Bağla",
      ayarlar: "Ayarlar",
    },
    genel: {
      yenile: "Yenile (R)",
      yenileIpucu: "Servis verisini yenile (hız sınırı havuz başına 1/dk)",
      genislet: "Genişlet",
      genisletIpucu: "Genişletilmiş ana pencereyi aç (Enter)",
      genisletKisayol: "Genişlet ↵",
      kucult: "Açılır pencereye dön (Esc)",
      kucultIpucu: "Küçült ve açılır pencereye dön (Esc)",
      tepsiKapali: "Açılır pencere Esc ile kapatıldı.",
      tepsidenAc: "Tepsiden tekrar aç",
      tepsiIkonIpucu: "Görev çubuğu tepsi simgesi önizlemesi",
      kisayolIpucu: "↑/↓ kart seç · ←/→ platform · Enter aç · R yenile · Esc kapat",
      surumIpucu: "Demo senaryo seçici (Alt+S)",
      baglanabilir: "Bağlanabilir ({n})",
      baglanabilirYok: "Tüm kaynaklar bağlı",
      baglanabilirIpucu: "Bağlanmayı bekleyen veya kurulabilecek platformlara git",
      enDar: "en dar: {ad}",
      tumuTukendi: "tüm kotalar tükendi",
      verilerBayat: "veriler bayat",
      bosVeri: "Henüz servis verisi alınmadı. Platform bağlamak için Bağla sekmesini aç.",
      veriYok: "veri yok",
      etkinKalan: "Etkin kalan",
      baglayiciPencereIpucu: "Bu pencere önce dolacağı için kullanılabilir hakkı bu belirliyor",
      belirleyici: "belirleyici",
      belirleyiciIpucu: "Bu pencere önce dolacağı için kullanılabilir hakkı bu belirliyor",
      yenilendi: "Anlık veriler yenilendi (demo önizleme).",
      yenileIstendi: "Servise yenileme isteği gönderildi; güncel okuma birazdan yansır.",
      hizSiniri: "Hız sınırı: Aynı dakika içinde tekrar yenilenemez.",
      hizSiniriSn: "Hız sınırı: {sn} sn sonra yeniden deneyebilirsin.",
      servisYok: "Arka plan servisi yanıt vermiyor.",
      servisKapali: "Arka plan servisi çalışmıyor — kota ve bağla verisi alınamıyor.",
      servisBaslatBtn: "Servisi başlat",
      servisBaslatiliyor: "Arka plan servisi başlatılıyor...",
      servisBaglandi: "Arka plan servisine bağlanıldı.",
      servisBaslatHata: "Arka plan servisi başlatılamadı",
      ayarlarYedeklendi: "Onaylandı — tarihli yedek alındı ve statusLine güncellendi.",
      islemHatasi: "İşlem hatası: {hata}",
      simdi: "şimdi",
      sifirlama: "sıfırlama",
      sifirlamaYok: "sıfırlama: bilinmiyor",
      kalan: "kalan",
      kullanildi: "kullanıldı",
      baglayici: "belirleyici",
      cubukKalan: "Doluluk: kalan",
      cubukKullanilan: "Doluluk: kullanılan",
      cubukTempoKalanIpucu: "Kalan pencere süresi ({oran}): çubukla karşılaştırarak temponu gör",
      cubukTempoGecenIpucu: "Geçen pencere süresi ({oran}): çubukla karşılaştırarak temponu gör",
      deneysel: "deneysel",
      deneyselIpucu: "Sınırlı geçmiş örnekten hesaplanan tahmindir; kesin garanti vermez.",
      bilinmiyor: "bilinmiyor",
      bilinmiyorSaat: "Bitiş tahmini: bilinmiyor",
      gunlukButce: "günlük bütçe {oran}",
      saatlikHiz: "hız: saatte {oran}",
      tukenmeTahmini: "Tükenme tahmini: {zaman}",
      sifirlanmadanBitmez: "Mevcut hızla sıfırlanmadan önce bitmez",
      erkenBitisUyari: "Erken bitiş uyarısı · {zaman}",
      kullanimProjeksiyon: "Bu hızla sıfırlanmaya kadar en çok {oran} kullanırsın",
      onceDolarProjeksiyon: "Bu hızla sıfırlanmadan önce dolar ({zaman})",
      projeksiyonBilinmiyor: "Hız projeksiyonu: bilinmiyor",
      tempoOnde: "Tempo önde ({fark})",
      tempoGeride: "Tempo geride ({fark})",
      tempoDengeli: "Zamanla dengeli",
      tempoBilinmiyor: "Tempo: bilinmiyor",
      tempoIpucu: "Pencere süresine kıyasla şu anki kullanım tempon",
      sifirlanincaAcilir: "Sıfırlanınca açılır: {zaman}",
      sifirlanincaAcilirIpucu: "Bu pencere sıfırlanınca havuz yeniden kullanılabilir olur",
      kilitliHaftaAciklama: "Haftalık limit tükendi — 5s dolu görünse de kapalı (etkin kalan %0).",
      ornekAzNeden: "3'ten az ayrık örnek",
      demoTekOkumaNot: "tek okuma (±{oran} oynak)",
      demoPencereSifirNot: "{pencere} {oran} kalan",
      demoKilitliHaftaNot: "haftalık bitti, 5s dolu görünse de kullanılamaz",
      demoBayatNot: "son okuma {yas} · veri bayat",
      demoAnahtarYokNot: "API anahtarı ayarlı değil",
      demoOpencodeNot: "resmî kota API'si yok; kurulu değil",
      sonOkuma: "okuma {yas}",
      yasSaniye: "{n} sn önce",
      yasDakika: "{n} dk önce",
      yasSaat: "{n} sa önce",
      kalanSureGunSaat: "{g} g {s} sa",
      kalanSureSaatDk: "{s} sa {dk} dk",
      kalanSureDk: "{dk} dk",
      kalanSureAz: "<1 dk",
      kalanSureYok: "bilinmiyor",
      sifirlamaKredisi: "{n} sıfırlama kredisi",
      eszamanliEtiket: "Eşzamanlı",
      seviyeEtiket: "Seviye",
      bitisEtiket: "Bitiş",
      planEtiket: "Plan",
      bakiyeEtiket: "Bakiye",
      hibeEtiket: "hibe",
      yuklenenEtiket: "yüklenen",
      kullanilamazEtiket: "Kullanılamaz",
      kilitliEtiket: "Kilitli",
      muafEtiket: "Kota muaf",
      platformSayi: "{n} havuz",
      ikincilPencereIpucu: "İkincil pencere durumu ({pencere}): {kalan} kalan",
      ikincilPencereKullanilanIpucu: "İkincil pencere durumu ({pencere}): {kullanilan} kullanıldı",
      kotaYok: "Kota yok · kullanım sayacı",
      sayac24: "Son 24 sa: {oturum} oturum · {belirtec} belirteç",
      sayac24Oturum: "Son 24 sa: {oturum} oturum",
      sayac7g: "Son 7 gün: {oturum} oturum · {belirtec} belirteç",
      sayac7gOturum: "Son 7 gün: {oturum} oturum",
      sayacToplam: "Toplam {n} oturum",
      sayacSon: "Son etkinlik: {zaman}",
      kaldi: "{oran} kaldı",
      kullanildiOran: "{oran} kullanıldı",
      sonraSifirlanir: "{sure} sonra sıfırlanır",
      guncellendiAzOnce: "az önce güncellendi",
      guncellendi: "{yas} güncellendi",
      tempoYeter: "Sıfırlamaya kadar yeter",
      tempoBiter: "{zaman} civarı biter",
      tempoYedek: "{oran} yedekte",
      tempoAcik: "{oran} açıkta",
      tempoYedekIpucu: "Doğrusal tempoya göre: kalan payın, kalan süreden ne kadar önde ya da geride",
      enDarIpucu: "Etkin kalan · en dar pencere: {pencere}",
      enDarKullanilanIpucu: "Etkin kullanılan · en dar pencere: {pencere}",
      enDarOzet: "En dar kota: {ad} · {deger} · {sure} sonra",
      enDarOzetSuresiz: "En dar kota: {ad} · {deger}",
      ayrintiAc: "Ayrıntıları göster",
      ayrintiGizle: "Ayrıntıları gizle",
      platformSekmesineGit: "Geçmiş ve ayrıntılar",
      baglaAciklamaKisa: "Bağlantı ayarlarını Bağla sekmesinde yönet",
      kotaYokKisa: "kota yok",
      gecmisBaslik: "Son 30 gün · günlük tepe",
      gecmisGunBilgi: "{gun}: {oran} kullanıldı",
      gecmisVeriYok: "{gun}: veri yok",
      otuzGunOnce: "30 gün önce",
      bugun: "bugün",
      hakkinda: "Hakkında",
      cikis: "Çıkış",
      kisayollar: "Kısayollar",
      kaynakBilgi: "{kaynak} · {guven} · okuma {yas}",
    },
    gorunum: {
      genel: "Genel",
      platformlarAria: "Platformlar",
      gostergeEtiket: "Gösterge",
      gostergeTurleri: {
        halka: "Halka",
        pasta: "Pasta",
        cubuk: "Çubuk",
      },
      pastaAltEtiket: "Pasta biçimi",
      pastaAlt: {
        ayri: "Platform başına ayrı halka",
        tek: "Tek segmentli halka",
      },
      modEtiket: "Yerleşim",
      modlar: {
        birlesik: "Birleşik",
        platform: "Platform başına",
      },
      yonEtiket: "Değer",
      yonler: {
        kalan: "Kalan %",
        kullanilan: "Kullanılan %",
      },
      tumuPlatform: "Tümü",
      segmentHalkaBaslik: "Birleşik Havuz Halkası",
      segmentHalkaAlt: "Her dilim ilgili havuzun etkin kalan payını gösterir; havuzlar birbirine toplanmaz.",
      segmentSeciliIpucu: "Vurgulamak için bir dilime veya etikete odaklan",
      kompaktRozet: "Kompakt",
    },
    baski: {
      rahat: "Rahat",
      dikkat: "Dikkat",
      kacin: "Sıkışık",
      bayat: "Bayat",
      bilinmiyor: "Bilinmiyor",
      baglanmadi: "Bağlı değil",
      kapali: "Kapalı",
      hata: "Hata",
    },
    pencereAdlari: {
      "5dk": "5 dakikalık pencere",
      "5s": "5 saatlik pencere",
      gun: "Günlük pencere",
      hafta: "Haftalık pencere",
      ay: "Aylık pencere",
    },
    pencereEtiket: {
      "5dk": "5 dk",
      "5s": "5 saat",
      gun: "Günlük",
      hafta: "Haftalık",
      ay: "Aylık",
    },
    pencereBaslik: {
      "5dk": "5 dakika",
      "5s": "Oturum (5 saat)",
      gun: "Günlük",
      hafta: "Haftalık",
      ay: "Aylık",
    },
    pencereKisa: {
      "5dk": "5dk",
      "5s": "5s",
      gun: "gün",
      hafta: "hafta",
      ay: "ay",
    },
    pencereIpucu: {
      "5dk": "Kısa süreli istek hızı limiti (5 dakikalık kayan pencere)",
      "5s": "5 saatlik oturum/kota penceresi",
      gun: "24 saatlik günlük token veya istek penceresi",
      hafta: "7 günlük haftalık toplam limit penceresi",
      ay: "Aylık kota penceresi",
    },
    kaynakTur: {
      mod_koprusu: "Eklenti köprüsü",
      statusline: "Claude statusLine",
      codex_app_server: "Codex app-server",
      rollout_yedek: "Yerel oturum yedeği",
      agy_usage: "agy CLI usage",
      resmi_api: "Resmî API",
      bakiye_api: "Bakiye API",
      hesap: "Yerel hesap",
      bilinmiyor: "Bilinmiyor",
      yerel_gunluk: "Yerel oturum kaydı",
    },
    guvenEtiketi: {
      resmi_arayuz: "resmî arayüz",
      resmi_api: "resmî API",
      tahmini: "tahmini",
      bayat: "bayat",
      bilinmiyor: "bilinmiyor",
      yerel_kayit: "Yerel kayıt",
    },
    simdiOzet: {
      kullanilabilir: "Kullanılabilir havuz",
      enDar: "En dar darboğaz",
      tukenmis: "Tükenen / kilitli",
      bayat: "Bayat kaynak",
      baglanabilir: "Bağlanabilir",
    },
    gecmis: {
      baslik: "Son 24 saatlik kullanım",
      aciklama: "Önizleme serisi (?demo=1 · eşik çizgisi ve sıfırlama geçişleriyle)",
      canliAciklama: "Arka plan servisinin SQLite zaman serisi (salt okuma)",
      yukleniyor: "Geçmiş örnekleri yükleniyor...",
      bos: "Bu havuz için henüz geçmiş verisi birikmedi.",
      secimEtiket: "Havuz seçimi",
      esikCizgisi: "Uyarı eşiği ({esik})",
      tepe: "Tepe kullanım",
      ortalama: "Ortalama",
      sifirlamaSayisi: "Sıfırlama sayısı",
      sonDeger: "Son ölçüm",
      noktaBilgi: "{zaman} · {kullanilan} kullanıldı",
      ilkNokta: "24s önce",
    },
    bagla: {
      baslik: "Platform bağlantıları",
      aciklama: "Yalnız varlık kontrolü ve onaylı servis sorgusu yapılır; tarayıcı çerezi ve oturum dosyaları asla okunmaz.",
      tara: "Tara",
      taraniyor: "Taranıyor...",
      sonTaramaEtiket: "Son tarama: {zaman}",
      olayGudumlu: "Olay güdümlü",
      hataYok: "Yok",
      veriKaynagiBaslik: "Bu veri nereden geliyor?",
      sutunKaynak: "Kaynak",
      sutunSonOkuma: "Son okuma",
      sutunSonraki: "Sonraki",
      sutunSonHata: "Son hata",
      sutunGuven: "Güven",
      kopyala: "Kopyala",
      kopyalandi: "Kopyalandı",
      onizle: "Değişikliği önizle",
      onaylaYaz: "Onayla ve yedekleyerek yaz",
      testEt: "Test et",
      testCalisiyor: "Sınanıyor...",
      baglaBtn: "Kaydet",
      kaydet: "Kaydet",
      anahtarYerTutucu: "API anahtarını yapıştır (kaydedilince gizlenir)",
      taranmadi: "Henüz taranmadı",
      tarandi: "Platformlar tarandı.",
      taraIpucu: "Mevcut araçları ve bağlantıları doğrulamak için \"Tara\" düğmesini kullan.",
      kopruBagli: "Bağlı ({yas})",
      kopruYok: "Kurulu · veri bekleniyor",
      claudeYok: "Claude Code kurulu değil",
      statuslineOnizle: "statusLine önizle",
      statuslineNot: "Mevcut statusLine komutu korunarak zincirlenir; onay verdiğinde tarihli yedek alınır.",
      statuslineZaten: "Claude statusLine zaten Cetvel'e bağlı.",
      statuslineYazildi: "Onaylandı — tarihli yedek ({yedek}) alındı ve statusLine yazıldı.",
      codexBulundu: "Bulundu (app-server hazır)",
      codexGiris: "Bulundu · giriş gerek",
      codexYok: "codex.exe bulunamadı",
      agyYok: "agy CLI bulunamadı",
      anahtarVar: "Anahtar kayıtlı",
      anahtarGerek: "API anahtarı gerek",
      anahtarKaydedildi: "Anahtar Windows Credential Manager'a kaydedildi; servise bildirildi.",
      anahtarServisBekliyor: "Anahtar kaydedildi; servis açılınca kullanılacak.",
      anahtarHata: "Anahtar kaydedilemedi: {hata}",
      sonBilinen: "önceki okuma",
      agyRisk: "Uyarı: agy usage çalıştırılması üçüncü taraf araç politikaları açısından risk içerebilir. İzin açıkça verilmedikçe kapalıdır ve hub_*.json dosyaları asla okunmaz.",
      agySecenekler: {
        kapali: "Kapalı",
        istek_uzerine: "İstek üzerine",
        otomatik: "Otomatik",
      },
      platformAdlari: {
        claude: "Claude (Claude Code)",
        codex: "Codex (OpenAI)",
        agy: "Antigravity (agy)",
        deepseek: "DeepSeek",
        opencode: "OpenCode",
        dsh: "DeepSeek Harness",
      },
      yerelVar: "Bulundu · kota yok",
      yerelYok: "Kurulu değil",
      yerelBagli: "Bağlı · kullanım sayacı",
      yerelBagla: "Bağla",
      yerelKes: "Bağlantıyı kes",
      yerelNot: "Resmî kota yok. Cetvel yalnız yerel oturum kayıtlarını sayar (oturum, belirteç); istem ve yanıt içeriği okunmaz.",
      sutunPlatform: "Platform",
      dakikaKisa: "{n} dk",
      dakikaCift: "{etkin} / {bosta} dk",
      devreDisiEtiket: "Ayarlarda kapalı",
    },
    ayarlar: {
      bolumler: {
        genel: "Genel",
        davranis: "Davranış",
        bildirimler: "Bildirimler",
        baglantilar: "Bağlantılar",
        platformlar: "Platformlar",
        teknik: "Teknik",
        hakkinda: "Hakkında ve Yasal",
      },
      hakkindaKisa: "Hakkında",
      grupGorunum: "Görünüm ve dil",
      grupDavranis: "Pencere ve sistem",
      grupSifirla: "Ayarları sıfırla",
      dilAciklama: "Arayüz ve tarih biçimini değiştirir.",
      temaKipAciklama: "Açık, koyu veya Windows temasına göre seç.",
      renkTemasiAciklama: "Vurgu rengi ve yüzey paletini belirler.",
      degerYonuAciklama: "Çubuk ve tepside kalan veya kullanılan yüzdeyi gösterir.",
      windowsIleBaslatNot: "Oturum açıldığında Cetvel arka planda başlatılır.",
      seritAciklama: "Masaüstünün üstünde veya yanında ince kota şeridi.",
      varsayilanaDonBtn: "Varsayılana dön",
      varsayilanaDonNot: "Tüm tercihleri, eşikleri ve kart düzenini başlangıç ayarlarına döndürür.",
      varsayilanaDonOnay: "Sıfırlamayı onayla",
      vazgecBtn: "Vazgeç",
      varsayilanaDonduBildirim: "Ayarlar varsayılan değerlere döndürüldü.",
      grupPlatformAnahtarlari: "Etkin platformlar",
      codexEtkin: "Codex (OpenAI)",
      codexEtkinNot: "Yerel Codex CLI ve app-server kotasını izle.",
      deepseekEtkin: "DeepSeek",
      deepseekEtkinNot: "DeepSeek API bakiye sorgulamasını etkinleştir.",
      agyOkumaNot: "Antigravity (agy) kota okumasını açık tut veya kapat.",
      opencodeEtkin: "OpenCode sayacı",
      opencodeEtkinNot: "Yerel oturum ve belirteç kullanımını say.",
      dshEtkin: "DeepSeek Harness sayacı",
      dshEtkinNot: "DeepSeek Harness günlüklerinden oturum say.",
      grupBildirimGenel: "Bildirim tercihleri",
      grupBildirimTurleri: "Uyarı türleri",
      grupBildirimEsikleri: "Genel uyarı eşikleri",
      grupPlatformBildirimleri: "Platform bildirimleri ve sessize alma",
      platformEsikNot: "Genel eşik: %{genel}. Sessize alınırsa uyarı gönderilmez.",
      platformSessizeAl: "Sessize al",
      grupTeknikServis: "Arka plan servisi ve bağlantılar",
      grupTeknikAralik: "Yoklama ve veri dosyaları",
      codexEtkinDkNot: "Etkin kullanım sırasında Codex okuma sıklığı.",
      codexBostaDkNot: "Bilgisayar boştayken Codex okuma sıklığı.",
      grupHakkindaBilgi: "Uygulama ve gizlilik",
      grupHakkindaLisans: "Lisanslar ve yasal bildirimler",
      yakinda: "yakında",
      yakindaIpucu: "Bu denetim için masaüstü arka uç komutu henüz eklenmedi (yakında etkinleşecek).",
      genelBaslik: "Genel ve Görünüm",
      dilEtiket: "Arayüz dili",
      dilCeviriNotu: "Bu dil için topluluk çevirisi henüz tamamlanmadı; İngilizce yedek metinler gösteriliyor.",
      temaKipEtiket: "Tema kipi",
      temaKipleri: {
        sistem: "Sistemi izle",
        acik: "Açık",
        koyu: "Koyu",
      },
      renkTemasiEtiket: "Renk teması (60-30-10)",
      renkTemalari: {
        terakota: "Terakota",
        okyanus: "Okyanus",
        adacayi: "Adaçayı",
        karbon: "Karbon",
      },
      gostergeEtiket: "Varsayılan gösterge türü",
      pastaAltEtiket: "Pasta görünüm seçimi",
      gorunumModuEtiket: "Kart yerleşimi",
      degerYonuEtiket: "Ana yüzde ölçütü",
      davranisBaslik: "Pencere ve Sistem Davranışı",
      windowsIleBaslat: "Windows açılışında başlat",
      tepsiyeKucult: "Pencereyi kapatınca sistem tepsisine küçült",
      tepsiyeKucultNot: "Varsayılan açık: çarpı tuşu uygulamayı kapatmak yerine tepside çalıştırır.",
      seritBolum: "Şerit penceresi",
      seritAcBtn: "Şeridi göster",
      seritGizleBtn: "Şeridi gizle",
      seritUstte: "Şerit penceresi her zaman üstte",
      seritGecirgen: "Şerit tıklama geçirgen",
      bildirimBaslik: "Eşikler ve Sessiz Saatler",
      bildirimEtkin: "Bildirimler etkin",
      claude5sEsik: "Claude 5 saatlik uyarı eşiği (% kalan)",
      claudeHaftaEsik: "Claude haftalık uyarı eşiği (% kalan)",
      digerEsik: "Diğer platformlar uyarı eşiği (% kalan)",
      esikBildirimi: "Eşik aşımı bildirimi",
      tahminBildirimi: "Erken tükenme tahmin bildirimi",
      krediBildirimi: "Sıfırlama kredisi bildirimi",
      bayatBildirimi: "Veri bayatlama uyarısı",
      sifirlandiBildirimi: "Kota sıfırlama bildirimi",
      sessizSaatler: "Sessiz saatler",
      sessizSaatNot: "Sessiz saatlerde acil olmayan toast bildirimleri ertelenir.",
      ozelSessizSaatler: "Sessiz saat aralığı",
      ozelSessizSaatIpucu: "Yerel saat. Başlangıç ve bitiş aynıysa sessiz saat yoktur; ertelenen uyarı sessiz saat bitince gelir.",
      baglantilarBaslik: "Platform Bağlantıları ve İzinler",
      baglantilarAciklama: "Araç taraması, API anahtarı saklama ve statusLine ayarlarını yönet.",
      baglaSekmesineGit: "Bağla sekmesini aç",
      agyOkumaEtiket: "Antigravity (agy) okuma izni",
      teknikBaslik: "Teknik ve Servis Durumu",
      yenilemeBolum: "Yenileme aralıkları",
      codexEtkinDk: "Codex etkin yenileme aralığı (dk)",
      codexBostaDk: "Codex boşta yenileme aralığı (dk)",
      servisDurumEtiket: "Arka plan servisi",
      servisCalisiyor: "Çalışıyor",
      servisKapaliEtiket: "Çalışmıyor",
      servisDemoEtiket: "Demo önizleme (?demo=1)",
      servisYenidenKontrol: "Durumu yenile",
      servisPort: "Servis yerel HTTP portu (0 = otomatik)",
      otomatikPort: "Boş port otomatik seçilsin",
      portNot: "Port değişikliği servis yeniden başlatıldığında uygulanır.",
      pipeEtiket: "Named Pipe kanalı (salt okuma)",
      sonSiraEtiket: "Son anlık sıra no (salt okuma)",
      saltOkumaEtiket: "Salt okuma",
      gunlukKlasoruBtn: "Günlük klasörünü aç",
      gunlukKlasoruIpucu: "Servis ve masaüstü günlüklerinin klasörünü Dosya Gezgini'nde açar.",
      tarihVeriDisaAktar: "Geçmişi dışa aktar (CSV)",
      tarihVeriIpucu: "Son 90 günün okumalarını CSV dosyasına yazar ve dosyayı Dosya Gezgini'nde gösterir.",
      hakkindaBaslik: "Hakkında, Gizlilik ve Yasal Bilgiler",
      surumEtiket: "Sürüm",
      mimariNot: "Rust + Tauri 2 yerel izleme mimarisi — arka plan servisi okur, arayüz salt gösterir.",
      gizlilikBaslik: "Gizlilik Sözü",
      gizlilikNot: "Veri yalnız bu bilgisayarda kalır, tarayıcı çerezi ve oturum dosyası okunmaz. Üçüncü taraf sunuculara telemetri veya kullanım günlüğü gönderilmez; API anahtarları yalnız Windows Credential Manager üzerinde korunur.",
      lisansBaslik: "Lisans",
      lisansOzeti: "Cetvel, MIT ya da Apache 2.0 lisansıyla (seçim sizin) dağıtılan açık kaynak yazılımdır. Dış CDN ya da uzak betik yüklenmez.",
      ucuncuTarafBaslik: "Üçüncü taraf bileşen lisansları",
      ucuncuTarafListe: "Dağıtılan ikililerdeki tüm Rust bileşenlerinin adı, sürümü ve lisans metni bu belgededir (Cargo.lock'tan üretilir).",
      markaBaslik: "Marka Uyarısı",
      markaUyarisi: "Claude, ChatGPT, Gemini ilgili şirketlerin markalarıdır; Cetvel bağımsızdır.",
      ayarKaydedildi: "Ayarlar kaydedildi.",
      tahminBildirimiNot: "Bu hızla kota sıfırlanmadan bitecekse önceden uyarır; her pencere için bir kez.",
      sessizBaslangic: "Başlangıç",
      sessizBitis: "Bitiş",
      disaAktarildi: "Dışa aktarıldı: {ad}",
      klasorAcildi: "Klasör açıldı",
      lisansMitBtn: "MIT lisansı",
      lisansApacheBtn: "Apache 2.0 lisansı",
      ucuncuTarafBtn: "Bildirimleri aç",
      belgeKapat: "Kapat",
      belgeYukleniyor: "Yükleniyor…",
    },
    serit: {
      baslik: "Şerit Modu (Her zaman üstte)",
      aciklama: "Ekranın üstünde veya yanında ince, sürüklenebilir kota şeridi. Tıklama geçirgen seçildiğinde fare alt pencereye geçer.",
      tiklamaGecirgen: "Tıklama geçirgen (önizleme)",
      acilirDon: "Açılır pencereye dön",
      tutamakIpucu: "Şeridi sürükle",
      genisletIpucu: "Ana pencereyi aç",
      gizleIpucu: "Şeridi gizle",
      gizliMesaj: "Tıklama geçirgen",
    },
    kartlar: {
      duzenle: "Düzenle",
      bitti: "Bitti",
      duzenleIpucu: "Kart sırasını, sabitleri ve görünürlüğü düzenle",
      duzenleAcikNot: "Kartları tutamaçtan sürükle veya Space + ok tuşlarıyla taşı.",
      varsayilanaDon: "Varsayılana dön",
      varsayilanaDondu: "Kart düzeni varsayılana sıfırlandı.",
      tutamakAria: "{ad} kartını sırala",
      tutamakIpucu: "Sürükle veya Space + ok tuşlarıyla taşı",
      sabitle: "Sabitle",
      sabitiKaldir: "Sabitlemeyi kaldır",
      sabitRozet: "Sabit",
      goster: "Göster",
      gizle: "Gizle",
      gizliRozet: "Gizli",
      gosterGizleAria: "{ad} kartını göster veya gizle",
      tumuGizli: "Tüm kartlar gizlendi. Göstermek için Düzenle'yi aç.",
      duyuruAlindi: "{ad} kartı alındı, {sira}. sırada. Ok tuşlarıyla taşı, Space ile bırak, Esc ile iptal et.",
      duyuruTasindi: "{ad} kartı {sira}. sıraya taşındı",
      duyuruBirakildi: "{ad} kartı {sira}. sıraya bırakıldı",
      duyuruIptal: "{ad} kartı taşıma iptal edildi",
      duyuruSinir: "Sabitlenen kartlar her zaman en üstte kalır",
      duyuruSabitlendi: "{ad} kartı en üste sabitlendi",
      duyuruSabitKaldirildi: "{ad} kartının sabitlemesi kaldırıldı",
      duyuruGizlendi: "{ad} kartı gizlendi",
      duyuruGosterildi: "{ad} kartı gösterildi",
    },
    altSayacAdlari: {
      fable_hafta: "Fable haftalık",
      fable: "Fable",
      sonnet: "Sonnet",
      sonnet_hafta: "Sonnet haftalık",
      opus: "Opus",
      opus_hafta: "Opus haftalık",
      cowork: "Cowork",
      cowork_hafta: "Cowork haftalık",
      oauth_apps: "OAuth uygulamaları",
      oauth_apps_hafta: "OAuth uygulamaları haftalık",
    },
    altSayacSozcukler: {
      hafta: "haftalık",
      haftalik: "haftalık",
      weekly: "haftalık",
      gun: "günlük",
      gunluk: "günlük",
      daily: "günlük",
      ay: "aylık",
      aylik: "aylık",
      monthly: "aylık",
      saatlik: "saatlik",
      hourly: "saatlik",
      apps: "uygulamaları",
      app: "uygulama",
      extra: "ekstra",
      ekstra: "ekstra",
      usage: "kullanım",
      kullanim: "kullanım",
      limit: "limit",
      kota: "kota",
      quota: "kota",
    },
    hataTurleri: {
      zaman_asimi: "Zaman aşımı",
      ag_hatasi: "Ağ hatası",
      yetki_hatasi: "Yetki hatası",
      giris_gerek: "Giriş gerek",
      hiz_siniri: "Hız sınırı",
      sunucu_hatasi: "Sunucu hatası",
      ayristirma_hatasi: "Ayrıştırma hatası",
      veritabani_acilamadi: "Veritabanı açılamadı",
      oturum_tablosu_yok: "Oturum tablosu yok",
      zaman_sutunu_yok: "Zaman sütunu yok",
      oturum_sayilamadi: "Oturumlar sayılamadı",
      belirtec_sayilamadi: "Belirteç sayılamadı",
      ileti_okunamadi: "İletiler okunamadı",
      klasor_okunamadi: "Klasör okunamadı",
      okunamadi: "Okunamadı",
      sema_degisti: "Biçim değişti",
      bilinmiyor: "Bilinmiyor",
    },
    aria: {
      satirDurumlu: "{ad} {pencere}: {kullanilanYuzde} kullanıldı, {kalanYuzde} kalan, sıfırlama {sifirlamaMetin}, durum {durumMetin}, kaynak {kaynakMetin} ({yasMetin})",
      satirBilgisiz: "{ad}: durum {durumMetin}, kaynak {kaynakMetin}",
      onizlemeDenetimleri: "Önizleme denetimleri",
      arayuzYuzeyleri: "Arayüz yüzeyleri",
      hizliRenkKip: "Hızlı renk ve kip",
      kartDuzenleme: "Kart düzenleme",
      kotaHavuzaListe: "Kota platformları",
      tepsiOnizleme: "Görev çubuğu tepsi önizlemesi",
      anaSekmeler: "Ana sekmeler",
      ayarKategorileri: "Ayar kategorileri",
    },
  };

  const DESTEKLENEN_DILLER = [
    { kod: "tr", etiket: "Türkçe" },
    { kod: "en", etiket: "English" },
    { kod: "de", etiket: "Deutsch" },
    { kod: "fr", etiket: "Français" },
    { kod: "es", etiket: "Español" },
    { kod: "pt-BR", etiket: "Português (Brasil)" },
    { kod: "ru", etiket: "Русский" },
    { kod: "ja", etiket: "日本語" },
    { kod: "ko", etiket: "한국어" },
    { kod: "zh-Hans", etiket: "简体中文" },
    { kod: "zh-Hant", etiket: "繁體中文" },
  ];
  const dilDestekleniyorMu = (kod) => DESTEKLENEN_DILLER.some((d) => d.kod === kod);

  const RENK_TEMALARI = [
    { id: "terakota", zemin: "#fbf9f5", yuzey: "#ffffff", vurgu: "#b2411f" },
    { id: "okyanus", zemin: "#f4f7fb", yuzey: "#ffffff", vurgu: "#1b54c2" },
    { id: "adacayi", zemin: "#f3f7f4", yuzey: "#ffffff", vurgu: "#1b6b4c" },
    { id: "karbon", zemin: "#f5f5f7", yuzey: "#ffffff", vurgu: "#3f43c4" },
  ];

  const PLATFORM_RENK = {
    claude: "var(--plt-claude)",
    codex: "var(--plt-codex)",
    agy: "var(--plt-agy)",
    deepseek: "var(--plt-deepseek)",
    opencode: "var(--plt-opencode)",
  };

  const PLATFORM_KISA = {
    claude: "CL",
    codex: "CX",
    agy: "AG",
    deepseek: "DS",
    opencode: "OC",
  };

  // --------------------------------------------------------------------------
  // 2. Sahte Demo Verisi (?demo=1 dışı koda ASLA karışmaz)
  // --------------------------------------------------------------------------
  const SENARYO_NORMAL = {
    sema: 2,
    yazan: "cetvel",
    surum: "0.1.0",
    zaman: "2026-11-15T19:50:00Z",
    sira: 1842,
    havuzlar: {
      claude_max: {
        platform: "claude",
        etiket: "Claude Max",
        durum: "ok",
        etkin_kalan: 0.15,
        baglayici_pencere: "5s",
        kullanilabilir: true,
        kaynak: { tur: "mod_koprusu", guven: "resmi_arayuz", okuma: "2026-11-15T19:49:19Z", yas_sn: 41, hassasiyet: 0.1 },
        pencereler: {
          "5s": { uzunluk_dk: 300, kullanilan: 0.85, kalan: 0.15, sifirlama: "2026-11-15T21:10:00Z" },
          hafta: { uzunluk_dk: 10080, kullanilan: 0.39, kalan: 0.61, sifirlama: "2026-11-20T07:00:00Z" },
        },
        alt_sayaclar: {
          fable_hafta: { kullanilan: 0.12, kalan: 0.88, uzunluk_dk: 10080, sifirlama: "2026-11-20T07:00:00Z" },
        },
        tahmin: {
          "5s": {
            durum: "deneysel",
            etiket: "deneysel",
            bitis_tahmini: "2026-11-15T20:22:00Z",
            bitis_erken: "2026-11-15T20:10:00Z",
            bitis_gec: "2026-11-15T20:44:00Z",
            saatlik_hiz: 0.281,
            sifirlanmadan_biter: true,
          },
          hafta: { durum: "deneysel", etiket: "deneysel", gunluk_butce: 0.136, sifirlanmadan_biter: false },
        },
        baski: "kacin",
        hata: null,
        notlar: [],
      },
      codex_plus: {
        platform: "codex",
        etiket: "Codex",
        durum: "ok",
        etkin_kalan: 0.0,
        baglayici_pencere: "hafta",
        kullanilabilir: false,
        kaynak: { tur: "codex_app_server", guven: "resmi_arayuz", okuma: "2026-11-15T19:48:00Z", yas_sn: 120 },
        pencereler: {
          "5s": { uzunluk_dk: 300, kullanilan: 0.0, kalan: 1.0, sifirlama: "2026-11-16T00:50:00Z" },
          hafta: { uzunluk_dk: 10080, kullanilan: 1.0, kalan: 0.0, sifirlama: "2026-11-18T10:28:00Z" },
        },
        tahmin: {},
        baski: "kacin",
        ek: { sifirlama_kredisi: { adet: 1, son: "2026-11-22T00:00:00Z" }, olagan_kullanim: false, plan_turu: "Plus" },
        hata: null,
        notlar: ["haftalık bitti, 5s dolu görünse de kullanılamaz"],
      },
      agy_gemini: {
        platform: "agy",
        etiket: "Antigravity Gemini",
        durum: "ok",
        etkin_kalan: 0.46,
        baglayici_pencere: "5s",
        kullanilabilir: true,
        kaynak: { tur: "agy_usage", guven: "tahmini", okuma: "2026-11-15T19:41:20Z", yas_sn: 520 },
        pencereler: {
          "5s": { uzunluk_dk: 300, kullanilan: 0.54, kalan: 0.46, sifirlama: "2026-11-15T22:40:00Z" },
          hafta: { uzunluk_dk: 10080, kullanilan: 0.34, kalan: 0.66, sifirlama: "2026-11-21T07:00:00Z" },
        },
        tahmin: {
          "5s": {
            durum: "deneysel",
            etiket: "deneysel",
            bitis_tahmini: "2026-11-15T23:15:00Z",
            bitis_erken: "2026-11-15T22:25:00Z",
            bitis_gec: "2026-11-16T00:10:00Z",
            saatlik_hiz: 0.135,
            sifirlanmadan_biter: false,
          },
        },
        baski: "rahat",
        hata: null,
        notlar: ["tek okuma (±%1 oynak)"],
      },
      agy_claude_gpt: {
        platform: "agy",
        etiket: "Antigravity Claude-GPT",
        durum: "ok",
        etkin_kalan: 0.0,
        baglayici_pencere: "5s",
        kullanilabilir: false,
        kaynak: { tur: "agy_usage", guven: "tahmini", okuma: "2026-11-15T19:41:20Z", yas_sn: 520 },
        pencereler: {
          "5s": { uzunluk_dk: 300, kullanilan: 1.0, kalan: 0.0, sifirlama: "2026-11-15T21:55:00Z" },
          hafta: { uzunluk_dk: 10080, kullanilan: 0.72, kalan: 0.28, sifirlama: "2026-11-21T07:00:00Z" },
        },
        tahmin: {},
        baski: "kacin",
        hata: null,
        notlar: ["5s penceresi %0 kalan"],
      },
      agy_argon: {
        platform: "agy",
        etiket: "Antigravity Argon",
        durum: "bayat",
        etkin_kalan: 0.62,
        baglayici_pencere: "hafta",
        kullanilabilir: false,
        kaynak: { tur: "agy_usage", guven: "bayat", okuma: "2026-11-15T19:11:00Z", yas_sn: 2340 },
        pencereler: { hafta: { uzunluk_dk: 10080, kullanilan: 0.38, kalan: 0.62, sifirlama: "2026-11-22T07:00:00Z" } },
        tahmin: {},
        baski: "rahat",
        hata: { tur: "zaman_asimi" },
        notlar: ["son okuma 39 dk önce · veri bayat"],
      },
      deepseek: {
        platform: "deepseek",
        etiket: "DeepSeek",
        durum: "baglanmadi",
        kullanilabilir: false,
        bakiye: null,
        hata: null,
        notlar: ["API anahtarı ayarlı değil"],
      },
      opencode: {
        platform: "opencode",
        etiket: "OpenCode",
        durum: "ok",
        kullanilabilir: true,
        kaynak: { tur: "yerel_gunluk", guven: "yerel_kayit", okuma: "2026-11-15T19:50:00Z", yas_sn: 0 },
        ek: {
          kullanim: {
            kota_yok: true,
            oturum_24s: 4,
            oturum_7g: 23,
            oturum_toplam: 187,
            belirtec_24s: 1284000,
            belirtec_7g: 9630500,
            son_etkinlik: "2026-11-15T19:42:00Z",
          },
        },
        hata: null,
        notlar: ["kota yok, kullanım sayacı"],
      },
      dsh: {
        platform: "dsh",
        etiket: "DeepSeek Harness",
        durum: "baglanmadi",
        kullanilabilir: false,
        hata: null,
        notlar: ["DeepSeek Harness kurulu değil"],
      },
    },
  };

  function klonla(obj) {
    return JSON.parse(JSON.stringify(obj));
  }

  function senaryoUret(ad, temel) {
    const kopya = klonla(temel);
    const h = kopya.havuzlar;
    if (ad === "sikisik") {
      if (h.agy_gemini) {
        h.agy_gemini.pencereler["5s"].kullanilan = 0.89;
        h.agy_gemini.pencereler["5s"].kalan = 0.11;
        h.agy_gemini.etkin_kalan = 0.11;
        h.agy_gemini.baski = "kacin";
        h.agy_gemini.tahmin["5s"] = {
          durum: "deneysel",
          etiket: "deneysel",
          bitis_tahmini: "2026-11-15T20:14:00Z",
          saatlik_hiz: 0.34,
          sifirlanmadan_biter: true,
        };
      }
    } else if (ad === "bayat") {
      for (const havuz of Object.values(h)) {
        if (havuz.durum === "ok") {
          havuz.durum = "bayat";
          havuz.kullanilabilir = false;
          if (havuz.kaynak) {
            havuz.kaynak.guven = "bayat";
            havuz.kaynak.yas_sn = 3120;
          }
          havuz.notlar = ["son okuma 52 dk önce · veri bayat"];
        }
      }
    } else if (ad === "hepsi_bitti") {
      for (const havuz of Object.values(h)) {
        if (havuz.pencereler) {
          for (const olcum of Object.values(havuz.pencereler)) {
            olcum.kullanilan = 1.0;
            olcum.kalan = 0.0;
          }
          havuz.etkin_kalan = 0.0;
          havuz.kullanilabilir = false;
          havuz.baski = "kacin";
        }
      }
    }
    return kopya;
  }

  const GECMIS_SERILER = {
    claude_max: {
      ad: "Claude Max",
      pencere: "5s",
      platform: "claude",
      esik: 85,
      noktalar: [
        18, 22, 26, 30, 38, 44, 51, 58, 66, 73, 81, 88, 94, 0, 4, 8, 12, 16, 18, 20, 22, 25, 29, 34, 39, 43,
        48, 52, 57, 61, 66, 70, 74, 77, 79, 81, 83, 85, 85, 86, 84, 79, 71, 64, 72, 78, 82, 85,
      ],
    },
    codex_plus: {
      ad: "Codex",
      pencere: "hafta",
      platform: "codex",
      esik: 80,
      noktalar: [
        62, 63, 64, 65, 67, 68, 70, 72, 74, 75, 77, 78, 80, 81, 82, 84, 85, 86, 88, 89, 90, 91, 92, 93, 94, 95,
        96, 97, 98, 99, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
      ],
    },
    agy_gemini: {
      ad: "Antigravity Gemini",
      pencere: "5s",
      platform: "agy",
      esik: 80,
      noktalar: [
        10, 14, 18, 22, 26, 28, 31, 35, 39, 44, 48, 52, 55, 59, 63, 68, 0, 5, 9, 13, 16, 20, 24, 27, 30, 32,
        34, 36, 38, 40, 41, 43, 44, 46, 47, 48, 49, 50, 51, 51, 52, 52, 53, 53, 53, 54, 54, 54,
      ],
    },
  };

  // --------------------------------------------------------------------------
  // 3. Uygulama Durumu & Yerel Tercihler (localStorage)
  // --------------------------------------------------------------------------
  const TERCIH_ANAHTARI = "cetvel.ui.tercihler.v2";
  const KART_TERCIH_ANAHTARI = "cetvel.ui.kartlar.v1";
  const urlParams = new URLSearchParams(window.location.search);
  const demoMuBaslangic = urlParams.get("demo") === "1";
  const tauriVarMiBaslangic = Boolean(window.__TAURI__?.core?.invoke);
  const yuzeyParam = urlParams.get("yuzey");
  const baslangicYuzey =
    yuzeyParam === "pencere" || yuzeyParam === "serit" || yuzeyParam === "acilir" ? yuzeyParam : "acilir";

  function varsayilanTercihler() {
    return {
      uiDil: "tr",
      renkTemasi: "terakota",
      gorunumKipi: "koyu",
      gostergeTuru: "cubuk",
      pastaAltTuru: "ayri",
      gorunumModu: "birlesik",
      degerYonu: "kalan",
      ayarBolum: "genel",
    };
  }

  function tercihleriOku() {
    const temel = varsayilanTercihler();
    try {
      const ham = window.localStorage.getItem(TERCIH_ANAHTARI);
      if (!ham) return temel;
      const yuklenen = JSON.parse(ham);
      if (yuklenen && typeof yuklenen === "object") {
        Object.assign(temel, yuklenen);
      }
    } catch (_) {
      // Depolama erişilemiyorsa varsayılanlarla devam et
    }
    return temel;
  }

  function tercihleriYaz(tercih) {
    try {
      window.localStorage.setItem(TERCIH_ANAHTARI, JSON.stringify(tercih));
    } catch (_) {
      // Yoksay
    }
  }

  function varsayilanKartTercihleri() {
    return { kartSirasi: [], gizliKartlar: [], sabitKartlar: [] };
  }

  function kartTercihTemizle(ham) {
    const diziTemizle = (d) => {
      if (!Array.isArray(d)) return [];
      const gorulen = new Set();
      const sonuc = [];
      for (const oge of d) {
        if (typeof oge === "string" && /^[a-z0-9_]{1,40}$/.test(oge) && !gorulen.has(oge)) {
          gorulen.add(oge);
          sonuc.push(oge);
          if (sonuc.length >= 64) break;
        }
      }
      return sonuc;
    };
    return {
      kartSirasi: diziTemizle(ham?.kartSirasi),
      gizliKartlar: diziTemizle(ham?.gizliKartlar),
      sabitKartlar: diziTemizle(ham?.sabitKartlar),
    };
  }

  function kartTercihleriniOku() {
    try {
      const ham = window.localStorage.getItem(KART_TERCIH_ANAHTARI);
      if (!ham) return varsayilanKartTercihleri();
      const yuklenen = JSON.parse(ham);
      if (yuklenen && typeof yuklenen === "object") {
        return kartTercihTemizle(yuklenen);
      }
    } catch (_) {
      // Depolama erişilemiyorsa varsayılanlarla devam et
    }
    return varsayilanKartTercihleri();
  }

  function kartTercihleriniYaz(tercih) {
    try {
      window.localStorage.setItem(KART_TERCIH_ANAHTARI, JSON.stringify(kartTercihTemizle(tercih)));
    } catch (_) {
      // Yoksay
    }
  }

  let ayarKaydetKuyruk = Promise.resolve();
  function kartTercihleriniKaydet() {
    const temiz = kartTercihTemizle(durum.kartTercihler);
    durum.kartTercihler = temiz;
    kartTercihleriniYaz(temiz);
    if (canliMi()) {
      ayarKaydet(
        {
          kartSirasi: [...temiz.kartSirasi],
          gizliKartlar: [...temiz.gizliKartlar],
          sabitKartlar: [...temiz.sabitKartlar],
        },
        false
      );
    }
  }

  function ayarBolumuNormalizeEt(bolum) {
    if (bolum === "davranis") return "genel";
    if (bolum === "baglantilar") return "platformlar";
    const gecerli = ["genel", "platformlar", "bildirimler", "teknik", "hakkinda"];
    return gecerli.includes(bolum) ? bolum : "genel";
  }

  function bosAnlikOlustur() {
    return { sema: 2, yazan: "cetvel", surum: "0.1.0", zaman: new Date().toISOString(), sira: 0, havuzlar: {} };
  }

  const baslangicTercih = tercihleriOku();
  const durum = {
    demoMu: demoMuBaslangic,
    tauriVarMi: tauriVarMiBaslangic,
    dil: baslangicTercih.uiDil,
    sozlukCache: { tr: YEDEK_TR },
    aktifSozluk: YEDEK_TR,
    dilTamMi: true,
    renkTemasi: baslangicTercih.renkTemasi,
    gorunumKipi: baslangicTercih.gorunumKipi,
    gostergeTuru: baslangicTercih.gostergeTuru,
    pastaAltTuru: baslangicTercih.pastaAltTuru,
    gorunumModu: baslangicTercih.gorunumModu,
    platformSekme: "genel",
    degerYonu: baslangicTercih.degerYonu,
    ayarBolum: ayarBolumuNormalizeEt(baslangicTercih.ayarBolum),
    vurguluSegmentId: null,
    tema: "murekkep",
    yuzey: baslangicYuzey,
    sekme: "simdi",
    senaryo: "normal",
    temelAnlik: demoMuBaslangic ? klonla(SENARYO_NORMAL) : bosAnlikOlustur(),
    anlik: demoMuBaslangic ? anlikSadelestir(klonla(SENARYO_NORMAL)) : bosAnlikOlustur(),
    seciliHavuzId: demoMuBaslangic ? "claude" : null,
    acikPlatformlar: new Set(),
    ayarSifirlaOnay: false,
    gecmisHavuzId: "claude_max",
    gecmisCanli: {},
    gunlukGecmis: {},
    acilirKapali: false,
    duzenleModu: false,
    kartTercihler: kartTercihleriniOku(),
    senaryoAcik: false,
    sonYenileMs: 0,
    tiklamaGecirgen: false,
    servisDurumu: {
      calisiyor: demoMuBaslangic || !tauriVarMiBaslangic,
      baslatmaOnerisi: false,
      baslatiliyor: false,
      port: demoMuBaslangic ? 43117 : null,
      pipe: demoMuBaslangic ? "\\\\.\\pipe\\cetvel-demo" : null,
      sonSira: demoMuBaslangic ? 1842 : null,
      hata: null,
    },
    ayarUyarisi: null,
    lisansBelge: null,
    gecmisYukleniyor: null,
    ayarlar: {
      claude5sEsik: 15,
      claudeHaftaEsik: 30,
      digerEsik: 20,
      platformEsikleri: {},
      sessizPlatformlar: [],
      bildirimEtkin: true,
      windowsIleBaslat: false,
      tepsiyeKucult: true,
      servisPort: 43117,
      otomatikPort: true,
      codexEtkin: true,
      codexEtkinDk: 5,
      codexBostaDk: 30,
      deepseekEtkin: false,
      esikBildirimi: true,
      tahminBildirimi: true,
      krediBildirimi: true,
      bayatBildirimi: true,
      sifirlandiBildirimi: true,
      sessizSaatler: true,
      sessizBaslangic: 0,
      sessizBitis: 8,
      agyOkuma: demoMuBaslangic ? "istek_uzerine" : "kapali",
      seritUstte: true,
      opencodeEtkin: demoMuBaslangic,
      dshEtkin: false,
    },
    baglaDurum: {
      claudeFarkAcik: false,
      claudeOnaylandi: false,
      deepseekAnahtarVar: false,
      deepseekSonuc: null,
      codexSonuc: null,
      agySonuc: null,
      sonTarama: null,
      tarama: null,
      taraniyor: false,
      canliSonuc: {},
      testCalisiyor: {},
      statusline: null,
      statuslineSonuc: null,
    },
  };

  function tercihSenkronizeKaydet() {
    tercihleriYaz({
      uiDil: durum.dil,
      renkTemasi: durum.renkTemasi,
      gorunumKipi: durum.gorunumKipi,
      gostergeTuru: durum.gostergeTuru,
      pastaAltTuru: durum.pastaAltTuru,
      gorunumModu: durum.gorunumModu,
      degerYonu: durum.degerYonu,
      ayarBolum: durum.ayarBolum,
    });
  }

  // --------------------------------------------------------------------------
  // 4. i18n Çözücü, Intl Biçimleyiciler (Sistem Saat Dilimi) ve Temel Yardımcılar
  // --------------------------------------------------------------------------
  const sistemKoyuMuMedya = window.matchMedia("(prefers-color-scheme: dark)");

  function temaNitelikleriniUygula() {
    const html = document.documentElement;
    const mod =
      durum.gorunumKipi === "sistem"
        ? sistemKoyuMuMedya.matches
          ? "koyu"
          : "acik"
        : durum.gorunumKipi === "koyu"
        ? "koyu"
        : "acik";
    html.dataset.renk = RENK_TEMALARI.some((r) => r.id === durum.renkTemasi) ? durum.renkTemasi : "terakota";
    html.dataset.kip = durum.gorunumKipi;
    html.dataset.mod = mod;
    html.dataset.tema = mod === "koyu" ? "murekkep" : "kagit";
    html.lang = durum.dil;
  }

  function hamYoldanAl(nesne, yol) {
    if (!nesne || !yol) return undefined;
    const parcalar = yol.split(".");
    let cur = nesne;
    for (const p of parcalar) {
      if (cur && typeof cur === "object" && p in cur) {
        cur = cur[p];
      } else {
        return undefined;
      }
    }
    return cur;
  }

  function cevirMetinTemizle(deger) {
    if (typeof deger !== "string") return "";
    return deger.startsWith("__CEVIRI__") ? deger.slice(10).trimStart() : deger;
  }

  function metinYuzdeNormalize(metin) {
    if (typeof metin !== "string" || !metin.includes("%")) return metin;
    return metin
      .replace(/(^|[^\d])(?:%\s*100|100\s*%)(?=$|[^\d])/g, (_, on) => `${on}${yuzdeBicimle(1)}`)
      .replace(/(^|[^\d])(?:%\s*0|0\s*%)(?=$|[^\d])/g, (_, on) => `${on}${yuzdeBicimle(0)}`);
  }

  function t(yol, varsayilan) {
    let val = hamYoldanAl(durum.aktifSozluk, yol);
    if (val === undefined) val = hamYoldanAl(YEDEK_TR, yol);
    if (typeof val === "string") return metinYuzdeNormalize(cevirMetinTemizle(val));
    return varsayilan !== undefined ? varsayilan : yol;
  }

  function fmt(yol, params) {
    let sablon = t(yol);
    if (!params) return sablon;
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined && v !== null && String(v).includes("%")) {
        const re = new RegExp(`([+\\-%]\\s*)?\\{${k}\\}(\\s*%)?`, "g");
        sablon = sablon.replace(re, `{${k}}`);
      }
    }
    return sablon.replace(/\{([a-zA-Z0-9_]+)\}/g, (_, k) =>
      k in params && params[k] !== undefined && params[k] !== null ? String(params[k]) : ""
    );
  }

  async function dilYukleVeUygula(kod) {
    const gecerliKod = DESTEKLENEN_DILLER.some((d) => d.kod === kod) ? kod : "tr";
    durum.dil = gecerliKod;
    tercihSenkronizeKaydet();

    if (durum.sozlukCache[gecerliKod]) {
      durum.aktifSozluk = durum.sozlukCache[gecerliKod];
      durum.dilTamMi = Boolean(durum.aktifSozluk?.meta?.tamMi);
      temaNitelikleriniUygula();
      tumunuCiz();
      return;
    }

    if (gecerliKod === "tr") {
      durum.aktifSozluk = YEDEK_TR;
      durum.dilTamMi = true;
      temaNitelikleriniUygula();
      tumunuCiz();
      return;
    }

    try {
      const yanit = await fetch(`./diller/${encodeURIComponent(gecerliKod)}.json`, { cache: "no-store" });
      if (yanit.ok) {
        const paket = await yanit.json();
        durum.sozlukCache[gecerliKod] = paket;
        durum.aktifSozluk = paket;
        durum.dilTamMi = Boolean(paket?.meta?.tamMi);
      } else {
        durum.aktifSozluk = YEDEK_TR;
        durum.dilTamMi = gecerliKod === "tr";
      }
    } catch (_) {
      durum.aktifSozluk = YEDEK_TR;
      durum.dilTamMi = gecerliKod === "tr";
    }
    temaNitelikleriniUygula();
    tumunuCiz();
  }

  function clamp(v, min, max) {
    return Math.max(min, Math.min(max, v));
  }

  function yuzde(oran) {
    if (typeof oran !== "number" || !Number.isFinite(oran)) return null;
    return Math.round(clamp(oran, 0, 1) * 100);
  }

  function yuzdeBicimle(oran, opts) {
    if (typeof oran !== "number" || !Number.isFinite(oran)) {
      const bilin = hamYoldanAl(durum.aktifSozluk, "genel.bilinmiyor") ?? hamYoldanAl(YEDEK_TR, "genel.bilinmiyor");
      return typeof bilin === "string" ? cevirMetinTemizle(bilin) : "—";
    }
    const guvenli = clamp(oran, -10, 10);
    const ayar = Object.assign({ style: "percent", maximumFractionDigits: 0 }, opts || {});
    try {
      return new Intl.NumberFormat(durum.dil, ayar).format(guvenli);
    } catch (_) {
      return `${Math.round(guvenli * 100)}%`;
    }
  }

  const KISALTMA_SOZLUK = {
    oauth: "OAuth",
    api: "API",
    cli: "CLI",
    gpt: "GPT",
    ui: "UI",
    mcp: "MCP",
    sdk: "SDK",
  };

  function sozlukEslesmeBul(bolum, anahtar) {
    if (!anahtar) return null;
    let val = hamYoldanAl(durum.aktifSozluk, `${bolum}.${anahtar}`);
    if (typeof val !== "string") val = hamYoldanAl(YEDEK_TR, `${bolum}.${anahtar}`);
    return typeof val === "string" ? cevirMetinTemizle(val) : null;
  }

  function anahtarAdiOkunurYap(ham, bolum) {
    if (ham === undefined || ham === null) return t("genel.bilinmiyor");
    const metin = String(ham).trim();
    if (!metin) return t("genel.bilinmiyor");
    if (bolum) {
      const dogrudan = sozlukEslesmeBul(bolum, metin);
      if (dogrudan) return dogrudan;
    }
    const parcalar = metin.split(/[_\-\s]+/).filter(Boolean);
    if (parcalar.length === 0) return metin;
    return parcalar
      .map((p, idx) => {
        const kucuk = p.toLowerCase();
        if (KISALTMA_SOZLUK[kucuk]) return KISALTMA_SOZLUK[kucuk];
        const cevrilen = sozlukEslesmeBul("altSayacSozcukler", kucuk);
        if (cevrilen) {
          return idx === 0 ? cevrilen.charAt(0).toLocaleUpperCase(durum.dil) + cevrilen.slice(1) : cevrilen;
        }
        if (idx === 0) {
          return p.charAt(0).toLocaleUpperCase(durum.dil) + p.slice(1);
        }
        return p;
      })
      .join(" ");
  }

  function altSayacAdiBicimle(hamAd) {
    return anahtarAdiOkunurYap(hamAd, "altSayacAdlari");
  }

  function hataTuruBicimle(tur) {
    return anahtarAdiOkunurYap(tur, "hataTurleri");
  }

  function sayiBicimle(n, opts) {
    try {
      return new Intl.NumberFormat(durum.dil, opts).format(n);
    } catch (_) {
      return String(n);
    }
  }

  // Sistem saat dilimini otomatik kullanır (sabit +3 yoktur).
  function saatBicimle(isoStr) {
    if (!isoStr) return "—";
    const tMs = Date.parse(isoStr);
    if (!Number.isFinite(tMs)) return "—";
    try {
      return new Intl.DateTimeFormat(durum.dil, { hour: "2-digit", minute: "2-digit" }).format(tMs);
    } catch (_) {
      return new Date(tMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    }
  }

  function gunVeSaatBicimle(isoStr) {
    if (!isoStr) return t("genel.bilinmiyor");
    const tMs = Date.parse(isoStr);
    if (!Number.isFinite(tMs)) return t("genel.bilinmiyor");
    const simdiMs = Date.parse(simdiIso());
    const farkGun = Math.abs(tMs - simdiMs) / 86400000;
    try {
      if (farkGun >= 6) {
        return new Intl.DateTimeFormat(durum.dil, {
          weekday: "short",
          day: "numeric",
          month: "short",
          hour: "2-digit",
          minute: "2-digit",
        }).format(tMs);
      }
      return new Intl.DateTimeFormat(durum.dil, {
        weekday: "short",
        hour: "2-digit",
        minute: "2-digit",
      }).format(tMs);
    } catch (_) {
      return saatBicimle(isoStr);
    }
  }

  function simdiIso() {
    return durum.demoMu ? durum.anlik.zaman : new Date().toISOString();
  }

  function yasMetni(yasSn) {
    if (typeof yasSn !== "number" || !Number.isFinite(yasSn)) return t("genel.bilinmiyor");
    if (yasSn < 90) return fmt("genel.yasSaniye", { n: sayiBicimle(Math.round(yasSn)) });
    const dk = Math.round(yasSn / 60);
    if (dk < 90) return fmt("genel.yasDakika", { n: sayiBicimle(dk) });
    return fmt("genel.yasSaat", { n: sayiBicimle(Math.round(dk / 60)) });
  }

  function kaynakYasSn(kaynak) {
    if (!kaynak) return null;
    if (!durum.demoMu && kaynak.okuma) {
      const ms = Date.parse(kaynak.okuma);
      if (Number.isFinite(ms)) return Math.max(0, Math.round((Date.now() - ms) / 1000));
    }
    return typeof kaynak.yas_sn === "number" && Number.isFinite(kaynak.yas_sn) ? Math.max(0, kaynak.yas_sn) : null;
  }

  function kaynakYasMetni(kaynak) {
    const yasSn = kaynakYasSn(kaynak);
    return yasSn !== null ? yasMetni(yasSn) : t("genel.bilinmiyor");
  }

  function kalanSureMetni(sifirlamaIso, simdiStr) {
    if (!sifirlamaIso) return t("genel.kalanSureYok");
    const hedef = Date.parse(sifirlamaIso);
    const baz = Date.parse(simdiStr);
    if (!Number.isFinite(hedef) || !Number.isFinite(baz)) return t("genel.kalanSureYok");
    const farkDk = Math.round((hedef - baz) / 60000);
    if (farkDk <= 0) return t("genel.kalanSureAz");
    const gun = Math.floor(farkDk / 1440);
    const saat = Math.floor((farkDk % 1440) / 60);
    const dk = farkDk % 60;
    if (gun > 0) return fmt("genel.kalanSureGunSaat", { g: sayiBicimle(gun), s: sayiBicimle(saat) });
    if (saat > 0) return fmt("genel.kalanSureSaatDk", { s: sayiBicimle(saat), dk: sayiBicimle(dk) });
    return fmt("genel.kalanSureDk", { dk: sayiBicimle(dk) });
  }

  function gecenOranHesapla(olcum, simdiStr) {
    if (!olcum || !olcum.sifirlama || !(olcum.uzunluk_dk > 0)) return null;
    const bitisMs = Date.parse(olcum.sifirlama);
    const simdiMs = Date.parse(simdiStr);
    if (!Number.isFinite(bitisMs) || !Number.isFinite(simdiMs)) return null;
    const sureMs = olcum.uzunluk_dk * 60000;
    if (!(sureMs > 0)) return null;
    const basMs = bitisMs - sureMs;
    const oran = (simdiMs - basMs) / sureMs;
    if (!Number.isFinite(oran) || oran < 0 || oran > 1.05) return null;
    return clamp(oran, 0, 1);
  }

  function zamanEksenOrani(hedefIso, olcum) {
    if (!hedefIso || !olcum || !olcum.sifirlama || !(olcum.uzunluk_dk > 0)) return null;
    const bitisMs = Date.parse(olcum.sifirlama);
    const sureMs = olcum.uzunluk_dk * 60000;
    const basMs = bitisMs - sureMs;
    const hedefMs = Date.parse(hedefIso);
    if (!Number.isFinite(hedefMs) || bitisMs <= basMs) return null;
    return clamp((hedefMs - basMs) / sureMs, 0, 1);
  }

  // Uzun açıklama cümlesi ekranda değil, başlığın yanındaki küçük bilgi simgesinin ipucunda durur.
  function baslikBilgiYaz(baslikId, aciklamaId, baslik, aciklama) {
    const baslikEl = document.getElementById(baslikId);
    if (baslikEl) {
      baslikEl.innerHTML = `${escHtml(baslik)}${
        aciklama
          ? ` <span class="bilgi-simge" tabindex="0" role="img" aria-label="${escHtml(aciklama)}" data-ipucu="${escHtml(aciklama)}"><svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.4"/><path d="M8 7.2v4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="5" r="0.9" fill="currentColor"/></svg></span>`
          : ""
      }`;
    }
    const aciklamaEl = document.getElementById(aciklamaId);
    if (aciklamaEl) {
      aciklamaEl.textContent = "";
      aciklamaEl.classList.add("gizli");
    }
  }

  function escHtml(metin) {
    return String(metin ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  // --------------------------------------------------------------------------
  // 5. Özgün Geometrik Monogramlar ve Durum Simgeleri (CodexBar'dan kopyalanmaz)
  // --------------------------------------------------------------------------
  function svgMonogram(platform) {
    let ic = "";
    if (platform === "claude") {
      ic = '<path d="M9 2.5v13M3.4 5.75l11.2 6.5M14.6 5.75l-11.2 6.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>';
    } else if (platform === "codex") {
      ic = '<circle cx="9" cy="9" r="5.5" stroke="currentColor" stroke-width="1.6" stroke-dasharray="10 4"/><circle cx="9" cy="9" r="2.2" fill="currentColor"/>';
    } else if (platform === "agy") {
      ic = '<ellipse cx="9" cy="9" rx="6.2" ry="3.2" transform="rotate(-28 9 9)" stroke="currentColor" stroke-width="1.5"/><circle cx="9" cy="9" r="2" fill="currentColor"/>';
    } else if (platform === "deepseek") {
      ic = '<path d="M3 7.2c2-1.8 4-1.8 6 0s4 1.8 6 0M3 11.4c2-1.8 4-1.8 6 0s4 1.8 6 0" stroke="currentColor" stroke-width="1.65" stroke-linecap="round"/>';
    } else {
      ic = '<path d="M6.8 5.2L3.5 9l3.3 3.8M11.2 5.2L14.5 9l-3.3 3.8" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round"/>';
    }
    return `<svg width="16" height="16" viewBox="0 0 18 18" fill="none" aria-hidden="true">${ic}</svg>`;
  }

  function svgDurumSimge(sinif) {
    if (sinif === "rahat") {
      return '<svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true"><circle cx="6" cy="6" r="4.5" fill="currentColor"/></svg>';
    }
    if (sinif === "dikkat") {
      return '<svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M6 1.6L11 10.2H1L6 1.6Z" fill="currentColor"/></svg>';
    }
    if (sinif === "kacin") {
      return '<svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true"><rect x="2" y="2" width="8" height="8" rx="1.8" fill="currentColor"/></svg>';
    }
    return '<svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true"><circle cx="6" cy="6" r="4" stroke="currentColor" stroke-width="1.5" stroke-dasharray="2.5 2"/></svg>';
  }

  function platformEtkinMi(platform) {
    const p = String(platform || "").toLowerCase();
    const a = durum.ayarlar;
    if (p === "deepseek") return Boolean(a.deepseekEtkin);
    if (p === "codex") return a.codexEtkin !== false;
    if (p === "agy") return a.agyOkuma !== "kapali";
    if (p === "opencode") return Boolean(a.opencodeEtkin);
    if (p === "dsh") return Boolean(a.dshEtkin);
    return true;
  }

  // Ana ekranda yalnız etkin ve bağlı havuzlar görünür; kapalı veya bağlı olmayanlar gizlenir.
  function bagliMi(h) {
    if (!h || h.durum === "baglanmadi" || h.durum === "kapali") return false;
    if (!platformEtkinMi(h.platform)) return false;
    if (h.durum === "bilinmiyor") {
      const veriVar = (h.pencereler && Object.keys(h.pencereler).length > 0) || h.bakiye || h.okuma_zamani;
      return Boolean(veriVar);
    }
    return true;
  }

  // Abonelik/plan adı görünmez: "Claude Max" → "Claude", "ChatGPT Plus" → "ChatGPT".
  // (sabit değil işlev içi: durum nesnesi bu satırdan önce kurulup çağırıyor)
  function sadeEtiket(etiket) {
    if (typeof etiket !== "string") return etiket;
    const planSozcukleri = /\s+(max|plus|pro|team|teams|ultra|free|enterprise|business|premium)(\s*\d+x)?$/i;
    return etiket.replace(/\s*\((max|plus|pro|team|ultra)[^)]*\)\s*$/i, "").replace(planSozcukleri, "").trim();
  }
  function anlikSadelestir(anlik) {
    if (anlik && anlik.havuzlar) {
      for (const h of Object.values(anlik.havuzlar)) {
        if (h && typeof h.etiket === "string") h.etiket = sadeEtiket(h.etiket);
      }
    }
    return anlik;
  }

  function svgTutamak() {
    return `<svg width="12" height="16" viewBox="0 0 12 16" fill="currentColor" aria-hidden="true"><circle cx="3.5" cy="3" r="1.25"/><circle cx="8.5" cy="3" r="1.25"/><circle cx="3.5" cy="8" r="1.25"/><circle cx="8.5" cy="8" r="1.25"/><circle cx="3.5" cy="13" r="1.25"/><circle cx="8.5" cy="13" r="1.25"/></svg>`;
  }

  function svgYildiz(dolu) {
    return dolu
      ? `<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 1.5l2 4.1 4.5.6-3.25 3.15.77 4.48L8 11.7l-4.02 2.13.77-4.48L1.5 6.2 6 5.6 8 1.5z"/></svg>`
      : `<svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M8 1.8l1.85 3.85 4.25.6-3.08 2.98.73 4.22L8 11.45l-3.75 2 .73-4.22L1.9 6.25l4.25-.6L8 1.8z" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round"/></svg>`;
  }

  function kartTutamakHtml(id, h) {
    return `<button type="button" class="kart-tutamak" data-kart-tutamak="${escHtml(id)}" aria-label="${escHtml(fmt("kartlar.tutamakAria", { ad: h.etiket }))}" aria-pressed="false" data-ipucu="${escHtml(t("kartlar.tutamakIpucu"))}">${svgTutamak()}</button>`;
  }

  function kartDuzenleKontrollerHtml(id, h) {
    const sabitMi = durum.kartTercihler.sabitKartlar.includes(id);
    const gizliMi = durum.kartTercihler.gizliKartlar.includes(id);
    return `
      <div class="kart-duzenle-arac">
        <button type="button" class="kart-eylem-btn kart-sabit-btn${sabitMi ? " aktif" : ""}" data-kart-sabit="${escHtml(id)}" aria-pressed="${sabitMi ? "true" : "false"}" aria-label="${escHtml(sabitMi ? t("kartlar.sabitiKaldir") : t("kartlar.sabitle"))}" data-ipucu="${escHtml(sabitMi ? t("kartlar.sabitiKaldir") : t("kartlar.sabitle"))}">
          ${svgYildiz(sabitMi)}
          <span>${escHtml(sabitMi ? t("kartlar.sabitRozet") : t("kartlar.sabitle"))}</span>
        </button>
        <button type="button" role="switch" class="kart-anahtar${!gizliMi ? " acik" : ""}" data-kart-gizle="${escHtml(id)}" aria-checked="${!gizliMi ? "true" : "false"}" aria-label="${escHtml(fmt("kartlar.gosterGizleAria", { ad: h.etiket }))}" data-ipucu="${escHtml(gizliMi ? t("kartlar.goster") : t("kartlar.gizle"))}">
          <span class="kart-anahtar-iz" aria-hidden="true"><span class="kart-anahtar-top"></span></span>
          <span>${escHtml(gizliMi ? t("kartlar.gizliRozet") : t("kartlar.goster"))}</span>
        </button>
      </div>
    `;
  }

  function duzenleCubuguCiz(hedefId) {
    const el = document.getElementById(hedefId);
    if (!el) return;
    if (!durum.duzenleModu) {
      el.classList.add("gizli");
      el.innerHTML = "";
      return;
    }
    const { kartSirasi, gizliKartlar, sabitKartlar } = durum.kartTercihler;
    const varsayilanMi = kartSirasi.length === 0 && gizliKartlar.length === 0 && sabitKartlar.length === 0;
    el.classList.remove("gizli");
    el.innerHTML = `
      <span class="duzenle-arac-not">${escHtml(t("kartlar.duzenleAcikNot"))}</span>
      <div class="duzenle-arac-eylemler">
        <button type="button" class="duzenle-sifirla-btn" data-kart-sifirla="1" ${varsayilanMi ? "disabled" : ""}>${escHtml(t("kartlar.varsayilanaDon"))}</button>
        <button type="button" class="duzenle-bitti-btn" data-kart-duzenle-bitir="1">${escHtml(t("kartlar.bitti"))}</button>
      </div>
    `;
  }

  let duyuruZamanlayici = null;
  function ekranOkuyucuDuyur(metin) {
    const el = document.getElementById("ekran-okuyucu-duyuru");
    if (!el || !metin) return;
    if (duyuruZamanlayici) window.clearTimeout(duyuruZamanlayici);
    el.textContent = "";
    duyuruZamanlayici = window.setTimeout(() => {
      el.textContent = metin;
    }, 20);
  }

  function havuzVarsayilanKarsilastir([, a], [, b]) {
    const oncelik = (h) => {
      // Kullanım sayacı kota değildir: kotaların arkasında durur.
      if (h.ek?.kullanim && h.durum === "ok") return 4.5;
      if (h.durum === "ok") {
        if (h.baski === "kacin" && h.kullanilabilir) return 0;
        if (h.kullanilabilir === false) return 1;
        if (h.baski === "dikkat") return 2;
        return 3;
      }
      if (h.durum === "bayat") return 4;
      if (h.durum === "bilinmiyor") return 5;
      if (h.durum === "baglanmadi") return 6;
      return 7;
    };

    const pa = oncelik(a);
    const pb = oncelik(b);
    if (pa !== pb) return pa - pb;
    const ka = typeof a.etkin_kalan === "number" ? a.etkin_kalan : 2;
    const kb = typeof b.etkin_kalan === "number" ? b.etkin_kalan : 2;
    if (Math.abs(ka - kb) > 1e-9) return ka - kb;
    return (a.etiket || "").localeCompare(b.etiket || "", durum.dil);
  }

  function havuzSirala(havuzlar, secenekler = {}) {
    const { gizlileriDahilEt = false } = secenekler;
    const siraPref = kartListesiniPlatformaEsle(durum.kartTercihler.kartSirasi, havuzlar);
    const gizliSet = new Set(kartListesiniPlatformaEsle(durum.kartTercihler.gizliKartlar, havuzlar));
    const sabitList = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, havuzlar);
    const sabitSet = new Set(sabitList);
    const siraMap = new Map(siraPref.map((id, idx) => [id, idx]));
    const sabitMap = new Map(sabitList.map((id, idx) => [id, idx]));

    return Object.entries(havuzlar || {})
      .filter(([id, h]) => {
        if (!bagliMi(h)) return false;
        const p = h.platform || id;
        return gizlileriDahilEt || !gizliSet.has(p);
      })
      .sort((entA, entB) => {
        const [idA, hA] = entA;
        const [idB, hB] = entB;
        const pA = hA.platform || idA;
        const pB = hB.platform || idB;
        const sabA = sabitSet.has(pA);
        const sabB = sabitSet.has(pB);
        if (sabA !== sabB) return sabA ? -1 : 1;

        const idxA = siraMap.get(pA);
        const idxB = siraMap.get(pB);
        if (idxA !== undefined && idxB !== undefined && idxA !== idxB) return idxA - idxB;
        if (idxA !== undefined && idxB === undefined) return -1;
        if (idxB !== undefined && idxA === undefined) return 1;

        if (sabA && sabB && siraMap.size === 0) {
          const siA = sabitMap.get(pA);
          const siB = sabitMap.get(pB);
          if (siA !== undefined && siB !== undefined && siA !== siB) return siA - siB;
        }
        return havuzVarsayilanKarsilastir(entA, entB);
      });
  }

  function genelBaskiHesapla() {
    const havuzlar = Object.values(durum.anlik.havuzlar || {}).filter((h) => bagliMi(h));
    const uygunlar = havuzlar
      .filter((h) => h.durum === "ok" && h.kullanilabilir && typeof h.etkin_kalan === "number")
      .sort((a, b) => a.etkin_kalan - b.etkin_kalan);

    if (uygunlar.length > 0) {
      const enDar = uygunlar[0];
      const kisaAd = platformAdi(enDar.platform) || (enDar.etiket || "").split(" ")[0];
      const pencereKisa = enDar.baglayici_pencere
        ? sozlukEslesmeBul("pencereKisa", enDar.baglayici_pencere) || anahtarAdiOkunurYap(enDar.baglayici_pencere)
        : "";
      const { olcum } = baglayiciBul(enDar);
      return {
        metin: fmt("genel.enDar", { ad: `${kisaAd} ${pencereKisa}`.trim() }),
        adPencere: `${kisaAd} ${pencereKisa}`.trim(),
        seviye: enDar.baski || "rahat",
        sure: olcum?.sifirlama ? sureKisa(olcum.sifirlama) : null,
        enDar,
      };
    }

    if (havuzlar.some((h) => h.durum === "ok")) {
      return { metin: t("genel.tumuTukendi"), adPencere: t("genel.tumuTukendi"), seviye: "kacin", sure: null, enDar: null };
    }
    if (havuzlar.length === 0 && !durum.demoMu) {
      return {
        metin: durum.servisDurumu.calisiyor ? t("baski.bilinmiyor") : t("baski.kapali"),
        adPencere: durum.servisDurumu.calisiyor ? t("baski.bilinmiyor") : t("baski.kapali"),
        seviye: "bayat",
        sure: null,
        enDar: null,
      };
    }
    return { metin: t("genel.verilerBayat"), adPencere: t("genel.verilerBayat"), seviye: "bayat", sure: null, enDar: null };
  }

  function dolguSinifiBul(havuz, olcum, id) {
    if (havuz.durum === "bayat") return "bayat";
    if (!olcum) return havuz.baski || "bilinmiyor";
    const esik = (Number(esikYuzdesi(id, null, havuz.platform)) || 20) / 100;
    const kalan = clamp(typeof olcum.kalan === "number" ? olcum.kalan : 1 - (olcum.kullanilan ?? 1), 0, 1);
    if (kalan <= esik || havuz.baski === "kacin" || havuz.kullanilabilir === false) return "kacin";
    if (kalan <= Math.min(0.99, esik + 0.12) || havuz.baski === "dikkat") return "dikkat";
    return "rahat";
  }

  function baglayiciBul(havuz) {
    const pencereler = havuz?.pencereler;
    if (!pencereler || Object.keys(pencereler).length === 0) return { bp: null, olcum: null, etkinKalan: null };
    if (havuz.baglayici_pencere && pencereler[havuz.baglayici_pencere]) {
      const bp = havuz.baglayici_pencere;
      const etkinKalan =
        typeof havuz.etkin_kalan === "number" ? clamp(havuz.etkin_kalan, 0, 1) : clamp(pencereler[bp].kalan, 0, 1);
      return { bp, olcum: pencereler[bp], etkinKalan };
    }
    let enDarAd = null;
    let enDarOlcum = null;
    for (const [ad, olcum] of Object.entries(pencereler)) {
      if (typeof olcum?.kalan !== "number" || !Number.isFinite(olcum.kalan)) continue;
      if (!enDarOlcum || olcum.kalan < enDarOlcum.kalan) {
        enDarAd = ad;
        enDarOlcum = olcum;
      }
    }
    return {
      bp: enDarAd,
      olcum: enDarOlcum,
      etkinKalan: enDarOlcum ? clamp(enDarOlcum.kalan, 0, 1) : null,
    };
  }

  function esikYuzdesi(havuzId, pencere, platform) {
    const a = durum.ayarlar;
    const pNorm = String(platform || durum.anlik.havuzlar?.[havuzId]?.platform || "").trim().toLowerCase();
    if (pNorm && typeof a.platformEsikleri?.[pNorm] === "number" && Number.isFinite(a.platformEsikleri[pNorm])) {
      return clamp(Math.round(a.platformEsikleri[pNorm]), 1, 99);
    }
    if (havuzId === "claude_max" || pNorm === "claude") return pencere === "hafta" ? a.claudeHaftaEsik : a.claude5sEsik;
    return a.digerEsik;
  }

  function havuzNotuYerel(id, havuz) {
    if (!durum.demoMu) {
      return (havuz.notlar || []).map((n) => metinYuzdeNormalize(n));
    }
    if (durum.senaryo === "bayat" && havuz.durum === "bayat") {
      return [fmt("genel.demoBayatNot", { yas: kaynakYasMetni(havuz.kaynak) })];
    }
    if (id === "codex_plus") return [t("genel.demoKilitliHaftaNot")];
    if (id === "agy_gemini") return [fmt("genel.demoTekOkumaNot", { oran: yuzdeBicimle(0.01) })];
    if (id === "agy_claude_gpt") {
      return [
        fmt("genel.demoPencereSifirNot", {
          pencere: sozlukEslesmeBul("pencereAdlari", "5s") || "5s",
          oran: yuzdeBicimle(0),
        }),
      ];
    }
    if (id === "agy_argon") return [fmt("genel.demoBayatNot", { yas: kaynakYasMetni(havuz.kaynak) })];
    if (id === "deepseek") return [t("genel.demoAnahtarYokNot")];
    if (havuz.ek?.kullanim) return [];
    return (havuz.notlar || []).map((n) => metinYuzdeNormalize(n));
  }

  // Kotası olmayan yerel araçların (OpenCode, DeepSeek Harness) kullanım sayacı metinleri.
  function kompaktSayi(n) {
    if (typeof n !== "number" || !Number.isFinite(n)) return "—";
    try {
      return new Intl.NumberFormat(durum.dil, { notation: "compact", maximumFractionDigits: 1 }).format(n);
    } catch {
      return sayiBicimle(n);
    }
  }
  function sayacMetinleri(k) {
    const s24 =
      typeof k.belirtec_24s === "number"
        ? fmt("genel.sayac24", { oturum: sayiBicimle(k.oturum_24s || 0), belirtec: kompaktSayi(k.belirtec_24s) })
        : fmt("genel.sayac24Oturum", { oturum: sayiBicimle(k.oturum_24s || 0) });
    const s7 =
      typeof k.belirtec_7g === "number"
        ? fmt("genel.sayac7g", { oturum: sayiBicimle(k.oturum_7g || 0), belirtec: kompaktSayi(k.belirtec_7g) })
        : fmt("genel.sayac7gOturum", { oturum: sayiBicimle(k.oturum_7g || 0) });
    const toplam = fmt("genel.sayacToplam", { n: sayiBicimle(k.oturum_toplam || 0) });
    const son = k.son_etkinlik ? fmt("genel.sayacSon", { zaman: gunVeSaatBicimle(k.son_etkinlik) }) : "";
    return { s24, s7, toplam, son };
  }

  // --------------------------------------------------------------------------
  // 6. Göstergeler (Çubuk + Tempo, Halka, Pasta Ayrı, Pasta Tek Segmentli)
  // --------------------------------------------------------------------------
  function anaDegerOrani(olcum) {
    if (!olcum) return 0;
    const kalanGecerli = typeof olcum.kalan === "number" && Number.isFinite(olcum.kalan);
    const kulGecerli = typeof olcum.kullanilan === "number" && Number.isFinite(olcum.kullanilan);
    const hamKalan = kalanGecerli ? olcum.kalan : kulGecerli ? 1 - olcum.kullanilan : 0;
    const hamKul = kulGecerli ? olcum.kullanilan : kalanGecerli ? 1 - olcum.kalan : 0;
    return durum.degerYonu === "kullanilan" ? clamp(hamKul, 0, 1) : clamp(hamKalan, 0, 1);
  }

  // Tek vurgu rengi: yalnız sıkışık (eşik altı) kırmızı, bayat gri.
  function gostergeRenkKodu(sinif) {
    if (sinif === "kacin") return "var(--durum-kacin-bar)";
    if (sinif === "bayat") return "var(--durum-bayat)";
    return "var(--vurgu)";
  }

  function svgHalkaHtml(havuz, olcum, ikincilOlcum, boyut) {
    const sinif = dolguSinifiBul(havuz, olcum);
    const renk = gostergeRenkKodu(sinif);
    const gosterilenOran = anaDegerOrani(olcum);
    const pctMetin = yuzdeBicimle(gosterilenOran);
    const c = boyut / 2;
    const disR = c - 6;
    const icR = ikincilOlcum ? c - 14 : disR;
    const disCevre = 2 * Math.PI * disR;
    const icCevre = 2 * Math.PI * icR;

    const anaCevre = ikincilOlcum ? icCevre : disCevre;
    const anaR = ikincilOlcum ? icR : disR;
    const anaOffset = anaCevre * (1 - gosterilenOran);

    let disHalkaSvg = "";
    if (ikincilOlcum) {
      const ikOran = anaDegerOrani(ikincilOlcum);
      const ikOffset = disCevre * (1 - ikOran);
      disHalkaSvg = `
        <circle cx="${c}" cy="${c}" r="${disR}" fill="none" stroke="var(--iz-ray)" stroke-width="5"/>
        <circle cx="${c}" cy="${c}" r="${disR}" fill="none" stroke="var(--metin-soluk)" stroke-opacity="0.45" stroke-width="5"
                stroke-dasharray="${disCevre.toFixed(2)}" stroke-dashoffset="${ikOffset.toFixed(2)}" stroke-linecap="round"
                transform="rotate(-90 ${c} ${c})" class="halka-yay"/>
      `;
    }

    const desenSvg =
      sinif === "dikkat" || sinif === "kacin"
        ? `<circle cx="${c}" cy="${c}" r="${anaR}" fill="none" stroke="url(#${sinif === "kacin" ? "desen-kacin" : "desen-dikkat"})" stroke-width="${boyut >= 76 ? 8 : 6}" stroke-dasharray="${anaCevre.toFixed(2)}" stroke-dashoffset="${anaOffset.toFixed(2)}" transform="rotate(-90 ${c} ${c})"/>`
        : "";

    const ipucu = `${havuz.etiket}: ${pctMetin} ${durum.degerYonu === "kullanilan" ? t("genel.kullanildi") : t("genel.kalan")}`;
    const yaziBoyut = boyut >= 76 ? (pctMetin.length >= 5 ? 15 : 18) : pctMetin.length >= 5 ? 10 : 12;

    return `
      <div class="gosterge-svg-kutu" data-ipucu="${escHtml(ipucu)}" tabindex="0" role="img" aria-label="${escHtml(ipucu)}">
        <svg width="${boyut}" height="${boyut}" viewBox="0 0 ${boyut} ${boyut}">
          ${disHalkaSvg}
          <circle cx="${c}" cy="${c}" r="${anaR}" fill="none" stroke="var(--iz-ray)" stroke-width="${boyut >= 76 ? 8 : 6}"/>
          <circle cx="${c}" cy="${c}" r="${anaR}" fill="none" stroke="${renk}" stroke-width="${boyut >= 76 ? 8 : 6}"
                  stroke-dasharray="${anaCevre.toFixed(2)}" stroke-dashoffset="${anaOffset.toFixed(2)}" stroke-linecap="round"
                  transform="rotate(-90 ${c} ${c})" class="halka-yay"/>
          ${desenSvg}
          <text x="${c}" y="${c + (boyut >= 76 ? 2 : 4)}" text-anchor="middle" class="halka-merkez-metin" font-size="${yaziBoyut}">${escHtml(pctMetin)}</text>
          ${boyut >= 76 ? `<text x="${c}" y="${c + 15}" text-anchor="middle" class="halka-merkez-alt">${escHtml(durum.degerYonu === "kullanilan" ? t("genel.kullanildi") : t("genel.kalan"))}</text>` : ""}
        </svg>
      </div>
    `;
  }

  function svgPastaAyriHtml(havuz, olcum, boyut) {
    const sinif = dolguSinifiBul(havuz, olcum);
    const renk = gostergeRenkKodu(sinif);
    const oran = anaDegerOrani(olcum);
    const pctMetin = yuzdeBicimle(oran);
    const c = boyut / 2;
    const r = c - 4;
    const pastaR = r / 2;
    const cevre = 2 * Math.PI * pastaR;
    const offset = cevre * (1 - oran);

    const ipucu = `${havuz.etiket}: ${pctMetin} ${durum.degerYonu === "kullanilan" ? t("genel.kullanildi") : t("genel.kalan")}`;
    const yaziBoyut = boyut >= 76 ? (pctMetin.length >= 5 ? 12 : 14) : pctMetin.length >= 5 ? 9.5 : 11;
    return `
      <div class="gosterge-svg-kutu" data-ipucu="${escHtml(ipucu)}" tabindex="0" role="img" aria-label="${escHtml(ipucu)}">
        <svg width="${boyut}" height="${boyut}" viewBox="0 0 ${boyut} ${boyut}">
          <circle cx="${c}" cy="${c}" r="${r}" fill="var(--iz-ray)"/>
          <circle cx="${c}" cy="${c}" r="${pastaR}" fill="none" stroke="${renk}" stroke-width="${r}"
                  stroke-dasharray="${cevre.toFixed(2)}" stroke-dashoffset="${offset.toFixed(2)}"
                  transform="rotate(-90 ${c} ${c})"/>
          ${sinif === "dikkat" || sinif === "kacin" ? `<circle cx="${c}" cy="${c}" r="${pastaR}" fill="none" stroke="url(#${sinif === "kacin" ? "desen-kacin" : "desen-dikkat"})" stroke-width="${r}" stroke-dasharray="${cevre.toFixed(2)}" stroke-dashoffset="${offset.toFixed(2)}" transform="rotate(-90 ${c} ${c})"/>` : ""}
          <circle cx="${c}" cy="${c}" r="${boyut * 0.27}" fill="var(--yuzey)"/>
          <text x="${c}" y="${c + 4}" text-anchor="middle" class="halka-merkez-metin" font-size="${yaziBoyut}">${escHtml(pctMetin)}</text>
        </svg>
      </div>
    `;
  }

  function tekSegmentliHalkaHtml(siraliHavuzlar, kucukMu) {
    const aktifler = siraliHavuzlar.filter(([, h]) => typeof h.etkin_kalan === "number" || Object.keys(h.pencereler || {}).length > 0);
    if (aktifler.length === 0) return "";

    const boyut = kucukMu ? 82 : 136;
    const kalinlik = kucukMu ? 11 : 16;
    const c = boyut / 2;
    const r = c - kalinlik / 2 - 3;
    const cevre = 2 * Math.PI * r;
    const adimAci = 360 / aktifler.length;
    const boslukAci = aktifler.length > 1 ? Math.min(6, adimAci * 0.25) : 0;
    const dilimAci = Math.max(1, adimAci - boslukAci);
    const dilimYay = (dilimAci / 360) * cevre;

    const yaylar = aktifler
      .map(([id, h], idx) => {
        const { olcum, etkinKalan } = baglayiciBul(h);
        const oran = durum.degerYonu === "kullanilan" ? 1 - (etkinKalan ?? 0) : etkinKalan ?? 0;
        const dolguYay = Math.max(2, dilimYay * clamp(oran, 0.04, 1));
        const aci = -90 + idx * (360 / aktifler.length);
        const sinif = dolguSinifiBul(h, olcum, id);
        const renk = PLATFORM_RENK[h.platform] || gostergeRenkKodu(sinif);
        const vurgulu = durum.vurguluSegmentId === id || (!durum.vurguluSegmentId && durum.seciliHavuzId === id);
        const pctMetin = yuzdeBicimle(oran || 0);
        const ipucu = `${h.etiket}: ${pctMetin} ${durum.degerYonu === "kullanilan" ? t("genel.kullanildi") : t("genel.kalan")}`;

        return `
          <g transform="rotate(${aci.toFixed(2)} ${c} ${c})">
            <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--iz-ray)" stroke-width="${kalinlik}"
                    stroke-dasharray="${dilimYay.toFixed(2)} ${cevre.toFixed(2)}" stroke-linecap="round"/>
            <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="${renk}" stroke-width="${kalinlik}"
                    stroke-dasharray="${dolguYay.toFixed(2)} ${cevre.toFixed(2)}" stroke-linecap="round"
                    opacity="${vurgulu || !durum.vurguluSegmentId ? "1" : "0.42"}"
                    class="pasta-dilim${vurgulu ? " vurgulu" : ""}"
                    data-segment-havuz="${escHtml(id)}" data-ipucu="${escHtml(ipucu)}"
                    tabindex="0" role="button" aria-label="${escHtml(ipucu)}"/>
          </g>
        `;
      })
      .join("");

    const seciliId = durum.vurguluSegmentId || durum.seciliHavuzId;
    const merkezHavuz = aktifler.find(([id]) => id === seciliId)?.[1] || aktifler[0][1];
    const { etkinKalan: merkezKalan } = baglayiciBul(merkezHavuz);
    const merkezOran = durum.degerYonu === "kullanilan" ? 1 - (merkezKalan ?? 0) : merkezKalan ?? 0;
    const merkezMetin = yuzdeBicimle(merkezOran || 0);
    const merkezYaziBoyut = kucukMu ? (merkezMetin.length >= 5 ? 11.5 : 14) : merkezMetin.length >= 5 ? 18 : 21;

    const svg = `
      <svg width="${boyut}" height="${boyut}" viewBox="0 0 ${boyut} ${boyut}" class="gosterge-svg-kutu" aria-label="${escHtml(t("gorunum.segmentHalkaBaslik"))}">
        ${yaylar}
        <text x="${c}" y="${c + 2}" text-anchor="middle" class="halka-merkez-metin" font-size="${merkezYaziBoyut}">${escHtml(merkezMetin)}</text>
        <text x="${c}" y="${c + (kucukMu ? 14 : 17)}" text-anchor="middle" class="halka-merkez-alt">${escHtml(PLATFORM_KISA[merkezHavuz.platform] || merkezHavuz.etiket.slice(0, 6))}</text>
      </svg>
    `;

    if (kucukMu) {
      return `
        <div class="pasta-tek-acilir-kart">
          ${svg}
          <div class="pasta-tek-lejan">
            <div class="pasta-tek-baslik">${escHtml(merkezHavuz.etiket)}</div>
            <div class="pasta-tek-alt">${escHtml(t("gorunum.segmentSeciliIpucu"))}</div>
          </div>
        </div>
      `;
    }

    const lejan = aktifler
      .map(([id, h]) => {
        const { olcum, etkinKalan } = baglayiciBul(h);
        const oran = durum.degerYonu === "kullanilan" ? 1 - (etkinKalan ?? 0) : etkinKalan ?? 0;
        const pctMetin = yuzdeBicimle(oran || 0);
        const renk = PLATFORM_RENK[h.platform] || gostergeRenkKodu(dolguSinifiBul(h, olcum));
        const vurgulu = seciliId === id;
        return `
          <button type="button" class="pasta-lejan-chip${vurgulu ? " vurgulu" : ""}" data-segment-havuz="${escHtml(id)}"
                  data-ipucu="${escHtml(`${h.etiket}: ${pctMetin}`)}">
            <span class="pasta-nokta" style="background:${renk};"></span>
            <span>${escHtml(h.etiket)}</span>
            <strong class="rakam">${escHtml(pctMetin)}</strong>
          </button>
        `;
      })
      .join("");

    return `
      <div class="pasta-tek-kart">
        ${svg}
        <div class="pasta-tek-lejan">
          <h3 class="pasta-tek-baslik">${escHtml(t("gorunum.segmentHalkaBaslik"))}</h3>
          <p class="pasta-tek-alt">${escHtml(t("gorunum.segmentHalkaAlt"))}</p>
          <div class="pasta-lejan-liste">${lejan}</div>
        </div>
      </div>
    `;
  }

  function havuzSatirAria(havuz) {
    const { bp, olcum } = baglayiciBul(havuz);
    const durumMetin = t(`baski.${havuz.durum === "ok" ? havuz.baski || "rahat" : havuz.durum}`, havuz.durum);
    const kaynakMetin = havuz.kaynak
      ? sozlukEslesmeBul("guvenEtiketi", havuz.kaynak.guven) || anahtarAdiOkunurYap(havuz.kaynak.guven)
      : t("kaynakTur.bilinmiyor");
    const yasMetin = havuz.kaynak ? kaynakYasMetni(havuz.kaynak) : t("genel.bilinmiyor");

    if (!olcum) {
      return fmt("aria.satirBilgisiz", { ad: havuz.etiket, durumMetin, kaynakMetin });
    }
    const pAd = sozlukEslesmeBul("pencereAdlari", bp) || anahtarAdiOkunurYap(bp);
    const sifirlamaMetin = olcum.sifirlama ? `${kalanSureMetni(olcum.sifirlama, simdiIso())} (${saatBicimle(olcum.sifirlama)})` : t("genel.bilinmiyor");
    return fmt("aria.satirDurumlu", {
      ad: havuz.etiket,
      pencere: pAd,
      kullanilanYuzde: yuzdeBicimle(olcum.kullanilan),
      kalanYuzde: yuzdeBicimle(olcum.kalan),
      sifirlamaMetin,
      durumMetin,
      kaynakMetin,
      yasMetin,
    });
  }

  // --------------------------------------------------------------------------
  // 7. Sade Görünüm Yardımcıları (Genel kartları, platform sekmeleri, ayrıntı, geçmiş)
  // --------------------------------------------------------------------------
  // Platform adları özel addır; logo yerine yalnız metin ve özgün geometrik simge kullanılır.
  const PLATFORM_AD = {
    claude: "Claude",
    codex: "Codex",
    agy: "Antigravity",
    deepseek: "DeepSeek",
    opencode: "OpenCode",
    dsh: "DeepSeek Harness",
  };

  function platformAdi(p) {
    return PLATFORM_AD[p] || anahtarAdiOkunurYap(p);
  }

  // "Antigravity Gemini" → "Gemini" (platform sekmesinin içinde ad tekrarı olmasın).
  function havuzKisaAd(h) {
    const ad = String(h?.etiket || "");
    const pAd = PLATFORM_AD[h?.platform];
    if (pAd && ad.length > pAd.length + 1 && ad.toLowerCase().startsWith(`${pAd.toLowerCase()} `)) {
      return ad.slice(pAd.length + 1);
    }
    return ad;
  }

  function pencereEtiketi(pAd) {
    return sozlukEslesmeBul("pencereEtiket", pAd) || anahtarAdiOkunurYap(pAd);
  }

  function pencereBasligi(pAd) {
    return sozlukEslesmeBul("pencereBaslik", pAd) || pencereEtiketi(pAd);
  }

  function kalanModuMu() {
    return durum.degerYonu !== "kullanilan";
  }

  // Gösterilen oranın tek satırlık metni: "%15 kaldı" ya da "%85 kullanıldı".
  function oranMetni(oran) {
    return fmt(kalanModuMu() ? "genel.kaldi" : "genel.kullanildiOran", { oran: yuzdeBicimle(oran) });
  }

  function sureKisa(iso) {
    return iso ? kalanSureMetni(iso, simdiIso()) : "—";
  }

  function pencereSirali(h) {
    const p = h?.pencereler || {};
    return Object.keys(p).sort((a, b) => (p[a]?.uzunluk_dk ?? 0) - (p[b]?.uzunluk_dk ?? 0));
  }

  // Genel kartındaki iki satır: 5 saat + haftalık (Antigravity Argon'da 5 saat "—");
  // bu ikisi yoksa en kısa iki pencere.
  function kompaktPencereler(h) {
    const p = h?.pencereler || {};
    const adlar = Object.keys(p);
    if (adlar.length === 0) return [];
    if (h.platform === "agy" || "5s" in p || "hafta" in p) return ["5s", "hafta"];
    return pencereSirali(h).slice(0, 2);
  }

  function pencereKilitliMi(h, olcum) {
    return Boolean(h.durum === "ok" && h.kullanilabilir === false && olcum && olcum.kalan > 0);
  }

  // Çubuk rengi: tek vurgu rengi; yalnız eşiğin altındaki pencere kırmızı, bayat gri, kilitli soluk.
  function cubukSinifi(id, h, pAd, olcum) {
    if (h.durum === "bayat") return "bayat";
    if (!olcum) return "";
    if (pencereKilitliMi(h, olcum)) return "kilitli";
    const kalan = clamp(Number(olcum.kalan) || 0, 0, 1);
    if (kalan * 100 <= (Number(esikYuzdesi(id, pAd)) || 20)) return "kacin";
    return "";
  }

  function havuzKritikMi(id, h) {
    const { bp, etkinKalan } = baglayiciBul(h);
    if (h.durum !== "ok" || typeof etkinKalan !== "number") return false;
    return h.kullanilabilir === false || etkinKalan * 100 <= (Number(esikYuzdesi(id, bp)) || 20);
  }

  function inceCubukHtml({ oran = 0, sinif = "", ipucu = "", tempoOran = null, kalin = false, bos = false } = {}) {
    const pct = bos ? 0 : clamp(Number(oran) || 0, 0, 1) * 100;
    const tempo =
      tempoOran !== null && !bos
        ? `<span class="ince-tempo" style="left:${(clamp(tempoOran, 0, 1) * 100).toFixed(1)}%" aria-hidden="true"></span>`
        : "";
    const ipucuNit = ipucu ? ` aria-label="${escHtml(ipucu)}" data-ipucu="${escHtml(ipucu)}"` : ' aria-hidden="true"';
    return `<div class="ince-cubuk${kalin ? " kalin" : ""}${bos ? " bos" : ""}" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(pct)}"${ipucuNit}><span class="ince-dolgu ${escHtml(sinif)}" style="width:${pct.toFixed(1)}%"></span>${tempo}</div>`;
  }

  // Tempo satırı: "Sıfırlamaya kadar yeter", "Pzt 20:22 civarı biter", "%5 yedekte" ya da "Sıfırlanınca açılır".
  function tempoBilgisi(h, pAd, olcum) {
    if (!olcum || h.durum !== "ok") return null;
    const p = h.pencereler || {};
    const kilitli = pencereKilitliMi(h, olcum);
    if (kilitli || (typeof olcum.kalan === "number" && olcum.kalan <= 0)) {
      let acilisIso = olcum.sifirlama;
      if (kilitli) {
        const tukenen = Object.values(p).find((o) => typeof o?.kalan === "number" && o.kalan <= 0);
        acilisIso = tukenen?.sifirlama || baglayiciBul(h).olcum?.sifirlama || olcum.sifirlama;
      }
      return {
        metin: fmt("genel.sifirlanincaAcilir", { zaman: acilisIso ? gunVeSaatBicimle(acilisIso) : t("genel.bilinmiyor") }),
        ipucu: kilitli ? t("genel.kilitliHaftaAciklama") : t("genel.sifirlanincaAcilirIpucu"),
        ton: "kapali",
      };
    }
    const th = h.tahmin?.[pAd];
    const deneyselEk =
      th && (th.durum === "deneysel" || th.etiket === "deneysel") ? ` · ${t("genel.deneysel")}: ${t("genel.deneyselIpucu")}` : "";
    if (th?.sifirlanmadan_biter === true) {
      const z = th.bitis_tahmini ? gunVeSaatBicimle(th.bitis_tahmini) : t("genel.bilinmiyor");
      return { metin: fmt("genel.tempoBiter", { zaman: z }), ipucu: `${fmt("genel.onceDolarProjeksiyon", { zaman: z })}${deneyselEk}`, ton: "uyari" };
    }
    if (th?.sifirlanmadan_biter === false) {
      let ipucu = t("genel.sifirlanmadanBitmez");
      if (typeof th.saatlik_hiz === "number" && Number.isFinite(th.saatlik_hiz) && olcum.sifirlama && typeof olcum.kullanilan === "number") {
        const kalanSaat = (Date.parse(olcum.sifirlama) - Date.parse(simdiIso())) / 3600000;
        if (Number.isFinite(kalanSaat) && kalanSaat > 0) {
          const ongoru = clamp(olcum.kullanilan + th.saatlik_hiz * kalanSaat, clamp(olcum.kullanilan, 0, 1), 1);
          ipucu = fmt("genel.kullanimProjeksiyon", { oran: yuzdeBicimle(ongoru) });
        }
      }
      return { metin: t("genel.tempoYeter"), ipucu: `${ipucu}${deneyselEk}`, ton: "" };
    }
    const gecen = gecenOranHesapla(olcum, simdiIso());
    if (gecen === null || typeof olcum.kalan !== "number") return null;
    const yedek = olcum.kalan - (1 - gecen);
    if (Math.abs(yedek) < 0.02) return { metin: t("genel.tempoDengeli"), ipucu: t("genel.tempoYedekIpucu"), ton: "" };
    return {
      metin: fmt(yedek > 0 ? "genel.tempoYedek" : "genel.tempoAcik", { oran: yuzdeBicimle(Math.abs(yedek)) }),
      ipucu: t("genel.tempoYedekIpucu"),
      ton: yedek > 0 ? "" : "uyari",
    };
  }

  function kaynakIpucu(h) {
    if (!h?.kaynak) return "";
    const kaynak = sozlukEslesmeBul("kaynakTur", h.kaynak.tur) || anahtarAdiOkunurYap(h.kaynak.tur);
    const guven = sozlukEslesmeBul("guvenEtiketi", h.kaynak.guven) || anahtarAdiOkunurYap(h.kaynak.guven);
    return fmt("genel.kaynakBilgi", { kaynak, guven, yas: kaynakYasMetni(h.kaynak) });
  }

  function odakKimligiAl(konteyner) {
    const aktif = document.activeElement;
    if (!konteyner || !aktif || !konteyner.contains(aktif)) return null;
    if (aktif.id) {
      return {
        sel: `#${CSS.escape(aktif.id)}`,
        start: typeof aktif.selectionStart === "number" ? aktif.selectionStart : null,
        end: typeof aktif.selectionEnd === "number" ? aktif.selectionEnd : null,
      };
    }
    const attrs = [
      "data-platform-ac",
      "data-platform-sekme",
      "data-platform-git",
      "data-ayar-bolum",
      "data-ayar-anahtar",
      "data-platform-esik",
      "data-platform-sessiz",
      "data-ayar-sifirla",
      "data-kart-tutamak",
      "data-kart-sabit",
      "data-kart-gizle",
      "data-gecmis-id",
    ];
    for (const ad of attrs) {
      const val = aktif.getAttribute?.(ad);
      if (val !== null && val !== undefined) {
        return {
          sel: `[${ad}="${CSS.escape(val)}"]`,
          start: typeof aktif.selectionStart === "number" ? aktif.selectionStart : null,
          end: typeof aktif.selectionEnd === "number" ? aktif.selectionEnd : null,
        };
      }
    }
    return null;
  }

  function odakKimligiYukle(konteyner, bilgi) {
    if (!konteyner || !bilgi?.sel) return;
    const hedef = konteyner.querySelector(bilgi.sel);
    if (!hedef || typeof hedef.focus !== "function") return;
    hedef.focus({ preventScroll: true });
    if (bilgi.start !== null && typeof hedef.setSelectionRange === "function") {
      try {
        hedef.setSelectionRange(bilgi.start, bilgi.end ?? bilgi.start);
      } catch (_) {
        // number/select girdilerinde desteklenmez
      }
    }
  }

  function guncellikMetni(kaynak) {
    if (!kaynak) return "";
    const yasSn = kaynakYasSn(kaynak);
    if (typeof yasSn !== "number" || !Number.isFinite(yasSn)) return "";
    return yasSn < 60 ? t("genel.guncellendiAzOnce") : fmt("genel.guncellendi", { yas: yasMetni(yasSn) });
  }

  function ustGuncellikMetni() {
    if (!durum.demoMu && !durum.servisDurumu.calisiyor) return "";
    const baglilar = Object.values(durum.anlik?.havuzlar || {}).filter((h) => bagliMi(h));
    if (baglilar.length === 0) return "";
    let enTazeYas = null;
    for (const h of baglilar) {
      const y = kaynakYasSn(h.kaynak);
      if (typeof y === "number" && (enTazeYas === null || y < enTazeYas)) enTazeYas = y;
    }
    if (enTazeYas === null) {
      const ms = Date.parse(durum.anlik?.zaman || "");
      if (!Number.isFinite(ms)) return "";
      enTazeYas = durum.demoMu ? 0 : Math.max(0, Math.round((Date.now() - ms) / 1000));
    }
    return enTazeYas < 60 ? t("genel.guncellendiAzOnce") : fmt("genel.guncellendi", { yas: yasMetni(enTazeYas) });
  }

  function platformKimligineEsle(id, havuzlar) {
    if (typeof id !== "string" || !id) return "";
    if (havuzlar?.[id]?.platform) return havuzlar[id].platform;
    if (id.startsWith("claude_")) return "claude";
    if (id.startsWith("codex_")) return "codex";
    if (id.startsWith("agy_")) return "agy";
    return id;
  }

  function kartListesiniPlatformaEsle(liste, havuzlar) {
    const sonuc = [];
    const gorulen = new Set();
    for (const oge of liste || []) {
      const p = platformKimligineEsle(oge, havuzlar);
      if (p && /^[a-z0-9_]{1,40}$/.test(p) && !gorulen.has(p)) {
        gorulen.add(p);
        sonuc.push(p);
        if (sonuc.length >= 64) break;
      }
    }
    return sonuc;
  }

  function kartGorunenAdi(id) {
    if (PLATFORM_AD[id]) return PLATFORM_AD[id];
    const h = durum.anlik.havuzlar?.[id];
    return h?.etiket || platformAdi(id);
  }

  function platformMevcutMu(platform) {
    return Object.values(durum.anlik.havuzlar || {}).some((h) => bagliMi(h) && h.platform === platform);
  }

  function platformGruplariOlustur(havuzlar, secenekler = {}) {
    const { gizlileriDahilEt = false } = secenekler;
    const siraPref = kartListesiniPlatformaEsle(durum.kartTercihler.kartSirasi, havuzlar);
    const gizliSet = new Set(kartListesiniPlatformaEsle(durum.kartTercihler.gizliKartlar, havuzlar));
    const sabitList = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, havuzlar);
    const sabitSet = new Set(sabitList);
    const siraMap = new Map(siraPref.map((p, idx) => [p, idx]));
    const sabitMap = new Map(sabitList.map((p, idx) => [p, idx]));

    const gruplarMap = new Map();
    for (const [id, h] of Object.entries(havuzlar || {})) {
      if (!bagliMi(h) || !h.platform) continue;
      if (!gruplarMap.has(h.platform)) gruplarMap.set(h.platform, []);
      gruplarMap.get(h.platform).push([id, h]);
    }

    const gruplar = [];
    for (const [platform, havuzListesi] of gruplarMap.entries()) {
      if (!gizlileriDahilEt && gizliSet.has(platform)) continue;
      havuzListesi.sort(havuzVarsayilanKarsilastir);
      const [repId, repH] = havuzListesi[0];
      gruplar.push({
        platform,
        ad: platformAdi(platform),
        havuzlar: havuzListesi,
        repId,
        repH,
      });
    }

    return gruplar.sort((a, b) => {
      const sabA = sabitSet.has(a.platform);
      const sabB = sabitSet.has(b.platform);
      if (sabA !== sabB) return sabA ? -1 : 1;

      const idxA = siraMap.get(a.platform);
      const idxB = siraMap.get(b.platform);
      if (idxA !== undefined && idxB !== undefined) return idxA - idxB;
      if (idxA !== undefined) return -1;
      if (idxB !== undefined) return 1;

      if (sabA && sabB && siraMap.size === 0) {
        const siA = sabitMap.get(a.platform);
        const siB = sabitMap.get(b.platform);
        if (siA !== undefined && siB !== undefined && siA !== siB) return siA - siB;
      }
      return havuzVarsayilanKarsilastir([a.repId, a.repH], [b.repId, b.repH]);
    });
  }

  function platformListesi(sirali) {
    const liste = [];
    for (const [, h] of sirali) {
      if (h?.platform && !liste.includes(h.platform)) liste.push(h.platform);
    }
    return liste;
  }

  // Seçili sekme artık yoksa (platform gizlendi, kapatıldı ya da bağlantısı koptu) Genel'e dön.
  function platformSekmesiDogrula(sirali) {
    if (durum.platformSekme !== "genel" && !platformListesi(sirali).includes(durum.platformSekme)) {
      durum.platformSekme = "genel";
    }
  }

  function svgGenelSimge() {
    return '<svg width="16" height="16" viewBox="0 0 18 18" fill="none" aria-hidden="true"><rect x="3" y="3" width="5" height="5" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="10" y="3" width="5" height="5" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="3" y="10" width="5" height="5" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="10" y="10" width="5" height="5" rx="1.5" stroke="currentColor" stroke-width="1.5"/></svg>';
  }

  // Platformun en dar etkin kalanı (sekme altındaki ince çizgi için).
  function platformEnDar(platform, sirali) {
    let enDar = null;
    for (const [id, h] of sirali) {
      if (h.platform !== platform) continue;
      const { bp, etkinKalan } = baglayiciBul(h);
      if (typeof etkinKalan !== "number") continue;
      if (!enDar || etkinKalan < enDar.kalan) enDar = { id, h, bp, kalan: etkinKalan };
    }
    return enDar;
  }

  function platformSekmeleriHtml(sirali, yan) {
    const sekmeHtml = (anahtar, ikon, ad, enDar) => {
      const aktif = durum.platformSekme === anahtar;
      let cizgi = '<span class="ps-cizgi bos" aria-hidden="true"></span>';
      let deger = "";
      let ipucu = "";
      if (enDar) {
        const oran = kalanModuMu() ? enDar.kalan : 1 - enDar.kalan;
        const sinif = enDar.h.durum === "bayat" ? "bayat" : havuzKritikMi(enDar.id, enDar.h) ? "kacin" : "";
        cizgi = `<span class="ps-cizgi" aria-hidden="true"><span class="ince-dolgu ${sinif}" style="width:${(clamp(oran, 0, 1) * 100).toFixed(1)}%"></span></span>`;
        ipucu = `${ad}: ${oranMetni(oran)}`;
        if (yan) deger = `<span class="ps-deger rakam${sinif === "kacin" ? " kritik" : ""}">${escHtml(oranMetni(oran))}</span>`;
      }
      return `
        <button type="button" class="platform-sekme${aktif ? " aktif" : ""}" role="tab" aria-selected="${aktif ? "true" : "false"}" tabindex="${aktif ? "0" : "-1"}" data-platform-sekme="${escHtml(anahtar)}"${ipucu ? ` data-ipucu="${escHtml(ipucu)}"` : ""}>
          <span class="ps-ust"><span class="ps-ikon">${ikon}</span><span class="ps-ad">${escHtml(ad)}</span>${deger}</span>
          ${cizgi}
        </button>`;
    };
    return [
      sekmeHtml("genel", svgGenelSimge(), t("gorunum.genel"), null),
      ...platformListesi(sirali).map((p) => sekmeHtml(p, svgMonogram(p), platformAdi(p), platformEnDar(p, sirali))),
    ].join("");
  }

  function svgChevron() {
    return '<svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M4 6l4 4 4-4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  }

  function enDarOzetCiz(hedefId, gruplar) {
    const el = document.getElementById(hedefId);
    if (!el) return;
    if (!gruplar || gruplar.length === 0) {
      el.classList.add("gizli");
      el.innerHTML = "";
      return;
    }
    const ozet = genelBaskiHesapla();
    el.classList.remove("gizli");
    if (ozet.enDar) {
      const { olcum, etkinKalan } = baglayiciBul(ozet.enDar);
      const oran = typeof etkinKalan === "number" ? (kalanModuMu() ? etkinKalan : 1 - etkinKalan) : null;
      const deger = oran !== null ? oranMetni(oran) : t(`baski.${ozet.seviye}`, ozet.seviye);
      const sure = olcum?.sifirlama ? sureKisa(olcum.sifirlama) : null;
      const solMetin = fmt(sure && sure !== t("genel.kalanSureYok") ? "genel.enDarOzet" : "genel.enDarOzetSuresiz", {
        ad: ozet.adPencere,
        deger,
        sure: sure || "",
      });
      el.innerHTML = `<span class="ozet-sol ${escHtml(ozet.seviye)}">${svgDurumSimge(ozet.seviye)}<span>${escHtml(solMetin)}</span></span>`;
      return;
    }
    el.innerHTML = `<span class="ozet-sol ${escHtml(ozet.seviye)}">${svgDurumSimge(ozet.seviye)}<span>${escHtml(ozet.metin)}</span></span>`;
  }

  function platformDetayAcikIcerikHtml(grup) {
    const bloklar = grup.havuzlar.map(([id, h]) => {
      const pencereler = h.pencereler || {};
      const pencereListesi = pencereSirali(h);
      const satirlar = pencereListesi.map((pAd) => {
        const olcum = pencereler[pAd];
        const oran = anaDegerOrani(olcum);
        const sinif = cubukSinifi(id, h, pAd, olcum);
        const gecen = gecenOranHesapla(olcum, simdiIso());
        const tempoOran = gecen === null || sinif === "kilitli" ? null : kalanModuMu() ? 1 - gecen : gecen;
        const tempo = tempoBilgisi(h, pAd, olcum);
        const sure = olcum.sifirlama ? fmt("genel.sonraSifirlanir", { sure: sureKisa(olcum.sifirlama) }) : t("genel.kalanSureYok");
        const sureIpucu = olcum.sifirlama ? gunVeSaatBicimle(olcum.sifirlama) : "";
        return `
          <div class="kp-satir-bilgi">
            <div class="kp-satir-ust">
              <span class="kp-ad">${escHtml(pencereBasligi(pAd))} · <span class="rakam">${escHtml(oranMetni(oran))}</span></span>
              <span class="kp-sure rakam"${sureIpucu ? ` data-ipucu="${escHtml(sureIpucu)}"` : ""}>${escHtml(sure)}</span>
            </div>
            ${inceCubukHtml({ oran, sinif, tempoOran })}
            ${tempo ? `<div class="dp-tempo${tempo.ton ? ` ${tempo.ton}` : ""}">${escHtml(tempo.metin)}</div>` : ""}
          </div>
        `;
      });

      if (satirlar.length === 0) {
        if (h.ek?.kullanim) {
          const sm = sayacMetinleri(h.ek.kullanim);
          satirlar.push(`<div class="kart-not rakam">${escHtml([sm.s24, sm.s7, sm.toplam].filter(Boolean).join(" · "))}</div>`);
        } else if (h.bakiye && typeof h.bakiye.toplam === "number") {
          satirlar.push(
            `<div class="kart-not rakam">${escHtml(t("genel.bakiyeEtiket"))}: ${escHtml(sayiBicimle(h.bakiye.toplam, { minimumFractionDigits: 2, maximumFractionDigits: 2 }))} ${escHtml(h.bakiye.para_birimi || "")}</div>`
          );
        }
      }

      const ekler = havuzEkleriMetni(h);
      const notlar = havuzNotuYerel(id, h);
      const dipNot = [ekler, ...notlar].filter(Boolean).join(" · ");

      return `
        <div class="kart-detay-havuz">
          ${grup.havuzlar.length > 1 ? `<div class="kart-detay-havuz-ad">${escHtml(havuzKisaAd(h))}</div>` : ""}
          ${satirlar.join("")}
          ${dipNot ? `<div class="kart-not">${escHtml(dipNot)}</div>` : ""}
        </div>
      `;
    });

    const kaynakli = grup.havuzlar
      .map(([, h]) => h)
      .filter((h) => h.kaynak)
      .sort((a, b) => (kaynakYasSn(a.kaynak) ?? 1e9) - (kaynakYasSn(b.kaynak) ?? 1e9));
    const meta = kaynakli[0] ? kaynakIpucu(kaynakli[0]) : t("genel.bilinmiyor");

    return `
      <div class="kart-detay-acik">
        ${bloklar.join("")}
        <div class="kart-detay-alt">
          <span class="kart-detay-meta">${escHtml(meta)}</span>
          <button type="button" class="kart-detay-bag" data-platform-git="${escHtml(grup.platform)}">${escHtml(t("genel.platformSekmesineGit"))}</button>
        </div>
      </div>
    `;
  }

  // Genel görünümünde platform başına tek sade satır/kart (ikincil bilgi tıklanınca açılır).
  function platformKartHtml(grup, genis) {
    const p = grup.platform;
    const h = grup.repH;
    const id = grup.repId;
    const secili = durum.seciliHavuzId === p;
    const acik = !durum.duzenleModu && durum.acikPlatformlar.has(p);
    const sabitList = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, durum.anlik.havuzlar);
    const gizliList = kartListesiniPlatformaEsle(durum.kartTercihler.gizliKartlar, durum.anlik.havuzlar);
    const sabitMi = durum.duzenleModu && sabitList.includes(p);
    const gizliMi = durum.duzenleModu && gizliList.includes(p);
    const tumBayat = grup.havuzlar.every(([, x]) => x.durum === "bayat");

    const siniflar = [
      genis ? "detay-kart" : "havuz-satir",
      "kompakt-kart",
      secili ? "secili" : "",
      acik ? "acik" : "",
      tumBayat ? "bayat" : "",
      durum.duzenleModu ? "duzenle-aktif" : "",
      sabitMi ? "kart-sabit" : "",
      gizliMi ? "kart-gizli" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const { bp, olcum, etkinKalan } = baglayiciBul(h);
    let degerHtml = "";
    let sureHtml = "";
    let cubukHtml = "";

    if (olcum && typeof etkinKalan === "number") {
      const oran = kalanModuMu() ? etkinKalan : 1 - etkinKalan;
      const kritik = havuzKritikMi(id, h);
      const pencereKisaEtiket = bp ? sozlukEslesmeBul("pencereKisa", bp) || "" : "";
      const adEtiket = grup.havuzlar.length > 1 && h.etiket ? ` · ${havuzKisaAd(h)}` : "";
      const sure = olcum.sifirlama ? sureKisa(olcum.sifirlama) : "";
      const cSinif = cubukSinifi(id, h, bp, olcum);

      degerHtml = `<span class="kart-deger rakam${kritik ? " kritik" : ""}" data-ipucu="${escHtml(fmt(kalanModuMu() ? "genel.enDarIpucu" : "genel.enDarKullanilanIpucu", { pencere: pencereBasligi(bp) }))}">${escHtml(oranMetni(oran))}</span>`;
      sureHtml = sure ? `<span class="kart-sure rakam">${escHtml(pencereKisaEtiket ? `${pencereKisaEtiket}${adEtiket} · ${sure}` : sure)}</span>` : "";
      cubukHtml = inceCubukHtml({ oran, sinif: cSinif });
    } else if (h.ek?.kullanim) {
      const sm = sayacMetinleri(h.ek.kullanim);
      degerHtml = `<span class="kart-deger soluk rakam">${escHtml(sm.s24)}</span>`;
    } else if (h.bakiye && typeof h.bakiye.toplam === "number") {
      degerHtml = `<span class="kart-deger rakam">${escHtml(sayiBicimle(h.bakiye.toplam, { minimumFractionDigits: 2, maximumFractionDigits: 2 }))} ${escHtml(h.bakiye.para_birimi || "")}</span>`;
    } else {
      degerHtml = `<span class="kart-deger soluk">${escHtml(t(`baski.${h.durum}`, h.durum))}</span>`;
    }

    const etiket = genis ? "article" : "div";
    const aciklamaAria = `${grup.ad}${olcum && typeof etkinKalan === "number" ? `: ${oranMetni(kalanModuMu() ? etkinKalan : 1 - etkinKalan)}${olcum.sifirlama ? `, ${sureKisa(olcum.sifirlama)}` : ""}` : ""}`;

    return `
      <${etiket} class="${siniflar}" data-havuz-id="${escHtml(p)}" role="listitem">
        ${
          durum.duzenleModu
            ? `<div class="kart-ust">
                 ${kartTutamakHtml(p, { etiket: grup.ad })}
                 <span class="kart-ikon">${svgMonogram(p)}</span>
                 <span class="kart-ad">${escHtml(grup.ad)}</span>
                 <span class="kart-ozet-sag">${degerHtml}${sureHtml}</span>
               </div>
               ${kartDuzenleKontrollerHtml(p, { etiket: grup.ad })}
               ${cubukHtml}`
            : `<button type="button" class="kart-ac-btn" data-platform-ac="${escHtml(p)}" aria-expanded="${acik ? "true" : "false"}" aria-label="${escHtml(aciklamaAria)}">
                 <span class="kart-ust">
                   <span class="kart-ikon">${svgMonogram(p)}</span>
                   <span class="kart-ad">${escHtml(grup.ad)}</span>
                   <span class="kart-ozet-sag">${degerHtml}${sureHtml}<span class="kart-ok">${svgChevron()}</span></span>
                 </span>
                 ${cubukHtml}
               </button>
               ${acik ? platformDetayAcikIcerikHtml(grup) : ""}`
        }
      </${etiket}>
    `;
  }

  function genelListeHtml(gruplar, genis) {
    if (gruplar.length === 0) {
      const tumBagliSayi = platformGruplariOlustur(durum.anlik.havuzlar, { gizlileriDahilEt: true }).length;
      if (tumBagliSayi > 0 && !durum.duzenleModu) {
        return `
          <div class="bos-durum kart-bos-gizli-durum">
            <p>${escHtml(t("kartlar.tumuGizli"))}</p>
            <button type="button" class="ikincil-btn" data-kart-duzenle-ac="1">${escHtml(t("kartlar.duzenle"))}</button>
          </div>`;
      }
      return `<div class="bos-durum"><p>${escHtml(t("genel.bosVeri"))}</p></div>`;
    }
    return gruplar.map((g) => platformKartHtml(g, genis)).join("");
  }

  function detayPencereHtml(id, h, pAd, olcum, baslik) {
    const oran = anaDegerOrani(olcum);
    const sinif = cubukSinifi(id, h, pAd, olcum);
    const gecen = gecenOranHesapla(olcum, simdiIso());
    const tempoOran = gecen === null || sinif === "kilitli" ? null : kalanModuMu() ? 1 - gecen : gecen;
    const tempo = tempoBilgisi(h, pAd, olcum);
    const sure = olcum.sifirlama ? fmt("genel.sonraSifirlanir", { sure: sureKisa(olcum.sifirlama) }) : "";
    const sureIpucu = olcum.sifirlama ? gunVeSaatBicimle(olcum.sifirlama) : "";
    const ipucu = `${baslik}: ${yuzdeBicimle(olcum.kalan)} ${t("genel.kalan")} · ${yuzdeBicimle(olcum.kullanilan)} ${t("genel.kullanildi")}`;
    return `
      <div class="detay-pencere">
        <div class="dp-baslik">${escHtml(baslik)}</div>
        ${inceCubukHtml({ oran, sinif, ipucu, tempoOran, kalin: true })}
        <div class="dp-alt">
          <span class="dp-oran rakam${sinif === "kacin" ? " kritik" : ""}">${escHtml(oranMetni(oran))}</span>
          ${sure ? `<span class="dp-sure rakam" data-ipucu="${escHtml(sureIpucu)}">${escHtml(sure)}</span>` : ""}
        </div>
        ${tempo ? `<div class="dp-tempo${tempo.ton ? ` ${tempo.ton}` : ""}" tabindex="0" data-ipucu="${escHtml(tempo.ipucu)}">${escHtml(tempo.metin)}</div>` : ""}
      </div>`;
  }

  function havuzEkleriMetni(h) {
    const ekler = [];
    if (h.ek?.sifirlama_kredisi?.adet) ekler.push(fmt("genel.sifirlamaKredisi", { n: sayiBicimle(h.ek.sifirlama_kredisi.adet) }));
    if (typeof h.ek?.eszamanli_istek === "number" && typeof h.ek?.eszamanli_tavan === "number") {
      ekler.push(`${t("genel.eszamanliEtiket")} ${sayiBicimle(h.ek.eszamanli_istek)}/${sayiBicimle(h.ek.eszamanli_tavan)}`);
    }
    if (h.ek?.seviye) ekler.push(`${t("genel.seviyeEtiket")} ${h.ek.seviye}`);
    if (h.ek?.muaf) ekler.push(t("genel.muafEtiket"));
    if (h.ek?.kampanya_bitis) ekler.push(`${t("genel.bitisEtiket")} ${gunVeSaatBicimle(h.ek.kampanya_bitis)}`);
    return ekler.join(" · ");
  }

  function detayHavuzHtml(id, h, cokluMu) {
    const pencereler = h.pencereler || {};
    const notlar = havuzNotuYerel(id, h);
    const parcalar = [];

    if (cokluMu) {
      parcalar.push(`
        <div class="detay-havuz-ust">
          <span class="detay-havuz-ad">${escHtml(havuzKisaAd(h))}</span>
          ${h.durum === "bayat" ? `<span class="kart-durum" tabindex="0" data-ipucu="${escHtml(kaynakIpucu(h))}">${escHtml(t("baski.bayat"))}</span>` : ""}
        </div>`);
    }

    const { bp, olcum: anaOlcum, etkinKalan } = baglayiciBul(h);
    if (anaOlcum && typeof etkinKalan === "number" && (durum.gostergeTuru === "halka" || (durum.gostergeTuru === "pasta" && durum.pastaAltTuru !== "tek"))) {
      const ikincil = pencereSirali(h).filter((p) => p !== bp).map((p) => pencereler[p])[0] || null;
      const halka = durum.gostergeTuru === "halka" ? svgHalkaHtml(h, anaOlcum, ikincil, 64) : svgPastaAyriHtml(h, anaOlcum, 64);
      parcalar.push(`
        <div class="detay-gosterge">
          ${halka}
          <div class="detay-gosterge-metin">
            <span>${escHtml(kalanModuMu() ? t("genel.etkinKalan") : t("genel.kullanildi"))}</span>
            <span class="soluk">${escHtml(pencereBasligi(bp))}</span>
          </div>
        </div>`);
    }

    const pencereHtml = pencereSirali(h).map((pAd) => detayPencereHtml(id, h, pAd, pencereler[pAd], pencereBasligi(pAd)));
    for (const [altAd, altVal] of Object.entries(h.alt_sayaclar || {})) {
      if (!altVal || typeof altVal !== "object") continue;
      const kalan = clamp(altVal.kalan ?? 1 - (altVal.kullanilan ?? 0), 0, 1);
      const olcum = { ...altVal, kalan, kullanilan: clamp(altVal.kullanilan ?? 1 - kalan, 0, 1) };
      pencereHtml.push(detayPencereHtml(id, { ...h, tahmin: {} }, altAd, olcum, altSayacAdiBicimle(altAd)));
    }
    if (pencereHtml.length > 0) parcalar.push(`<div class="detay-pencereler">${pencereHtml.join("")}</div>`);

    if (pencereHtml.length === 0) {
      if (h.ek?.kullanim) {
        const sm = sayacMetinleri(h.ek.kullanim);
        parcalar.push(`
          <div class="detay-sayac rakam">
            <span>${escHtml(sm.s24)}</span>
            <span>${escHtml(sm.s7)}</span>
            <span class="soluk">${escHtml([sm.toplam, sm.son].filter(Boolean).join(" · "))}</span>
          </div>`);
      } else if (h.bakiye && typeof h.bakiye.toplam === "number") {
        parcalar.push(`
          <div class="detay-sayac rakam">
            <span>${escHtml(t("genel.bakiyeEtiket"))}: ${escHtml(sayiBicimle(h.bakiye.toplam, { minimumFractionDigits: 2, maximumFractionDigits: 2 }))} ${escHtml(h.bakiye.para_birimi || "")}</span>
          </div>`);
      } else {
        parcalar.push(`<div class="detay-sayac"><span class="soluk">${escHtml(notlar[0] || t(`baski.${h.durum}`, h.durum))}</span></div>`);
      }
    }

    const ekler = havuzEkleriMetni(h);
    if (ekler) {
      parcalar.push(`<div class="detay-ek rakam"${notlar.length ? ` tabindex="0" data-ipucu="${escHtml(notlar.join(" · "))}"` : ""}>${escHtml(ekler)}</div>`);
    }

    return `<section class="detay-havuz${durum.seciliHavuzId === id ? " secili" : ""}" data-havuz-id="${escHtml(id)}" aria-label="${escHtml(h.etiket)}">${parcalar.join("")}</section>`;
  }

  // --- 30 günlük geçmiş (günlük tepe kullanım) ---
  function gecmisPenceresi(h) {
    const p = h?.pencereler || {};
    if (p["5s"]) return "5s";
    if (h?.baglayici_pencere && p[h.baglayici_pencere]) return h.baglayici_pencere;
    return pencereSirali(h)[0] || null;
  }

  function demoGunlukSeri(havuzId) {
    let tohum = 0;
    for (const c of String(havuzId)) tohum = (tohum * 31 + c.charCodeAt(0)) >>> 0;
    const rastgele = () => {
      tohum = (tohum + 0x6d2b79f5) >>> 0;
      let x = Math.imul(tohum ^ (tohum >>> 15), 1 | tohum);
      x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
      return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
    };
    const bazMs = Date.parse(simdiIso());
    const bugun = new Date(Number.isFinite(bazMs) ? bazMs : Date.now());
    const degerler = [];
    for (let i = 29; i >= 0; i--) {
      const gun = new Date(bugun.getFullYear(), bugun.getMonth(), bugun.getDate() - i);
      const haftaSonu = gun.getDay() === 0 || gun.getDay() === 6;
      const r = rastgele();
      const deger = i > 26 && havuzId === "agy_argon" ? null : clamp((haftaSonu ? 0.12 : 0.34) + r * (haftaSonu ? 0.3 : 0.62), 0, 1);
      degerler.push({ tarih: gun.toISOString(), deger });
    }
    return degerler;
  }

  async function gunlukGecmisGetir(havuzId, pencere) {
    const anahtar = `${havuzId}/${pencere}`;
    const onceki = durum.gunlukGecmis[anahtar];
    durum.gunlukGecmis[anahtar] = { ...(onceki || {}), yukleniyor: true, alindi: onceki?.alindi || 0 };
    let degerler = null;
    try {
      const y = await invoke("gecmis_al", { havuz: havuzId, pencere, gun: 30 });
      const ornekler = Array.isArray(y?.ornekler) ? y.ornekler : [];
      const gunlere = new Map();
      for (const o of ornekler) {
        const ms = Date.parse(o?.zaman || "");
        if (!Number.isFinite(ms)) continue;
        const d = new Date(ms);
        const k = `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
        gunlere.set(k, Math.max(gunlere.get(k) ?? 0, clamp(Number(o.kullanilan) || 0, 0, 1)));
      }
      const bugun = new Date();
      degerler = [];
      for (let i = 29; i >= 0; i--) {
        const gun = new Date(bugun.getFullYear(), bugun.getMonth(), bugun.getDate() - i);
        const k = `${gun.getFullYear()}-${gun.getMonth()}-${gun.getDate()}`;
        degerler.push({ tarih: gun.toISOString(), deger: gunlere.has(k) ? gunlere.get(k) : null });
      }
    } catch (_) {
      degerler = [];
    }
    durum.gunlukGecmis[anahtar] = { degerler, alindi: Date.now(), yukleniyor: false };
    if (durum.yuzey === "acilir") acilirCiz();
    else if (durum.yuzey === "pencere" && durum.sekme === "simdi") simdiSekmesiCiz();
  }

  function gunlukGecmisAl(havuzId, pencere) {
    if (durum.demoMu) return { degerler: demoGunlukSeri(havuzId), yukleniyor: false };
    if (!durum.tauriVarMi || !pencere) return null;
    const anahtar = `${havuzId}/${pencere}`;
    const kayit = durum.gunlukGecmis[anahtar];
    if (!kayit || (!kayit.yukleniyor && Date.now() - kayit.alindi > 300000)) {
      gunlukGecmisGetir(havuzId, pencere);
    }
    return durum.gunlukGecmis[anahtar] || { degerler: null, yukleniyor: true };
  }

  function gecmisCubuklariHtml(havuzId, h) {
    const pencere = gecmisPenceresi(h);
    const kayit = pencere ? gunlukGecmisAl(havuzId, pencere) : null;
    if (!kayit) return "";
    const ustHtml = `
      <div class="gm-ust">
        <span class="gm-baslik">${escHtml(t("genel.gecmisBaslik"))}</span>
        <span class="soluk">${escHtml(`${havuzKisaAd(h)} · ${pencereEtiketi(pencere)}`)}</span>
      </div>`;
    const degerler = kayit.degerler;
    if (!Array.isArray(degerler) || degerler.length === 0 || degerler.every((d) => d.deger === null)) {
      const metin = kayit.yukleniyor ? t("gecmis.yukleniyor") : t("gecmis.bos");
      return `<div class="gecmis-mini">${ustHtml}<p class="gm-bos soluk">${escHtml(metin)}</p></div>`;
    }
    const gunBicim = (iso) => {
      try {
        return new Intl.DateTimeFormat(durum.dil, { day: "numeric", month: "short" }).format(Date.parse(iso));
      } catch (_) {
        return iso.slice(0, 10);
      }
    };
    const cubuklar = degerler
      .map(({ tarih, deger }) => {
        const gun = gunBicim(tarih);
        if (deger === null) {
          return `<span class="gm-cubuk bos" data-ipucu="${escHtml(fmt("genel.gecmisVeriYok", { gun }))}"></span>`;
        }
        const yukseklik = Math.max(4, Math.round(clamp(deger, 0, 1) * 100));
        return `<span class="gm-cubuk" style="height:${yukseklik}%" data-ipucu="${escHtml(fmt("genel.gecmisGunBilgi", { gun, oran: yuzdeBicimle(deger) }))}"></span>`;
      })
      .join("");
    return `
      <div class="gecmis-mini">
        ${ustHtml}
        <div class="gm-cubuklar" role="img" aria-label="${escHtml(t("genel.gecmisBaslik"))}">${cubuklar}</div>
        <div class="gm-eksen soluk"><span>${escHtml(t("genel.otuzGunOnce"))}</span><span>${escHtml(t("genel.bugun"))}</span></div>
      </div>`;
  }

  function platformDetayHtml(platform, sirali) {
    const havuzlar = sirali.filter(([, h]) => h.platform === platform);
    if (havuzlar.length === 0) return "";
    const kaynakli = havuzlar
      .map(([, h]) => h)
      .filter((h) => h.kaynak)
      .sort((a, b) => (kaynakYasSn(a.kaynak) ?? 1e9) - (kaynakYasSn(b.kaynak) ?? 1e9));
    const enTaze = kaynakli[0] || null;
    const zaman = enTaze ? guncellikMetni(enTaze.kaynak) : "";
    const notlar = havuzlar.flatMap(([id, h]) => (havuzlar.length > 1 ? [] : havuzNotuYerel(id, h)));
    const zamanIpucu = [enTaze ? kaynakIpucu(enTaze) : "", ...notlar].filter(Boolean).join(" · ");
    const cokluMu = havuzlar.length > 1;
    const gecmisHavuz = havuzlar.find(([, h]) => h.pencereler && Object.keys(h.pencereler).length > 0);

    return `
      <div class="detay-ust">
        <span class="kart-ikon">${svgMonogram(platform)}</span>
        <span class="detay-ad">${escHtml(platformAdi(platform))}</span>
        ${zaman ? `<span class="detay-zaman soluk" tabindex="0"${zamanIpucu ? ` data-ipucu="${escHtml(zamanIpucu)}"` : ""}>${escHtml(zaman)}</span>` : ""}
      </div>
      <div class="detay-havuzlar">
        ${havuzlar.map(([id, h]) => detayHavuzHtml(id, h, cokluMu)).join("")}
      </div>
      ${gecmisHavuz ? gecmisCubuklariHtml(gecmisHavuz[0], gecmisHavuz[1]) : ""}
    `;
  }

  // --------------------------------------------------------------------------
  // 7b. Açılır Pencere (340 px) Çizimi
  // --------------------------------------------------------------------------
  function acilirCiz() {
    if (suruklemeKonteynerAktifMi("acilir-havuz-listesi")) return;

    const pencereEl = document.getElementById("acilir-pencere");
    const kapaliKart = document.getElementById("tepsi-kapali-kart");
    if (!pencereEl || !kapaliKart) return;
    pencereEl.classList.toggle("gizli", durum.acilirKapali);
    kapaliKart.classList.toggle("gizli", !durum.acilirKapali);

    duzenleCubuguCiz("acilir-duzenle-cubuk");

    const sirali = havuzSirala(durum.anlik.havuzlar, { gizlileriDahilEt: durum.duzenleModu });
    const gruplar = platformGruplariOlustur(durum.anlik.havuzlar, { gizlileriDahilEt: durum.duzenleModu });
    if (durum.duzenleModu) durum.platformSekme = "genel";
    platformSekmesiDogrula(sirali);
    const genelMi = durum.platformSekme === "genel";

    const guncelEl = document.getElementById("acilir-guncel");
    if (guncelEl) guncelEl.textContent = genelMi ? ustGuncellikMetni() : "";

    const sekmeEl = document.getElementById("acilir-platform-sekmeler");
    if (sekmeEl) {
      const sekmeOdak = odakKimligiAl(sekmeEl);
      sekmeEl.innerHTML = platformSekmeleriHtml(sirali, false);
      sekmeEl.setAttribute("aria-label", t("gorunum.platformlarAria"));
      odakKimligiYukle(sekmeEl, sekmeOdak);
    }

    const ozetEl = document.getElementById("acilir-ozet");
    if (genelMi && !durum.duzenleModu) {
      enDarOzetCiz("acilir-ozet", gruplar);
    } else if (ozetEl) {
      ozetEl.classList.add("gizli");
      ozetEl.innerHTML = "";
    }

    const tekPastaEl = document.getElementById("acilir-pasta-tek-alan");
    if (tekPastaEl) {
      tekPastaEl.classList.add("gizli");
      tekPastaEl.innerHTML = "";
    }

    const listeEl = document.getElementById("acilir-havuz-listesi");
    const detayEl = document.getElementById("acilir-detay");
    if (listeEl && detayEl) {
      listeEl.classList.toggle("gizli", !genelMi);
      detayEl.classList.toggle("gizli", genelMi);
      if (genelMi) {
        const listeOdak = odakKimligiAl(listeEl);
        listeEl.innerHTML = genelListeHtml(gruplar, false);
        detayEl.innerHTML = "";
        odakKimligiYukle(listeEl, listeOdak);
      } else {
        listeEl.innerHTML = "";
        detayEl.innerHTML = platformDetayHtml(durum.platformSekme, sirali);
      }
    }

    const baglanabilirSayisi = Object.values(durum.anlik.havuzlar || {}).filter(
      (h) => h && (h.durum === "baglanmadi" || h.durum === "bilinmiyor") && platformEtkinMi(h.platform)
    ).length;
    const baglaBtn = document.getElementById("btn-acilir-baglanabilir");
    if (baglaBtn) {
      baglaBtn.textContent =
        baglanabilirSayisi > 0 ? fmt("genel.baglanabilir", { n: sayiBicimle(baglanabilirSayisi) }) : t("genel.baglanabilirYok");
      baglaBtn.classList.toggle("gizli", !genelMi || baglanabilirSayisi === 0);
    }

    tepsiIkonGuncelle();
  }

  function tepsiIkonGuncelle() {
    let enDar5s = null;
    let enDarHafta = null;
    for (const h of Object.values(durum.anlik.havuzlar || {})) {
      if (!bagliMi(h) || h.ek?.kullanim) continue;
      if (h.durum !== "ok" && h.durum !== "bayat") continue;
      const p5 = h.pencereler?.["5s"];
      if (p5 && typeof p5.kalan === "number") {
        const k = clamp(p5.kalan, 0, 1);
        const u = clamp(p5.kullanilan ?? 1 - k, 0, 1);
        if (!enDar5s || k < enDar5s.kalan) enDar5s = { kalan: k, kullanilan: u };
      }
      const ph = h.pencereler?.hafta;
      if (ph && typeof ph.kalan === "number") {
        const k = clamp(ph.kalan, 0, 1);
        const u = clamp(ph.kullanilan ?? 1 - k, 0, 1);
        if (!enDarHafta || k < enDarHafta.kalan) enDarHafta = { kalan: k, kullanilan: u };
      }
    }

    const kalanMod = durum.degerYonu !== "kullanilan";
    const oran5s = enDar5s ? (kalanMod ? enDar5s.kalan : enDar5s.kullanilan) : null;
    const oranHft = enDarHafta ? (kalanMod ? enDarHafta.kalan : enDarHafta.kullanilan) : null;
    const b5 = document.getElementById("tepsi-bar-5s");
    const bH = document.getElementById("tepsi-bar-hafta");
    if (b5) b5.setAttribute("width", oran5s === null ? "0" : String(Math.max(1.5, Math.round(clamp(oran5s, 0, 1) * 14))));
    if (bH) bH.setAttribute("width", oranHft === null ? "0" : String(Math.max(1.5, Math.round(clamp(oranHft, 0, 1) * 14))));

    const ipucuEl = document.getElementById("tepsi-ipucu-metin");
    const ikonBtn = document.getElementById("btn-tepsi-ikon");
    const clockEl = document.getElementById("gorev-saat");
    if (clockEl) clockEl.textContent = saatBicimle(simdiIso());

    let ozetMetin = t("genel.veriYok");
    if (oran5s !== null || oranHft !== null) {
      const genel = genelBaskiHesapla();
      if (genel.enDar) {
        const oran = kalanMod
          ? clamp(genel.enDar.etkin_kalan ?? 0, 0, 1)
          : clamp(1 - (genel.enDar.etkin_kalan ?? 0), 0, 1);
        const yonEtiket = kalanMod ? t("genel.kalan") : t("genel.kullanildi");
        ozetMetin = `${genel.metin} (${yuzdeBicimle(oran)} ${yonEtiket})`;
      } else {
        ozetMetin = genel.metin;
      }
    }
    if (ipucuEl) ipucuEl.textContent = ozetMetin;
    if (ikonBtn) ikonBtn.dataset.ipucu = `${t("genel.tepsiIkonIpucu")} · ${ozetMetin}`;
  }

  // --------------------------------------------------------------------------
  // 8. Genişletilmiş Pencere — Şimdi Sekmesi (solda platform listesi, sağda Genel ya da ayrıntı)
  // --------------------------------------------------------------------------
  function simdiSekmesiCiz() {
    if (suruklemeKonteynerAktifMi("simdi-havuz-grid")) return;
    duzenleCubuguCiz("simdi-duzenle-cubuk");

    const sirali = havuzSirala(durum.anlik.havuzlar, { gizlileriDahilEt: durum.duzenleModu });
    const gruplar = platformGruplariOlustur(durum.anlik.havuzlar, { gizlileriDahilEt: durum.duzenleModu });
    if (durum.duzenleModu) durum.platformSekme = "genel";
    platformSekmesiDogrula(sirali);
    const genelMi = durum.platformSekme === "genel";

    const sekmeEl = document.getElementById("simdi-platform-sekmeler");
    if (sekmeEl) {
      const sekmeOdak = odakKimligiAl(sekmeEl);
      sekmeEl.innerHTML = platformSekmeleriHtml(sirali, true);
      sekmeEl.setAttribute("aria-label", t("gorunum.platformlarAria"));
      odakKimligiYukle(sekmeEl, sekmeOdak);
    }

    const ozetEl = document.getElementById("simdi-ozet");
    if (genelMi && !durum.duzenleModu) {
      enDarOzetCiz("simdi-ozet", gruplar);
    } else if (ozetEl) {
      ozetEl.classList.add("gizli");
      ozetEl.innerHTML = "";
    }

    const tekPastaEl = document.getElementById("simdi-pasta-tek-alan");
    if (tekPastaEl) {
      tekPastaEl.classList.add("gizli");
      tekPastaEl.innerHTML = "";
    }

    const gridEl = document.getElementById("simdi-havuz-grid");
    const detayEl = document.getElementById("simdi-detay");
    if (!gridEl || !detayEl) return;
    gridEl.classList.toggle("gizli", !genelMi);
    detayEl.classList.toggle("gizli", genelMi);
    if (!genelMi) {
      gridEl.innerHTML = "";
      detayEl.innerHTML = platformDetayHtml(durum.platformSekme, sirali);
      return;
    }
    detayEl.innerHTML = "";
    const gridOdak = odakKimligiAl(gridEl);
    gridEl.innerHTML = genelListeHtml(gruplar, true);
    odakKimligiYukle(gridEl, gridOdak);
  }

  // --------------------------------------------------------------------------
  // 9. Geçmiş Sekmesi (24 Saatlik SVG Zaman Serisi Grafiği)
  // --------------------------------------------------------------------------
  function canliGecmisSerisiBul(havuzId, pencere) {
    const kayit = durum.gecmisCanli[havuzId];
    if (!kayit) return null;
    if (pencere && kayit.anahtar !== `${havuzId}/${pencere}`) return null;
    return kayit;
  }

  function gecmisSekmesiCiz() {
    baslikBilgiYaz("gecmis-baslik", "gecmis-aciklama", t("gecmis.baslik"), durum.demoMu ? t("gecmis.aciklama") : t("gecmis.canliAciklama"));

    const adayHavuzlar = durum.demoMu
      ? Object.entries(GECMIS_SERILER)
          .filter(([, info]) => platformEtkinMi(info.platform))
          .map(([id, info]) => ({
            id,
            etiket: `${sadeEtiket(info.ad)} · ${pencereEtiketi(info.pencere)}`,
            pencere: info.pencere,
          }))
      : havuzSirala(durum.anlik.havuzlar, { gizlileriDahilEt: true })
          .filter(([, h]) => h.pencereler && Object.keys(h.pencereler).length > 0)
          .map(([id, h]) => {
            const pAd = h.baglayici_pencere || Object.keys(h.pencereler)[0];
            return { id, etiket: `${h.etiket} · ${pencereEtiketi(pAd)}`, pencere: pAd };
          });

    if (adayHavuzlar.length > 0 && !adayHavuzlar.some((a) => a.id === durum.gecmisHavuzId)) {
      durum.gecmisHavuzId = adayHavuzlar[0].id;
    }

    const seciciEl = document.getElementById("gecmis-havuz-secici");
    if (seciciEl) {
      const sOdak = odakKimligiAl(seciciEl);
      seciciEl.innerHTML = adayHavuzlar
        .map(
          (info) =>
            `<button type="button" class="mini-btn${durum.gecmisHavuzId === info.id ? " aktif" : ""}" data-gecmis-id="${escHtml(info.id)}">${escHtml(info.etiket)}</button>`
        )
        .join("");
      odakKimligiYukle(seciciEl, sOdak);
    }

    const seciliAday = adayHavuzlar.find((a) => a.id === durum.gecmisHavuzId);
    if (!durum.demoMu && durum.tauriVarMi && seciliAday && durum.yuzey === "pencere" && durum.sekme === "gecmis") {
      canliGecmisGetir(seciliAday.id, seciliAday.pencere);
    }

    const demoHam = durum.demoMu ? GECMIS_SERILER[durum.gecmisHavuzId] || Object.values(GECMIS_SERILER)[0] : null;
    const seri = demoHam
      ? Object.assign({}, demoHam, {
          etiket: `${sadeEtiket(demoHam.ad)} · ${pencereEtiketi(demoHam.pencere)}`,
        })
      : canliGecmisSerisiBul(durum.gecmisHavuzId, seciliAday?.pencere);

    const ustBilgiEl = document.getElementById("grafik-ust-bilgi");
    const svgKapsayiciEl = document.getElementById("gecmis-svg-kapsayici");
    const altMetrikEl = document.getElementById("grafik-alt-metrikler");
    if (!ustBilgiEl || !svgKapsayiciEl || !altMetrikEl) return;

    if (!seri || !Array.isArray(seri.noktalar) || seri.noktalar.length === 0) {
      ustBilgiEl.innerHTML = `<span><strong>${escHtml(seciliAday?.etiket || "—")}</strong></span><span class="rakam" id="grafik-imlec-bilgi"></span>`;
      svgKapsayiciEl.innerHTML = `<div class="satir-neden" style="padding:28px 16px;text-align:center;">${escHtml(durum.gecmisYukleniyor ? t("gecmis.yukleniyor") : t("gecmis.bos"))}</div>`;
      altMetrikEl.innerHTML = `
        <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.tepe"))}:</span> <strong class="rakam">—</strong></div>
        <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.ortalama"))}:</span> <strong class="rakam">—</strong></div>
        <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.sifirlamaSayisi"))}:</span> <strong class="rakam">0</strong></div>
        <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.sonDeger"))}:</span> <strong class="rakam">—</strong></div>
      `;
      return;
    }

    const aktifEsikKalanYuzde = clamp(
      Number(esikYuzdesi(durum.gecmisHavuzId, seciliAday?.pencere || seri.pencere, seri.platform)) || 20,
      1,
      99
    );
    const dinamikEsik = 100 - aktifEsikKalanYuzde;

    const pts = seri.noktalar;
    const zamanlar = seri.zamanlar || null;
    const renk = PLATFORM_RENK[seri.platform] || "var(--vurgu)";

    // Bu hızla sıfırlamaya kadar izdüşüm (kesikli çizgi): servis tahmini (saatlik hız) varsa
    // grafiğin sağına "şimdi → sıfırlama" dilimi eklenir.
    const izdusum = (() => {
      const pAd = seciliAday?.pencere || seri.pencere;
      const h = durum.anlik.havuzlar?.[durum.gecmisHavuzId];
      const th = h?.tahmin?.[pAd];
      const ol = h?.pencereler?.[pAd];
      if (!th || th.durum === "tahmin_yok" || typeof th.saatlik_hiz !== "number" || !ol?.sifirlama) return null;
      const kalanSaat = (Date.parse(ol.sifirlama) - Date.parse(simdiIso())) / 3600000;
      if (!Number.isFinite(kalanSaat) || kalanSaat <= 0) return null;
      const sonDeger = pts[pts.length - 1];
      const hedef = sonDeger + th.saatlik_hiz * kalanSaat * 100;
      return {
        hedef: clamp(hedef, sonDeger, 100),
        dolarMi: hedef >= 100,
        dolmaOrani: hedef > sonDeger ? clamp((100 - sonDeger) / (hedef - sonDeger), 0, 1) : 1,
        metin:
          th.sifirlanmadan_biter && th.bitis_tahmini
            ? fmt("genel.onceDolarProjeksiyon", { zaman: gunVeSaatBicimle(th.bitis_tahmini) })
            : fmt("genel.kullanimProjeksiyon", { oran: yuzdeBicimle(clamp(hedef, 0, 100) / 100) }),
        sifirlama: ol.sifirlama,
      };
    })();

    const W = 760;
    const H = 220;
    const padL = 48;
    const padR = izdusum ? 96 : 16;
    const padT = 16;
    const padB = 28;
    const gW = W - padL - padR;
    const gH = H - padT - padB;
    const xOf = (i) => padL + (pts.length <= 1 ? gW * 0.5 : (i / (pts.length - 1)) * gW);
    const yOf = (val) => padT + gH - (clamp(val, 0, 100) / 100) * gH;

    const yataylar = [0, 25, 50, 75, 100]
      .map((val) => {
        const y = yOf(val).toFixed(1);
        return `<line x1="${padL}" y1="${y}" x2="${W - padR}" y2="${y}" stroke="var(--cizgi)" stroke-width="1"/><text x="${padL - 8}" y="${Number(y) + 4}" text-anchor="end" fill="var(--metin-soluk)" font-size="11.5">${escHtml(yuzdeBicimle(val / 100))}</text>`;
      })
      .join("");

    const saatIdx =
      pts.length === 48 && !zamanlar
        ? [0, 8, 16, 24, 32, 40, 47]
        : pts.length <= 1
        ? [0]
        : Array.from(new Set([0, Math.floor((pts.length - 1) / 3), Math.floor(((pts.length - 1) * 2) / 3), pts.length - 1]));

    const dikeyler = saatIdx
      .map((idx) => {
        const x = xOf(idx).toFixed(1);
        let et;
        if (zamanlar && zamanlar[idx]) {
          et = idx === pts.length - 1 ? t("genel.simdi") : saatBicimle(zamanlar[idx]);
        } else {
          const saat = (22 + Math.floor(idx / 2)) % 24;
          et = idx === pts.length - 1 ? t("genel.simdi") : `${String(saat).padStart(2, "0")}:00`;
        }
        return `<text x="${x}" y="${H - 8}" text-anchor="middle" fill="var(--metin-soluk)" font-size="11.5">${escHtml(et)}</text>`;
      })
      .join("");

    let cizgiPath = "";
    let alanPath = "";
    let sonKopma = 0;
    const sifirlar = [];
    for (let i = 0; i < pts.length; i++) {
      const x = xOf(i).toFixed(1);
      const y = yOf(pts[i]).toFixed(1);
      if (i === 0) {
        cizgiPath = `M ${x} ${y}`;
        alanPath = `M ${x} ${yOf(0).toFixed(1)} L ${x} ${y}`;
      } else if (pts[i - 1] - pts[i] >= 30) {
        alanPath += ` L ${xOf(i - 1).toFixed(1)} ${yOf(0).toFixed(1)} Z M ${x} ${yOf(0).toFixed(1)} L ${x} ${y}`;
        cizgiPath += ` M ${x} ${y}`;
        sifirlar.push(i);
        sonKopma = i;
      } else {
        cizgiPath += ` L ${x} ${y}`;
        alanPath += ` L ${x} ${y}`;
      }
      if (i === pts.length - 1 && i >= sonKopma) alanPath += ` L ${x} ${yOf(0).toFixed(1)} Z`;
    }

    const esikY = yOf(dinamikEsik).toFixed(1);
    const tepe = Math.max(...pts);
    const ort = Math.round(pts.reduce((a, b) => a + b, 0) / pts.length);
    const son = pts[pts.length - 1];

    const noktalarSvg = pts
      .map((v, idx) => {
        const saat = zamanlar && zamanlar[idx] ? saatBicimle(zamanlar[idx]) : `${String((22 + Math.floor(idx / 2)) % 24).padStart(2, "0")}:${idx % 2 === 0 ? "00" : "30"}`;
        const bilgi = fmt("gecmis.noktaBilgi", { zaman: saat, kullanilan: yuzdeBicimle(v / 100) });
        return `<circle cx="${xOf(idx).toFixed(1)}" cy="${yOf(v).toFixed(1)}" r="4.5" fill="transparent" data-ipucu="${escHtml(bilgi)}" tabindex="0" role="img" aria-label="${escHtml(bilgi)}"/>`;
      })
      .join("");

    let izdusumSvg = "";
    if (izdusum) {
      const x0 = xOf(pts.length - 1);
      const x1 = W - 16;
      const y0 = yOf(son);
      const xDol = x0 + (x1 - x0) * izdusum.dolmaOrani;
      const renkIz = izdusum.dolarMi ? "var(--durum-kacin)" : renk;
      izdusumSvg = `
        <line x1="${x0.toFixed(1)}" y1="${padT}" x2="${x0.toFixed(1)}" y2="${padT + gH}" stroke="var(--cizgi-guclu)" stroke-width="1"/>
        <path d="M ${x0.toFixed(1)} ${y0.toFixed(1)} L ${xDol.toFixed(1)} ${yOf(izdusum.hedef).toFixed(1)}${izdusum.dolarMi ? ` L ${x1.toFixed(1)} ${yOf(100).toFixed(1)}` : ""}" fill="none" stroke="${renkIz}" stroke-width="2" stroke-dasharray="5 4" stroke-linecap="round"/>
        <circle cx="${xDol.toFixed(1)}" cy="${yOf(izdusum.hedef).toFixed(1)}" r="3.5" fill="${renkIz}" data-ipucu="${escHtml(izdusum.metin)}" tabindex="0" role="img" aria-label="${escHtml(izdusum.metin)}"/>
        <text x="${x1.toFixed(1)}" y="${H - 8}" text-anchor="end" fill="var(--metin-soluk)" font-size="11.5">${escHtml(saatBicimle(izdusum.sifirlama))}</text>`;
    }

    ustBilgiEl.innerHTML = `
      <span><strong>${escHtml(seri.etiket)}</strong></span>
      <span class="rakam" style="color:var(--metin-soluk);">${escHtml(fmt("gecmis.esikCizgisi", { esik: yuzdeBicimle(dinamikEsik / 100) }))}</span>
      ${izdusum ? `<span class="rakam gecmis-izdusum${izdusum.dolarMi ? " dolar" : ""}">${escHtml(izdusum.metin)}</span>` : ""}
    `;

    svgKapsayiciEl.innerHTML = `
      <svg class="gecmis-svg" viewBox="0 0 ${W} ${H}" role="img" aria-label="${escHtml(seri.etiket)}">
        ${yataylar}
        ${dikeyler}
        <line x1="${padL}" y1="${esikY}" x2="${W - padR}" y2="${esikY}" stroke="var(--durum-kacin)" stroke-width="1.2" stroke-dasharray="5 4"/>
        <path d="${alanPath}" fill="${renk}" fill-opacity="0.14"/>
        <path d="${cizgiPath}" fill="none" stroke="${renk}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>
        ${sifirlar.map((idx) => `<line x1="${xOf(idx).toFixed(1)}" y1="${padT}" x2="${xOf(idx).toFixed(1)}" y2="${padT + gH}" stroke="var(--cizgi-guclu)" stroke-dasharray="3 3"/>`).join("")}
        ${izdusumSvg}
        <circle cx="${xOf(pts.length - 1).toFixed(1)}" cy="${yOf(son).toFixed(1)}" r="4" fill="${renk}"/>
        ${noktalarSvg}
      </svg>
    `;

    altMetrikEl.innerHTML = `
      <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.tepe"))}:</span> <strong class="rakam">${escHtml(yuzdeBicimle(tepe / 100))}</strong></div>
      <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.ortalama"))}:</span> <strong class="rakam">${escHtml(yuzdeBicimle(ort / 100))}</strong></div>
      <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.sifirlamaSayisi"))}:</span> <strong class="rakam">${sayiBicimle(sifirlar.length)}</strong></div>
      <div><span style="color:var(--metin-soluk);">${escHtml(t("gecmis.sonDeger"))}:</span> <strong class="rakam">${escHtml(yuzdeBicimle(son / 100))}</strong></div>
    `;
  }

  // --------------------------------------------------------------------------
  // 10. Bağla Sekmesi (Canlı & ?demo=1 Önizleme)
  // --------------------------------------------------------------------------
  // OpenCode / DeepSeek Harness: kotası olmayan yerel araçlar. Varsayılan kapalı; yalnız buradan
  // "Bağla" ile açılır. Kurulu değilse Bağla düğmesi kapalıdır ve ana ekranda görünmez.
  function yerelSayacKarti(platform) {
    const h = durum.anlik.havuzlar?.[platform];
    const etkin = Boolean(platform === "opencode" ? durum.ayarlar.opencodeEtkin : durum.ayarlar.dshEtkin);
    const tar = durum.baglaDurum.tarama;
    const kurulu = canliMi() ? Boolean(tar && (platform === "opencode" ? tar.opencode_var : tar.dsh_var)) : platform === "opencode";
    const okunuyor = h?.durum === "ok" && h?.ek?.kullanim;
    const rozet = etkin && okunuyor
      ? { sinif: "rahat", metin: t("bagla.yerelBagli") }
      : kurulu
      ? { sinif: "dikkat", metin: t("bagla.yerelVar") }
      : { sinif: "bilinmiyor", metin: t("bagla.yerelYok") };
    const cal = durum.baglaDurum.testCalisiyor || {};
    const sayac = okunuyor ? sayacMetinleri(h.ek.kullanim) : null;
    const eylemHtml = `
      <div class="bagla-eylem-satiri">
        ${
          etkin
            ? `<button type="button" class="ikincil-btn" data-bagla-eylem="${platform}-kes">${escHtml(t("bagla.yerelKes"))}</button>
               <button type="button" class="ikincil-btn" data-bagla-eylem="${platform}-test" ${cal[platform] ? "disabled" : ""}>${escHtml(cal[platform] ? t("bagla.testCalisiyor") : t("bagla.testEt"))}</button>`
            : `<button type="button" class="birincil-btn" data-bagla-eylem="${platform}-bagla" ${kurulu ? "" : "disabled"}>${escHtml(t("bagla.yerelBagla"))}</button>`
        }
      </div>
      ${sayac ? `<div class="test-sonuc">${escHtml(`${sayac.s24} · ${sayac.toplam}`)}</div>` : ""}
      ${canliMi() ? canliSonucHtml(platform) : ""}
    `;
    return {
      platform,
      ad: t(`bagla.platformAdlari.${platform}`),
      rozet,
      tespit: t("bagla.yerelNot"),
      eylemHtml,
      yerelSayac: true,
      kaynak: {
        tur: t("kaynakTur.yerel_gunluk"),
        sonOkuma: h?.kaynak ? saatBicimle(h.kaynak.okuma) : "—",
        sonrakiOkuma: etkin ? fmt("bagla.dakikaKisa", { n: sayiBicimle(5) }) : "—",
        sonHata: h?.hata ? hataTuruBicimle(h.hata.tur) : t("bagla.hataYok"),
        guven: t("guvenEtiketi.yerel_kayit"),
      },
    };
  }

  function baglaSekmesiCiz() {
    baslikBilgiYaz("bagla-baslik", "bagla-aciklama", t("bagla.baslik"), t("bagla.aciklama"));
    const taraBtn = document.getElementById("btn-bagla-tara");
    if (taraBtn) {
      taraBtn.textContent = durum.baglaDurum.taraniyor ? t("bagla.taraniyor") : t("bagla.tara");
      taraBtn.disabled = durum.baglaDurum.taraniyor;
    }
    const sonTarEl = document.getElementById("bagla-son-tarama");
    if (sonTarEl) {
      sonTarEl.textContent = durum.baglaDurum.sonTarama
        ? fmt("bagla.sonTaramaEtiket", { zaman: durum.baglaDurum.sonTarama })
        : t("bagla.taranmadi");
    }

    const cl = durum.anlik.havuzlar?.claude_max;
    const cx = durum.anlik.havuzlar?.codex_plus;
    const ag = durum.anlik.havuzlar?.agy_gemini;
    const ds = durum.anlik.havuzlar?.deepseek;
    const agyIzin = durum.ayarlar.agyOkuma;
    const cxEtkin = durum.ayarlar.codexEtkin !== false;
    const dsEtkin = Boolean(durum.ayarlar.deepseekEtkin);

    const demoKartlar = [
      {
        platform: "claude",
        ad: t("bagla.platformAdlari.claude"),
        rozet: cl?.durum === "ok" ? { sinif: "rahat", metin: fmt("bagla.kopruBagli", { yas: kaynakYasMetni(cl.kaynak) }) } : { sinif: "bayat", metin: t("bagla.kopruYok") },
        tespit: t("bagla.statuslineNot"),
        eylemHtml: `
          <div class="kod-satir">
            <code>claude plugin update kota-bandi@cetvel</code>
            <button type="button" class="ikincil-btn" data-bagla-eylem="claude-kopyala">${escHtml(t("bagla.kopyala"))}</button>
          </div>
          <div class="bagla-eylem-satiri">
            <button type="button" class="ikincil-btn" data-bagla-eylem="claude-onizle">${escHtml(t("bagla.onizle"))}</button>
          </div>
          ${
            durum.baglaDurum.claudeFarkAcik
              ? `<div class="onizleme-fark">+ "statusLine": { "type": "command", "command": "cetvel kopru statusline" }</div>
                 <div class="bagla-eylem-satiri"><button type="button" class="birincil-btn" data-bagla-eylem="claude-onayla">${escHtml(t("bagla.onaylaYaz"))}</button></div>`
              : ""
          }
          ${durum.baglaDurum.claudeOnaylandi ? `<div class="test-sonuc">${escHtml(t("genel.ayarlarYedeklendi"))}</div>` : ""}
        `,
        kaynak: {
          tur: cl?.kaynak ? sozlukEslesmeBul("kaynakTur", cl.kaynak.tur) || anahtarAdiOkunurYap(cl.kaynak.tur) : t("kaynakTur.mod_koprusu"),
          sonOkuma: cl?.kaynak ? saatBicimle(cl.kaynak.okuma) : "—",
          sonrakiOkuma: t("bagla.olayGudumlu"),
          sonHata: cl?.hata ? hataTuruBicimle(cl.hata.tur) : t("bagla.hataYok"),
          guven: cl?.kaynak ? sozlukEslesmeBul("guvenEtiketi", cl.kaynak.guven) || anahtarAdiOkunurYap(cl.kaynak.guven) : t("guvenEtiketi.resmi_arayuz"),
        },
      },
      {
        platform: "codex",
        ad: t("bagla.platformAdlari.codex"),
        rozet: !cxEtkin ? { sinif: "bilinmiyor", metin: t("bagla.devreDisiEtiket") } : { sinif: "rahat", metin: t("bagla.codexBulundu") },
        tespit: `${t("kaynakTur.codex_app_server")} · auth.json`,
        eylemHtml: `
          <div class="bagla-eylem-satiri"><button type="button" class="ikincil-btn" data-bagla-eylem="codex-test" ${cxEtkin ? "" : "disabled"}>${escHtml(t("bagla.testEt"))}</button></div>
          ${durum.baglaDurum.codexSonuc ? `<div class="test-sonuc">${escHtml(durum.baglaDurum.codexSonuc)}</div>` : ""}
        `,
        kaynak: {
          tur: t("kaynakTur.codex_app_server"),
          sonOkuma: cx?.kaynak ? saatBicimle(cx.kaynak.okuma) : "—",
          sonrakiOkuma: cxEtkin ? fmt("bagla.dakikaCift", { etkin: sayiBicimle(durum.ayarlar.codexEtkinDk), bosta: sayiBicimle(durum.ayarlar.codexBostaDk) }) : t("baski.kapali"),
          sonHata: cx?.hata ? hataTuruBicimle(cx.hata.tur) : t("bagla.hataYok"),
          guven: t("guvenEtiketi.resmi_arayuz"),
        },
      },
      {
        platform: "agy",
        ad: t("bagla.platformAdlari.agy"),
        rozet: agyIzin === "kapali" ? { sinif: "dikkat", metin: t("bagla.agySecenekler.kapali") } : { sinif: "rahat", metin: t(`bagla.agySecenekler.${agyIzin}`) },
        tespit: t("kaynakTur.agy_usage"),
        eylemHtml: `
          <div class="risk-kutusu">${escHtml(t("bagla.agyRisk"))}</div>
          <div class="bagla-eylem-satiri">
            <div class="ayar-secim-grup" role="group" aria-label="${escHtml(t("ayarlar.agyOkumaEtiket"))}">
              ${["kapali", "istek_uzerine", "otomatik"]
                .map((m) => `<button type="button" class="mini-btn${agyIzin === m ? " aktif" : ""}" data-agy-izin="${m}">${escHtml(t(`bagla.agySecenekler.${m}`))}</button>`)
                .join("")}
            </div>
            <button type="button" class="ikincil-btn" data-bagla-eylem="agy-test" ${agyIzin === "kapali" ? "disabled" : ""}>${escHtml(t("bagla.testEt"))}</button>
          </div>
          ${durum.baglaDurum.agySonuc ? `<div class="test-sonuc">${escHtml(durum.baglaDurum.agySonuc)}</div>` : ""}
        `,
        kaynak: {
          tur: t("kaynakTur.agy_usage"),
          sonOkuma: ag?.kaynak ? saatBicimle(ag.kaynak.okuma) : "—",
          sonrakiOkuma: agyIzin === "kapali" ? t("bagla.agySecenekler.kapali") : t(`bagla.agySecenekler.${agyIzin}`),
          sonHata: durum.anlik.havuzlar?.agy_argon?.hata?.tur ? hataTuruBicimle(durum.anlik.havuzlar.agy_argon.hata.tur) : t("bagla.hataYok"),
          guven: t("guvenEtiketi.tahmini"),
        },
      },
      {
        platform: "deepseek",
        ad: t("bagla.platformAdlari.deepseek"),
        rozet: !dsEtkin
          ? { sinif: "bilinmiyor", metin: t("bagla.devreDisiEtiket") }
          : durum.baglaDurum.deepseekAnahtarVar
          ? { sinif: "rahat", metin: t("bagla.anahtarVar") }
          : { sinif: "bilinmiyor", metin: t("bagla.anahtarGerek") },
        tespit: t("kaynakTur.bakiye_api"),
        eylemHtml: `
          <div class="bagla-eylem-satiri">
            <input type="password" class="maskeli-girdi" id="girdi-deepseek-anahtar" placeholder="${escHtml(t("bagla.anahtarYerTutucu"))}" autocomplete="off" maxlength="512">
            <button type="button" class="ikincil-btn" data-bagla-eylem="deepseek-kaydet">${escHtml(t("bagla.kaydet"))}</button>
            <button type="button" class="ikincil-btn" data-bagla-eylem="deepseek-test" ${!dsEtkin || !durum.baglaDurum.deepseekAnahtarVar ? "disabled" : ""}>${escHtml(t("bagla.testEt"))}</button>
          </div>
          ${durum.baglaDurum.deepseekSonuc ? `<div class="test-sonuc">${escHtml(durum.baglaDurum.deepseekSonuc)}</div>` : ""}
        `,
        kaynak: {
          tur: t("kaynakTur.bakiye_api"),
          sonOkuma: ds?.kaynak ? saatBicimle(ds.kaynak.okuma) : "—",
          sonrakiOkuma: dsEtkin && durum.baglaDurum.deepseekAnahtarVar ? fmt("bagla.dakikaKisa", { n: sayiBicimle(15) }) : t("baski.kapali"),
          sonHata: ds?.hata ? hataTuruBicimle(ds.hata.tur) : t("bagla.hataYok"),
          guven: t("guvenEtiketi.resmi_api"),
        },
      },
      yerelSayacKarti("opencode"),
      yerelSayacKarti("dsh"),
    ];

    const kartlar = canliMi() ? demoKartlar.map(canliKartUyarla) : demoKartlar;
    const kartHtml = kartlar
      .map((k) => {
        const renk = PLATFORM_RENK[k.platform] || "var(--cizgi-guclu)";
        return `
          <article class="bagla-kart" style="--platform-renk:${renk};">
            <div class="bagla-kart-ust">
              <div class="satir-kimlik">
                <span class="monogram">${svgMonogram(k.platform)}</span>
                <strong>${escHtml(k.ad)}</strong>
              </div>
              <span class="durum-rozet ${escHtml(k.rozet.sinif)}">${svgDurumSimge(k.rozet.sinif)} ${escHtml(k.rozet.metin)}</span>
            </div>
            <div class="satir-neden">${escHtml(k.tespit)}</div>
            ${k.eylemHtml}
          </article>
        `;
      })
      .join("");

    const tabloHtml = `
      <article class="bagla-kart">
        <h3>${escHtml(t("bagla.veriKaynagiBaslik"))}</h3>
        <div class="veri-kaynagi-tablo-sarici">
          <table class="veri-kaynagi-tablo">
            <thead>
              <tr>
                <th>${escHtml(t("bagla.sutunPlatform"))}</th>
                <th>${escHtml(t("bagla.sutunKaynak"))}</th>
                <th>${escHtml(t("bagla.sutunSonOkuma"))}</th>
                <th>${escHtml(t("bagla.sutunSonraki"))}</th>
                <th>${escHtml(t("bagla.sutunSonHata"))}</th>
                <th>${escHtml(t("bagla.sutunGuven"))}</th>
              </tr>
            </thead>
            <tbody>
              ${kartlar
                .map(
                  (k) => `
                  <tr>
                    <td data-etiket="${escHtml(t("bagla.sutunPlatform"))}"><strong>${escHtml(k.ad)}</strong></td>
                    <td data-etiket="${escHtml(t("bagla.sutunKaynak"))}">${escHtml(k.kaynak.tur)}</td>
                    <td data-etiket="${escHtml(t("bagla.sutunSonOkuma"))}" class="rakam">${escHtml(k.kaynak.sonOkuma)}</td>
                    <td data-etiket="${escHtml(t("bagla.sutunSonraki"))}" class="rakam">${escHtml(k.kaynak.sonrakiOkuma)}</td>
                    <td data-etiket="${escHtml(t("bagla.sutunSonHata"))}">${escHtml(k.kaynak.sonHata)}</td>
                    <td data-etiket="${escHtml(t("bagla.sutunGuven"))}">${escHtml(k.kaynak.guven)}</td>
                  </tr>
                `
                )
                .join("")}
            </tbody>
          </table>
        </div>
      </article>
    `;

    const listeEl = document.getElementById("bagla-liste");
    if (listeEl) {
      const bOdak = odakKimligiAl(listeEl);
      listeEl.innerHTML = kartHtml + tabloHtml;
      odakKimligiYukle(listeEl, bOdak);
    }
  }

  // --------------------------------------------------------------------------
  // 11. Ayarlar Sekmesi (6 Bölüm)
  // --------------------------------------------------------------------------
  function saatSecenekleri(secili) {
    return Array.from({ length: 24 }, (_, h) => {
      const etiket = `${String(h).padStart(2, "0")}:00`;
      return `<option value="${h}"${Number(secili) === h ? " selected" : ""}>${etiket}</option>`;
    }).join("");
  }

  function svgAyarBolumIkon(bolum) {
    if (bolum === "genel") {
      return '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="2.3" stroke="currentColor" stroke-width="1.4"/><path d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M12.4 3.6l-1.1 1.1M4.7 11.3l-1.1 1.1" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>';
    }
    if (bolum === "platformlar") {
      return '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="2.2" y="2.2" width="4.8" height="4.8" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="9" y="2.2" width="4.8" height="4.8" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="2.2" y="9" width="4.8" height="4.8" rx="1.2" stroke="currentColor" stroke-width="1.4"/><rect x="9" y="9" width="4.8" height="4.8" rx="1.2" stroke="currentColor" stroke-width="1.4"/></svg>';
    }
    if (bolum === "bildirimler") {
      return '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M12 6.4a4 4 0 1 0-8 0c0 3.4-1.4 4.5-1.4 4.5h10.8S12 9.8 12 6.4z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M6.6 13.2a1.6 1.6 0 0 0 2.8 0" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>';
    }
    if (bolum === "teknik") {
      return '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M5.5 4.2L2.2 8l3.3 3.8M10.5 4.2L13.8 8l-3.3 3.8M8.8 3l-1.6 10" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>';
    }
    return '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="5.8" stroke="currentColor" stroke-width="1.4"/><path d="M8 7.2v3.8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="4.9" r="0.9" fill="currentColor"/></svg>';
  }

  function ayarAnahtarHtml({ id = "", checked = false, label = "", attr = "", val = "" }) {
    const idNit = id ? ` id="${escHtml(id)}"` : "";
    const attrNit = attr ? ` ${attr}="${escHtml(val)}"` : "";
    const labelNit = label ? ` aria-label="${escHtml(label)}"` : "";
    return `<button type="button" role="switch" class="ayar-anahtar"${idNit}${attrNit}${labelNit} aria-checked="${checked ? "true" : "false"}"><span class="ayar-anahtar-top" aria-hidden="true"></span></button>`;
  }

  function ayarSatirHtml({ id = "", baslik, not = "", denetimHtml }) {
    const etiketEl = id
      ? `<label for="${escHtml(id)}" class="ayar-satir-baslik">${escHtml(baslik)}</label>`
      : `<span class="ayar-satir-baslik">${escHtml(baslik)}</span>`;
    return `
      <div class="ayar-satir">
        <div class="ayar-satir-bilgi">
          ${etiketEl}
          ${not ? `<span class="ayar-satir-not">${escHtml(not)}</span>` : ""}
        </div>
        <div class="ayar-denetim">${denetimHtml}</div>
      </div>
    `;
  }

  function ayarGrupHtml(baslik, satirlarHtml) {
    return `
      <section class="ayar-bolum-grup">
        <div class="ayar-grup-baslik">${escHtml(baslik)}</div>
        <div class="ayar-kart">${satirlarHtml}</div>
      </section>
    `;
  }

  async function lisansBelgesiAc(tur) {
    const basliklar = { mit: t("ayarlar.lisansMitBtn"), apache: t("ayarlar.lisansApacheBtn"), ucuncu_taraf: t("ayarlar.ucuncuTarafBaslik") };
    durum.lisansBelge = { tur, baslik: basliklar[tur] || tur, metin: t("ayarlar.belgeYukleniyor") };
    ayarlarSekmesiCiz();
    let metin;
    try {
      metin = canliMi()
        ? await invoke("lisans_metni_al", { tur })
        : "MIT OR Apache-2.0 — LICENSE-MIT · LICENSE-APACHE · THIRD-PARTY-NOTICES.txt";
    } catch (e) {
      metin = fmt("genel.islemHatasi", { hata: hataMetni(e) });
    }
    if (durum.lisansBelge?.tur === tur) {
      durum.lisansBelge.metin = metin;
      ayarlarSekmesiCiz();
    }
  }

  async function ayarEylemi(eylem) {
    if (!canliMi()) return;
    try {
      if (eylem === "gunluk") {
        await invoke("gunluk_klasorunu_ac");
        bildirimGoster(t("ayarlar.klasorAcildi"));
      } else if (eylem === "disa-aktar") {
        const ad = await invoke("gecmis_disa_aktar");
        bildirimGoster(fmt("ayarlar.disaAktarildi", { ad }));
      }
    } catch (e) {
      bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) }));
    }
  }

  function bilinenBildirimPlatformlari() {
    const sirali = ["claude", "codex", "agy", "deepseek"];
    for (const h of Object.values(durum.anlik.havuzlar || {})) {
      if (h?.platform && !h.ek?.kullanim && !sirali.includes(h.platform)) {
        sirali.push(h.platform);
      }
    }
    return sirali;
  }

  function ayarlarSekmesiCiz() {
    const bolumler = ["genel", "platformlar", "bildirimler", "teknik", "hakkinda"];
    durum.ayarBolum = ayarBolumuNormalizeEt(durum.ayarBolum);

    const yanMenuEl = document.getElementById("ayarlar-bolum-menu");
    if (yanMenuEl) {
      const mOdak = odakKimligiAl(yanMenuEl);
      yanMenuEl.innerHTML = bolumler
        .map((b) => {
          const ad = b === "hakkinda" ? t("ayarlar.hakkindaKisa") : t(`ayarlar.bolumler.${b}`);
          return `
            <button type="button" class="ayar-bolum-btn${durum.ayarBolum === b ? " aktif" : ""}" data-ayar-bolum="${b}" aria-current="${durum.ayarBolum === b ? "page" : "false"}">
              ${svgAyarBolumIkon(b)}
              <span>${escHtml(ad)}</span>
            </button>`;
        })
        .join("");
      odakKimligiYukle(yanMenuEl, mOdak);
    }

    const a = durum.ayarlar;
    const b = durum.ayarBolum;
    let html = "";

    if (b === "genel") {
      html =
        ayarGrupHtml(
          t("ayarlar.grupGorunum"),
          [
            ayarSatirHtml({
              id: "ayar-dil-secim",
              baslik: t("ayarlar.dilEtiket"),
              not: !durum.dilTamMi ? t("ayarlar.dilCeviriNotu") : t("ayarlar.dilAciklama"),
              denetimHtml: `<select id="ayar-dil-secim" class="ayar-select">${DESTEKLENEN_DILLER.map((d) => `<option value="${escHtml(d.kod)}"${durum.dil === d.kod ? " selected" : ""}>${escHtml(d.etiket)} (${escHtml(d.kod)})</option>`).join("")}</select>`,
            }),
            ayarSatirHtml({
              id: "ayar-kip-secim",
              baslik: t("ayarlar.temaKipEtiket"),
              not: t("ayarlar.temaKipAciklama"),
              denetimHtml: `<select id="ayar-kip-secim" class="ayar-select">${["sistem", "acik", "koyu"].map((kip) => `<option value="${kip}"${durum.gorunumKipi === kip ? " selected" : ""}>${escHtml(t(`ayarlar.temaKipleri.${kip}`))}</option>`).join("")}</select>`,
            }),
            ayarSatirHtml({
              id: "ayar-renk-secim",
              baslik: t("ayarlar.renkTemasiEtiket"),
              not: t("ayarlar.renkTemasiAciklama"),
              denetimHtml: `<select id="ayar-renk-secim" class="ayar-select">${RENK_TEMALARI.map((rt) => `<option value="${rt.id}"${durum.renkTemasi === rt.id ? " selected" : ""}>${escHtml(t(`ayarlar.renkTemalari.${rt.id}`))}</option>`).join("")}</select>`,
            }),
            ayarSatirHtml({
              id: "ayar-yon-secim",
              baslik: t("ayarlar.degerYonuEtiket"),
              not: t("ayarlar.degerYonuAciklama"),
              denetimHtml: `<select id="ayar-yon-secim" class="ayar-select">${["kalan", "kullanilan"].map((y) => `<option value="${y}"${durum.degerYonu === y ? " selected" : ""}>${escHtml(t(`gorunum.yonler.${y}`))}</option>`).join("")}</select>`,
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.grupDavranis"),
          [
            ayarSatirHtml({
              id: "ayar-windows-baslat",
              baslik: t("ayarlar.windowsIleBaslat"),
              not: t("ayarlar.windowsIleBaslatNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-windows-baslat",
                checked: Boolean(a.windowsIleBaslat),
                label: t("ayarlar.windowsIleBaslat"),
                attr: "data-ayar-anahtar",
                val: "windowsIleBaslat",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-tepsiye-kucult",
              baslik: t("ayarlar.tepsiyeKucult"),
              not: t("ayarlar.tepsiyeKucultNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-tepsiye-kucult",
                checked: Boolean(a.tepsiyeKucult),
                label: t("ayarlar.tepsiyeKucult"),
                attr: "data-ayar-anahtar",
                val: "tepsiyeKucult",
              }),
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.seritBolum"),
              not: t("ayarlar.seritAciklama"),
              denetimHtml: `<div class="ayar-secim-grup"><button type="button" class="ikincil-btn" data-ayar-serit="ac">${escHtml(t("ayarlar.seritAcBtn"))}</button><button type="button" class="ikincil-btn" data-ayar-serit="gizle">${escHtml(t("ayarlar.seritGizleBtn"))}</button></div>`,
            }),
            ayarSatirHtml({
              id: "ayar-serit-ustte",
              baslik: t("ayarlar.seritUstte"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-serit-ustte",
                checked: Boolean(a.seritUstte),
                label: t("ayarlar.seritUstte"),
                attr: "data-ayar-anahtar",
                val: "seritUstte",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-serit-gecirgen",
              baslik: t("ayarlar.seritGecirgen"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-serit-gecirgen",
                checked: Boolean(durum.tiklamaGecirgen),
                label: t("ayarlar.seritGecirgen"),
                attr: "data-ayar-anahtar",
                val: "seritGecirgen",
              }),
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.grupSifirla"),
          ayarSatirHtml({
            baslik: t("ayarlar.varsayilanaDonBtn"),
            not: t("ayarlar.varsayilanaDonNot"),
            denetimHtml: durum.ayarSifirlaOnay
              ? `<div class="ayar-secim-grup"><button type="button" class="tehlike-btn" data-ayar-sifirla="onayla">${escHtml(t("ayarlar.varsayilanaDonOnay"))}</button><button type="button" class="ikincil-btn" data-ayar-sifirla="iptal">${escHtml(t("ayarlar.vazgecBtn"))}</button></div>`
              : `<button type="button" class="ikincil-btn" data-ayar-sifirla="iste">${escHtml(t("ayarlar.varsayilanaDonBtn"))}</button>`,
          })
        );
    } else if (b === "platformlar") {
      html =
        ayarGrupHtml(
          t("ayarlar.grupPlatformAnahtarlari"),
          [
            ayarSatirHtml({
              id: "ayar-codex-etkin",
              baslik: t("ayarlar.codexEtkin"),
              not: t("ayarlar.codexEtkinNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-codex-etkin",
                checked: a.codexEtkin !== false,
                label: t("ayarlar.codexEtkin"),
                attr: "data-ayar-anahtar",
                val: "codexEtkin",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-agy-secim",
              baslik: t("ayarlar.agyOkumaEtiket"),
              not: t("ayarlar.agyOkumaNot"),
              denetimHtml: `<select id="ayar-agy-secim" class="ayar-select">${["kapali", "istek_uzerine", "otomatik"].map((m) => `<option value="${m}"${a.agyOkuma === m ? " selected" : ""}>${escHtml(t(`bagla.agySecenekler.${m}`))}</option>`).join("")}</select>`,
            }),
            ayarSatirHtml({
              id: "ayar-deepseek-etkin",
              baslik: t("ayarlar.deepseekEtkin"),
              not: t("ayarlar.deepseekEtkinNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-deepseek-etkin",
                checked: Boolean(a.deepseekEtkin),
                label: t("ayarlar.deepseekEtkin"),
                attr: "data-ayar-anahtar",
                val: "deepseekEtkin",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-opencode-etkin",
              baslik: t("ayarlar.opencodeEtkin"),
              not: t("ayarlar.opencodeEtkinNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-opencode-etkin",
                checked: Boolean(a.opencodeEtkin),
                label: t("ayarlar.opencodeEtkin"),
                attr: "data-ayar-anahtar",
                val: "opencodeEtkin",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-dsh-etkin",
              baslik: t("ayarlar.dshEtkin"),
              not: t("ayarlar.dshEtkinNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-dsh-etkin",
                checked: Boolean(a.dshEtkin),
                label: t("ayarlar.dshEtkin"),
                attr: "data-ayar-anahtar",
                val: "dshEtkin",
              }),
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.baglantilarBaslik"),
          ayarSatirHtml({
            baslik: t("sekmeler.bagla"),
            not: t("ayarlar.baglantilarAciklama"),
            denetimHtml: `<button type="button" class="birincil-btn" data-ayar-git-sekme="bagla">${escHtml(t("ayarlar.baglaSekmesineGit"))}</button>`,
          })
        );
    } else if (b === "bildirimler") {
      const sessizSet = new Set(a.sessizPlatformlar || []);
      const pltSatirlari = bilinenBildirimPlatformlari()
        .map((p) => {
          const genel = p === "claude" ? a.claude5sEsik : a.digerEsik;
          const ozelDeger = typeof a.platformEsikleri?.[p] === "number" ? a.platformEsikleri[p] : "";
          const sessiz = sessizSet.has(p);
          return ayarSatirHtml({
            baslik: platformAdi(p),
            not: fmt("ayarlar.platformEsikNot", { genel: sayiBicimle(genel) }),
            denetimHtml: `
              <div class="plt-bildirim-kontrol">
                <label class="esik-sarici">
                  <input type="number" min="1" max="99" class="esik-girdi" data-platform-esik="${escHtml(p)}" value="${escHtml(ozelDeger)}" placeholder="${escHtml(genel)}" aria-label="${escHtml(`${platformAdi(p)} %`)}">
                  <span>%</span>
                </label>
                <span class="sessiz-secim-etiket">
                  <span>${escHtml(t("ayarlar.platformSessizeAl"))}</span>
                  ${ayarAnahtarHtml({
                    checked: sessiz,
                    label: `${platformAdi(p)}: ${t("ayarlar.platformSessizeAl")}`,
                    attr: "data-platform-sessiz",
                    val: p,
                  })}
                </span>
              </div>
            `,
          });
        })
        .join("");

      html =
        ayarGrupHtml(
          t("ayarlar.grupBildirimGenel"),
          [
            ayarSatirHtml({
              id: "ayar-b-etkin",
              baslik: t("ayarlar.bildirimEtkin"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-b-etkin",
                checked: Boolean(a.bildirimEtkin),
                label: t("ayarlar.bildirimEtkin"),
                attr: "data-ayar-anahtar",
                val: "bildirimEtkin",
              }),
            }),
            ayarSatirHtml({
              id: "ayar-b-sessiz",
              baslik: t("ayarlar.sessizSaatler"),
              not: t("ayarlar.sessizSaatNot"),
              denetimHtml: ayarAnahtarHtml({
                id: "ayar-b-sessiz",
                checked: Boolean(a.sessizSaatler),
                label: t("ayarlar.sessizSaatler"),
                attr: "data-ayar-anahtar",
                val: "sessizSaatler",
              }),
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.ozelSessizSaatler"),
              not: t("ayarlar.ozelSessizSaatIpucu"),
              denetimHtml: `
                <div class="saat-aralik">
                  <label class="saat-secim"><span>${escHtml(t("ayarlar.sessizBaslangic"))}</span>
                    <select id="ayar-sessiz-bas" class="ayar-select">${saatSecenekleri(a.sessizBaslangic ?? 0)}</select></label>
                  <label class="saat-secim"><span>${escHtml(t("ayarlar.sessizBitis"))}</span>
                    <select id="ayar-sessiz-bit" class="ayar-select">${saatSecenekleri(a.sessizBitis ?? 8)}</select></label>
                </div>
              `,
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.grupBildirimEsikleri"),
          [
            ayarSatirHtml({
              id: "ayar-c5s",
              baslik: t("ayarlar.claude5sEsik"),
              denetimHtml: `<span class="esik-sarici"><input type="number" min="1" max="99" class="esik-girdi" id="ayar-c5s" value="${a.claude5sEsik}"><span>%</span></span>`,
            }),
            ayarSatirHtml({
              id: "ayar-chf",
              baslik: t("ayarlar.claudeHaftaEsik"),
              denetimHtml: `<span class="esik-sarici"><input type="number" min="1" max="99" class="esik-girdi" id="ayar-chf" value="${a.claudeHaftaEsik}"><span>%</span></span>`,
            }),
            ayarSatirHtml({
              id: "ayar-cx",
              baslik: t("ayarlar.digerEsik"),
              denetimHtml: `<span class="esik-sarici"><input type="number" min="1" max="99" class="esik-girdi" id="ayar-cx" value="${a.digerEsik}"><span>%</span></span>`,
            }),
          ].join("")
        ) +
        ayarGrupHtml(t("ayarlar.grupPlatformBildirimleri"), pltSatirlari) +
        ayarGrupHtml(
          t("ayarlar.grupBildirimTurleri"),
          [
            ayarSatirHtml({
              id: "ayar-b-esik",
              baslik: t("ayarlar.esikBildirimi"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-b-esik", checked: Boolean(a.esikBildirimi), label: t("ayarlar.esikBildirimi"), attr: "data-ayar-anahtar", val: "esikBildirimi" }),
            }),
            ayarSatirHtml({
              id: "ayar-b-tahmin",
              baslik: t("ayarlar.tahminBildirimi"),
              not: t("ayarlar.tahminBildirimiNot"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-b-tahmin", checked: Boolean(a.tahminBildirimi), label: t("ayarlar.tahminBildirimi"), attr: "data-ayar-anahtar", val: "tahminBildirimi" }),
            }),
            ayarSatirHtml({
              id: "ayar-b-kredi",
              baslik: t("ayarlar.krediBildirimi"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-b-kredi", checked: Boolean(a.krediBildirimi), label: t("ayarlar.krediBildirimi"), attr: "data-ayar-anahtar", val: "krediBildirimi" }),
            }),
            ayarSatirHtml({
              id: "ayar-b-bayat",
              baslik: t("ayarlar.bayatBildirimi"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-b-bayat", checked: Boolean(a.bayatBildirimi), label: t("ayarlar.bayatBildirimi"), attr: "data-ayar-anahtar", val: "bayatBildirimi" }),
            }),
            ayarSatirHtml({
              id: "ayar-b-sifir",
              baslik: t("ayarlar.sifirlandiBildirimi"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-b-sifir", checked: Boolean(a.sifirlandiBildirimi), label: t("ayarlar.sifirlandiBildirimi"), attr: "data-ayar-anahtar", val: "sifirlandiBildirimi" }),
            }),
          ].join("")
        );
    } else if (b === "teknik") {
      const sd = durum.servisDurumu;
      const durumMetin = durum.demoMu
        ? t("ayarlar.servisDemoEtiket")
        : sd.calisiyor
        ? t("ayarlar.servisCalisiyor")
        : t("ayarlar.servisKapaliEtiket");
      const durumSinif = durum.demoMu ? "dikkat" : sd.calisiyor ? "rahat" : "kacin";
      const okunanPort = sd.port ? String(sd.port) : a.servisPort > 0 ? `${a.servisPort} (${t("genel.bilinmiyor")})` : t("genel.bilinmiyor");
      const okunanPipe = sd.pipe || t("genel.bilinmiyor");
      const okunanSira = typeof sd.sonSira === "number" ? sayiBicimle(sd.sonSira) : t("genel.bilinmiyor");

      html =
        ayarGrupHtml(
          t("ayarlar.grupTeknikServis"),
          [
            ayarSatirHtml({
              baslik: t("ayarlar.servisDurumEtiket"),
              denetimHtml: `<div class="ayar-secim-grup"><span class="durum-rozet ${durumSinif}">${svgDurumSimge(durumSinif)} ${escHtml(durumMetin)}</span>${canliMi() && !sd.calisiyor ? `<button type="button" class="birincil-btn" data-servis-baslat="1">${escHtml(t("genel.servisBaslatBtn"))}</button>` : ""}<button type="button" class="ikincil-btn" data-ayar-servis-kontrol="1">${escHtml(t("ayarlar.servisYenidenKontrol"))}</button></div>`,
            }),
            ayarSatirHtml({
              id: "ayar-port",
              baslik: t("ayarlar.servisPort"),
              not: `${t("ayarlar.portNot")} · ${t("ayarlar.saltOkumaEtiket")}: ${okunanPort}`,
              denetimHtml: `<input type="number" min="0" max="65535" class="esik-girdi" id="ayar-port" value="${a.servisPort}">`,
            }),
            ayarSatirHtml({
              id: "ayar-port-oto",
              baslik: t("ayarlar.otomatikPort"),
              denetimHtml: ayarAnahtarHtml({ id: "ayar-port-oto", checked: Boolean(a.otomatikPort), label: t("ayarlar.otomatikPort"), attr: "data-ayar-anahtar", val: "otomatikPort" }),
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.pipeEtiket"),
              denetimHtml: `<code class="salt-okuma-kutu">${escHtml(okunanPipe)}</code>`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.sonSiraEtiket"),
              denetimHtml: `<code class="salt-okuma-kutu rakam">${escHtml(okunanSira)}</code>`,
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.grupTeknikAralik"),
          [
            ayarSatirHtml({
              id: "ayar-cx-etkin",
              baslik: t("ayarlar.codexEtkinDk"),
              not: t("ayarlar.codexEtkinDkNot"),
              denetimHtml: `<input type="number" min="1" max="1440" class="esik-girdi" id="ayar-cx-etkin" value="${a.codexEtkinDk}">`,
            }),
            ayarSatirHtml({
              id: "ayar-cx-bosta",
              baslik: t("ayarlar.codexBostaDk"),
              not: t("ayarlar.codexBostaDkNot"),
              denetimHtml: `<input type="number" min="1" max="1440" class="esik-girdi" id="ayar-cx-bosta" value="${a.codexBostaDk}">`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.gunlukKlasoruBtn"),
              not: t("ayarlar.gunlukKlasoruIpucu"),
              denetimHtml: `<button type="button" class="ikincil-btn" data-ayar-eylem="gunluk" ${canliMi() ? "" : "disabled"}>${escHtml(t("ayarlar.gunlukKlasoruBtn"))}</button>`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.tarihVeriDisaAktar"),
              not: t("ayarlar.tarihVeriIpucu"),
              denetimHtml: `<button type="button" class="ikincil-btn" data-ayar-eylem="disa-aktar" ${canliMi() && sd.calisiyor ? "" : "disabled"}>${escHtml(t("ayarlar.tarihVeriDisaAktar"))}</button>`,
            }),
          ].join("")
        );
    } else {
      const belgeBasliklar = {
        mit: t("ayarlar.lisansMitBtn"),
        apache: t("ayarlar.lisansApacheBtn"),
        ucuncu_taraf: t("ayarlar.ucuncuTarafBaslik"),
      };
      const belgeBaslik = durum.lisansBelge ? belgeBasliklar[durum.lisansBelge.tur] || durum.lisansBelge.baslik : "";
      html =
        ayarGrupHtml(
          t("ayarlar.grupHakkindaBilgi"),
          [
            ayarSatirHtml({
              baslik: t("ayarlar.surumEtiket"),
              not: t("ayarlar.mimariNot"),
              denetimHtml: `<code class="salt-okuma-kutu rakam">v${escHtml(durum.anlik?.surum || "0.1.0")}</code>`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.gizlilikBaslik"),
              not: t("ayarlar.gizlilikNot"),
              denetimHtml: "",
            }),
          ].join("")
        ) +
        ayarGrupHtml(
          t("ayarlar.grupHakkindaLisans"),
          [
            ayarSatirHtml({
              baslik: t("ayarlar.lisansBaslik"),
              not: t("ayarlar.lisansOzeti"),
              denetimHtml: `<div class="ayar-secim-grup"><button type="button" class="ikincil-btn" data-lisans-belge="mit">${escHtml(t("ayarlar.lisansMitBtn"))}</button><button type="button" class="ikincil-btn" data-lisans-belge="apache">${escHtml(t("ayarlar.lisansApacheBtn"))}</button></div>`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.ucuncuTarafBaslik"),
              not: t("ayarlar.ucuncuTarafListe"),
              denetimHtml: `<button type="button" class="ikincil-btn" data-lisans-belge="ucuncu_taraf">${escHtml(t("ayarlar.ucuncuTarafBtn"))}</button>`,
            }),
            ayarSatirHtml({
              baslik: t("ayarlar.markaBaslik"),
              not: t("ayarlar.markaUyarisi"),
              denetimHtml: "",
            }),
          ].join("")
        ) +
        (durum.lisansBelge
          ? `<div class="lisans-belge" role="region" aria-label="${escHtml(belgeBaslik)}">
               <div class="lisans-belge-ust">
                 <strong>${escHtml(belgeBaslik)}</strong>
                 <button type="button" class="ikincil-btn" data-lisans-kapat="1">${escHtml(t("ayarlar.belgeKapat"))}</button>
               </div>
               <pre class="lisans-metin" tabindex="0">${escHtml(durum.lisansBelge.metin)}</pre>
             </div>`
          : "");
    }

    const gridEl = document.getElementById("ayarlar-grid");
    if (gridEl) {
      const gOdak = odakKimligiAl(gridEl);
      gridEl.innerHTML = html;
      odakKimligiYukle(gridEl, gOdak);
    }
  }

  // --------------------------------------------------------------------------
  // 12. Şerit Modu Çizimi (520 x 44 px içine sığacak şekilde tasarlı)
  // --------------------------------------------------------------------------
  function seritCiz() {
    const basEl = document.getElementById("serit-baslik");
    if (basEl) basEl.textContent = t("serit.baslik");
    const acikEl = document.getElementById("serit-aciklama");
    if (acikEl) acikEl.textContent = t("serit.aciklama");
    const lblEl = document.getElementById("lbl-tiklama-gecirgen");
    if (lblEl) lblEl.textContent = t("serit.tiklamaGecirgen");
    const donBtn = document.getElementById("btn-serit-acilir");
    if (donBtn) donBtn.textContent = t("serit.acilirDon");

    const cubuk = document.getElementById("serit-cubuk");
    if (!cubuk) return;
    cubuk.classList.toggle("gecirgen", Boolean(durum.tiklamaGecirgen));
    const tutamak = document.getElementById("serit-tutamak");
    if (tutamak) tutamak.title = t("serit.tutamakIpucu");

    const genel = genelBaskiHesapla();
    const baskiEl = document.getElementById("serit-baski");
    if (baskiEl) {
      baskiEl.textContent = genel.metin;
      baskiEl.title = genel.metin;
    }

    const ogelerEl = document.getElementById("serit-ogeler");
    if (!ogelerEl) return;
    // Şerit platform başına tek özet yüzde gösterir (kotası olmayan sayaçlar hariç).
    const gruplar = platformGruplariOlustur(durum.anlik.havuzlar).filter(
      (g) => (g.repH.durum === "ok" || g.repH.durum === "bayat") && !g.repH.ek?.kullanim
    );
    if (gruplar.length === 0) {
      ogelerEl.innerHTML = `<span class="serit-oge">${escHtml(t("baski.bilinmiyor"))}</span>`;
      return;
    }

    ogelerEl.innerHTML = gruplar
      .slice(0, 4)
      .map((g) => {
        const h = g.repH;
        const { olcum, etkinKalan } = baglayiciBul(h);
        const oran = typeof etkinKalan === "number" ? (durum.degerYonu === "kullanilan" ? 1 - etkinKalan : etkinKalan) : null;
        const yuzdeMetin = oran !== null ? yuzdeBicimle(oran) : "—";
        const sinif = dolguSinifiBul(h, olcum, g.repId);
        const kisaAd = PLATFORM_KISA[g.platform] || g.ad.slice(0, 3).toUpperCase();
        const metin = `${kisaAd} ${yuzdeMetin}`;
        const tamBilgi = `${g.ad}: ${oran !== null ? `${yuzdeMetin} ${durum.degerYonu === "kullanilan" ? t("genel.kullanildi") : t("genel.kalan")}` : t("baski.bilinmiyor")}`;
        return `<span class="serit-oge ${escHtml(sinif)}" title="${escHtml(tamBilgi)}" aria-label="${escHtml(tamBilgi)}">${svgDurumSimge(sinif)}<span class="serit-oge-ad rakam">${escHtml(metin)}</span></span>`;
      })
      .join("");
    seritSigdir(ogelerEl, gruplar.length);
  }

  // Şerit sabit genişlikte: öğeler kırpılmaz; sığmayanlar sondan çıkarılıp "+N" çipine toplanır.
  function seritSigdir(ogelerEl, toplam) {
    if (!ogelerEl.isConnected || ogelerEl.offsetParent === null) return;
    const cubuk = ogelerEl.closest(".serit-cubuk");
    cubuk?.classList.remove("baski-gizli");
    const fazlaCipi = (n) => {
      const cip = document.createElement("span");
      cip.className = "serit-oge serit-fazla rakam";
      cip.textContent = `+${sayiBicimle(n)}`;
      cip.title = fmt("genel.platformSayi", { n: sayiBicimle(n) });
      return cip;
    };
    let gizli = Math.max(0, toplam - ogelerEl.children.length);
    if (gizli > 0) ogelerEl.appendChild(fazlaCipi(gizli));
    while (ogelerEl.scrollWidth > ogelerEl.clientWidth + 1) {
      const fazla = ogelerEl.querySelector(".serit-fazla");
      const ogeler = [...ogelerEl.querySelectorAll(".serit-oge:not(.serit-fazla)")];
      if (ogeler.length <= 1) break;
      ogeler[ogeler.length - 1].remove();
      gizli++;
      if (fazla) fazla.remove();
      ogelerEl.appendChild(fazlaCipi(gizli));
    }
    // Tek öğe bile sığmıyorsa "en dar" özeti gizlenir (aynı bilgi öğe ipucunda ve tepside var).
    if (ogelerEl.scrollWidth > ogelerEl.clientWidth + 1) cubuk?.classList.add("baski-gizli");
  }

  // --------------------------------------------------------------------------
  // 13. Genel Metin Senkronizasyonu ve Yenileme
  // --------------------------------------------------------------------------
  function metinSet(id, metin) {
    const el = document.getElementById(id);
    if (el) el.textContent = metin;
  }

  function metinleriGuncelle() {
    document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
      const k = el.getAttribute("data-i18n-aria");
      if (k) el.setAttribute("aria-label", t(k));
    });
    document.querySelectorAll("[data-i18n-title]").forEach((el) => {
      const k = el.getAttribute("data-i18n-title");
      if (k) el.setAttribute("title", t(k));
    });

    metinSet("proto-baslik", t("proto.baslik"));
    metinSet("btn-yuzey-acilir", t("proto.yuzeyler.acilir"));
    metinSet("btn-yuzey-pencere", t("proto.yuzeyler.pencere"));
    metinSet("btn-yuzey-serit", t("proto.yuzeyler.serit"));

    const kipBtn = document.getElementById("btn-proto-kip");
    if (kipBtn) {
      kipBtn.textContent = t(`ayarlar.temaKipleri.${durum.gorunumKipi}`);
      kipBtn.dataset.ipucu = `${t("ayarlar.temaKipEtiket")}: ${kipBtn.textContent}`;
    }

    const renkBtn = document.getElementById("btn-proto-renk");
    if (renkBtn) {
      renkBtn.textContent = t(`ayarlar.renkTemalari.${durum.renkTemasi}`);
      renkBtn.dataset.ipucu = `${t("ayarlar.renkTemasiEtiket")}: ${renkBtn.textContent}`;
    }

    const dilSel = document.getElementById("sec-proto-dil");
    if (dilSel && dilSel.value !== durum.dil) dilSel.value = durum.dil;

    metinSet("senaryo-baslik", t("proto.senaryoBaslik"));
    metinSet("senaryo-kisayol", t("proto.senaryoKisayol"));
    document.querySelectorAll(".senaryo-btn").forEach((btn) => {
      const k = btn.dataset.senaryo;
      btn.textContent = t(`proto.senaryolar.${k}`);
      btn.classList.toggle("aktif", k === durum.senaryo);
      btn.setAttribute("aria-checked", k === durum.senaryo ? "true" : "false");
    });

    metinSet("acilir-uygulama-ad", t("uygulama.ad"));
    metinSet("genis-uygulama-ad", t("uygulama.ad"));
    metinSet("serit-marka", t("uygulama.ad"));

    metinSet("tab-simdi", t("sekmeler.simdi"));
    metinSet("tab-gecmis", t("sekmeler.gecmis"));
    metinSet("tab-bagla", t("sekmeler.bagla"));
    metinSet("tab-ayarlar", t("sekmeler.ayarlar"));

    const acilirYenile = document.getElementById("btn-acilir-yenile");
    if (acilirYenile) acilirYenile.dataset.ipucu = t("genel.yenileIpucu");
    const genisYenile = document.getElementById("btn-genis-yenile");
    if (genisYenile) genisYenile.dataset.ipucu = t("genel.yenileIpucu");
    const genisletBtn = document.getElementById("btn-acilir-genislet");
    if (genisletBtn) genisletBtn.dataset.ipucu = t("genel.genisletIpucu");
    metinSet("lbl-menu-genislet", t("genel.genisletKisayol"));
    metinSet("lbl-menu-yenile", t("genel.yenile"));
    metinSet("lbl-menu-ayarlar", t("sekmeler.ayarlar"));
    metinSet("lbl-menu-hakkinda", t("genel.hakkinda"));
    metinSet("lbl-menu-cikis", t("genel.cikis"));
    const bagBtn = document.getElementById("btn-acilir-baglanabilir");
    if (bagBtn) bagBtn.dataset.ipucu = t("genel.baglanabilirIpucu");
    const kucultBtn = document.getElementById("btn-genis-kucult");
    if (kucultBtn) kucultBtn.dataset.ipucu = t("genel.kucultIpucu");

    const duzenleMetin = durum.duzenleModu ? t("kartlar.bitti") : t("kartlar.duzenle");
    const acilirDuzenleBtn = document.getElementById("btn-acilir-duzenle");
    if (acilirDuzenleBtn) {
      acilirDuzenleBtn.classList.toggle("aktif", durum.duzenleModu);
      acilirDuzenleBtn.setAttribute("aria-pressed", durum.duzenleModu ? "true" : "false");
      acilirDuzenleBtn.dataset.ipucu = t("kartlar.duzenleIpucu");
      metinSet("lbl-acilir-duzenle", duzenleMetin);
      acilirDuzenleBtn.setAttribute("aria-label", duzenleMetin);
    }
    const genisDuzenleBtn = document.getElementById("btn-genis-duzenle");
    if (genisDuzenleBtn) {
      genisDuzenleBtn.classList.toggle("aktif", durum.duzenleModu);
      genisDuzenleBtn.classList.toggle("gizli", durum.sekme !== "simdi");
      genisDuzenleBtn.setAttribute("aria-pressed", durum.duzenleModu ? "true" : "false");
      genisDuzenleBtn.dataset.ipucu = t("kartlar.duzenleIpucu");
      metinSet("lbl-genis-duzenle", duzenleMetin);
    }

    metinSet("tepsi-kapali-yazi", t("genel.tepsiKapali"));
    metinSet("btn-tepsi-ac", t("genel.tepsidenAc"));
    const kisayolEl = document.getElementById("pencere-kisayol-ipucu");
    if (kisayolEl) {
      kisayolEl.textContent = t("genel.kisayollar");
      kisayolEl.dataset.ipucu = t("genel.kisayolIpucu");
    }

    const sVer = `v${durum.anlik?.surum || "0.1.0"}`;
    for (const id of ["btn-surum-acilir", "btn-surum-genis"]) {
      const el = document.getElementById(id);
      if (el) {
        el.textContent = sVer;
        el.dataset.ipucu = durum.demoMu ? t("genel.surumIpucu") : t("ayarlar.surumEtiket");
      }
    }

    document.querySelectorAll(".yuzey-btn").forEach((b) => {
      const aktif = b.dataset.yuzey === durum.yuzey;
      b.classList.toggle("aktif", aktif);
      b.setAttribute("aria-selected", aktif ? "true" : "false");
    });

    document.querySelectorAll(".yuzey-kapsayici").forEach((s) => s.classList.remove("aktif"));
    document.getElementById(`yuzey-${durum.yuzey}`)?.classList.add("aktif");
    document.documentElement.dataset.yuzey = durum.yuzey;

    document.querySelectorAll(".sekme-btn").forEach((b) => {
      const aktif = b.dataset.sekme === durum.sekme;
      b.classList.toggle("aktif", aktif);
      b.setAttribute("aria-selected", aktif ? "true" : "false");
    });
    document.querySelectorAll(".sekme-icerik").forEach((p) => {
      p.classList.toggle("aktif", p.id === `panel-${durum.sekme}`);
    });
  }

  function tumunuCiz() {
    temaNitelikleriniUygula();
    metinleriGuncelle();
    acilirCiz();
    simdiSekmesiCiz();
    gecmisSekmesiCiz();
    baglaSekmesiCiz();
    ayarlarSekmesiCiz();
    seritCiz();
    servisUyariCiz();
  }

  let bildirimZamanlayici = null;
  function bildirimGoster(metin) {
    if (bildirimZamanlayici) window.clearTimeout(bildirimZamanlayici);
    for (const id of ["acilir-bildirim", "genis-bildirim"]) {
      const el = document.getElementById(id);
      if (!el) continue;
      el.textContent = metin;
      el.classList.remove("gizli");
    }
    const duyuruEl = document.getElementById("ekran-okuyucu-duyuru");
    if (duyuruEl) duyuruEl.textContent = metin;
    bildirimZamanlayici = window.setTimeout(() => {
      for (const id of ["acilir-bildirim", "genis-bildirim"]) {
        document.getElementById(id)?.classList.add("gizli");
      }
    }, 3200);
  }

  function yenileTetikle() {
    if (canliMi()) {
      canliYenile();
      return;
    }
    const simdi = Date.now();
    if (simdi - durum.sonYenileMs < 4000) {
      bildirimGoster(t("genel.hizSiniri"));
      return;
    }
    durum.sonYenileMs = simdi;
    bildirimGoster(t("genel.yenilendi"));
    tumunuCiz();
  }

  function senaryoUygula(yeniSenaryo) {
    if (!durum.demoMu) return;
    durum.senaryo = yeniSenaryo;
    durum.anlik = anlikSadelestir(senaryoUret(yeniSenaryo, durum.temelAnlik));
    tumunuCiz();
  }

  function yuzeyDegistir(yuzey, sekme) {
    durum.yuzey = yuzey;
    if (sekme) durum.sekme = sekme;
    tumunuCiz();
  }

  // --------------------------------------------------------------------------
  // 14. Erişilebilir Yüzen İpucu Denetleyicisi (Hover + Focus + Esc)
  // --------------------------------------------------------------------------
  let aktifIpucuHedef = null;

  function ipucuGoster(hedef) {
    const metin = hedef?.dataset?.ipucu;
    const kutu = document.getElementById("cetvel-ipucu");
    if (!kutu || !metin || durum.yuzey === "serit") {
      ipucuGizle();
      return;
    }
    aktifIpucuHedef = hedef;
    kutu.textContent = metin;
    kutu.classList.remove("gizli");
    hedef.setAttribute("aria-describedby", "cetvel-ipucu");

    const r = hedef.getBoundingClientRect();
    const kr = kutu.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    let sol = clamp(r.left + r.width / 2 - kr.width / 2, 8, Math.max(8, vw - kr.width - 8));
    let ust = r.top - kr.height - 8;
    if (ust < 8) ust = Math.min(vh - kr.height - 8, r.bottom + 8);

    kutu.style.left = `${Math.round(sol)}px`;
    kutu.style.top = `${Math.round(ust)}px`;
  }

  function ipucuGizle() {
    if (aktifIpucuHedef) {
      aktifIpucuHedef.removeAttribute("aria-describedby");
      aktifIpucuHedef = null;
    }
    document.getElementById("cetvel-ipucu")?.classList.add("gizli");
  }

  // --------------------------------------------------------------------------
  // 14b. Kart Özelleştirme, Sürükle-Bırak (Pointer + FLIP) ve Klavye Taşıma
  // --------------------------------------------------------------------------
  const hareketAzaltMedya = window.matchMedia("(prefers-reduced-motion: reduce)");
  let aktifSurukleme = null;
  let klavyeSurukleme = null;

  function animasyonAktifMi() {
    return !hareketAzaltMedya.matches;
  }

  function suruklemeKonteynerAktifMi(konteynerId) {
    return Boolean(
      (aktifSurukleme && aktifSurukleme.konteyner?.id === konteynerId) ||
        (klavyeSurukleme && klavyeSurukleme.konteyner?.id === konteynerId)
    );
  }

  function kartSinirlariHesapla(kartlar) {
    const sabitSet = new Set(durum.kartTercihler.sabitKartlar);
    let sabitSayi = 0;
    for (const k of kartlar) {
      if (sabitSet.has(k.dataset.havuzId)) sabitSayi++;
      else break;
    }
    return sabitSayi;
  }

  function kartSiraGuncelle(gorunenYeniSira) {
    const temizGorunen = [];
    const gorulen = new Set();
    for (const id of gorunenYeniSira) {
      const p = platformKimligineEsle(id, durum.anlik.havuzlar);
      if (typeof p === "string" && p && !gorulen.has(p)) {
        gorulen.add(p);
        temizGorunen.push(p);
      }
    }
    const tumSirali = platformGruplariOlustur(durum.anlik.havuzlar, { gizlileriDahilEt: true }).map((g) => g.platform);
    let ptr = 0;
    const yeniTamSira = tumSirali.map((p) => (gorulen.has(p) ? temizGorunen[ptr++] : p));
    const mevcutSet = new Set(yeniTamSira);
    for (const eskiId of kartListesiniPlatformaEsle(durum.kartTercihler.kartSirasi, durum.anlik.havuzlar)) {
      if (!mevcutSet.has(eskiId)) {
        yeniTamSira.push(eskiId);
        mevcutSet.add(eskiId);
      }
    }
    durum.kartTercihler.kartSirasi = yeniTamSira;
    kartTercihleriniKaydet();
  }

  function flipAnimasyonuUygula(konteyner, eskiKonumlar, haricKart) {
    if (!animasyonAktifMi() || !konteyner) return;
    const tasinacaklar = [];
    for (const kart of konteyner.children) {
      if (kart === haricKart) continue;
      const eski = eskiKonumlar.get(kart) || eskiKonumlar.get(kart.dataset?.havuzId);
      if (!eski) continue;
      const yeni = kart.getBoundingClientRect();
      const dx = eski.left - yeni.left;
      const dy = eski.top - yeni.top;
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
      tasinacaklar.push({ kart, dx, dy });
    }
    if (tasinacaklar.length === 0) return;

    for (const { kart, dx, dy } of tasinacaklar) {
      if (kart._flipTemizle) kart._flipTemizle();
      kart.style.transition = "none";
      kart.style.transform = `translate3d(${dx}px, ${dy}px, 0)`;
    }
    void konteyner.offsetHeight;
    for (const { kart } of tasinacaklar) {
      kart.style.transition = "transform 200ms cubic-bezier(0.22, 1, 0.36, 1)";
      kart.style.transform = "";
      let zamanlayici = null;
      const bitir = () => {
        if (zamanlayici) window.clearTimeout(zamanlayici);
        kart.removeEventListener("transitionend", onEnd);
        kart.style.transition = "";
        kart._flipTemizle = null;
      };
      const onEnd = (e) => {
        if (e.target === kart && e.propertyName === "transform") bitir();
      };
      kart.addEventListener("transitionend", onEnd);
      zamanlayici = window.setTimeout(bitir, 240);
      kart._flipTemizle = bitir;
    }
  }

  function yerlesimKonumlariniYenile(konteyner, scrollEl, itemSelector) {
    const sTop = scrollEl ? scrollEl.scrollTop : 0;
    const sLeft = scrollEl ? scrollEl.scrollLeft : 0;
    const kartlar = [...konteyner.children].filter((el) => el.matches(itemSelector));
    for (const k of kartlar) {
      if (k._flipTemizle) k._flipTemizle();
      k.style.transition = "none";
      k.style.transform = "";
    }
    for (const k of kartlar) {
      const r = k.getBoundingClientRect();
      k._layoutRect = {
        left: r.left + sLeft,
        top: r.top + sTop,
        right: r.right + sLeft,
        bottom: r.bottom + sTop,
        width: r.width,
        height: r.height,
      };
    }
  }

  function gorunumKutusu(kart, scrollEl) {
    const lr = kart._layoutRect;
    if (!lr) return kart.getBoundingClientRect();
    const sTop = scrollEl ? scrollEl.scrollTop : 0;
    const sLeft = scrollEl ? scrollEl.scrollLeft : 0;
    return {
      left: lr.left - sLeft,
      top: lr.top - sTop,
      right: lr.right - sLeft,
      bottom: lr.bottom - sTop,
      width: lr.width,
      height: lr.height,
    };
  }

  function konteynerSirasiniGeriYukle(konteyner, sira, itemSelector) {
    const harita = new Map();
    for (const el of konteyner.children) {
      if (el.matches(itemSelector) && el.dataset.havuzId) harita.set(el.dataset.havuzId, el);
    }
    for (const id of sira) {
      const el = harita.get(id);
      if (el) konteyner.appendChild(el);
    }
  }

  function duzenleModuDegistir(yeniDurum) {
    const acik = typeof yeniDurum === "boolean" ? yeniDurum : !durum.duzenleModu;
    if (!acik) {
      if (aktifSurukleme) suruklemeBitir(true);
      if (klavyeSurukleme) klavyeTasimaBitir(true);
    }
    durum.duzenleModu = acik;
    metinleriGuncelle();
    acilirCiz();
    simdiSekmesiCiz();
  }

  function idKonumHaritasiAl(konteyner, itemSelector) {
    const harita = new Map();
    if (!konteyner) return harita;
    for (const el of konteyner.querySelectorAll(`:scope > ${itemSelector}`)) {
      if (el.dataset.havuzId) harita.set(el.dataset.havuzId, el.getBoundingClientRect());
    }
    return harita;
  }

  function kartSabitDegistir(id) {
    if (!id || (!platformMevcutMu(id) && !durum.anlik.havuzlar?.[id])) return;
    const hedefId = platformKimligineEsle(id, durum.anlik.havuzlar);
    const konteyner =
      durum.yuzey === "acilir"
        ? document.getElementById("acilir-havuz-listesi")
        : durum.yuzey === "pencere" && durum.sekme === "simdi"
        ? document.getElementById("simdi-havuz-grid")
        : null;
    const itemSelector = durum.yuzey === "acilir" ? ".havuz-satir" : ".detay-kart";
    const eskiKonumlar = idKonumHaritasiAl(konteyner, itemSelector);

    const liste = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, durum.anlik.havuzlar);
    const idx = liste.indexOf(hedefId);
    const sabitlendi = idx === -1;
    if (sabitlendi) liste.push(hedefId);
    else liste.splice(idx, 1);
    durum.kartTercihler.sabitKartlar = liste;
    kartTercihleriniKaydet();

    acilirCiz();
    simdiSekmesiCiz();
    seritCiz();

    if (konteyner) flipAnimasyonuUygula(konteyner, eskiKonumlar, null);
    const ad = kartGorunenAdi(hedefId);
    ekranOkuyucuDuyur(fmt(sabitlendi ? "kartlar.duyuruSabitlendi" : "kartlar.duyuruSabitKaldirildi", { ad }));
  }

  function kartGizleDegistir(id) {
    if (!id || (!platformMevcutMu(id) && !durum.anlik.havuzlar?.[id])) return;
    const hedefId = platformKimligineEsle(id, durum.anlik.havuzlar);
    const liste = kartListesiniPlatformaEsle(durum.kartTercihler.gizliKartlar, durum.anlik.havuzlar);
    const idx = liste.indexOf(hedefId);
    const gizlendi = idx === -1;
    if (gizlendi) liste.push(hedefId);
    else liste.splice(idx, 1);
    durum.kartTercihler.gizliKartlar = liste;
    kartTercihleriniKaydet();

    acilirCiz();
    simdiSekmesiCiz();
    seritCiz();

    const ad = kartGorunenAdi(hedefId);
    ekranOkuyucuDuyur(fmt(gizlendi ? "kartlar.duyuruGizlendi" : "kartlar.duyuruGosterildi", { ad }));
  }

  function kartDuzeniniSifirla() {
    const konteyner =
      durum.yuzey === "acilir"
        ? document.getElementById("acilir-havuz-listesi")
        : durum.yuzey === "pencere" && durum.sekme === "simdi"
        ? document.getElementById("simdi-havuz-grid")
        : null;
    const itemSelector = durum.yuzey === "acilir" ? ".havuz-satir" : ".detay-kart";
    const eskiKonumlar = idKonumHaritasiAl(konteyner, itemSelector);

    durum.kartTercihler = varsayilanKartTercihleri();
    kartTercihleriniKaydet();
    acilirCiz();
    simdiSekmesiCiz();
    seritCiz();

    if (konteyner) flipAnimasyonuUygula(konteyner, eskiKonumlar, null);
    ekranOkuyucuDuyur(t("kartlar.varsayilanaDondu"));
  }

  function suruklemeHedefKontrol(s, zorla = false) {
    if (!s || !s.basladi) return;
    const { konteyner, scrollEl, kart, itemSelector, cokSutun, sonX, sonY } = s;
    const kartlar = [...konteyner.children].filter((el) => el.matches(itemSelector));
    const phIdx = kartlar.indexOf(kart);
    if (phIdx === -1) return;

    const sabitSayi = kartSinirlariHesapla(kartlar);
    const sabitList = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, durum.anlik.havuzlar);
    const sabitMi = sabitList.includes(s.id);
    const minIdx = sabitMi ? 0 : sabitSayi;
    const maxIdx = sabitMi ? Math.max(0, sabitSayi - 1) : kartlar.length - 1;

    const phR = gorunumKutusu(kart, scrollEl);
    if (sonX >= phR.left - 4 && sonX <= phR.right + 4 && sonY >= phR.top - 4 && sonY <= phR.bottom + 4) {
      return;
    }

    let hedefIdx = -1;
    const simdiMs = window.performance.now();
    for (let i = minIdx; i <= maxIdx; i++) {
      const aday = kartlar[i];
      if (aday === kart) continue;
      if (!zorla && aday === s.sonTakasKart && simdiMs - s.sonTakasZamani < 220) continue;
      const r = gorunumKutusu(aday, scrollEl);
      if (sonX >= r.left && sonX <= r.right && sonY >= r.top && sonY <= r.bottom) {
        if (!cokSutun) {
          if (i > phIdx && sonY < r.top + Math.min(r.height * 0.4, phR.height * 0.55)) continue;
          if (i < phIdx && sonY > r.bottom - Math.min(r.height * 0.4, phR.height * 0.55)) continue;
        }
        hedefIdx = i;
        break;
      }
    }
    if (hedefIdx === -1 || hedefIdx === phIdx) return;

    const hedefKart = kartlar[hedefIdx];
    const eskiKonumlar = new Map();
    for (const k of kartlar) {
      eskiKonumlar.set(k, k.getBoundingClientRect());
    }

    if (hedefIdx > phIdx) {
      konteyner.insertBefore(kart, hedefKart.nextSibling);
    } else {
      konteyner.insertBefore(kart, hedefKart);
    }
    yerlesimKonumlariniYenile(konteyner, scrollEl, itemSelector);
    flipAnimasyonuUygula(konteyner, eskiKonumlar, kart);

    s.sonTakasKart = hedefKart;
    s.sonTakasZamani = simdiMs;
  }

  function otomatikKaydirmaAdimi() {
    const s = aktifSurukleme;
    if (!s || !s.basladi) return;
    const sc = s.scrollEl;
    if (sc) {
      const sr = sc.getBoundingClientRect();
      const esik = 48;
      let delta = 0;
      if (s.sonY < sr.top + esik && sc.scrollTop > 0) {
        delta = -Math.ceil(((sr.top + esik - s.sonY) / esik) * 12);
      } else if (s.sonY > sr.bottom - esik && sc.scrollTop + sc.clientHeight < sc.scrollHeight) {
        delta = Math.ceil(((s.sonY - (sr.bottom - esik)) / esik) * 12);
      }
      if (delta !== 0) {
        sc.scrollTop += delta;
        suruklemeHedefKontrol(s);
      }
    }
    s.rafId = window.requestAnimationFrame(otomatikKaydirmaAdimi);
  }

  function suruklemeBitir(iptal) {
    const s = aktifSurukleme;
    if (!s) return;
    aktifSurukleme = null;

    if (s.rafId) window.cancelAnimationFrame(s.rafId);
    window.removeEventListener("pointermove", s.onMove);
    window.removeEventListener("pointerup", s.onUp);
    window.removeEventListener("pointercancel", s.onCancel);
    try {
      if (s.tutamak.hasPointerCapture?.(s.pointerId)) s.tutamak.releasePointerCapture(s.pointerId);
    } catch (_) {
      // Yoksay
    }

    if (!s.basladi) return;

    if (!iptal) suruklemeHedefKontrol(s, true);

    s.kopya?.remove();
    s.kart.classList.remove("kart-yer-tutucu");
    s.tutamak.setAttribute("aria-pressed", "false");
    for (const el of s.konteyner.children) {
      if (el._flipTemizle) el._flipTemizle();
      el.style.transition = "";
      el.style.transform = "";
      delete el._layoutRect;
    }

    if (iptal) {
      konteynerSirasiniGeriYukle(s.konteyner, s.baslangicSirasi, s.itemSelector);
      const ad = kartGorunenAdi(s.id);
      ekranOkuyucuDuyur(fmt("kartlar.duyuruIptal", { ad }));
      return;
    }

    const yeniSira = [...s.konteyner.children]
      .filter((el) => el.matches(s.itemSelector))
      .map((el) => el.dataset.havuzId)
      .filter(Boolean);
    const degisti =
      yeniSira.length === s.baslangicSirasi.length && yeniSira.some((id, idx) => id !== s.baslangicSirasi[idx]);
    if (degisti) {
      kartSiraGuncelle(yeniSira);
    }
    const yeniIdx = yeniSira.indexOf(s.id);
    const ad = kartGorunenAdi(s.id);
    ekranOkuyucuDuyur(fmt("kartlar.duyuruBirakildi", { ad, sira: yeniIdx + 1 }));

    seritCiz();
    if (s.konteyner.id === "acilir-havuz-listesi") simdiSekmesiCiz();
    else acilirCiz();
  }

  function klavyeTasimaBitir(iptal) {
    const k = klavyeSurukleme;
    if (!k) return;
    klavyeSurukleme = null;

    k.kart.classList.remove("kart-klavye-tasiniyor");
    const tutamak = k.kart.querySelector(`[data-kart-tutamak="${CSS.escape(k.id)}"]`);
    if (tutamak) tutamak.setAttribute("aria-pressed", "false");

    const ad = kartGorunenAdi(k.id);
    if (iptal) {
      k.domTasiniyor = true;
      konteynerSirasiniGeriYukle(k.konteyner, k.baslangicSirasi, k.itemSelector);
      k.domTasiniyor = false;
      tutamak?.focus({ preventScroll: true });
      ekranOkuyucuDuyur(fmt("kartlar.duyuruIptal", { ad }));
      return;
    }

    const yeniSira = [...k.konteyner.children]
      .filter((el) => el.matches(k.itemSelector))
      .map((el) => el.dataset.havuzId)
      .filter(Boolean);
    const degisti =
      yeniSira.length === k.baslangicSirasi.length && yeniSira.some((id, idx) => id !== k.baslangicSirasi[idx]);
    if (degisti) kartSiraGuncelle(yeniSira);

    const yeniIdx = yeniSira.indexOf(k.id);
    ekranOkuyucuDuyur(fmt("kartlar.duyuruBirakildi", { ad, sira: yeniIdx + 1 }));

    seritCiz();
    if (k.konteyner.id === "acilir-havuz-listesi") simdiSekmesiCiz();
    else acilirCiz();
  }

  function klavyeTasimaBaslat(tutamak) {
    const kart = tutamak.closest(".havuz-satir, .detay-kart");
    const konteyner = kart?.parentElement;
    const id = tutamak.dataset.kartTutamak;
    if (!kart || !konteyner || !id) return;

    const itemSelector = konteyner.id === "acilir-havuz-listesi" ? ".havuz-satir" : ".detay-kart";
    const baslangicSirasi = [...konteyner.children]
      .filter((el) => el.matches(itemSelector))
      .map((el) => el.dataset.havuzId)
      .filter(Boolean);
    const idx = baslangicSirasi.indexOf(id);

    klavyeSurukleme = {
      id,
      kart,
      konteyner,
      itemSelector,
      baslangicSirasi,
      domTasiniyor: false,
    };
    kart.classList.add("kart-klavye-tasiniyor");
    tutamak.setAttribute("aria-pressed", "true");
    const ad = kartGorunenAdi(id);
    ekranOkuyucuDuyur(fmt("kartlar.duyuruAlindi", { ad, sira: idx + 1 }));
  }

  function klavyeTasimaAdim(yon) {
    const k = klavyeSurukleme;
    if (!k) return;
    const kartlar = [...k.konteyner.children].filter((el) => el.matches(k.itemSelector));
    const mevcutIdx = kartlar.indexOf(k.kart);
    if (mevcutIdx === -1) return;

    const sabitSayi = kartSinirlariHesapla(kartlar);
    const sabitList = kartListesiniPlatformaEsle(durum.kartTercihler.sabitKartlar, durum.anlik.havuzlar);
    const sabitMi = sabitList.includes(k.id);
    const minIdx = sabitMi ? 0 : sabitSayi;
    const maxIdx = sabitMi ? Math.max(0, sabitSayi - 1) : kartlar.length - 1;
    const hedefIdx = mevcutIdx + yon;

    if (hedefIdx < minIdx || hedefIdx > maxIdx) {
      if ((sabitMi && hedefIdx > maxIdx) || (!sabitMi && hedefIdx < minIdx && sabitSayi > 0)) {
        ekranOkuyucuDuyur(t("kartlar.duyuruSinir"));
      }
      return;
    }

    const eskiKonumlar = new Map(kartlar.map((el) => [el, el.getBoundingClientRect()]));
    const hedefKart = kartlar[hedefIdx];

    k.domTasiniyor = true;
    if (hedefIdx > mevcutIdx) {
      k.konteyner.insertBefore(k.kart, hedefKart.nextSibling);
    } else {
      k.konteyner.insertBefore(k.kart, hedefKart);
    }
    const tutamak = k.kart.querySelector(`[data-kart-tutamak="${CSS.escape(k.id)}"]`);
    tutamak?.focus({ preventScroll: true });
    k.kart.scrollIntoView({ block: "nearest" });
    k.domTasiniyor = false;

    flipAnimasyonuUygula(k.konteyner, eskiKonumlar, null);
    const ad = kartGorunenAdi(k.id);
    ekranOkuyucuDuyur(fmt("kartlar.duyuruTasindi", { ad, sira: hedefIdx + 1 }));
  }

  function kartSuruklemeBagla(konteynerId, scrollSelector, itemSelector, cokSutun) {
    const konteyner = document.getElementById(konteynerId);
    if (!konteyner) return;

    konteyner.addEventListener("dragstart", (ev) => {
      if (durum.duzenleModu) ev.preventDefault();
    });

    konteyner.addEventListener("pointerdown", (ev) => {
      if (!durum.duzenleModu || klavyeSurukleme) return;
      if (ev.pointerType === "mouse" && ev.button !== 0) return;
      const tutamak = ev.target.closest("[data-kart-tutamak]");
      if (!tutamak) return;
      const kart = tutamak.closest(itemSelector);
      const id = tutamak.dataset.kartTutamak;
      if (!kart || !id) return;

      ev.preventDefault();
      const scrollEl = scrollSelector === "self" ? konteyner : document.querySelector(scrollSelector);
      const durumNesne = {
        pointerId: ev.pointerId,
        tutamak,
        kart,
        id,
        konteyner,
        scrollEl,
        itemSelector,
        cokSutun,
        basX: ev.clientX,
        basY: ev.clientY,
        sonX: ev.clientX,
        sonY: ev.clientY,
        basladi: false,
        baslangicSirasi: [],
        kopya: null,
        rafId: 0,
        sonTakasKart: null,
        sonTakasZamani: 0,
      };

      const onMove = (mEv) => {
        if (mEv.pointerId !== durumNesne.pointerId) return;
        durumNesne.sonX = mEv.clientX;
        durumNesne.sonY = mEv.clientY;
        if (!durumNesne.basladi) {
          if (Math.hypot(mEv.clientX - durumNesne.basX, mEv.clientY - durumNesne.basY) < 6) return;
          durumNesne.basladi = true;
          ipucuGizle();
          try {
            tutamak.setPointerCapture(durumNesne.pointerId);
          } catch (_) {
            // Yoksay
          }
          const rect = kart.getBoundingClientRect();
          durumNesne.offsetX = durumNesne.basX - rect.left;
          durumNesne.offsetY = durumNesne.basY - rect.top;
          durumNesne.baslangicSirasi = [...konteyner.children]
            .filter((el) => el.matches(itemSelector))
            .map((el) => el.dataset.havuzId)
            .filter(Boolean);

          const kopya = kart.cloneNode(true);
          if (kopya.id) kopya.removeAttribute("id");
          kopya.querySelectorAll("[id]").forEach((el) => el.removeAttribute("id"));
          kopya.classList.remove("kart-yer-tutucu");
          kopya.classList.add("kart-suruklenen-kopya");
          kopya.setAttribute("aria-hidden", "true");
          kopya.style.left = "0px";
          kopya.style.top = "0px";
          kopya.style.width = `${rect.width}px`;
          kopya.style.height = `${rect.height}px`;
          document.body.appendChild(kopya);
          durumNesne.kopya = kopya;

          kart.classList.add("kart-yer-tutucu");
          tutamak.setAttribute("aria-pressed", "true");
          yerlesimKonumlariniYenile(konteyner, scrollEl, itemSelector);
          durumNesne.rafId = window.requestAnimationFrame(otomatikKaydirmaAdimi);
        }
        mEv.preventDefault();
        const tx = mEv.clientX - durumNesne.offsetX;
        const ty = mEv.clientY - durumNesne.offsetY;
        durumNesne.kopya.style.transform = `translate3d(${tx}px, ${ty}px, 0) scale(1.025)`;
        suruklemeHedefKontrol(durumNesne);
      };

      const onUp = (uEv) => {
        if (uEv.pointerId !== durumNesne.pointerId) return;
        suruklemeBitir(false);
      };

      const onCancel = (cEv) => {
        if (cEv.pointerId !== durumNesne.pointerId) return;
        suruklemeBitir(true);
      };

      durumNesne.onMove = onMove;
      durumNesne.onUp = onUp;
      durumNesne.onCancel = onCancel;
      aktifSurukleme = durumNesne;

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      window.addEventListener("pointercancel", onCancel);
    });

    konteyner.addEventListener("focusout", (ev) => {
      if (!klavyeSurukleme || klavyeSurukleme.domTasiniyor || klavyeSurukleme.konteyner !== konteyner) return;
      if (!ev.target.closest?.("[data-kart-tutamak]")) return;
      window.setTimeout(() => {
        if (klavyeSurukleme && !klavyeSurukleme.domTasiniyor) {
          const aktif = document.activeElement?.closest?.("[data-kart-tutamak]");
          if (!aktif || aktif.dataset.kartTutamak !== klavyeSurukleme.id) {
            klavyeTasimaBitir(false);
          }
        }
      }, 0);
    });
  }

  // --------------------------------------------------------------------------
  // 15. Olay Bağlantıları ve Klavye Kısayolları
  // --------------------------------------------------------------------------
  function gostergeTercihiUygula(tur, deger) {
    if (tur === "gosterge") {
      durum.gostergeTuru = deger;
    } else if (tur === "pastaAlt") {
      durum.pastaAltTuru = deger;
    } else if (tur === "mod") {
      durum.gorunumModu = deger;
    } else if (tur === "yon") {
      durum.degerYonu = deger;
      if (canliMi()) ayarKaydet({ degerYonu: deger }, false);
    } else if (tur === "renk") {
      durum.renkTemasi = deger;
      if (canliMi()) ayarKaydet({ renkTemasi: deger }, false);
    } else if (tur === "kip") {
      durum.gorunumKipi = deger;
      if (canliMi()) ayarKaydet({ kip: deger }, false);
    }
    tercihSenkronizeKaydet();
    tumunuCiz();
  }

  function platformSekmesiSec(anahtar, odakla) {
    if (!anahtar) return;
    if (durum.duzenleModu && anahtar !== "genel") duzenleModuDegistir(false);
    durum.platformSekme = anahtar;
    acilirCiz();
    simdiSekmesiCiz();
    if (odakla) {
      const kap = durum.yuzey === "acilir" ? "#acilir-platform-sekmeler" : "#simdi-platform-sekmeler";
      document.querySelector(`${kap} [data-platform-sekme="${CSS.escape(anahtar)}"]`)?.focus();
    }
  }

  // Genel kartı → platform ayrıntısını satır içinde aç/kapat.
  function platformAcikDegistir(platform) {
    if (!platform) return;
    durum.seciliHavuzId = platform;
    if (durum.acikPlatformlar.has(platform)) durum.acikPlatformlar.delete(platform);
    else durum.acikPlatformlar.add(platform);
    acilirCiz();
    simdiSekmesiCiz();
  }

  function olaylariBagla() {
    window.addEventListener("resize", () => {
      if (durum.yuzey === "serit") seritCiz();
    });

    sistemKoyuMuMedya.addEventListener("change", () => {
      if (durum.gorunumKipi === "sistem") temaNitelikleriniUygula();
    });

    window.addEventListener("storage", (ev) => {
      if (ev.key === KART_TERCIH_ANAHTARI) {
        durum.kartTercihler = kartTercihleriniOku();
        if (!aktifSurukleme && !klavyeSurukleme) {
          acilirCiz();
          simdiSekmesiCiz();
          seritCiz();
        }
        return;
      }
      if (ev.key !== TERCIH_ANAHTARI || !ev.newValue) return;
      try {
        const t2 = JSON.parse(ev.newValue);
        durum.renkTemasi = t2.renkTemasi || durum.renkTemasi;
        durum.gorunumKipi = t2.gorunumKipi || durum.gorunumKipi;
        durum.gostergeTuru = t2.gostergeTuru || durum.gostergeTuru;
        durum.pastaAltTuru = t2.pastaAltTuru || durum.pastaAltTuru;
        durum.gorunumModu = t2.gorunumModu || durum.gorunumModu;
        durum.degerYonu = t2.degerYonu || durum.degerYonu;
        if (typeof t2.ayarBolum === "string") durum.ayarBolum = ayarBolumuNormalizeEt(t2.ayarBolum);
        if (t2.uiDil && t2.uiDil !== durum.dil) {
          dilYukleVeUygula(t2.uiDil);
        } else {
          tumunuCiz();
        }
      } catch (_) {
        // Yoksay
      }
    });

    document.querySelectorAll(".yuzey-btn").forEach((btn) => {
      btn.addEventListener("click", () => yuzeyDegistir(btn.dataset.yuzey));
    });

    document.getElementById("btn-proto-kip")?.addEventListener("click", () => {
      const sira = ["sistem", "acik", "koyu"];
      const yeni = sira[(sira.indexOf(durum.gorunumKipi) + 1) % sira.length];
      gostergeTercihiUygula("kip", yeni);
    });

    document.getElementById("btn-proto-renk")?.addEventListener("click", () => {
      const ids = RENK_TEMALARI.map((r) => r.id);
      const yeni = ids[(ids.indexOf(durum.renkTemasi) + 1) % ids.length];
      gostergeTercihiUygula("renk", yeni);
    });

    document.getElementById("sec-proto-dil")?.addEventListener("change", (ev) => {
      const kod = ev.target.value;
      if (canliMi() && dilDestekleniyorMu(kod)) ayarKaydet({ dil: kod }, false);
      dilYukleVeUygula(kod);
    });

    document.addEventListener("click", (ev) => {
      if (ev.target.closest("[data-servis-baslat]")) {
        servisBaslat();
        return;
      }
      const gBtn = ev.target.closest("[data-gosterge-sec]");
      if (gBtn) {
        gostergeTercihiUygula("gosterge", gBtn.dataset.gostergeSec);
        return;
      }
      const pAlt = ev.target.closest("[data-pasta-alt]");
      if (pAlt) {
        gostergeTercihiUygula("pastaAlt", pAlt.dataset.pastaAlt);
        return;
      }
      const mBtn = ev.target.closest("[data-mod-sec]");
      if (mBtn) {
        gostergeTercihiUygula("mod", mBtn.dataset.modSec);
        return;
      }
      const yBtn = ev.target.closest("[data-yon-sec]");
      if (yBtn) {
        gostergeTercihiUygula("yon", yBtn.dataset.yonSec);
        return;
      }
      const seg = ev.target.closest("[data-segment-havuz]");
      if (seg) {
        durum.seciliHavuzId = seg.dataset.segmentHavuz;
        durum.vurguluSegmentId = seg.dataset.segmentHavuz;
        acilirCiz();
        simdiSekmesiCiz();
        return;
      }
      const sabitBtn = ev.target.closest("[data-kart-sabit]");
      if (sabitBtn) {
        kartSabitDegistir(sabitBtn.dataset.kartSabit);
        return;
      }
      const gizleBtn = ev.target.closest("[data-kart-gizle]");
      if (gizleBtn) {
        kartGizleDegistir(gizleBtn.dataset.kartGizle);
        return;
      }
      if (ev.target.closest("[data-kart-sifirla]")) {
        kartDuzeniniSifirla();
        return;
      }
      if (ev.target.closest("[data-kart-duzenle-bitir]")) {
        duzenleModuDegistir(false);
        return;
      }
      if (ev.target.closest("[data-kart-duzenle-ac]")) {
        duzenleModuDegistir(true);
      }
    });

    // İpucu olayları
    document.addEventListener("mouseover", (ev) => {
      const hedef = ev.target.closest("[data-ipucu]");
      if (hedef) ipucuGoster(hedef);
      else ipucuGizle();
    });
    document.addEventListener("mouseout", (ev) => {
      if (!ev.relatedTarget || !ev.relatedTarget.closest?.("[data-ipucu]")) ipucuGizle();
    });
    document.addEventListener("focusin", (ev) => {
      const hedef = ev.target.closest?.("[data-ipucu]");
      if (hedef) ipucuGoster(hedef);
      else ipucuGizle();
    });
    document.addEventListener("focusout", () => {
      window.setTimeout(() => {
        const aktif = document.activeElement?.closest?.("[data-ipucu]");
        if (!aktif) ipucuGizle();
      }, 10);
    });

    const senaryoAcKapa = () => {
      if (!durum.demoMu) return;
      durum.senaryoAcik = !durum.senaryoAcik;
      document.getElementById("senaryo-cekmecesi")?.classList.toggle("gizli", !durum.senaryoAcik);
    };

    document.getElementById("btn-surum-acilir")?.addEventListener("click", senaryoAcKapa);
    document.getElementById("btn-surum-genis")?.addEventListener("click", senaryoAcKapa);
    document.querySelectorAll(".senaryo-btn").forEach((btn) => {
      btn.addEventListener("click", () => senaryoUygula(btn.dataset.senaryo));
    });

    document.getElementById("btn-acilir-yenile")?.addEventListener("click", yenileTetikle);
    document.getElementById("btn-genis-yenile")?.addEventListener("click", yenileTetikle);
    document.getElementById("btn-acilir-duzenle")?.addEventListener("click", () => duzenleModuDegistir());
    document.getElementById("btn-genis-duzenle")?.addEventListener("click", () => duzenleModuDegistir());
    document.getElementById("btn-acilir-genislet")?.addEventListener("click", () => genislet("simdi"));
    document.getElementById("btn-acilir-hakkinda")?.addEventListener("click", () => {
      durum.ayarBolum = "hakkinda";
      tercihSenkronizeKaydet();
      genislet("ayarlar");
    });
    document.getElementById("btn-acilir-baglanabilir")?.addEventListener("click", () => genislet("bagla"));
    document.getElementById("btn-acilir-ayarlar")?.addEventListener("click", () => genislet("ayarlar"));
    document.getElementById("btn-acilir-cikis")?.addEventListener("click", () => {
      if (canliMi()) invoke("uygulamadan_cik").catch(() => {});
    });
    document.getElementById("btn-genis-kucult")?.addEventListener("click", acilirDon);
    document.getElementById("btn-tepsi-ac")?.addEventListener("click", () => {
      durum.acilirKapali = false;
      acilirCiz();
    });
    document.getElementById("btn-tepsi-ikon")?.addEventListener("click", () => {
      durum.acilirKapali = !durum.acilirKapali;
      acilirCiz();
    });

    document.querySelectorAll(".sekme-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        durum.ayarSifirlaOnay = false;
        durum.sekme = btn.dataset.sekme;
        tumunuCiz();
      });
    });

    for (const id of ["acilir-havuz-listesi", "simdi-havuz-grid"]) {
      document.getElementById(id)?.addEventListener("click", (ev) => {
        if (ev.target.closest("[data-kart-tutamak], [data-kart-sabit], [data-kart-gizle], [data-kart-duzenle-ac]")) return;
        const gitBtn = ev.target.closest("[data-platform-git]");
        if (gitBtn) {
          platformSekmesiSec(gitBtn.dataset.platformGit, false);
          return;
        }
        const acBtn = ev.target.closest("[data-platform-ac]");
        if (acBtn) {
          platformAcikDegistir(acBtn.dataset.platformAc);
        }
      });
    }

    for (const id of ["acilir-platform-sekmeler", "simdi-platform-sekmeler"]) {
      document.getElementById(id)?.addEventListener("click", (ev) => {
        const btn = ev.target.closest("[data-platform-sekme]");
        if (btn) platformSekmesiSec(btn.dataset.platformSekme, false);
      });
    }

    kartSuruklemeBagla("acilir-havuz-listesi", "#acilir-icerik", ".havuz-satir", false);
    kartSuruklemeBagla("simdi-havuz-grid", "#yuzey-pencere .pencere-govde", ".detay-kart", true);

    document.getElementById("gecmis-havuz-secici")?.addEventListener("click", (ev) => {
      const btn = ev.target.closest("[data-gecmis-id]");
      if (!btn) return;
      durum.gecmisHavuzId = btn.dataset.gecmisId;
      gecmisSekmesiCiz();
    });

    document.getElementById("btn-bagla-tara")?.addEventListener("click", () => {
      if (canliMi()) {
        canliTara();
        return;
      }
      durum.baglaDurum.sonTarama = saatBicimle(new Date().toISOString());
      bildirimGoster(t("bagla.tarandi"));
      baglaSekmesiCiz();
    });

    document.getElementById("bagla-liste")?.addEventListener("click", (ev) => {
      if (canliMi()) {
        canliBaglaEylemi(ev);
        return;
      }
      const agyBtn = ev.target.closest("[data-agy-izin]");
      if (agyBtn) {
        durum.ayarlar.agyOkuma = agyBtn.dataset.agyIzin;
        tumunuCiz();
        return;
      }
      const btn = ev.target.closest("[data-bagla-eylem]");
      if (!btn) return;
      const eylem = btn.dataset.baglaEylem;
      if (eylem === "claude-kopyala") {
        navigator.clipboard?.writeText("claude plugin update kota-bandi@cetvel").catch(() => {});
        btn.textContent = t("bagla.kopyalandi");
        window.setTimeout(() => (btn.textContent = t("bagla.kopyala")), 1400);
      } else if (eylem === "claude-onizle") {
        durum.baglaDurum.claudeFarkAcik = !durum.baglaDurum.claudeFarkAcik;
        baglaSekmesiCiz();
      } else if (eylem === "claude-onayla") {
        durum.baglaDurum.claudeOnaylandi = true;
        baglaSekmesiCiz();
      } else if (eylem === "codex-test") {
        durum.baglaDurum.codexSonuc = `890 ms · ${t("pencereKisa.5s")} ${yuzdeBicimle(1)} ${t("genel.kalan")} · ${t("pencereKisa.hafta")} ${yuzdeBicimle(0)} ${t("genel.kalan")}`;
        baglaSekmesiCiz();
      } else if (eylem === "agy-test") {
        durum.baglaDurum.agySonuc = `3,420 ms · Antigravity Gemini ${t("pencereKisa.5s")} ${yuzdeBicimle(0.46)} ${t("genel.kalan")}`;
        baglaSekmesiCiz();
      } else if (eylem === "deepseek-kaydet") {
        const g = document.getElementById("girdi-deepseek-anahtar");
        if (g && g.value.trim()) {
          g.value = "";
          durum.baglaDurum.deepseekAnahtarVar = true;
          durum.baglaDurum.deepseekSonuc = t("bagla.anahtarKaydedildi");
          baglaSekmesiCiz();
        }
      } else if (eylem === "deepseek-test") {
        durum.baglaDurum.deepseekSonuc = "340 ms · $12.40 USD";
        baglaSekmesiCiz();
      } else if (/^(opencode|dsh)-(bagla|kes)$/.test(eylem)) {
        yerelSayacAyarla(eylem);
      }
    });

    // Ayarlar yan menüsü & panel etkileşimleri
    document.getElementById("ayarlar-bolum-menu")?.addEventListener("click", (ev) => {
      const btn = ev.target.closest("[data-ayar-bolum]");
      if (!btn) return;
      durum.ayarSifirlaOnay = false;
      durum.ayarBolum = ayarBolumuNormalizeEt(btn.dataset.ayarBolum);
      tercihSenkronizeKaydet();
      ayarlarSekmesiCiz();
    });

    const ayarGrid = document.getElementById("ayarlar-grid");
    if (ayarGrid) {
      ayarGrid.addEventListener("click", (ev) => {
        const belgeBtn = ev.target.closest("[data-lisans-belge]");
        if (belgeBtn) {
          lisansBelgesiAc(belgeBtn.dataset.lisansBelge);
          return;
        }
        if (ev.target.closest("[data-lisans-kapat]")) {
          durum.lisansBelge = null;
          ayarlarSekmesiCiz();
          return;
        }
        const eylemBtn = ev.target.closest("[data-ayar-eylem]");
        if (eylemBtn) {
          ayarEylemi(eylemBtn.dataset.ayarEylem);
          return;
        }
        const sifirBtn = ev.target.closest("[data-ayar-sifirla]");
        if (sifirBtn) {
          const islem = sifirBtn.dataset.ayarSifirla;
          if (islem === "iste") {
            durum.ayarSifirlaOnay = true;
            ayarlarSekmesiCiz();
          } else if (islem === "iptal") {
            durum.ayarSifirlaOnay = false;
            ayarlarSekmesiCiz();
          } else if (islem === "onayla") {
            ayarlariSifirla();
          }
          return;
        }
        const sessizBtn = ev.target.closest("[data-platform-sessiz]");
        if (sessizBtn) {
          const p = sessizBtn.dataset.platformSessiz;
          if (!p) return;
          const liste = [...(durum.ayarlar.sessizPlatformlar || [])];
          const idx = liste.indexOf(p);
          if (idx === -1) liste.push(p);
          else liste.splice(idx, 1);
          ayarKaydet({ sessizPlatformlar: liste });
          return;
        }
        const anahtarBtn = ev.target.closest("[data-ayar-anahtar]");
        if (anahtarBtn) {
          const anahtar = anahtarBtn.dataset.ayarAnahtar;
          const acik = anahtarBtn.getAttribute("aria-checked") === "true";
          const yeniDeger = !acik;
          if (anahtar === "seritUstte") {
            if (canliMi()) invoke("serit_ustte_ayarla", { ustte: yeniDeger }).then(seritDurumuUygula).catch(() => {});
            else {
              durum.ayarlar.seritUstte = yeniDeger;
              ayarlarSekmesiCiz();
            }
            return;
          }
          if (anahtar === "seritGecirgen") {
            if (canliMi()) {
              invoke("serit_tiklama_gecirgen_ayarla", { gecirgen: yeniDeger }).then(seritDurumuUygula).catch(() => {});
            } else {
              durum.tiklamaGecirgen = yeniDeger;
              seritCiz();
              ayarlarSekmesiCiz();
            }
            return;
          }
          ayarKaydet({ [anahtar]: yeniDeger });
          return;
        }
        const gitBtn = ev.target.closest("[data-ayar-git-sekme]");
        if (gitBtn) {
          durum.ayarSifirlaOnay = false;
          durum.sekme = gitBtn.dataset.ayarGitSekme;
          tumunuCiz();
          return;
        }
        const seritBtn = ev.target.closest("[data-ayar-serit]");
        if (seritBtn) {
          const ac = seritBtn.dataset.ayarSerit === "ac";
          if (canliMi()) {
            invoke(ac ? "pencere_ac" : "serit_gizle", ac ? { yuzey: "serit" } : {}).catch(() => {});
          } else {
            yuzeyDegistir(ac ? "serit" : "pencere");
          }
          return;
        }
        if (ev.target.closest("[data-ayar-servis-kontrol]")) {
          if (canliMi()) {
            invoke("servis_durumu")
              .then((s) => servisDurumuUygula(s))
              .catch((e) => bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) })));
          } else {
            bildirimGoster(t("ayarlar.servisDemoEtiket"));
          }
        }
      });

      const AYAR_ESLEME = {
        "ayar-c5s": ["claude5sEsik", "sayi", 1, 99],
        "ayar-chf": ["claudeHaftaEsik", "sayi", 1, 99],
        "ayar-cx": ["digerEsik", "sayi", 1, 99],
        "ayar-port": ["servisPort", "port", 0, 65535],
        "ayar-cx-etkin": ["codexEtkinDk", "sayi", 1, 1440],
        "ayar-cx-bosta": ["codexBostaDk", "sayi", 1, 1440],
        "ayar-sessiz-bas": ["sessizBaslangic", "sayi", 0, 23],
        "ayar-sessiz-bit": ["sessizBitis", "sayi", 0, 23],
      };

      ayarGrid.addEventListener("change", (ev) => {
        const target = ev.target;
        if (target.id === "ayar-dil-secim") {
          const kod = target.value;
          if (canliMi() && dilDestekleniyorMu(kod)) ayarKaydet({ dil: kod }, false);
          dilYukleVeUygula(kod);
          return;
        }
        if (target.id === "ayar-kip-secim") {
          gostergeTercihiUygula("kip", target.value);
          return;
        }
        if (target.id === "ayar-renk-secim") {
          gostergeTercihiUygula("renk", target.value);
          return;
        }
        if (target.id === "ayar-yon-secim") {
          gostergeTercihiUygula("yon", target.value);
          return;
        }
        if (target.id === "ayar-agy-secim") {
          ayarKaydet({ agyOkuma: target.value });
          return;
        }
        if (target.dataset?.platformEsik) {
          const p = target.dataset.platformEsik;
          const esikler = { ...(durum.ayarlar.platformEsikleri || {}) };
          const ham = target.value.trim();
          if (ham === "") {
            delete esikler[p];
            ayarKaydet({ platformEsikleri: esikler });
            return;
          }
          const sayi = Math.round(Number(ham));
          if (!Number.isFinite(sayi)) {
            ayarlarSekmesiCiz();
            return;
          }
          esikler[p] = clamp(sayi, 1, 99);
          target.value = String(esikler[p]);
          ayarKaydet({ platformEsikleri: esikler });
          return;
        }
        const esleme = AYAR_ESLEME[target.id];
        if (!esleme) return;
        const [anahtar, tur, min, max] = esleme;
        let deger = Math.round(Number(target.value));
        if (!Number.isFinite(deger)) {
          ayarlarSekmesiCiz();
          return;
        }
        if (tur === "port") {
          if (deger !== 0 && deger < 1024) deger = 1024;
          deger = clamp(deger, 0, 65535);
        } else {
          deger = clamp(deger, min, max);
        }
        target.value = String(deger);
        const not = anahtar === "servisPort" || anahtar === "otomatikPort" ? t("ayarlar.portNot") : undefined;
        ayarKaydet({ [anahtar]: deger }, not);
      });
    }

    document.getElementById("chk-tiklama-gecirgen")?.addEventListener("change", (ev) => {
      if (canliMi()) {
        invoke("serit_tiklama_gecirgen_ayarla", { gecirgen: ev.target.checked }).then(seritDurumuUygula).catch(() => {});
        return;
      }
      durum.tiklamaGecirgen = ev.target.checked;
      seritCiz();
    });
    document.getElementById("btn-serit-acilir")?.addEventListener("click", acilirDon);
    document.getElementById("btn-serit-genislet")?.addEventListener("click", () => genislet("simdi"));
    seritSuruklemeBagla();

    document.addEventListener("keydown", (ev) => {
      const tag = document.activeElement?.tagName;
      const yaziyorMu = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
      if (ev.altKey && ev.key.toLowerCase() === "s") {
        ev.preventDefault();
        senaryoAcKapa();
        return;
      }
      if (yaziyorMu) return;

      if (ev.key === "Escape") {
        ev.preventDefault();
        if (aktifSurukleme?.basladi) {
          suruklemeBitir(true);
          return;
        }
        if (klavyeSurukleme) {
          klavyeTasimaBitir(true);
          return;
        }
        if (aktifIpucuHedef) {
          ipucuGizle();
          return;
        }
        if (canliMi()) {
          canliKapat();
          return;
        }
        if (durum.yuzey === "pencere" || durum.yuzey === "serit") {
          yuzeyDegistir("acilir");
        } else if (durum.yuzey === "acilir" && !durum.acilirKapali) {
          durum.acilirKapali = true;
          acilirCiz();
        }
        return;
      }

      if (klavyeSurukleme) {
        if (ev.key === " " || ev.code === "Space") {
          ev.preventDefault();
          klavyeTasimaBitir(false);
          return;
        }
        if (ev.key === "ArrowUp" || ev.key === "ArrowLeft") {
          ev.preventDefault();
          klavyeTasimaAdim(-1);
          return;
        }
        if (ev.key === "ArrowDown" || ev.key === "ArrowRight") {
          ev.preventDefault();
          klavyeTasimaAdim(1);
          return;
        }
      }

      const aktifTutamak = document.activeElement?.closest?.("[data-kart-tutamak]");
      if (aktifTutamak && (ev.key === " " || ev.code === "Space")) {
        ev.preventDefault();
        klavyeTasimaBaslat(aktifTutamak);
        return;
      }

      if (!ev.ctrlKey && !ev.metaKey && !ev.altKey && (ev.key === "r" || ev.key === "R")) {
        ev.preventDefault();
        yenileTetikle();
        return;
      }

      const anaSekmeOdak = document.activeElement?.closest?.(".sekme-cubugu [role='tab']");
      if (anaSekmeOdak && (ev.key === "ArrowLeft" || ev.key === "ArrowRight")) {
        const sekmeler = ["simdi", "gecmis", "bagla", "ayarlar"];
        const idx = sekmeler.indexOf(durum.sekme);
        if (idx !== -1) {
          ev.preventDefault();
          const yeni = sekmeler[(idx + (ev.key === "ArrowRight" ? 1 : -1) + sekmeler.length) % sekmeler.length];
          durum.ayarSifirlaOnay = false;
          durum.sekme = yeni;
          tumunuCiz();
          document.querySelector(`.sekme-cubugu [data-sekme="${yeni}"]`)?.focus();
          return;
        }
      }

      const pltSekmeOdak = document.activeElement?.closest?.("[data-platform-sekme]");
      if (
        (pltSekmeOdak || durum.yuzey === "acilir" || (durum.yuzey === "pencere" && durum.sekme === "simdi")) &&
        (ev.key === "ArrowLeft" || ev.key === "ArrowRight")
      ) {
        const sekmeler = ["genel", ...platformListesi(havuzSirala(durum.anlik.havuzlar))];
        if (sekmeler.length <= 1) return;
        ev.preventDefault();
        const idx = Math.max(0, sekmeler.indexOf(durum.platformSekme));
        const yeni = sekmeler[(idx + (ev.key === "ArrowRight" ? 1 : -1) + sekmeler.length) % sekmeler.length];
        platformSekmesiSec(yeni, Boolean(pltSekmeOdak));
        return;
      }

      const genelKartAlaniOdakli =
        durum.platformSekme === "genel" &&
        (durum.yuzey === "acilir" || (durum.yuzey === "pencere" && durum.sekme === "simdi")) &&
        (Boolean(document.activeElement?.closest?.("#acilir-havuz-listesi, #simdi-havuz-grid")) ||
          document.activeElement === document.body ||
          document.activeElement?.id === "acilir-pencere");
      if ((ev.key === "ArrowDown" || ev.key === "ArrowUp") && genelKartAlaniOdakli) {
        const sirali = platformGruplariOlustur(durum.anlik.havuzlar, { gizlileriDahilEt: durum.duzenleModu }).map((g) => g.platform);
        if (sirali.length === 0) return;
        ev.preventDefault();
        const mevcutIdx = sirali.indexOf(durum.seciliHavuzId);
        const yon = ev.key === "ArrowDown" ? 1 : -1;
        const hedefIdx = mevcutIdx === -1 ? (yon > 0 ? 0 : sirali.length - 1) : (mevcutIdx + yon + sirali.length) % sirali.length;
        durum.seciliHavuzId = sirali[hedefIdx];
        const escId = CSS.escape(durum.seciliHavuzId);
        if (durum.yuzey === "acilir") {
          acilirCiz();
          document.querySelector(`#acilir-havuz-listesi [data-havuz-id="${escId}"] [data-platform-ac], #acilir-havuz-listesi [data-havuz-id="${escId}"] [data-kart-tutamak]`)?.focus();
        } else {
          simdiSekmesiCiz();
          document.querySelector(`#simdi-havuz-grid [data-havuz-id="${escId}"] [data-platform-ac], #simdi-havuz-grid [data-havuz-id="${escId}"] [data-kart-tutamak]`)?.focus();
        }
        return;
      }

      if (ev.key === "Enter" && durum.yuzey === "acilir" && !durum.acilirKapali) {
        if (document.activeElement?.tagName !== "BUTTON" && !document.activeElement?.closest?.(".kompakt-kart")) {
          ev.preventDefault();
          genislet("simdi");
        }
      }
    });
  }

  function seritSuruklemeBagla() {
    const cubuk = document.getElementById("serit-cubuk");
    const sahne = document.getElementById("serit-sahne");
    const tutamak = document.getElementById("serit-tutamak");
    let surukleniyor = false;
    let basX = 0;
    let basY = 0;
    let basSol = 0;
    let basUst = 0;

    tutamak.addEventListener("mousedown", (ev) => {
      if (durum.tiklamaGecirgen) return;
      if (canliMi()) {
        ev.preventDefault();
        const pencere = window.__TAURI__?.window?.getCurrentWindow?.();
        if (pencere) pencere.startDragging().catch(() => {});
        return;
      }
      surukleniyor = true;
      const sR = sahne.getBoundingClientRect();
      const cR = cubuk.getBoundingClientRect();
      basX = ev.clientX;
      basY = ev.clientY;
      basSol = cR.left - sR.left;
      basUst = cR.top - sR.top;
      cubuk.style.position = "absolute";
      cubuk.style.left = `${basSol}px`;
      cubuk.style.top = `${basUst}px`;
      tutamak.style.cursor = "grabbing";
    });

    window.addEventListener("mousemove", (ev) => {
      if (!surukleniyor) return;
      const maksSol = Math.max(8, sahne.clientWidth - cubuk.offsetWidth - 8);
      const maksUst = Math.max(8, sahne.clientHeight - cubuk.offsetHeight - 8);
      cubuk.style.left = `${clamp(basSol + (ev.clientX - basX), 8, maksSol)}px`;
      cubuk.style.top = `${clamp(basUst + (ev.clientY - basY), 8, maksUst)}px`;
    });

    window.addEventListener("mouseup", () => {
      if (!surukleniyor) return;
      surukleniyor = false;
      tutamak.style.cursor = "grab";
    });
  }

  // --------------------------------------------------------------------------
  // 16. Masaüstü (Tauri 2) Köprüsü — invoke / listen
  // --------------------------------------------------------------------------
  function canliMi() {
    return durum.tauriVarMi && !durum.demoMu;
  }

  function invoke(komut, args) {
    return window.__TAURI__.core.invoke(komut, args || {});
  }

  function hataMetni(e) {
    if (!e) return "?";
    if (typeof e === "string") return e;
    return e.message || JSON.stringify(e);
  }

  function servisDurumuUygula(p) {
    if (!p || typeof p !== "object") return;
    durum.servisDurumu.calisiyor = Boolean(p.calisiyor);
    durum.servisDurumu.port = p.port ?? null;
    durum.servisDurumu.pipe = p.pipe ?? null;
    durum.servisDurumu.sonSira = typeof p.sonSira === "number" ? p.sonSira : durum.servisDurumu.sonSira;
    durum.servisDurumu.hata = p.hata || null;
    servisUyariCiz();
    acilirCiz();
    if (durum.sekme === "ayarlar") ayarlarSekmesiCiz();
  }

  function anlikYanitiUygula(y) {
    if (!y) return;
    durum.servisDurumu.calisiyor = Boolean(y.servisCalisiyor);
    durum.servisDurumu.baslatmaOnerisi = Boolean(y.baslatmaOnerisi);
    durum.servisDurumu.hata = y.hata || null;
    if (y.anlik && y.anlik.havuzlar) {
      const islenmis = anlikSadelestir(klonla(y.anlik));
      if (!durum.servisDurumu.calisiyor) {
        for (const h of Object.values(islenmis.havuzlar || {})) {
          if (h && h.durum === "ok") {
            h.durum = "bayat";
            if (h.kaynak) h.kaynak.guven = "bayat";
          }
        }
      }
      durum.anlik = islenmis;
      durum.temelAnlik = islenmis;
      if (typeof y.anlik.sira === "number") durum.servisDurumu.sonSira = y.anlik.sira;
      if (!durum.seciliHavuzId || !platformMevcutMu(durum.seciliHavuzId)) {
        const gruplar = platformGruplariOlustur(durum.anlik.havuzlar);
        durum.seciliHavuzId = gruplar.length > 0 ? gruplar[0].platform : null;
      }
    }
    canliYenidenCiz();
  }

  function canliYenidenCiz() {
    temaNitelikleriniUygula();
    metinleriGuncelle();
    acilirCiz();
    simdiSekmesiCiz();
    if (durum.sekme === "gecmis") gecmisSekmesiCiz();
    if (durum.sekme !== "bagla") baglaSekmesiCiz();
    if (durum.sekme === "ayarlar") ayarlarSekmesiCiz();
    seritCiz();
    servisUyariCiz();
  }

  function ayarlariUygula(a) {
    if (!a || typeof a !== "object") return;
    const { uyari, ...digerleri } = a;
    Object.assign(durum.ayarlar, digerleri);
    if (!durum.ayarlar.platformEsikleri || typeof durum.ayarlar.platformEsikleri !== "object") {
      durum.ayarlar.platformEsikleri = {};
    }
    if (!Array.isArray(durum.ayarlar.sessizPlatformlar)) {
      durum.ayarlar.sessizPlatformlar = [];
    }
    if (["murekkep", "kagit", "terminal"].includes(a.tema)) durum.tema = a.tema;
    if (RENK_TEMALARI.some((r) => r.id === a.renkTemasi)) durum.renkTemasi = a.renkTemasi;
    if (["acik", "koyu", "sistem"].includes(a.kip)) durum.gorunumKipi = a.kip;
    if (["kalan", "kullanilan"].includes(a.degerYonu)) durum.degerYonu = a.degerYonu;
    if (Array.isArray(a.kartSirasi) || Array.isArray(a.gizliKartlar) || Array.isArray(a.sabitKartlar)) {
      durum.kartTercihler = kartTercihTemizle({
        kartSirasi: a.kartSirasi ?? durum.kartTercihler.kartSirasi,
        gizliKartlar: a.gizliKartlar ?? durum.kartTercihler.gizliKartlar,
        sabitKartlar: a.sabitKartlar ?? durum.kartTercihler.sabitKartlar,
      });
      kartTercihleriniYaz(durum.kartTercihler);
    }
    if (uyari) durum.ayarUyarisi = uyari;
    if (dilDestekleniyorMu(a.dil) && a.dil !== durum.dil) {
      dilYukleVeUygula(a.dil);
      return;
    }
    tumunuCiz();
  }

  function seritDurumuUygula(sd) {
    if (!sd) return;
    durum.tiklamaGecirgen = Boolean(sd.gecirgen);
    durum.ayarlar.seritUstte = Boolean(sd.ustte);
    seritCiz();
    if (durum.sekme === "ayarlar") ayarlarSekmesiCiz();
  }

  function ayarKaydet(guncelleme, basariMetni, sessizHata = false) {
    if (!canliMi()) {
      const kopya = { ...guncelleme };
      for (const k of ["claude5sEsik", "claudeHaftaEsik", "digerEsik"]) {
        if (k in kopya) kopya[k] = clamp(Math.round(Number(kopya[k]) || 20), 1, 99);
      }
      for (const k of ["codexEtkinDk", "codexBostaDk"]) {
        if (k in kopya) kopya[k] = clamp(Math.round(Number(kopya[k]) || 5), 1, 1440);
      }
      if ("servisPort" in kopya) {
        let p = Math.round(Number(kopya.servisPort) || 0);
        if (p !== 0 && p < 1024) p = 1024;
        kopya.servisPort = clamp(p, 0, 65535);
      }
      Object.assign(durum.ayarlar, kopya);
      if ("sessizSaatler" in guncelleme && !("sessizBaslangic" in guncelleme)) {
        durum.ayarlar.sessizBaslangic = 0;
        durum.ayarlar.sessizBitis = guncelleme.sessizSaatler ? 8 : 0;
      }
      if ("sessizBaslangic" in guncelleme || "sessizBitis" in guncelleme) {
        durum.ayarlar.sessizBaslangic = clamp(Math.round(Number(durum.ayarlar.sessizBaslangic) || 0), 0, 23);
        durum.ayarlar.sessizBitis = clamp(Math.round(Number(durum.ayarlar.sessizBitis) || 0), 0, 23);
        durum.ayarlar.sessizSaatler = durum.ayarlar.sessizBaslangic !== durum.ayarlar.sessizBitis;
      }
      tumunuCiz();
      if (basariMetni !== false) bildirimGoster(basariMetni || t("ayarlar.ayarKaydedildi"));
      return Promise.resolve(true);
    }
    const istek = ayarKaydetKuyruk.then(async () => {
      try {
        const a = await invoke("ayar_kaydet", { guncelleme });
        ayarlariUygula(a);
        if (basariMetni !== false) bildirimGoster(basariMetni || t("ayarlar.ayarKaydedildi"));
        return true;
      } catch (e) {
        if (!sessizHata) {
          bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) }));
          tumunuCiz();
        }
        return false;
      }
    });
    ayarKaydetKuyruk = istek.catch(() => {});
    return istek;
  }

  async function ayarlariSifirla() {
    durum.ayarSifirlaOnay = false;
    durum.kartTercihler = varsayilanKartTercihleri();
    kartTercihleriniYaz(durum.kartTercihler);
    const v = varsayilanTercihler();
    durum.renkTemasi = v.renkTemasi;
    durum.gorunumKipi = v.gorunumKipi;
    durum.gostergeTuru = v.gostergeTuru;
    durum.pastaAltTuru = v.pastaAltTuru;
    durum.gorunumModu = v.gorunumModu;
    durum.degerYonu = v.degerYonu;
    tercihSenkronizeKaydet();

    if (!canliMi()) {
      Object.assign(durum.ayarlar, {
        claude5sEsik: 15,
        claudeHaftaEsik: 30,
        digerEsik: 20,
        platformEsikleri: {},
        sessizPlatformlar: [],
        bildirimEtkin: true,
        windowsIleBaslat: false,
        tepsiyeKucult: true,
        servisPort: 43117,
        otomatikPort: true,
        codexEtkin: true,
        codexEtkinDk: 5,
        codexBostaDk: 30,
        deepseekEtkin: false,
        esikBildirimi: true,
        tahminBildirimi: true,
        krediBildirimi: true,
        bayatBildirimi: true,
        sifirlandiBildirimi: true,
        sessizSaatler: true,
        sessizBaslangic: 0,
        sessizBitis: 8,
        agyOkuma: durum.demoMu ? "istek_uzerine" : "kapali",
        seritUstte: true,
        opencodeEtkin: durum.demoMu,
        dshEtkin: false,
      });
      await dilYukleVeUygula("tr");
      bildirimGoster(t("ayarlar.varsayilanaDonduBildirim"));
      return;
    }

    try {
      const a = await invoke("ayarlari_sifirla");
      ayarlariUygula(a);
      bildirimGoster(t("ayarlar.varsayilanaDonduBildirim"));
    } catch (e) {
      bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) }));
      ayarlarSekmesiCiz();
    }
  }

  function servisUyariCiz() {
    const metinler = [];
    let dugme = "";
    if (canliMi() && !durum.servisDurumu.calisiyor) {
      metinler.push(durum.servisDurumu.baslatiliyor ? t("genel.servisBaslatiliyor") : t("genel.servisKapali"));
      if (!durum.servisDurumu.baslatiliyor) {
        dugme = `<button type="button" class="ikincil-btn" data-servis-baslat="1">${escHtml(t("genel.servisBaslatBtn"))}</button>`;
      }
    } else if (!durum.demoMu && !durum.tauriVarMi) {
      metinler.push(t("genel.servisKapali"));
    }
    if (canliMi() && durum.ayarUyarisi) metinler.push(durum.ayarUyarisi);
    for (const id of ["servis-uyari-acilir", "servis-uyari-genis"]) {
      const el = document.getElementById(id);
      if (!el) continue;
      el.classList.toggle("gizli", metinler.length === 0);
      el.innerHTML = metinler.length ? `<span class="servis-uyari-metin">${escHtml(metinler.join(" · "))}</span>${dugme}` : "";
    }
  }

  async function servisBaslat() {
    durum.servisDurumu.baslatiliyor = true;
    servisUyariCiz();
    try {
      const y = await invoke("servis_baslat");
      anlikYanitiUygula(y);
      bildirimGoster(y.servisCalisiyor ? t("genel.servisBaglandi") : `${t("genel.servisBaslatHata")}${y.hata ? ": " + y.hata : ""}`);
    } catch (e) {
      bildirimGoster(`${t("genel.servisBaslatHata")}: ${hataMetni(e)}`);
    } finally {
      durum.servisDurumu.baslatiliyor = false;
      servisUyariCiz();
    }
  }

  async function canliYenile() {
    try {
      const r = await invoke("yenile_iste", { havuz: null });
      if (r.istendi) bildirimGoster(t("genel.yenileIstendi"));
      else if (r.bekleSn) bildirimGoster(fmt("genel.hizSiniriSn", { sn: r.bekleSn }));
      else {
        durum.servisDurumu.calisiyor = Boolean(r.servisCalisiyor);
        bildirimGoster(r.hata ? fmt("genel.islemHatasi", { hata: r.hata }) : t("genel.servisYok"));
        servisUyariCiz();
      }
    } catch (e) {
      bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) }));
    }
  }

  async function canliTara() {
    durum.baglaDurum.taraniyor = true;
    baglaSekmesiCiz();
    try {
      durum.baglaDurum.tarama = await invoke("tara");
      durum.baglaDurum.sonTarama = saatBicimle(durum.baglaDurum.tarama.zaman);
      bildirimGoster(t("bagla.tarandi"));
    } catch (e) {
      bildirimGoster(fmt("genel.islemHatasi", { hata: hataMetni(e) }));
    } finally {
      durum.baglaDurum.taraniyor = false;
      baglaSekmesiCiz();
    }
  }

  async function canliTest(platform) {
    if (durum.baglaDurum.testCalisiyor[platform]) return;
    durum.baglaDurum.testCalisiyor[platform] = true;
    baglaSekmesiCiz();
    try {
      durum.baglaDurum.canliSonuc[platform] = { test: await invoke("test_et", { platform }) };
    } catch (e) {
      durum.baglaDurum.canliSonuc[platform] = { metin: fmt("genel.islemHatasi", { hata: hataMetni(e) }) };
    } finally {
      durum.baglaDurum.testCalisiyor[platform] = false;
      baglaSekmesiCiz();
    }
  }

  function canliSonucHtml(platform) {
    const k = durum.baglaDurum.canliSonuc[platform];
    if (!k) return "";
    let metin = k.metin || "";
    if (k.test) {
      const ozet = durum.dil === "tr" ? k.test.ozet_tr : k.test.ozet_en;
      metin = `${k.test.basarili ? "✓" : "×"} ${ozet}${k.test.onbellekten ? ` (${t("bagla.sonBilinen")})` : ""}`;
    }
    return metin ? `<div class="test-sonuc">${escHtml(metin)}</div>` : "";
  }

  async function canliAnahtarKaydet(platform, girdiId) {
    const girdi = document.getElementById(girdiId);
    if (!girdi) return;
    const anahtar = girdi.value;
    girdi.value = "";
    if (!anahtar.trim()) return;
    try {
      const r = await invoke("anahtar_kaydet", { platform, anahtar });
      durum.baglaDurum.canliSonuc[platform] = {
        metin: r.servisBildirildi ? t("bagla.anahtarKaydedildi") : t("bagla.anahtarServisBekliyor"),
      };
      await canliTara();
    } catch (e) {
      durum.baglaDurum.canliSonuc[platform] = { metin: fmt("bagla.anahtarHata", { hata: hataMetni(e) }) };
      baglaSekmesiCiz();
    }
  }

  async function statuslineOnizle() {
    try {
      durum.baglaDurum.statusline = await invoke("claude_statusline_onizle");
    } catch (e) {
      durum.baglaDurum.statusline = { hata: hataMetni(e), uygulanabilir: false, farkMetni: "" };
    }
    durum.baglaDurum.statuslineSonuc = null;
    baglaSekmesiCiz();
  }

  function baglaDurumMetni(kayit) {
    if (!kayit) return "";
    if (typeof kayit === "object" && kayit.key) return fmt(kayit.key, kayit.params || {});
    return String(kayit);
  }

  async function statuslineUygula() {
    const o = durum.baglaDurum.statusline;
    if (!o || !o.uygulanabilir) return;
    try {
      const r = await invoke("claude_statusline_uygula", { onay: true, beklenenFark: o.farkMetni });
      durum.baglaDurum.statuslineSonuc = r.yazildi
        ? { key: "bagla.statuslineYazildi", params: { yedek: r.yedek || "" } }
        : { key: "bagla.statuslineZaten" };
      durum.baglaDurum.statusline = null;
      await canliTara();
    } catch (e) {
      durum.baglaDurum.statuslineSonuc = { key: "genel.islemHatasi", params: { hata: hataMetni(e) } };
    }
    baglaSekmesiCiz();
  }

  // "opencode-bagla" / "dsh-kes": sayacı ayardan açar/kapatır, sonra taramayı tazeler.
  async function yerelSayacAyarla(eylem) {
    const [platform, islem] = eylem.split("-");
    const alan = platform === "opencode" ? "opencodeEtkin" : "dshEtkin";
    const ac = islem === "bagla";
    const tamam = await ayarKaydet({ [alan]: ac }, ac ? t("bagla.yerelBagli") : t("ayarlar.ayarKaydedildi"));
    if (tamam && canliMi()) {
      await canliTara();
      try {
        await invoke("yenile_iste", { havuz: platform });
      } catch {
        /* servis bir sonraki turda okur */
      }
    }
    if (durum.sekme === "bagla") baglaSekmesiCiz();
  }

  function canliBaglaEylemi(ev) {
    const agyBtn = ev.target.closest("[data-agy-izin]");
    if (agyBtn) {
      ayarKaydet({ agyOkuma: agyBtn.dataset.agyIzin });
      return;
    }
    const btn = ev.target.closest("[data-bagla-eylem]");
    if (!btn) return;
    const eylem = btn.dataset.baglaEylem;
    if (eylem.endsWith("-test")) canliTest(eylem.slice(0, -5));
    else if (/^(opencode|dsh)-(bagla|kes)$/.test(eylem)) yerelSayacAyarla(eylem);
    else if (eylem === "deepseek-kaydet") canliAnahtarKaydet("deepseek", "girdi-deepseek-anahtar");
    else if (eylem === "claude-onizle") statuslineOnizle();
    else if (eylem === "claude-onayla") statuslineUygula();
  }

  function canliKartUyarla(k) {
    const tar = durum.baglaDurum.tarama;
    const cal = durum.baglaDurum.testCalisiyor;
    const cxEtkin = durum.ayarlar.codexEtkin !== false;
    const dsEtkin = Boolean(durum.ayarlar.deepseekEtkin);
    const testBtn = (p, kapali) =>
      `<button type="button" class="ikincil-btn" data-bagla-eylem="${p}-test" ${kapali || cal[p] ? "disabled" : ""}>${escHtml(cal[p] ? t("bagla.testCalisiyor") : t("bagla.testEt"))}</button>`;
    const anahtarSatiri = (p, testKapali = false) => `
      <div class="bagla-eylem-satiri">
        <input type="password" class="maskeli-girdi" id="girdi-${p}-anahtar" placeholder="${escHtml(t("bagla.anahtarYerTutucu"))}" autocomplete="off" maxlength="512">
        <button type="button" class="ikincil-btn" data-bagla-eylem="${p}-kaydet">${escHtml(t("bagla.kaydet"))}</button>
        ${testBtn(p, testKapali)}
      </div>`;

    if (k.yerelSayac) return k;
    const yeni = { ...k };
    if (!tar) {
      yeni.rozet = { sinif: "bilinmiyor", metin: durum.baglaDurum.taraniyor ? t("bagla.taraniyor") : t("bagla.taranmadi") };
      yeni.tespit = t("bagla.taraIpucu");
    }

    if (k.platform === "claude") {
      const cl = durum.anlik.havuzlar?.claude_max;
      if (tar) {
        yeni.rozet =
          cl?.durum === "ok"
            ? { sinif: "rahat", metin: fmt("bagla.kopruBagli", { yas: kaynakYasMetni(cl.kaynak) }) }
            : tar.claude_kurulu
            ? { sinif: "dikkat", metin: t("bagla.kopruYok") }
            : { sinif: "bilinmiyor", metin: t("bagla.claudeYok") };
        const sl = tar.claude_statusline_cetvel ? "Cetvel" : tar.claude_statusline_tanimli ? "+" : "—";
        yeni.tespit = `~/.claude: ${tar.claude_kurulu ? "✓" : "—"} · kota-bandi: ${tar.kota_bandi_ipucu || "—"} · statusLine: ${sl}`;
      }
      const o = durum.baglaDurum.statusline;
      const slSonuc = baglaDurumMetni(durum.baglaDurum.statuslineSonuc);
      yeni.eylemHtml = `
        <div class="bagla-eylem-satiri">${testBtn("claude", false)}<button type="button" class="ikincil-btn" data-bagla-eylem="claude-onizle">${escHtml(t("bagla.statuslineOnizle"))}</button></div>
        ${o?.farkMetni ? `<div class="onizleme-fark">${escHtml(o.farkMetni)}</div>` : ""}
        ${o?.hata ? `<div class="test-sonuc">${escHtml(o.hata)}</div>` : ""}
        ${o?.uygulanabilir ? `<div class="bagla-eylem-satiri"><button type="button" class="birincil-btn" data-bagla-eylem="claude-onayla">${escHtml(t("bagla.onaylaYaz"))}</button></div>` : ""}
        ${o ? `<div class="satir-neden">${escHtml(t("bagla.statuslineNot"))}</div>` : ""}
        ${slSonuc ? `<div class="test-sonuc">${escHtml(slSonuc)}</div>` : ""}
        ${canliSonucHtml("claude")}
      `;
    } else if (k.platform === "codex") {
      if (!cxEtkin) {
        yeni.rozet = { sinif: "bilinmiyor", metin: t("bagla.devreDisiEtiket") };
      } else if (tar) {
        yeni.rozet =
          tar.codex_durum === "bulundu"
            ? { sinif: "rahat", metin: t("bagla.codexBulundu") }
            : tar.codex_durum === "giris_gerek"
            ? { sinif: "dikkat", metin: t("bagla.codexGiris") }
            : { sinif: "bilinmiyor", metin: t("bagla.codexYok") };
      }
      yeni.eylemHtml = `<div class="bagla-eylem-satiri">${testBtn("codex", !cxEtkin || (tar && tar.codex_durum === "yok"))}</div>${canliSonucHtml("codex")}`;
    } else if (k.platform === "agy") {
      const izin = durum.ayarlar.agyOkuma;
      if (tar && !tar.agy_ikili_var) yeni.rozet = { sinif: "bilinmiyor", metin: t("bagla.agyYok") };
      yeni.eylemHtml = `
        <div class="risk-kutusu">${escHtml(t("bagla.agyRisk"))}</div>
        <div class="bagla-eylem-satiri">
          <div class="ayar-secim-grup" role="group" aria-label="${escHtml(t("ayarlar.agyOkumaEtiket"))}">
            ${["kapali", "istek_uzerine", "otomatik"].map((m) => `<button type="button" class="mini-btn${izin === m ? " aktif" : ""}" data-agy-izin="${m}">${escHtml(t(`bagla.agySecenekler.${m}`))}</button>`).join("")}
          </div>
          ${testBtn("agy", izin === "kapali" || (tar && !tar.agy_ikili_var))}
        </div>
        ${canliSonucHtml("agy")}
      `;
    } else if (k.platform === "deepseek") {
      if (!dsEtkin) {
        yeni.rozet = { sinif: "bilinmiyor", metin: t("bagla.devreDisiEtiket") };
      } else if (tar) {
        yeni.rozet = tar.deepseek_anahtar_var ? { sinif: "rahat", metin: t("bagla.anahtarVar") } : { sinif: "bilinmiyor", metin: t("bagla.anahtarGerek") };
      }
      yeni.eylemHtml = `${anahtarSatiri("deepseek", !dsEtkin)}${canliSonucHtml("deepseek")}`;
    }
    return yeni;
  }

  async function canliGecmisGetir(havuzId, pencere) {
    if (!havuzId || !pencere) return;
    const anahtar = `${havuzId}/${pencere}`;
    const mevcut = durum.gecmisCanli[havuzId];
    if (mevcut && mevcut.anahtar === anahtar && Date.now() - mevcut.alindi < 60000) return;
    if (durum.gecmisYukleniyor === anahtar) return;
    durum.gecmisYukleniyor = anahtar;
    try {
      const y = await invoke("gecmis_al", { havuz: havuzId, pencere, gun: 1 });
      const h = durum.anlik.havuzlar?.[havuzId];
      const ornekler = Array.isArray(y?.ornekler) ? y.ornekler : [];
      durum.gecmisCanli[havuzId] = {
        anahtar,
        pencere,
        alindi: Date.now(),
        etiket: `${h?.etiket || anahtarAdiOkunurYap(havuzId)} · ${pencereEtiketi(pencere)}`,
        platform: h?.platform || "",
        noktalar: ornekler.map((o) => Math.round(clamp(Number(o.kullanilan) || 0, 0, 1) * 100)),
        zamanlar: ornekler.map((o) => o.zaman),
      };
    } catch (e) {
      durum.gecmisCanli[havuzId] = { anahtar, pencere, alindi: Date.now(), hata: hataMetni(e), noktalar: [] };
    } finally {
      durum.gecmisYukleniyor = null;
      if (durum.sekme === "gecmis") gecmisSekmesiCiz();
    }
  }

  function genislet(sekme) {
    if (canliMi()) {
      invoke("pencere_ac", { yuzey: "pencere", sekme: sekme || "simdi" }).catch(() => {});
      return;
    }
    yuzeyDegistir("pencere", sekme || "simdi");
  }

  function acilirDon() {
    if (canliMi()) {
      if (durum.yuzey === "serit") invoke("pencere_ac", { yuzey: "acilir" }).catch(() => {});
      else invoke("pencere_gizle", { acilirGoster: true }).catch(() => {});
      return;
    }
    yuzeyDegistir("acilir");
  }

  function canliKapat() {
    if (durum.yuzey === "acilir") invoke("acilir_gizle").catch(() => {});
    else if (durum.yuzey === "pencere") invoke("pencere_gizle", { acilirGoster: false }).catch(() => {});
    else invoke("serit_gizle").catch(() => {});
  }

  async function masaustuKoprusunuBaslat() {
    const olay = window.__TAURI__?.event;
    if (olay?.listen) {
      try {
        await olay.listen("anlik_degisti", (e) => anlikYanitiUygula(e.payload));
        await olay.listen("servis_durumu_degisti", (e) => servisDurumuUygula(e.payload));
        await olay.listen("pencere_sekme_sec", (e) => {
          if (typeof e.payload === "string" && durum.yuzey === "pencere") {
            durum.sekme = e.payload;
            tumunuCiz();
          }
        });
        await olay.listen("serit_durumu_degisti", (e) => seritDurumuUygula(e.payload));
        await olay.listen("ayarlar_degisti", (e) => ayarlariUygula(e.payload));
      } catch (e) {
        console.warn("Tauri olay dinleyicisi kurulamadı", e);
      }
    }

    try {
      ayarlariUygula(await invoke("ayarlar_al"));
    } catch (_) {
      // Varsayılanlarla devam
    }
    try {
      servisDurumuUygula(await invoke("servis_durumu"));
    } catch (_) {
      // Yoksay
    }
    try {
      anlikYanitiUygula(await invoke("anlik_al"));
    } catch (e) {
      durum.servisDurumu.calisiyor = false;
      durum.servisDurumu.hata = hataMetni(e);
      tumunuCiz();
    }
    if (durum.yuzey === "serit" || durum.yuzey === "pencere") {
      try {
        seritDurumuUygula(await invoke("serit_durumu_al"));
      } catch (_) {
        // Yoksay
      }
    }

    const tazele = () => {
      if (document.visibilityState === "hidden") return;
      acilirCiz();
      simdiSekmesiCiz();
      seritCiz();
      invoke("anlik_al").then(anlikYanitiUygula).catch(() => {});
    };
    document.addEventListener("visibilitychange", tazele);
    window.addEventListener("focus", tazele);

    window.setInterval(() => {
      acilirCiz();
      simdiSekmesiCiz();
      seritCiz();
    }, 30000);
  }

  // --------------------------------------------------------------------------
  // 17. Başlatma
  // --------------------------------------------------------------------------
  async function baslat() {
    document.documentElement.dataset.masaustu = canliMi() ? "1" : "0";
    document.documentElement.dataset.yuzey = durum.yuzey;
    temaNitelikleriniUygula();
    olaylariBagla();
    if (durum.dil !== "tr") {
      await dilYukleVeUygula(durum.dil);
    } else {
      tumunuCiz();
    }

    if (canliMi()) {
      await masaustuKoprusunuBaslat();
      return;
    }

    if (durum.demoMu && window.location.protocol.startsWith("http")) {
      try {
        const yanit = await fetch("./ornek-anlik.json", { cache: "no-store" });
        if (yanit.ok) {
          const json = await yanit.json();
          if (json?.sema === 2 && json.havuzlar) {
            durum.temelAnlik = json;
            if (durum.senaryo === "normal") {
              durum.anlik = anlikSadelestir(klonla(json));
              tumunuCiz();
            }
          }
        }
      } catch (_) {
        // Gömülü veriyle devam et
      }
    }
  }

  document.addEventListener("DOMContentLoaded", baslat);
})();
